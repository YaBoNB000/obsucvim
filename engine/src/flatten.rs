
use crate::ast::*;
use crate::mangle::gen_name;
use crate::rng::Rng;
use crate::symtab::{Sym, SymTable};
use std::collections::{HashMap, HashSet, VecDeque};


#[derive(Debug, Clone, Copy)]
enum Succ {
	B(usize),
	Brk,
	Cont,
}

#[derive(Debug, Clone)]
enum LoopKind {
	While { cond: Expr },
	Repeat { cond: Expr },
	ForNum { var: SymId, lim: SymId, stp: SymId, cur: SymId },
	ForGen { vars: Vec<SymId>, it: SymId, stt: SymId, ctl: SymId },
}

#[derive(Debug, Clone)]
struct LoopNode {
	kind: LoopKind,
	blocks: Vec<Blk>,
	start: usize,
}

#[derive(Debug, Clone)]
enum BlkBody {
	Stmt(Stmt),
	Cond(Expr),
	Join,
	Return(Vec<Expr>),
	Loop(Box<LoopNode>),
}

#[derive(Debug, Clone)]
struct Blk {
	body: BlkBody,
	succ: Vec<Succ>,
}

struct Ctx {
	blocks: Vec<Blk>,
	inner: bool,
}

impl Ctx {
	fn new(inner: bool) -> Ctx {
		Ctx {
			blocks: Vec::new(),
			inner,
		}
	}
	fn push(&mut self, body: BlkBody) -> usize {
		let i = self.blocks.len();
		self.blocks.push(Blk {
			body,
			succ: Vec::new(),
		});
		i
	}
}

enum Cont {
	Join,
	Done,
	Check { cond: Expr },
}

fn expand(stmts: &[Stmt]) -> Vec<Stmt> {
	let mut out = Vec::with_capacity(stmts.len());
	for s in stmts {
		match s {
			Stmt::Do(b) => out.extend(b.stmts.iter().cloned()),
			other => out.push(other.clone()),
		}
	}
	out
}

fn num(v: u32) -> Expr {
	Expr::Num {
		value: v as f64,
		isfloat: false,
	}
}

fn ident(sym: SymId, table: &SymTable) -> Expr {
	Expr::Ident {
		name: table.name_of(sym).to_string(),
		sym: Some(sym),
	}
}

fn new_rand_sym(
	table: &mut SymTable,
	rng: &mut Rng,
	reserved: &mut HashSet<String>,
) -> (SymId, String) {
	let name = gen_name(rng, reserved);
	reserved.insert(name.clone());
	let id = table.syms.len() as SymId;
	table.syms.push(Sym {
		name: name.clone(),
		is_param: false,
		keep_name: false,
	});
	(id, name)
}


fn build_graph(
	ctx: &mut Ctx,
	stmts: &[Stmt],
	cont: Cont,
	table: &mut SymTable,
	rng: &mut Rng,
	reserved: &mut HashSet<String>,
) -> (usize, usize) {
	let flat = expand(stmts);
	let start = ctx.push(BlkBody::Join);
	let mut cur: Option<usize> = Some(start);
	for stmt in flat.iter() {
		cur = match cur {
None => None,
			Some(c) => emit_stmt(ctx, c, stmt, table, rng, reserved),
		};
	}
	let cont_blk = match cont {
		Cont::Join | Cont::Done => {
			let c = ctx.push(BlkBody::Join);
			if let Some(x) = cur {
				ctx.blocks[x].succ.push(Succ::B(c));
			}
			c
		}
		Cont::Check { cond } => {
			let check = ctx.push(BlkBody::Cond(cond));
			ctx.blocks[check].succ = vec![Succ::Brk, Succ::B(start)];
			if let Some(x) = cur {
				ctx.blocks[x].succ.push(Succ::B(check));
			}
			check
		}
	};
	(start, cont_blk)
}

fn emit_stmt(
	ctx: &mut Ctx,
	cur: usize,
	stmt: &Stmt,
	table: &mut SymTable,
	rng: &mut Rng,
	reserved: &mut HashSet<String>,
) -> Option<usize> {
	match stmt {
		Stmt::Return(es) => {
			let b = ctx.push(BlkBody::Return(es.clone()));
			ctx.blocks[cur].succ.push(Succ::B(b));
			None
		}
		Stmt::Break => {
			assert!(ctx.inner, "break outside loop (parser must reject)");
			ctx.blocks[cur].succ.push(Succ::Brk);
			None
		}
		Stmt::Continue => {
			assert!(ctx.inner, "continue outside loop (parser must reject)");
			ctx.blocks[cur].succ.push(Succ::Cont);
			None
		}
		Stmt::If { cond, thenb, elsifs, elseb } => {
			let (t_s, t_e) = build_graph(ctx, &thenb.stmts, Cont::Join, table, rng, reserved);
			let mut els = Vec::new();
			for (_, b) in elsifs.iter() {
				els.push(build_graph(ctx, &b.stmts, Cont::Join, table, rng, reserved));
			}
			let else_pair = match elseb {
				Some(b) => Some(build_graph(ctx, &b.stmts, Cont::Join, table, rng, reserved)),
				None => None,
			};
			let if_cond = ctx.push(BlkBody::Cond((**cond).clone()));
			let c1: Vec<usize> = elsifs
				.iter()
				.map(|(c, _)| ctx.push(BlkBody::Cond(c.clone())))
				.collect();
			let exit = ctx.push(BlkBody::Join);

			ctx.blocks[cur].succ.push(Succ::B(if_cond));
			let else_target_0 = match c1.first() {
				Some(c) => *c,
				None => match else_pair {
					Some((s, _)) => s,
					None => exit,
				},
			};
			ctx.blocks[if_cond].succ = vec![Succ::B(t_s), Succ::B(else_target_0)];
			ctx.blocks[t_e].succ = vec![Succ::B(exit)];
			for (i, c) in c1.iter().enumerate() {
				let target = match c1.get(i + 1) {
					Some(nc) => *nc,
					None => match else_pair {
						Some((s, _)) => s,
						None => exit,
					},
				};
				ctx.blocks[*c].succ = vec![Succ::B(els[i].0), Succ::B(target)];
				ctx.blocks[els[i].1].succ = vec![Succ::B(exit)];
			}
			if let Some((s, e)) = else_pair {
				ctx.blocks[e].succ = vec![Succ::B(exit)];
				let _ = s;
			}
			Some(exit)
		}
		Stmt::While { .. } | Stmt::Repeat { .. } | Stmt::ForNum { .. } | Stmt::ForGen { .. } => {
			make_loop(ctx, cur, stmt, table, rng, reserved)
		}
		other => {
			let b = ctx.push(BlkBody::Stmt(other.clone()));
			ctx.blocks[cur].succ.push(Succ::B(b));
			Some(b)
		}
	}
}

fn make_loop(
	ctx: &mut Ctx,
	cur: usize,
	stmt: &Stmt,
	table: &mut SymTable,
	rng: &mut Rng,
	reserved: &mut HashSet<String>,
) -> Option<usize> {
	let (kind, init_local) = match stmt {
		Stmt::While { cond, .. } => (LoopKind::While { cond: (**cond).clone() }, None),
		Stmt::Repeat { cond, .. } => (LoopKind::Repeat { cond: (**cond).clone() }, None),
		Stmt::ForNum {
			var_sym,
			start,
			limit,
			step,
			..
		} => {
			let (s_cur, _) = new_rand_sym(table, rng, reserved);
			let (s_lim, _) = new_rand_sym(table, rng, reserved);
			let (s_stp, _) = new_rand_sym(table, rng, reserved);
			let step = step
				.as_ref()
				.map(|b| (**b).clone())
				.unwrap_or(Expr::Num {
					value: 1.0,
					isfloat: false,
				});
			let init = Stmt::Local {
				names: vec![
					table.name_of(s_cur).to_string(),
					table.name_of(s_lim).to_string(),
					table.name_of(s_stp).to_string(),
				],
				syms: vec![s_cur, s_lim, s_stp],
				values: vec![Some((**start).clone()), Some((**limit).clone()), Some(step)],
			};
			(
				LoopKind::ForNum {
					var: *var_sym,
					lim: s_lim,
					stp: s_stp,
					cur: s_cur,
				},
				Some(init),
			)
		}
		Stmt::ForGen { syms, iters, .. } => {
			let (s_it, _) = new_rand_sym(table, rng, reserved);
			let (s_stt, _) = new_rand_sym(table, rng, reserved);
			let (s_ctl, _) = new_rand_sym(table, rng, reserved);
			let init = Stmt::Local {
				names: vec![
					table.name_of(s_it).to_string(),
					table.name_of(s_stt).to_string(),
					table.name_of(s_ctl).to_string(),
				],
				syms: vec![s_it, s_stt, s_ctl],
				values: iters.iter().map(|e| Some(e.clone())).collect(),
			};
			(
				LoopKind::ForGen {
					vars: syms.clone(),
					it: s_it,
					stt: s_stt,
					ctl: s_ctl,
				},
				Some(init),
			)
		}
		_ => unreachable!("make_loop called on a non-loop"),
	};

	let mut prev = cur;
	if let Some(init) = init_local {
		let b = ctx.push(BlkBody::Stmt(init));
		ctx.blocks[cur].succ.push(Succ::B(b));
		prev = b;
	}

	let node_blk = ctx.push(BlkBody::Loop(Box::new(LoopNode {
		kind: kind.clone(),
		blocks: Vec::new(),
		start: 0,
	})));
	ctx.blocks[prev].succ.push(Succ::B(node_blk));
	let exit_blk = ctx.push(BlkBody::Join);
	ctx.blocks[node_blk].succ.push(Succ::B(exit_blk));

	let body = match stmt {
		Stmt::While { body, .. }
		| Stmt::Repeat { body, .. }
		| Stmt::ForNum { body, .. }
		| Stmt::ForGen { body, .. } => body,
		_ => unreachable!(),
	};
	let cont = match &kind {
		LoopKind::While { .. } | LoopKind::ForNum { .. } | LoopKind::ForGen { .. } => Cont::Done,
		LoopKind::Repeat { cond } => Cont::Check { cond: cond.clone() },
	};
	let mut ictx = Ctx::new(true);
	let (start, cont_blk) = build_graph(&mut ictx, &body.stmts, cont, table, rng, reserved);

	for b in ictx.blocks.iter_mut() {
		b.succ = b
			.succ
			.iter()
			.map(|s| match s {
				Succ::Cont => Succ::B(cont_blk),
				other => other.clone(),
			})
			.collect();
	}

	let node = LoopNode {
		kind,
		blocks: ictx.blocks,
		start,
	};
	if let BlkBody::Loop(ref mut ln) = ctx.blocks[node_blk].body {
		*ln = Box::new(node);
	}
	Some(exit_blk)
}

fn reachable(blocks: &[Blk], start: usize) -> HashSet<usize> {
	let mut seen: HashSet<usize> = HashSet::new();
	let mut q: VecDeque<usize> = VecDeque::new();
	q.push_back(start);
	seen.insert(start);
	while let Some(i) = q.pop_front() {
		for &s in &blocks[i].succ {
			if let Succ::B(x) = s {
				if seen.insert(x) {
					q.push_back(x);
				}
			}
		}
	}
	seen
}


fn collect_expr_syms(e: &Expr, out: &mut HashSet<SymId>) {
	match e {
		Expr::Ident { sym: Some(s), .. } => {
			out.insert(*s);
		}
		Expr::Dot { obj, .. } => collect_expr_syms(obj, out),
		Expr::Index { obj, idx } => {
			collect_expr_syms(obj, out);
			collect_expr_syms(idx, out);
		}
		Expr::Call { func, args } => {
			collect_expr_syms(func, out);
			for a in args {
				collect_expr_syms(a, out);
			}
		}
		Expr::Method { obj, args, .. } => {
			collect_expr_syms(obj, out);
			for a in args {
				collect_expr_syms(a, out);
			}
		}
		Expr::Paren { e } | Expr::Un { e, .. } => collect_expr_syms(e, out),
		Expr::Bin { l, r, .. } => {
			collect_expr_syms(l, out);
			collect_expr_syms(r, out);
		}
		Expr::IfExpr { arms, elseb } => {
			for (cond, value) in arms {
				collect_expr_syms(cond, out);
				collect_expr_syms(value, out);
			}
			collect_expr_syms(elseb, out);
		}
		Expr::Table { fields } => {
			for f in fields {
				match f {
					TableField::Array(e) => collect_expr_syms(e, out),
					TableField::Key { key, value } => {
						collect_expr_syms(key, out);
						collect_expr_syms(value, out);
					}
				}
			}
		}
		_ => {}
	}
}

fn collect_expr_syms_deep(e: &Expr, out: &mut HashSet<SymId>) {
	match e {
		Expr::Function { body, .. } => {
			collect_block_syms_deep(body, out);
		}
		other => {
			let mut tmp = HashSet::new();
			collect_expr_syms(other, &mut tmp);
			out.extend(tmp);
			match other {
				Expr::Dot { obj, .. } => collect_expr_syms_deep(obj, out),
				Expr::Index { obj, idx } => {
					collect_expr_syms_deep(obj, out);
					collect_expr_syms_deep(idx, out);
				}
				Expr::Call { func, args } => {
					collect_expr_syms_deep(func, out);
					for a in args {
						collect_expr_syms_deep(a, out);
					}
				}
				Expr::Method { obj, args, .. } => {
					collect_expr_syms_deep(obj, out);
					for a in args {
						collect_expr_syms_deep(a, out);
					}
				}
				Expr::Bin { l, r, .. } => {
					collect_expr_syms_deep(l, out);
					collect_expr_syms_deep(r, out);
				}
				Expr::Paren { e } | Expr::Un { e, .. } => collect_expr_syms_deep(e, out),
				Expr::IfExpr { arms, elseb } => {
					for (cond, value) in arms {
						collect_expr_syms_deep(cond, out);
						collect_expr_syms_deep(value, out);
					}
					collect_expr_syms_deep(elseb, out);
				}
				Expr::Table { fields } => {
					for f in fields {
						match f {
							TableField::Array(e) => collect_expr_syms_deep(e, out),
							TableField::Key { key, value } => {
								collect_expr_syms_deep(key, out);
								collect_expr_syms_deep(value, out);
							}
						}
					}
				}
				_ => {}
			}
		}
	}
}

fn collect_stmt_syms_deep(s: &Stmt, out: &mut HashSet<SymId>) {
	match s {
		Stmt::Local { values, .. } => {
			for v in values {
				if let Some(e) = v {
					collect_expr_syms_deep(e, out);
				}
			}
		}
		Stmt::LocalFunc { func, .. } => collect_block_syms_deep(&func.body, out),
		Stmt::FuncDecl { func, obj, .. } => {
			if let Some(o) = obj {
				collect_expr_syms_deep(o, out);
			}
			collect_block_syms_deep(&func.body, out);
		}
		Stmt::Assign { targets, values } => {
			for t in targets {
				collect_expr_syms_deep(t, out);
			}
			for v in values {
				collect_expr_syms_deep(v, out);
			}
		}
		Stmt::ExprStmt(e) => collect_expr_syms_deep(e, out),
		Stmt::If { cond, thenb, elsifs, elseb } => {
			collect_expr_syms_deep(cond, out);
			collect_block_syms_deep(thenb, out);
			for (c, b) in elsifs {
				collect_expr_syms_deep(c, out);
				collect_block_syms_deep(b, out);
			}
			if let Some(b) = elseb {
				collect_block_syms_deep(b, out);
			}
		}
		Stmt::While { cond, body } => {
			collect_expr_syms_deep(cond, out);
			collect_block_syms_deep(body, out);
		}
		Stmt::Repeat { body, cond } => {
			collect_block_syms_deep(body, out);
			collect_expr_syms_deep(cond, out);
		}
		Stmt::ForNum {
			start, limit, step, body, ..
		} => {
			collect_expr_syms_deep(start, out);
			collect_expr_syms_deep(limit, out);
			if let Some(st) = step {
				collect_expr_syms_deep(st, out);
			}
			collect_block_syms_deep(body, out);
		}
		Stmt::ForGen { iters, body, .. } => {
			for i in iters {
				collect_expr_syms_deep(i, out);
			}
			collect_block_syms_deep(body, out);
		}
		Stmt::Do(b) => collect_block_syms_deep(b, out),
		Stmt::Return(es) => {
			for e in es {
				collect_expr_syms_deep(e, out);
			}
		}
		_ => {}
	}
}

fn collect_block_syms_deep(b: &Block, out: &mut HashSet<SymId>) {
	for s in &b.stmts {
		collect_stmt_syms_deep(s, out);
	}
}

fn loop_node_refs(node: &LoopNode, out: &mut HashSet<SymId>) {
	match &node.kind {
		LoopKind::While { cond } | LoopKind::Repeat { cond } => collect_expr_syms_deep(cond, out),
		LoopKind::ForNum { var, lim, stp, cur } => {
			out.insert(*var);
			out.insert(*lim);
			out.insert(*stp);
			out.insert(*cur);
		}
		LoopKind::ForGen { vars, it, stt, ctl } => {
			for v in vars {
				out.insert(*v);
			}
			out.insert(*it);
			out.insert(*stt);
			out.insert(*ctl);
		}
	}
	for b in &node.blocks {
		match &b.body {
			BlkBody::Stmt(s) => collect_stmt_syms_deep(s, out),
			BlkBody::Cond(e) => collect_expr_syms_deep(e, out),
			BlkBody::Return(es) => {
				for e in es {
					collect_expr_syms_deep(e, out);
				}
			}
			BlkBody::Loop(n) => loop_node_refs(n, out),
			_ => {}
		}
	}
}

fn block_own_refs(b: &Blk, out: &mut HashSet<SymId>) {
	match &b.body {
		BlkBody::Stmt(s) => collect_stmt_syms_shallow(s, out),
		BlkBody::Cond(e) => collect_expr_syms(e, out),
		BlkBody::Return(es) => {
			for e in es {
				collect_expr_syms(e, out);
			}
		}
		BlkBody::Loop(node) => loop_node_refs(node, out),
		_ => {}
	}
}

fn collect_stmt_syms_shallow(s: &Stmt, out: &mut HashSet<SymId>) {
	match s {
		Stmt::Local { values, .. } => {
			for v in values {
				if let Some(e) = v {
					collect_expr_syms(e, out);
				}
			}
		}
		Stmt::Assign { targets, values } => {
			for t in targets {
				collect_expr_syms(t, out);
			}
			for v in values {
				collect_expr_syms(v, out);
			}
		}
		Stmt::ExprStmt(e) => collect_expr_syms(e, out),
		_ => {}
	}
}


fn machine_block(
	block: &Block,
	table: &mut SymTable,
	rng: &mut Rng,
	reserved: &mut HashSet<String>,
) -> Block {
	let mut ctx = Ctx::new(false);
	let (start, exit) = build_graph(&mut ctx, &block.stmts, Cont::Join, table, rng, reserved);
	let n = ctx.blocks.len();
	let reach = reachable(&ctx.blocks, start);

	let mut decl: HashMap<SymId, usize> = HashMap::new();
	let mut refs: HashMap<SymId, HashSet<usize>> = HashMap::new();
	for (i, b) in ctx.blocks.iter().enumerate() {
		if !reach.contains(&i) {
			continue;
		}
		match &b.body {
			BlkBody::Stmt(Stmt::Local { syms, .. }) => {
				for &s in syms {
					decl.entry(s).or_insert(i);
				}
			}
			BlkBody::Stmt(Stmt::LocalFunc { sym, .. }) => {
				decl.entry(*sym).or_insert(i);
			}
			_ => {}
		}
		let mut syms = HashSet::new();
		block_own_refs(b, &mut syms);
		for s in syms {
			refs.entry(s).or_default().insert(i);
		}
		let mut fns = Vec::new();
		collect_functions_stmt(b, &mut fns);
		for f in fns {
			let mut syms2 = HashSet::new();
			for st in &f.stmts {
				collect_stmt_syms_deep(st, &mut syms2);
			}
			for s in syms2 {
				refs.entry(s).or_default().insert(i);
			}
		}
	}

	let mut hoisted: HashSet<SymId> = HashSet::new();
	for (s, d) in &decl {
		if let Some(rs) = refs.get(s) {
			for r in rs {
				if *r != *d {
					hoisted.insert(*s);
					break;
				}
			}
		}
	}
	let mut hoisted_stmts: HashSet<usize> = HashSet::new();
	for (s, d) in &decl {
		if hoisted.contains(s) {
			hoisted_stmts.insert(*d);
		}
	}

	let mut top_locals: Vec<(SymId, String)> = Vec::new();
	{
		let mut seen = HashSet::new();
		for i in 0..n {
			if !reach.contains(&i) || !hoisted_stmts.contains(&i) {
				continue;
			}
			let pairs: Vec<(SymId, String)> = match &ctx.blocks[i].body {
				BlkBody::Stmt(Stmt::Local { names, syms, .. }) => names
					.iter()
					.zip(syms.iter())
					.map(|(nm, s)| (*s, nm.clone()))
					.collect(),
				BlkBody::Stmt(Stmt::LocalFunc { sym, .. }) => {
					vec![(*sym, table.name_of(*sym).to_string())]
				}
				_ => Vec::new(),
			};
			for (s, nm) in pairs {
				if seen.insert(s) {
					top_locals.push((s, nm));
				}
			}
		}
	}

	let (st_sym, st_name) = new_rand_sym(table, rng, reserved);
	let mut temps: HashMap<usize, (SymId, String)> = HashMap::new();
	for i in 0..n {
		if reach.contains(&i) && matches!(ctx.blocks[i].body, BlkBody::Cond(_)) {
			let (s, nm) = new_rand_sym(table, rng, reserved);
			temps.insert(i, (s, nm));
		}
	}

	let mut used: HashSet<u32> = HashSet::new();
	let mut ids: HashMap<usize, u32> = HashMap::new();
	for i in 0..n {
		if !reach.contains(&i) {
			continue;
		}
		loop {
			let id = rng.int(1000, 4_294_967_295) as u32;
			if used.insert(id) {
				ids.insert(i, id);
				break;
			}
		}
	}

	let st_ident = || Expr::Ident {
		name: st_name.clone(),
		sym: Some(st_sym),
	};
	let assign_st = |target: usize| Stmt::Assign {
		targets: vec![st_ident()],
		values: vec![num(ids[&target])],
	};

	let mut branches: Vec<(usize, Vec<Stmt>)> = Vec::new();
	for i in 0..n {
		if !reach.contains(&i) {
			continue;
		}
		let code = match &ctx.blocks[i].body {
			BlkBody::Stmt(s) => {
				let mut v = Vec::new();
				match s {
					Stmt::LocalFunc { sym, func, .. }
						if hoisted_stmts.contains(&i) =>
					{
						v.push(Stmt::Assign {
							targets: vec![Expr::Ident {
								name: table.name_of(*sym).to_string(),
								sym: Some(*sym),
							}],
							values: vec![Expr::Function {
								params: func.params.clone(),
								param_syms: func.param_syms.clone(),
								vararg: func.vararg,
								body: func.body.clone(),
							}],
						});
					}
					Stmt::Local { names, syms, values } if hoisted_stmts.contains(&i) => {
						let mut vals: Vec<Expr> = values.iter().filter_map(|x| x.clone()).collect();
						if vals.is_empty() {
							vals.push(Expr::Nil);
						}
						v.push(Stmt::Assign {
							targets: names
								.iter()
								.zip(syms.iter())
								.map(|(nm, sy)| Expr::Ident {
									name: nm.clone(),
									sym: Some(*sy),
								})
								.collect(),
							values: vals,
						});
					}
					other => v.push(other.clone()),
				}
				if ctx.blocks[i].succ.len() == 1 {
					if let Succ::B(x) = ctx.blocks[i].succ[0] {
						v.push(assign_st(x));
					}
				}
				v
			}
			BlkBody::Return(es) => vec![Stmt::Return(es.clone())],
			BlkBody::Join => {
				if i == exit || ctx.blocks[i].succ.is_empty() {
					vec![Stmt::Break]
				} else if let Succ::B(x) = ctx.blocks[i].succ[0] {
					vec![assign_st(x)]
				} else {
					vec![Stmt::Break]
				}
			}
			BlkBody::Cond(e) => {
				let succ = ctx.blocks[i].succ.clone();
				let (t, f) = (succ[0].clone(), succ[1].clone());
				let (tmp_sym, tmp_name) = temps[&i].clone();
				let tmp_ident = Expr::Ident {
					name: tmp_name.clone(),
					sym: Some(tmp_sym),
				};
				let tail = |su: Succ| -> Vec<Stmt> {
					match su {
						Succ::B(x) => vec![assign_st(x)],
						_ => vec![Stmt::Break],
					}
				};
				vec![
					Stmt::Local {
						names: vec![tmp_name],
						syms: vec![tmp_sym],
						values: vec![Some(e.clone())],
					},
					Stmt::If {
						cond: Box::new(tmp_ident),
						thenb: Block { stmts: tail(t) },
						elsifs: Vec::new(),
						elseb: Some(Block { stmts: tail(f) }),
					},
				]
			}
			BlkBody::Loop(node) => {
				let exit = match ctx.blocks[i].succ[0] {
					Succ::B(x) => x,
					_ => panic!("loop node must have a single successor"),
				};
				emit_loop(node, &st_ident(), ids[&i], ids[&exit], &mut used, table, rng, reserved)
			}
		};
		branches.push((i, code));
	}

	let dispatch = dispatch_chain(&branches, &ids, &st_ident, rng);

	let mut top = Vec::new();
	top.push(Stmt::Local {
		names: vec![st_name],
		syms: vec![st_sym],
		values: vec![Some(num(ids[&start]))],
	});
	if !top_locals.is_empty() {
		top.push(Stmt::Local {
			names: top_locals.iter().map(|(_, nm)| nm.clone()).collect(),
			syms: top_locals.iter().map(|(s, _)| *s).collect(),
			values: Vec::new(),
		});
	}
	top.push(Stmt::While {
		cond: Box::new(Expr::Bool { value: true }),
		body: Block {
			stmts: vec![dispatch],
		},
	});
	Block { stmts: top }
}

fn dispatch_chain(
	branches: &[(usize, Vec<Stmt>)],
	ids: &HashMap<usize, u32>,
	st_ident: &dyn Fn() -> Expr,
	rng: &mut Rng,
) -> Stmt {
	let order: Vec<usize> = {
		let mut idx: Vec<usize> = (0..branches.len()).collect();
		rng.shuffle(&mut idx);
		idx
	};
	let mut chain_conds: Vec<Expr> = Vec::new();
	let mut chain_bodies: Vec<Block> = Vec::new();
	for &k in &order {
		let (i, code) = &branches[k];
		chain_conds.push(Expr::Bin {
			op: BinOp::Eq,
			l: Box::new(st_ident()),
			r: Box::new(num(ids[i])),
		});
		chain_bodies.push(Block { stmts: code.clone() });
	}
	let first = chain_bodies
		.first()
		.cloned()
		.expect("machine has at least one block");
	let elsifs: Vec<(Expr, Block)> = chain_conds[1..]
		.iter()
		.zip(chain_bodies[1..].iter())
		.map(|(c, b)| (c.clone(), b.clone()))
		.collect();
	Stmt::If {
		cond: Box::new(chain_conds[0].clone()),
		thenb: first,
		elsifs,
		elseb: None,
	}
}

fn emit_loop(
	node: &LoopNode,
	state: &Expr,
	reenter_id: u32,
	exit_id: u32,
	used: &mut HashSet<u32>,
	table: &mut SymTable,
	rng: &mut Rng,
	reserved: &mut HashSet<String>,
) -> Vec<Stmt> {
	let n = node.blocks.len();
	let reach = reachable(&node.blocks, node.start);

	let mut ids: HashMap<usize, u32> = HashMap::new();
	for i in 0..n {
		if !reach.contains(&i) {
			continue;
		}
		loop {
			let id = rng.int(1000, 4_294_967_295) as u32;
			if used.insert(id) {
				ids.insert(i, id);
				break;
			}
		}
	}

	let mut temps: HashMap<usize, (SymId, String)> = HashMap::new();
	for i in 0..n {
		if reach.contains(&i) && matches!(node.blocks[i].body, BlkBody::Cond(_)) {
			let (s, nm) = new_rand_sym(table, rng, reserved);
			temps.insert(i, (s, nm));
		}
	}

	let mut perpass: Vec<(SymId, String)> = Vec::new();
	{
		let mut seen = HashSet::new();
		for i in 0..n {
			if !reach.contains(&i) {
				continue;
			}
			let pairs: Vec<(SymId, String)> = match &node.blocks[i].body {
				BlkBody::Stmt(Stmt::Local { names, syms, .. }) => names
					.iter()
					.zip(syms.iter())
					.map(|(nm, s)| (*s, nm.clone()))
					.collect(),
				BlkBody::Stmt(Stmt::LocalFunc { sym, .. }) => {
					vec![(*sym, table.name_of(*sym).to_string())]
				}
				_ => Vec::new(),
			};
			for (s, nm) in pairs {
				if seen.insert(s) {
					perpass.push((s, nm));
				}
			}
		}
	}

	let (stl_sym, stl_name) = new_rand_sym(table, rng, reserved);
	let (brk_sym, brk_name) = new_rand_sym(table, rng, reserved);
	let stl_ident = || Expr::Ident {
		name: stl_name.clone(),
		sym: Some(stl_sym),
	};
	let brk_ident = || Expr::Ident {
		name: brk_name.clone(),
		sym: Some(brk_sym),
	};
	let set_state = |id: u32| Stmt::Assign {
		targets: vec![state.clone()],
		values: vec![num(id)],
	};

	let mut branches: Vec<(usize, Vec<Stmt>)> = Vec::new();
	for i in 0..n {
		if !reach.contains(&i) {
			continue;
		}
		let code = match &node.blocks[i].body {
			BlkBody::Stmt(s) => {
				let mut v = Vec::new();
				match s {
					Stmt::Local { names, syms, values } => {
						let mut vals: Vec<Expr> = values.iter().filter_map(|x| x.clone()).collect();
						if vals.is_empty() {
							vals.push(Expr::Nil);
						}
						v.push(Stmt::Assign {
							targets: names
								.iter()
								.zip(syms.iter())
								.map(|(nm, sy)| Expr::Ident {
									name: nm.clone(),
									sym: Some(*sy),
								})
								.collect(),
							values: vals,
						});
					}
					Stmt::LocalFunc { sym, func, .. } => {
						v.push(Stmt::Assign {
							targets: vec![Expr::Ident {
								name: table.name_of(*sym).to_string(),
								sym: Some(*sym),
							}],
							values: vec![Expr::Function {
								params: func.params.clone(),
								param_syms: func.param_syms.clone(),
								vararg: func.vararg,
								body: func.body.clone(),
							}],
						});
					}
					other => v.push(other.clone()),
				}
				if node.blocks[i].succ.len() == 1 {
					match node.blocks[i].succ[0] {
						Succ::B(x) => v.push(Stmt::Assign {
							targets: vec![stl_ident()],
							values: vec![num(ids[&x])],
						}),
						Succ::Brk => {
							v.push(Stmt::Assign {
								targets: vec![brk_ident()],
								values: vec![Expr::Bool { value: true }],
							});
							v.push(Stmt::Break);
						}
						Succ::Cont => unreachable!("continue must be rewired"),
					}
				}
				v
			}
			BlkBody::Cond(e) => {
				let succ = node.blocks[i].succ.clone();
				let (t, f) = (succ[0].clone(), succ[1].clone());
				let (tmp_sym, tmp_name) = temps[&i].clone();
				let tmp_ident = Expr::Ident {
					name: tmp_name.clone(),
					sym: Some(tmp_sym),
				};
				let tail = |su: Succ| -> Vec<Stmt> {
					match su {
						Succ::B(x) => vec![Stmt::Assign {
							targets: vec![stl_ident()],
							values: vec![num(ids[&x])],
						}],
						Succ::Brk => vec![
							Stmt::Assign {
								targets: vec![brk_ident()],
								values: vec![Expr::Bool { value: true }],
							},
							Stmt::Break,
						],
						Succ::Cont => unreachable!("continue must be rewired"),
					}
				};
				vec![
					Stmt::Local {
						names: vec![tmp_name],
						syms: vec![tmp_sym],
						values: vec![Some(e.clone())],
					},
					Stmt::If {
						cond: Box::new(tmp_ident),
						thenb: Block { stmts: tail(t) },
						elsifs: Vec::new(),
						elseb: Some(Block { stmts: tail(f) }),
					},
				]
			}
			BlkBody::Join => {
				match node.blocks[i].succ.first() {
					None => {
						vec![Stmt::Break]
					}
					Some(Succ::B(x)) => vec![Stmt::Assign {
						targets: vec![stl_ident()],
						values: vec![num(ids[x])],
					}],
					Some(Succ::Brk) => vec![
						Stmt::Assign {
							targets: vec![brk_ident()],
							values: vec![Expr::Bool { value: true }],
						},
						Stmt::Break,
					],
					Some(Succ::Cont) => unreachable!("continue must be rewired"),
				}
			}
			BlkBody::Return(es) => vec![Stmt::Return(es.clone())],
			BlkBody::Loop(n2) => {
				let exit = match node.blocks[i].succ[0] {
					Succ::B(x) => x,
					_ => panic!("loop node must have a single successor"),
				};
				let stl = stl_ident();
				emit_loop(n2, &stl, ids[&i], ids[&exit], used, table, rng, reserved)
			}
		};
		branches.push((i, code));
	}

	let dispatch = dispatch_chain(&branches, &ids, &stl_ident, rng);
	let dispatch_while = Stmt::While {
		cond: Box::new(Expr::Bool { value: true }),
		body: Block {
			stmts: vec![dispatch],
		},
	};

	let stl_init = Stmt::Local {
		names: vec![stl_name.clone()],
		syms: vec![stl_sym],
		values: vec![Some(num(ids[&node.start]))],
	};
	let brk_init = Stmt::Local {
		names: vec![brk_name.clone()],
		syms: vec![brk_sym],
		values: vec![Some(Expr::Bool { value: false })],
	};
	let perpass_decl = |syms: &[(SymId, String)]| -> Option<Stmt> {
		if syms.is_empty() {
			None
		} else {
			Some(Stmt::Local {
				names: syms.iter().map(|(_, nm)| nm.clone()).collect(),
				syms: syms.iter().map(|(s, _)| *s).collect(),
				values: Vec::new(),
			})
		}
	};
	let broke_check = || {
		Stmt::If {
			cond: Box::new(brk_ident()),
			thenb: Block {
				stmts: vec![set_state(exit_id)],
			},
			elsifs: Vec::new(),
			elseb: Some(Block { stmts: vec![set_state(reenter_id)] }),
		}
	};

	match &node.kind {
		LoopKind::ForNum { var, lim, stp, cur } => {
			let lim_i = ident(*lim, table);
			let stp_i = ident(*stp, table);
			let cur_i = ident(*cur, table);
			let exceeded = Expr::Bin {
				op: BinOp::Or,
				l: Box::new(Expr::Bin {
					op: BinOp::And,
					l: Box::new(Expr::Bin {
						op: BinOp::Ge,
						l: Box::new(stp_i.clone()),
						r: Box::new(num(0)),
					}),
					r: Box::new(Expr::Bin {
						op: BinOp::Gt,
						l: Box::new(cur_i.clone()),
						r: Box::new(lim_i.clone()),
					}),
				}),
				r: Box::new(Expr::Bin {
					op: BinOp::And,
					l: Box::new(Expr::Bin {
						op: BinOp::Lt,
						l: Box::new(stp_i.clone()),
						r: Box::new(num(0)),
					}),
					r: Box::new(Expr::Bin {
						op: BinOp::Lt,
						l: Box::new(cur_i.clone()),
						r: Box::new(lim_i.clone()),
					}),
				}),
			};
			let pass = Block {
				stmts: {
					let mut v = vec![Stmt::Local {
						names: vec![table.name_of(*var).to_string()],
						syms: vec![*var],
						values: vec![Some(cur_i.clone())],
					}];
					if let Some(p) = perpass_decl(&perpass) {
						v.push(p);
					}
					v.push(stl_init);
					v.push(brk_init);
					v.push(dispatch_while);
					v.push(Stmt::If {
						cond: Box::new(brk_ident()),
						thenb: Block {
							stmts: vec![set_state(exit_id)],
						},
						elsifs: Vec::new(),
						elseb: Some(Block {
							stmts: vec![
								Stmt::Assign {
									targets: vec![cur_i.clone()],
									values: vec![Expr::Bin {
										op: BinOp::Add,
										l: Box::new(cur_i),
										r: Box::new(stp_i),
									}],
								},
								set_state(reenter_id),
							],
						}),
					});
					v
				},
			};
			vec![Stmt::If {
				cond: Box::new(exceeded),
				thenb: Block {
					stmts: vec![set_state(exit_id)],
				},
				elsifs: Vec::new(),
				elseb: Some(pass),
			}]
		}
		LoopKind::ForGen { vars, it, stt, ctl } => {
			let it_i = ident(*it, table);
			let stt_i = ident(*stt, table);
			let ctl_i = ident(*ctl, table);
			let call = Expr::Call {
				func: Box::new(it_i.clone()),
				args: vec![stt_i, ctl_i],
			};
			let v0 = ident(vars[0], table);
			let pass = Block {
				stmts: {
					let mut v = vec![Stmt::Assign {
						targets: vec![ident(*ctl, table)],
						values: vec![v0.clone()],
					}];
					if let Some(p) = perpass_decl(&perpass) {
						v.push(p);
					}
					v.push(stl_init);
					v.push(brk_init);
					v.push(dispatch_while);
					v.push(broke_check());
					v
				},
			};
			vec![
				Stmt::Local {
					names: vars.iter().map(|s| table.name_of(*s).to_string()).collect(),
					syms: vars.clone(),
					values: vec![Some(call)],
				},
				Stmt::If {
					cond: Box::new(Expr::Bin {
						op: BinOp::Eq,
						l: Box::new(v0),
						r: Box::new(Expr::Nil),
					}),
					thenb: Block {
						stmts: vec![set_state(exit_id)],
					},
					elsifs: Vec::new(),
					elseb: Some(pass),
				},
			]
		}
		LoopKind::While { cond } => {
			let (t_sym, t_name) = new_rand_sym(table, rng, reserved);
			let pass = Block {
				stmts: {
					let mut v = Vec::new();
					if let Some(p) = perpass_decl(&perpass) {
						v.push(p);
					}
					v.push(stl_init);
					v.push(brk_init);
					v.push(dispatch_while);
					v.push(broke_check());
					v
				},
			};
			vec![
				Stmt::Local {
					names: vec![t_name],
					syms: vec![t_sym],
					values: vec![Some(cond.clone())],
				},
				Stmt::If {
					cond: Box::new(Expr::Ident {
						name: table.name_of(t_sym).to_string(),
						sym: Some(t_sym),
					}),
					thenb: pass,
					elsifs: Vec::new(),
					elseb: Some(Block {
						stmts: vec![set_state(exit_id)],
					}),
				},
			]
		}
		LoopKind::Repeat { .. } => {
			let mut v = Vec::new();
			if let Some(p) = perpass_decl(&perpass) {
				v.push(p);
			}
			v.push(stl_init);
			v.push(brk_init);
			v.push(dispatch_while);
			v.push(broke_check());
			v
		}
	}
}

fn collect_functions_stmt<'a>(b: &'a Blk, out: &mut Vec<&'a Block>) {
	match &b.body {
		BlkBody::Stmt(s) => collect_functions_stmt2(s, out),
		BlkBody::Cond(e) => collect_functions_expr(e, out),
		BlkBody::Return(es) => {
			for e in es {
				collect_functions_expr(e, out);
			}
		}
		_ => {}
	}
}

fn collect_functions_stmt2<'a>(s: &'a Stmt, out: &mut Vec<&'a Block>) {
	match s {
		Stmt::Local { values, .. } => {
			for v in values {
				if let Some(e) = v {
					collect_functions_expr(e, out);
				}
			}
		}
		Stmt::LocalFunc { func, .. } => {
			out.push(&func.body);
			collect_block_functions(&func.body, out);
		}
		Stmt::FuncDecl { func, obj, .. } => {
			if let Some(o) = obj {
				collect_functions_expr(o, out);
			}
			out.push(&func.body);
			collect_block_functions(&func.body, out);
		}
		Stmt::Assign { targets, values } => {
			for t in targets {
				collect_functions_expr(t, out);
			}
			for v in values {
				collect_functions_expr(v, out);
			}
		}
		Stmt::ExprStmt(e) => collect_functions_expr(e, out),
		Stmt::Return(es) => {
			for e in es {
				collect_functions_expr(e, out);
			}
		}
		Stmt::If { cond, thenb, elsifs, elseb } => {
			collect_functions_expr(cond, out);
			collect_block_functions(thenb, out);
			for (_, b) in elsifs {
				collect_block_functions(b, out);
			}
			if let Some(b) = elseb {
				collect_block_functions(b, out);
			}
		}
		Stmt::Do(b) => collect_block_functions(b, out),
		_ => {}
	}
}

fn collect_block_functions<'a>(b: &'a Block, out: &mut Vec<&'a Block>) {
	for s in &b.stmts {
		collect_functions_stmt2(s, out);
	}
}

fn collect_functions_expr<'a>(e: &'a Expr, out: &mut Vec<&'a Block>) {
	match e {
		Expr::Function { body, .. } => {
			out.push(body);
			collect_block_functions(body, out);
		}
		Expr::Paren { e } => collect_functions_expr(e, out),
		Expr::IfExpr { arms, elseb } => {
			for (cond, value) in arms {
				collect_functions_expr(cond, out);
				collect_functions_expr(value, out);
			}
			collect_functions_expr(elseb, out);
		}
		Expr::Dot { obj, .. } => collect_functions_expr(obj, out),
		Expr::Index { obj, idx } => {
			collect_functions_expr(obj, out);
			collect_functions_expr(idx, out);
		}
		Expr::Call { func, args } => {
			collect_functions_expr(func, out);
			for a in args {
				collect_functions_expr(a, out);
			}
		}
		Expr::Method { obj, args, .. } => {
			collect_functions_expr(obj, out);
			for a in args {
				collect_functions_expr(a, out);
			}
		}
		Expr::Bin { l, r, .. } => {
			collect_functions_expr(l, out);
			collect_functions_expr(r, out);
		}
		Expr::Un { e, .. } => collect_functions_expr(e, out),
		Expr::Table { fields } => {
			for f in fields {
				match f {
					TableField::Array(e) => collect_functions_expr(e, out),
					TableField::Key { key, value } => {
						collect_functions_expr(key, out);
						collect_functions_expr(value, out);
					}
				}
			}
		}
		_ => {}
	}
}


pub fn flatten_block(
	block: &mut Block,
	table: &mut SymTable,
	rng: &mut Rng,
	reserved: &mut HashSet<String>,
) {
	flatten_functions(block, table, rng, reserved);
	*block = machine_block(block, table, rng, reserved);
}

fn flatten_functions(
	block: &mut Block,
	table: &mut SymTable,
	rng: &mut Rng,
	reserved: &mut HashSet<String>,
) {
	for s in block.stmts.iter_mut() {
		match s {
			Stmt::LocalFunc { func, .. } => flatten_block(&mut func.body, table, rng, reserved),
			Stmt::FuncDecl { func, .. } => flatten_block(&mut func.body, table, rng, reserved),
			Stmt::Local { values, .. } => {
				for v in values.iter_mut() {
					if let Some(e) = v {
						flatten_expr(e, table, rng, reserved);
					}
				}
			}
			Stmt::Assign { values, .. } => {
				for v in values.iter_mut() {
					flatten_expr(v, table, rng, reserved);
				}
			}
			Stmt::ExprStmt(e) => flatten_expr(e, table, rng, reserved),
			Stmt::Return(es) => {
				for e in es.iter_mut() {
					flatten_expr(e, table, rng, reserved);
				}
			}
			_ => {}
		}
	}
}

fn flatten_expr(
	e: &mut Expr,
	table: &mut SymTable,
	rng: &mut Rng,
	reserved: &mut HashSet<String>,
) {
	match e {
		Expr::Paren { e } => flatten_expr(e, table, rng, reserved),
		Expr::IfExpr { arms, elseb } => {
			for (cond, value) in arms.iter_mut() {
				flatten_expr(cond, table, rng, reserved);
				flatten_expr(value, table, rng, reserved);
			}
			flatten_expr(elseb, table, rng, reserved);
		}
		Expr::Function { body, .. } => flatten_block(body, table, rng, reserved),
		Expr::Call { func, args } => {
			flatten_expr(func, table, rng, reserved);
			for a in args.iter_mut() {
				flatten_expr(a, table, rng, reserved);
			}
		}
		Expr::Method { obj, args, .. } => {
			flatten_expr(obj, table, rng, reserved);
			for a in args.iter_mut() {
				flatten_expr(a, table, rng, reserved);
			}
		}
		Expr::Bin { l, r, .. } => {
			flatten_expr(l, table, rng, reserved);
			flatten_expr(r, table, rng, reserved);
		}
		Expr::Un { e, .. } => flatten_expr(e, table, rng, reserved),
		Expr::Table { fields } => {
			for f in fields.iter_mut() {
				match f {
					TableField::Array(e) => flatten_expr(e, table, rng, reserved),
					TableField::Key { key, value } => {
						flatten_expr(key, table, rng, reserved);
						flatten_expr(value, table, rng, reserved);
					}
				}
			}
		}
		Expr::Dot { obj, .. } => flatten_expr(obj, table, rng, reserved),
		Expr::Index { obj, idx } => {
			flatten_expr(obj, table, rng, reserved);
			flatten_expr(idx, table, rng, reserved);
		}
		_ => {}
	}
}
