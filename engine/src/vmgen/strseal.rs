
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

pub fn seal_literals(block: &mut Block, rng: &mut Rng) -> SealStats {
	let mut stats = SealStats::default();
	visit_block(block, rng, &mut stats);
	stats
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

fn seal_expr(expr: &mut Expr, rng: &mut Rng, stats: &mut SealStats) {
	match expr {
		Expr::Str { bytes, .. } if !bytes.is_empty() => {
			let (seed, step) = params_for(bytes, rng);
			let plain = std::mem::take(bytes);
			stats.literals += 1;
			stats.bytes += plain.len();
			*expr = Expr::SealedStr { bytes: plain, seed, step };
		}
		Expr::LongStr { bytes } if !bytes.is_empty() => {
			let (seed, step) = params_for(bytes, rng);
			let plain = std::mem::take(bytes);
			stats.literals += 1;
			stats.bytes += plain.len();
			*expr = Expr::SealedStr { bytes: plain, seed, step };
		}
		Expr::Dot { obj, .. } => seal_expr(obj, rng, stats),
		Expr::Index { obj, idx } => {
			seal_expr(obj, rng, stats);
			seal_expr(idx, rng, stats);
		}
		Expr::Call { func, args } => {
			seal_expr(func, rng, stats);
			for arg in args {
				seal_expr(arg, rng, stats);
			}
		}
		Expr::Method { obj, args, .. } => {
			seal_expr(obj, rng, stats);
			for arg in args {
				seal_expr(arg, rng, stats);
			}
		}
		Expr::Un { e, .. } => seal_expr(e, rng, stats),
		Expr::Bin { l, r, .. } => {
			seal_expr(l, rng, stats);
			seal_expr(r, rng, stats);
		}
		Expr::IfExpr { arms, elseb } => {
			for (cond, value) in arms {
				seal_expr(cond, rng, stats);
				seal_expr(value, rng, stats);
			}
			seal_expr(elseb, rng, stats);
		}
		Expr::Table { fields } => {
			for field in fields {
				match field {
					TableField::Array(value) => seal_expr(value, rng, stats),
					TableField::Key { key, value } => {
						seal_expr(key, rng, stats);
						seal_expr(value, rng, stats);
					}
				}
			}
		}
		Expr::Function { body, .. } => visit_block(body, rng, stats),
		_ => {}
	}
}

fn visit_block(block: &mut Block, rng: &mut Rng, stats: &mut SealStats) {
	for stmt in &mut block.stmts {
		match stmt {
			Stmt::Local { values, .. } => {
				for value in values.iter_mut().flatten() {
					seal_expr(value, rng, stats);
				}
			}
			Stmt::LocalFunc { func, .. } => visit_block(&mut func.body, rng, stats),
			Stmt::FuncDecl { obj, func, .. } => {
				if let Some(obj) = obj {
					seal_expr(obj, rng, stats);
				}
				visit_block(&mut func.body, rng, stats);
			}
			Stmt::Assign { targets, values } => {
				for target in targets {
					seal_expr(target, rng, stats);
				}
				for value in values {
					seal_expr(value, rng, stats);
				}
			}
			Stmt::ExprStmt(value) => seal_expr(value, rng, stats),
			Stmt::If { cond, thenb, elsifs, elseb } => {
				seal_expr(cond, rng, stats);
				visit_block(thenb, rng, stats);
				for (cond, body) in elsifs {
					seal_expr(cond, rng, stats);
					visit_block(body, rng, stats);
				}
				if let Some(body) = elseb {
					visit_block(body, rng, stats);
				}
			}
			Stmt::While { cond, body } => {
				seal_expr(cond, rng, stats);
				visit_block(body, rng, stats);
			}
			Stmt::Repeat { body, cond } => {
				visit_block(body, rng, stats);
				seal_expr(cond, rng, stats);
			}
			Stmt::ForNum { start, limit, step, body, .. } => {
				seal_expr(start, rng, stats);
				seal_expr(limit, rng, stats);
				if let Some(step) = step {
					seal_expr(step, rng, stats);
				}
				visit_block(body, rng, stats);
			}
			Stmt::ForGen { iters, body, .. } => {
				for iter in iters {
					seal_expr(iter, rng, stats);
				}
				visit_block(body, rng, stats);
			}
			Stmt::Do(body) => visit_block(body, rng, stats),
			Stmt::Return(values) => {
				for value in values {
					seal_expr(value, rng, stats);
				}
			}
			Stmt::Break | Stmt::Continue => {}
		}
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
