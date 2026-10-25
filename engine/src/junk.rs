
use crate::ast::*;
use crate::rng::Rng;
use crate::symtab::{Sym, SymTable};

fn num(v: i64) -> Expr {
	Expr::Num {
		value: v as f64,
		isfloat: false,
	}
}

fn junk_block(rng: &mut Rng, table: &mut SymTable) -> Block {
	let n1 = rng.int(1, 9999);
	let n2 = rng.int(1, 9999);
	let n3 = rng.int(2, 999);
	let n4 = rng.int(0, 9999);

	let (na, sa) = mk_local(table, "j");
	let (nb, sb) = mk_local(table, "j");
	let (nc, sc) = mk_local(table, "j");
	let (nd, sd) = mk_local(table, "j");

	let a = Expr::Ident {
		name: na.clone(),
		sym: Some(sa),
	};
	let b = Expr::Ident {
		name: nb.clone(),
		sym: Some(sb),
	};
	let s1 = Stmt::Local {
		names: vec![na],
		syms: vec![sa],
		values: vec![Some(Expr::Bin {
			op: BinOp::Add,
			l: Box::new(num(n1)),
			r: Box::new(num(n2)),
		})],
	};
	let s2 = Stmt::Local {
		names: vec![nb],
		syms: vec![sb],
		values: vec![Some(Expr::Bin {
			op: BinOp::Sub,
			l: Box::new(Expr::Bin {
				op: BinOp::Mul,
				l: Box::new(a.clone()),
				r: Box::new(num(n3)),
			}),
			r: Box::new(num(n4)),
		})],
	};
	let s3 = Stmt::If {
		cond: Box::new(Expr::Bin {
			op: BinOp::Ge,
			l: Box::new(Expr::Bin {
				op: BinOp::Mul,
				l: Box::new(b.clone()),
				r: Box::new(b.clone()),
			}),
			r: Box::new(num(0)),
		}),
		thenb: Block {
			stmts: vec![Stmt::Local {
				names: vec![nc],
				syms: vec![sc],
				values: vec![Some(Expr::Bin {
					op: BinOp::Sub,
					l: Box::new(b.clone()),
					r: Box::new(a.clone()),
				})],
			}],
		},
		elsifs: vec![],
		elseb: None,
	};
	let s4 = Stmt::Local {
		names: vec![nd],
		syms: vec![sd],
		values: vec![Some(Expr::Bin {
			op: BinOp::Sub,
			l: Box::new(Expr::Bin {
				op: BinOp::Add,
				l: Box::new(b.clone()),
				r: Box::new(num(1)),
			}),
			r: Box::new(num(1)),
		})],
	};
	Block {
		stmts: vec![s1, s2, s3, s4],
	}
}

fn mk_local(table: &mut SymTable, prefix: &str) -> (String, SymId) {
	let name = format!("_{prefix}_{}", table.syms.len());
	let id = table.syms.len() as SymId;
	table.syms.push(Sym {
		name: name.clone(),
		is_param: false,
		keep_name: false,
	});
	(name, id)
}

pub fn inject(block: &mut Block, table: &mut SymTable, rng: &mut Rng, n: usize) {
	let mut new_stmts = Vec::with_capacity(block.stmts.len() + n);
	for _ in 0..n {
		let j = junk_block(rng, table);
		new_stmts.extend(j.stmts);
	}
	for mut s in block.stmts.iter().cloned() {
		match &mut s {
			Stmt::LocalFunc { func, .. } => inject(&mut func.body, table, rng, n),
			Stmt::FuncDecl { func, .. } => inject(&mut func.body, table, rng, n),
			Stmt::Local { values, .. } => {
				for v in values.iter_mut() {
					if let Some(e) = v {
						if let Expr::Function { body, .. } = e {
							inject(body, table, rng, n);
						}
					}
				}
			}
			Stmt::ExprStmt(e) => {
				if let Expr::Function { body, .. } = e {
					inject(body, table, rng, n);
				}
			}
			_ => {}
		}
		let is_return = matches!(s, Stmt::Return(_));
		new_stmts.push(s);
		if !is_return && rng.int(0, 15) == 0 {
			let j = junk_block(rng, table);
			new_stmts.extend(j.stmts);
		}
	}
	block.stmts = new_stmts;
}
