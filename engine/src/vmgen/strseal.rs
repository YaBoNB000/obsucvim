
use crate::ast::{Block, Expr, Stmt, TableField};
use crate::rng::Rng;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SealStats {
	pub literals: usize,
	pub bytes: usize,
}

/// V19 R5-b（存在性硬化）：65521 域非线性折叠（交叉半积 + 高位回混，双精度
/// 精确域内最大中间值 < 2^25）+ 明文级联（前一明文字节并入推进）；
/// 镜像为模板 LoadStr handler 的同式 Lua 文本。替代「state%256」线性折叠——
/// 字符串池密封链的仿射相消面就此灭绝。
fn nl_mask_65521(state: u64) -> u8 {
	let mut x = (state + (state % 256) * (state / 256)) % 65_521;
	x = (x * 257 + x / 256) % 65_521;
	(((x % 256) + ((x / 256) % 256)) % 256) as u8
}

pub fn encode(plain: &[u8], seed: u16, step: u16) -> Vec<u8> {
	let mut state = seed as u64;
	let mut pv = 0u64;
	plain
		.iter()
		.enumerate()
		.map(|(i, &byte)| {
			state = (state * 257 + step as u64 + i as u64 + 1 + pv) % 65_521;
			let out = byte.wrapping_add(nl_mask_65521(state));
			pv = byte as u64;
			out
		})
		.collect()
}

pub fn decode(cipher: &[u8], seed: u16, step: u16) -> Vec<u8> {
	let mut state = seed as u64;
	let mut pv = 0u64;
	cipher
		.iter()
		.enumerate()
		.map(|(i, &byte)| {
			state = (state * 257 + step as u64 + i as u64 + 1 + pv) % 65_521;
			let out = byte.wrapping_sub(nl_mask_65521(state));
			pv = out as u64;
			out
		})
		.collect()
}

/// Phase 18 P1-1: identical plaintext literals share one sealed constant.
///
/// Before this change every occurrence drew a fresh (seed, step) pair, so two
/// occurrences of the same source string became two unrelated constants and a
/// "multi-use constant" could not even exist. Deduping by plaintext is what
/// makes the static reference count meaningful and lets the runtime cache pay
/// off (one decode per frame instead of one per reference).
pub fn seal_literals(block: &mut Block, rng: &mut Rng) -> SealStats {
	let mut stats = SealStats::default();
	let mut seen: std::collections::HashMap<Vec<u8>, (u16, u16)> = std::collections::HashMap::new();
	visit_block(block, rng, &mut stats, &mut seen);
	stats
}

/// Static reference count per sealed constant, keyed by ciphertext, over the
/// whole program (nested function bodies included).
pub fn sealed_total_counts(block: &Block) -> std::collections::HashMap<Vec<u8>, usize> {
	let mut n = std::collections::HashMap::new();
	count_block_total(block, &mut n);
	n
}

/// Ciphertexts whose *every* reference lives in one loop-free function body.
///
/// Phase 18 P1-2 scope: a cached slot may only be cleared after its last use
/// when (a) all references are inside the same function (the cache is
/// per-frame), (b) that function contains no loop construct at all (a stricter
/// reading of "no back edge between reference points" that can never clear a
/// slot that is still live), and (c) no reference sits inside a nested closure
/// (implied by (a): a nested function body is a different frame).
pub fn sealed_clearable(block: &Block) -> std::collections::HashSet<Vec<u8>> {
	let total = sealed_total_counts(block);
	let mut ok = std::collections::HashSet::new();
	collect_clearable(block, &total, &mut ok);
	ok
}

fn collect_clearable(
	block: &Block,
	total: &std::collections::HashMap<Vec<u8>, usize>,
	ok: &mut std::collections::HashSet<Vec<u8>>,
) {
	if !block_has_loop(block) {
		let mut here = std::collections::HashMap::new();
		count_block_in(block, &mut here);
		for (cipher, n) in here {
			if total.get(&cipher) == Some(&n) {
				ok.insert(cipher);
			}
		}
	}
	for stmt in &block.stmts {
		for body in nested_bodies(stmt) {
			collect_clearable(body, total, ok);
		}
	}
}

fn nested_bodies(stmt: &Stmt) -> Vec<&Block> {
	match stmt {
		Stmt::LocalFunc { func, .. } => vec![&func.body],
		Stmt::FuncDecl { func, .. } => vec![&func.body],
		_ => Vec::new(),
	}
}

fn block_has_loop(block: &Block) -> bool {
	block.stmts.iter().any(stmt_has_loop)
}

fn stmt_has_loop(stmt: &Stmt) -> bool {
	match stmt {
		Stmt::While { .. } | Stmt::Repeat { .. } | Stmt::ForNum { .. } | Stmt::ForGen { .. } => true,
		Stmt::Local { values, .. } => values.iter().flatten().any(expr_has_loop),
		// A nested function body is a different frame: a loop inside it cannot
		// re-execute the *enclosing* body's references, so it must not veto
		// clearing for the enclosing body (its own body is scored separately by
		// `collect_clearable` via `nested_bodies`).
		Stmt::LocalFunc { .. } => false,
		Stmt::FuncDecl { obj, .. } => obj.as_ref().is_some_and(expr_has_loop),
		Stmt::Assign { targets, values } => {
			targets.iter().chain(values).any(expr_has_loop)
		}
		Stmt::ExprStmt(expr) => expr_has_loop(expr),
		Stmt::If { cond, thenb, elsifs, elseb } => {
			expr_has_loop(cond)
				|| block_has_loop(thenb)
				|| elsifs.iter().any(|(c, b)| expr_has_loop(c) || block_has_loop(b))
				|| elseb.as_ref().is_some_and(block_has_loop)
		}
		Stmt::Do(body) => block_has_loop(body),
		Stmt::Return(values) => values.iter().any(expr_has_loop),
		Stmt::Break | Stmt::Continue => false,
	}
}

fn expr_has_loop(expr: &Expr) -> bool {
	match expr {
		// Nested closure: separate frame, see `stmt_has_loop`.
		Expr::Function { .. } => false,
		Expr::Paren { e } => expr_has_loop(e),
		Expr::Dot { obj, .. } => expr_has_loop(obj),
		Expr::Index { obj, idx } => expr_has_loop(obj) || expr_has_loop(idx),
		Expr::Call { func, args } => expr_has_loop(func) || args.iter().any(expr_has_loop),
		Expr::Method { obj, args, .. } => expr_has_loop(obj) || args.iter().any(expr_has_loop),
		Expr::Un { e, .. } => expr_has_loop(e),
		Expr::Bin { l, r, .. } => expr_has_loop(l) || expr_has_loop(r),
		Expr::IfExpr { arms, elseb } => {
			arms.iter().any(|(c, v)| expr_has_loop(c) || expr_has_loop(v)) || expr_has_loop(elseb)
		}
		Expr::Table { fields } => fields.iter().any(|field| match field {
			TableField::Array(value) => expr_has_loop(value),
			TableField::Key { key, value } => expr_has_loop(key) || expr_has_loop(value),
		}),
		_ => false,
	}
}

pub fn contains_sealed_literals(block: &Block) -> bool {
	block.stmts.iter().any(stmt_contains_sealed)
}

fn stmt_contains_sealed(stmt: &Stmt) -> bool {
	match stmt {
		Stmt::Local { values, .. } => values.iter().flatten().any(expr_contains_sealed),
		Stmt::LocalFunc { func, .. } => contains_sealed_literals(&func.body),
		Stmt::FuncDecl { obj, func, .. } => {
			obj.as_ref().is_some_and(expr_contains_sealed) || contains_sealed_literals(&func.body)
		}
		Stmt::Assign { targets, values } => targets.iter().chain(values).any(expr_contains_sealed),
		Stmt::ExprStmt(expr) => expr_contains_sealed(expr),
		Stmt::If { cond, thenb, elsifs, elseb } => {
			expr_contains_sealed(cond)
				|| contains_sealed_literals(thenb)
				|| elsifs.iter().any(|(cond, body)| {
					expr_contains_sealed(cond) || contains_sealed_literals(body)
				})
				|| elseb.as_ref().is_some_and(contains_sealed_literals)
		}
		Stmt::While { cond, body } => {
			expr_contains_sealed(cond) || contains_sealed_literals(body)
		}
		Stmt::Repeat { body, cond } => {
			contains_sealed_literals(body) || expr_contains_sealed(cond)
		}
		Stmt::ForNum { start, limit, step, body, .. } => {
			expr_contains_sealed(start)
				|| expr_contains_sealed(limit)
				|| step.as_ref().is_some_and(|step| expr_contains_sealed(step))
				|| contains_sealed_literals(body)
		}
		Stmt::ForGen { iters, body, .. } => {
			iters.iter().any(expr_contains_sealed) || contains_sealed_literals(body)
		}
		Stmt::Do(body) => contains_sealed_literals(body),
		Stmt::Return(values) => values.iter().any(expr_contains_sealed),
		Stmt::Break | Stmt::Continue => false,
	}
}

fn expr_contains_sealed(expr: &Expr) -> bool {
	match expr {
		Expr::SealedStr { .. } => true,
		Expr::Paren { e } => expr_contains_sealed(e),
		Expr::Dot { obj, .. } => expr_contains_sealed(obj),
		Expr::Index { obj, idx } => expr_contains_sealed(obj) || expr_contains_sealed(idx),
		Expr::Call { func, args } => {
			expr_contains_sealed(func) || args.iter().any(expr_contains_sealed)
		}
		Expr::Method { obj, args, .. } => {
			expr_contains_sealed(obj) || args.iter().any(expr_contains_sealed)
		}
		Expr::Un { e, .. } => expr_contains_sealed(e),
		Expr::Bin { l, r, .. } => expr_contains_sealed(l) || expr_contains_sealed(r),
		Expr::IfExpr { arms, elseb } => {
			arms.iter().any(|(cond, value)| {
				expr_contains_sealed(cond) || expr_contains_sealed(value)
			}) || expr_contains_sealed(elseb)
		}
		Expr::Table { fields } => fields.iter().any(|field| match field {
			TableField::Array(value) => expr_contains_sealed(value),
			TableField::Key { key, value } => {
				expr_contains_sealed(key) || expr_contains_sealed(value)
			}
		}),
		Expr::Function { body, .. } => contains_sealed_literals(body),
		_ => false,
	}
}

fn params_for(bytes: &[u8], rng: &mut Rng) -> (u16, u16) {
	loop {
		let seed = rng.int(0, 65_520) as u16;
		let step = rng.int(1, 65_520) as u16;
		if encode(bytes, seed, step) != bytes {
			return (seed, step);
		}
	}
}

fn seal_expr(
	expr: &mut Expr,
	rng: &mut Rng,
	stats: &mut SealStats,
	seen: &mut std::collections::HashMap<Vec<u8>, (u16, u16)>,
) {
	match expr {
		Expr::Str { bytes, .. } if !bytes.is_empty() => {
			let (seed, step) = seen.get(bytes.as_slice()).copied().unwrap_or_else(|| {
				let p = params_for(bytes, rng);
				seen.insert(bytes.clone(), p);
				p
			});
			let plain = std::mem::take(bytes);
			stats.literals += 1;
			stats.bytes += plain.len();
			*expr = Expr::SealedStr { bytes: plain, seed, step };
		}
		Expr::LongStr { bytes } if !bytes.is_empty() => {
			let (seed, step) = seen.get(bytes.as_slice()).copied().unwrap_or_else(|| {
				let p = params_for(bytes, rng);
				seen.insert(bytes.clone(), p);
				p
			});
			let plain = std::mem::take(bytes);
			stats.literals += 1;
			stats.bytes += plain.len();
			*expr = Expr::SealedStr { bytes: plain, seed, step };
		}
		Expr::Paren { e } => seal_expr(e, rng, stats, seen),
		Expr::Dot { obj, .. } => seal_expr(obj, rng, stats, seen),
		Expr::Index { obj, idx } => {
			seal_expr(obj, rng, stats, seen);
			seal_expr(idx, rng, stats, seen);
		}
		Expr::Call { func, args } => {
			seal_expr(func, rng, stats, seen);
			for arg in args {
				seal_expr(arg, rng, stats, seen);
			}
		}
		Expr::Method { obj, args, .. } => {
			seal_expr(obj, rng, stats, seen);
			for arg in args {
				seal_expr(arg, rng, stats, seen);
			}
		}
		Expr::Un { e, .. } => seal_expr(e, rng, stats, seen),
		Expr::Bin { l, r, .. } => {
			seal_expr(l, rng, stats, seen);
			seal_expr(r, rng, stats, seen);
		}
		Expr::IfExpr { arms, elseb } => {
			for (cond, value) in arms {
				seal_expr(cond, rng, stats, seen);
				seal_expr(value, rng, stats, seen);
			}
			seal_expr(elseb, rng, stats, seen);
		}
		Expr::Table { fields } => {
			for field in fields {
				match field {
					TableField::Array(value) => seal_expr(value, rng, stats, seen),
					TableField::Key { key, value } => {
						seal_expr(key, rng, stats, seen);
						seal_expr(value, rng, stats, seen);
					}
				}
			}
		}
		Expr::Function { body, .. } => visit_block(body, rng, stats, seen),
		_ => {}
	}
}

fn visit_block(
	block: &mut Block,
	rng: &mut Rng,
	stats: &mut SealStats,
	seen: &mut std::collections::HashMap<Vec<u8>, (u16, u16)>,
) {
	for stmt in &mut block.stmts {
		match stmt {
			Stmt::Local { values, .. } => {
				for value in values.iter_mut().flatten() {
					seal_expr(value, rng, stats, seen);
				}
			}
			Stmt::LocalFunc { func, .. } => visit_block(&mut func.body, rng, stats, seen),
			Stmt::FuncDecl { obj, func, .. } => {
				if let Some(obj) = obj {
					seal_expr(obj, rng, stats, seen);
				}
				visit_block(&mut func.body, rng, stats, seen);
			}
			Stmt::Assign { targets, values } => {
				for target in targets {
					seal_expr(target, rng, stats, seen);
				}
				for value in values {
					seal_expr(value, rng, stats, seen);
				}
			}
			Stmt::ExprStmt(value) => seal_expr(value, rng, stats, seen),
			Stmt::If { cond, thenb, elsifs, elseb } => {
				seal_expr(cond, rng, stats, seen);
				visit_block(thenb, rng, stats, seen);
				for (cond, body) in elsifs {
					seal_expr(cond, rng, stats, seen);
					visit_block(body, rng, stats, seen);
				}
				if let Some(body) = elseb {
					visit_block(body, rng, stats, seen);
				}
			}
			Stmt::While { cond, body } => {
				seal_expr(cond, rng, stats, seen);
				visit_block(body, rng, stats, seen);
			}
			Stmt::Repeat { body, cond } => {
				visit_block(body, rng, stats, seen);
				seal_expr(cond, rng, stats, seen);
			}
			Stmt::ForNum { start, limit, step, body, .. } => {
				seal_expr(start, rng, stats, seen);
				seal_expr(limit, rng, stats, seen);
				if let Some(step) = step {
					seal_expr(step, rng, stats, seen);
				}
				visit_block(body, rng, stats, seen);
			}
			Stmt::ForGen { iters, body, .. } => {
				for iter in iters {
					seal_expr(iter, rng, stats, seen);
				}
				visit_block(body, rng, stats, seen);
			}
			Stmt::Do(body) => visit_block(body, rng, stats, seen),
			Stmt::Return(values) => {
				for value in values {
					seal_expr(value, rng, stats, seen);
				}
			}
			Stmt::Break | Stmt::Continue => {}
		}
	}
}


// ---------------------------------------------------------------------------
// Phase 18 P1-1/P1-2: static reference analysis over sealed literals.
// ---------------------------------------------------------------------------

type RefMap = std::collections::HashMap<Vec<u8>, usize>;

fn count_block_total(block: &Block, n: &mut RefMap) {
	for stmt in &block.stmts {
		count_stmt_total(stmt, n);
	}
}

fn count_stmt_total(stmt: &Stmt, n: &mut RefMap) {
	match stmt {
		Stmt::Local { values, .. } => {
			for v in values.iter().flatten() {
				count_expr_total(v, n);
			}
		}
		Stmt::LocalFunc { func, .. } => count_block_total(&func.body, n),
		Stmt::FuncDecl { obj, func, .. } => {
			if let Some(o) = obj {
				count_expr_total(o, n);
			}
			count_block_total(&func.body, n);
		}
		Stmt::Assign { targets, values } => {
			for t in targets {
				count_expr_total(t, n);
			}
			for v in values {
				count_expr_total(v, n);
			}
		}
		Stmt::ExprStmt(e) => count_expr_total(e, n),
		Stmt::If { cond, thenb, elsifs, elseb } => {
			count_expr_total(cond, n);
			count_block_total(thenb, n);
			for (c, b) in elsifs {
				count_expr_total(c, n);
				count_block_total(b, n);
			}
			if let Some(b) = elseb {
				count_block_total(b, n);
			}
		}
		Stmt::While { cond, body } => {
			count_expr_total(cond, n);
			count_block_total(body, n);
		}
		Stmt::Repeat { body, cond } => {
			count_block_total(body, n);
			count_expr_total(cond, n);
		}
		Stmt::ForNum { start, limit, step, body, .. } => {
			count_expr_total(start, n);
			count_expr_total(limit, n);
			if let Some(st) = step {
				count_expr_total(st, n);
			}
			count_block_total(body, n);
		}
		Stmt::ForGen { iters, body, .. } => {
			for i in iters {
				count_expr_total(i, n);
			}
			count_block_total(body, n);
		}
		Stmt::Do(b) => count_block_total(b, n),
		Stmt::Return(es) => {
			for e in es {
				count_expr_total(e, n);
			}
		}
		Stmt::Break | Stmt::Continue => {}
	}
}

fn count_expr_total(e: &Expr, n: &mut RefMap) {
	match e {
		Expr::SealedStr { bytes, seed, step } => {
			*n.entry(encode(bytes, *seed, *step)).or_default() += 1;
		}
		Expr::Paren { e } => count_expr_total(e, n),
		Expr::Dot { obj, .. } => count_expr_total(obj, n),
		Expr::Index { obj, idx } => {
			count_expr_total(obj, n);
			count_expr_total(idx, n);
		}
		Expr::Call { func, args } => {
			count_expr_total(func, n);
			for a in args {
				count_expr_total(a, n);
			}
		}
		Expr::Method { obj, args, .. } => {
			count_expr_total(obj, n);
			for a in args {
				count_expr_total(a, n);
			}
		}
		Expr::Un { e, .. } => count_expr_total(e, n),
		Expr::Bin { l, r, .. } => {
			count_expr_total(l, n);
			count_expr_total(r, n);
		}
		Expr::IfExpr { arms, elseb } => {
			for (c, v) in arms {
				count_expr_total(c, n);
				count_expr_total(v, n);
			}
			count_expr_total(elseb, n);
		}
		Expr::Table { fields } => {
			for field in fields {
				match field {
					TableField::Array(value) => count_expr_total(value, n),
					TableField::Key { key, value } => {
						count_expr_total(key, n);
						count_expr_total(value, n);
					}
				}
			}
		}
		Expr::Function { body, .. } => count_block_total(body, n),
		_ => {}
	}
}

/// Counts references in this function body only (nested closures excluded).
fn count_block_in(block: &Block, n: &mut RefMap) {
	for stmt in &block.stmts {
		count_stmt_in(stmt, n);
	}
}

fn count_stmt_in(stmt: &Stmt, n: &mut RefMap) {
	match stmt {
		Stmt::Local { values, .. } => {
			for v in values.iter().flatten() {
				count_expr_in(v, n);
			}
		}
		Stmt::LocalFunc { .. } => {}
		Stmt::FuncDecl { obj, .. } => {
			if let Some(o) = obj {
				count_expr_in(o, n);
			}
		}
		Stmt::Assign { targets, values } => {
			for t in targets {
				count_expr_in(t, n);
			}
			for v in values {
				count_expr_in(v, n);
			}
		}
		Stmt::ExprStmt(e) => count_expr_in(e, n),
		Stmt::If { cond, thenb, elsifs, elseb } => {
			count_expr_in(cond, n);
			count_block_in(thenb, n);
			for (c, b) in elsifs {
				count_expr_in(c, n);
				count_block_in(b, n);
			}
			if let Some(b) = elseb {
				count_block_in(b, n);
			}
		}
		Stmt::While { cond, body } => {
			count_expr_in(cond, n);
			count_block_in(body, n);
		}
		Stmt::Repeat { body, cond } => {
			count_block_in(body, n);
			count_expr_in(cond, n);
		}
		Stmt::ForNum { start, limit, step, body, .. } => {
			count_expr_in(start, n);
			count_expr_in(limit, n);
			if let Some(st) = step {
				count_expr_in(st, n);
			}
			count_block_in(body, n);
		}
		Stmt::ForGen { iters, body, .. } => {
			for i in iters {
				count_expr_in(i, n);
			}
			count_block_in(body, n);
		}
		Stmt::Do(b) => count_block_in(b, n),
		Stmt::Return(es) => {
			for e in es {
				count_expr_in(e, n);
			}
		}
		Stmt::Break | Stmt::Continue => {}
	}
}

fn count_expr_in(e: &Expr, n: &mut RefMap) {
	match e {
		Expr::SealedStr { bytes, seed, step } => {
			*n.entry(encode(bytes, *seed, *step)).or_default() += 1;
		}
		Expr::Paren { e } => count_expr_in(e, n),
		Expr::Dot { obj, .. } => count_expr_in(obj, n),
		Expr::Index { obj, idx } => {
			count_expr_in(obj, n);
			count_expr_in(idx, n);
		}
		Expr::Call { func, args } => {
			count_expr_in(func, n);
			for a in args {
				count_expr_in(a, n);
			}
		}
		Expr::Method { obj, args, .. } => {
			count_expr_in(obj, n);
			for a in args {
				count_expr_in(a, n);
			}
		}
		Expr::Un { e, .. } => count_expr_in(e, n),
		Expr::Bin { l, r, .. } => {
			count_expr_in(l, n);
			count_expr_in(r, n);
		}
		Expr::IfExpr { arms, elseb } => {
			for (c, v) in arms {
				count_expr_in(c, n);
				count_expr_in(v, n);
			}
			count_expr_in(elseb, n);
		}
		Expr::Table { fields } => {
			for field in fields {
				match field {
					TableField::Array(value) => count_expr_in(value, n),
					TableField::Key { key, value } => {
						count_expr_in(key, n);
						count_expr_in(value, n);
					}
				}
			}
		}
		// nested closures belong to a different frame: not counted here
		Expr::Function { .. } => {}
		_ => {}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::parser;

	#[test]
	fn byte_transform_roundtrips_binary_and_utf8() {
		for source in [b"suda".as_slice(), "脚本".as_bytes(), b"a\0b\xff"] {
			let cipher = encode(source, 13_579, 24_681);
			assert_ne!(cipher, source);
			assert_eq!(decode(&cipher, 13_579, 24_681), source);
		}
	}

	#[test]
	fn seals_literals_recursively_including_long_strings_and_table_keys() {
		let mut block = parser::parse(
			"local t = {['key'] = [[long body]]}; local function f() return 'nested' end; return t['key'], f()",
			true,
		)
		.unwrap();
		let mut rng = Rng::new(62);
		let stats = seal_literals(&mut block, &mut rng);
		assert_eq!(stats.literals, 4);
		assert!(stats.bytes >= b"key".len() + b"long body".len() + b"nested".len());
	}
}
