
use crate::ast::*;
use crate::rng::Rng;
use crate::symtab::{Sym, SymTable};

// V-08 修复（2026-10-25）：旧噪声是纯常量算术
//   `local a=9742+7221; local b=a*298-4491; if b*b>=0 then ...; local d=b+1-1`
// 一条常量折叠 + 代数化简即可整块清空，形成稳定指纹。
// 现在每一块都把数值挂在**字面量的长度**（`#"..."`）与随机模数剩余上：
//   * 需要先求字符串长度才能折叠，单条代数规则不再奏效；
//   * 分支条件取自运行期余数（两侧都赋值，语义中性）；
//   * 形态、字面量长度、运算符组合按 seed 变化，不产生同签名族。

fn num(v: i64) -> Expr {
	Expr::Num {
		value: v as f64,
		isfloat: false,
	}
}

fn len_of(bytes: &[u8]) -> Expr {
	Expr::Un {
		op: UnOp::Len,
		e: Box::new(Expr::Str {
			bytes: bytes.to_vec(),
			is_binary: true,
		}),
	}
}

fn bin(op: BinOp, l: Expr, r: Expr) -> Expr {
	Expr::Bin {
		op,
		l: Box::new(l),
		r: Box::new(r),
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

fn filler_lit(rng: &mut Rng) -> Vec<u8> {
	// 可打印但无语义的短字面量：长度参与运算，内容不出现于程序其它部位。
	let n = rng.int(3, 17) as usize;
	(0..n)
		.map(|_| {
			let c = rng.int(0, 61) as u8;
			if c < 10 {
				b'0' + c
			} else if c < 36 {
				b'a' + (c - 10)
			} else {
				b'A' + (c - 36)
			}
		})
		.collect()
}

fn junk_block(rng: &mut Rng, table: &mut SymTable) -> Block {
	let n1 = rng.int(1, 9999);
	let n2 = rng.int(1, 9999);
	let n3 = rng.int(2, 999);
	let n4 = rng.int(0, 9999);
	let m1 = rng.int(2, 97);

	let (na, sa) = mk_local(table, "j");
	let (nb, sb) = mk_local(table, "j");
	let (nc, sc) = mk_local(table, "j");

	let a = Expr::Ident {
		name: na.clone(),
		sym: Some(sa),
	};
	let b = Expr::Ident {
		name: nb.clone(),
		sym: Some(sb),
	};

	// 形态 0/1/2：数值 = 常量 ± 字面量长度（长度随 seed 变化，非纯常量）。
	let s1 = Stmt::Local {
		names: vec![na],
		syms: vec![sa],
		values: vec![Some(match rng.int(0, 2) {
			0 => bin(BinOp::Add, num(n1), len_of(&filler_lit(rng))),
			1 => bin(BinOp::Sub, num(n2), len_of(&filler_lit(rng))),
			_ => bin(
				BinOp::Mul,
				len_of(&filler_lit(rng)),
				num(rng.int(1, 9)),
			),
		})],
	};

	let s2 = Stmt::Local {
		names: vec![nb],
		syms: vec![sb],
		values: vec![Some(match rng.int(0, 1) {
			0 => bin(
				BinOp::Sub,
				bin(BinOp::Mul, a.clone(), num(n3)),
				num(n4),
			),
			_ => bin(
				BinOp::Add,
				bin(BinOp::Mod, a.clone(), num(m1)),
				num(n4),
			),
		})],
	};

	// 分支条件取运行期余数：两侧都只做纯赋值，语义中性但不可判恒。
	let cond = bin(
		BinOp::Ne,
		bin(BinOp::Mod, b.clone(), num(m1)),
		bin(BinOp::Mod, num(n4), num(m1)),
	);
	let one = Stmt::Local {
		names: vec![nc.clone()],
		syms: vec![sc],
		values: vec![Some(bin(BinOp::Sub, b.clone(), a.clone()))],
	};
	let two = Stmt::Local {
		names: vec![nc],
		syms: vec![sc],
		values: vec![Some(bin(BinOp::Add, b.clone(), num(rng.int(1, 99))))],
	};
	let s3 = Stmt::If {
		cond: Box::new(cond),
		thenb: Block { stmts: vec![one] },
		elsifs: vec![],
		elseb: Some(Block { stmts: vec![two] }),
	};

	Block {
		stmts: vec![s1, s2, s3],
	}
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
		// 旧实现在 1/16 概率处额外插入一块（同一常量族）。改为同一概率但
		// 由上面的生成器出形态，避免“固定间隔 + 固定形态”双指纹。
		if !is_return && rng.int(0, 15) == 0 {
			let j = junk_block(rng, table);
			new_stmts.extend(j.stmts);
		}
	}
	block.stmts = new_stmts;
}
