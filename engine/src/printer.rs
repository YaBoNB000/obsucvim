
use crate::ast::*;
use crate::symtab::SymTable;

pub struct Printer<'a> {
	pub table: &'a SymTable,
	pub out: String,
	pub indent: usize,
	pub luau_fold: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Ctx {
	Top,
	Bin(BinOp, bool),
	Unary,
	Base,
	Suffix,
}

impl<'a> Printer<'a> {
	pub fn new(table: &'a SymTable) -> Printer<'a> {
		Printer {
			table,
			out: String::new(),
			indent: 0,
			luau_fold: false,
		}
	}

	pub fn new_luau(table: &'a SymTable) -> Printer<'a> {
		Printer {
			table,
			out: String::new(),
			indent: 0,
			luau_fold: true,
		}
	}

	fn ind(&self) -> String {
		"    ".repeat(self.indent)
	}

	pub fn print_block(&mut self, b: &Block) {
		for (i, s) in b.stmts.iter().enumerate() {
			self.print_stmt(s);
			if i + 1 < b.stmts.len() {
				self.out.push_str(";\n");
			}
		}
	}

	fn print_stmt(&mut self, s: &Stmt) {
		match s {
			Stmt::Local { names, syms, values } => {
				self.out.push_str(&self.ind());
				self.out.push_str("local ");
				for i in 0..names.len() {
					if i > 0 {
						self.out.push_str(", ");
					}
					self.out.push_str(self.table.name_of(syms[i]));
				}
				let any_val = values.iter().any(|v| v.is_some());
				if any_val {
					self.out.push_str(" = ");
					for (i, v) in values.iter().enumerate() {
						if i > 0 {
							self.out.push_str(", ");
						}
						match v {
							Some(e) => self.print_expr(e, Ctx::Top),
							None => self.out.push_str("nil"),
						}
					}
				}
			}
			Stmt::LocalFunc { name, sym, func } => {
				let _ = name;
				self.out.push_str(&self.ind());
				self.out.push_str("local function ");
				self.out.push_str(self.table.name_of(*sym));
				self.print_func_signature(func);
				self.indent += 1;
				self.out.push('\n');
				self.print_block(&func.body);
				self.indent -= 1;
				self.out.push('\n');
				self.out.push_str(&self.ind());
				self.out.push_str("end");
			}
			Stmt::FuncDecl { obj, name, ismethod, func } => {
			self.out.push_str(&self.ind());
			self.out.push_str("function ");
			if let Some(o) = obj {
				self.print_expr(o, Ctx::Suffix);
				self.out.push(if *ismethod { ':' } else { '.' });
			}
				self.out.push_str(name);
				self.print_func_signature(func);
				self.indent += 1;
				self.out.push('\n');
				self.print_block(&func.body);
				self.indent -= 1;
				self.out.push('\n');
				self.out.push_str(&self.ind());
				self.out.push_str("end");
			}
			Stmt::Assign { targets, values } => {
				self.out.push_str(&self.ind());
				if self.luau_fold && targets.len() == 1 && values.len() == 1 {
					if let Expr::Bin { op, l, r } = &values[0] {
						if let Some(sym) = compound_sym(op) {
							if foldable_target(&targets[0]) && expr_same(&targets[0], l) {
								self.print_expr(&targets[0], Ctx::Top);
								self.out.push(' ');
								self.out.push_str(sym);
								self.out.push(' ');
								self.print_expr(r, Ctx::Top);
								return;
							}
						}
					}
				}
				for (i, t) in targets.iter().enumerate() {
					if i > 0 {
						self.out.push_str(", ");
					}
					self.print_expr(t, Ctx::Top);
				}
				self.out.push_str(" = ");
				for (i, v) in values.iter().enumerate() {
					if i > 0 {
						self.out.push_str(", ");
					}
					self.print_expr(v, Ctx::Top);
				}
			}
			Stmt::ExprStmt(e) => {
				self.out.push_str(&self.ind());
				self.print_expr(e, Ctx::Top);
			}
			Stmt::If { cond, thenb, elsifs, elseb } => {
				self.out.push_str(&self.ind());
				self.out.push_str("if ");
				self.print_expr(cond, Ctx::Top);
				self.out.push_str(" then");
				self.indent += 1;
				self.out.push('\n');
				self.print_block(thenb);
				for (c, b) in elsifs {
					self.indent -= 1;
					self.out.push('\n');
					self.out.push_str(&self.ind());
					self.out.push_str("elseif ");
					self.print_expr(c, Ctx::Top);
					self.out.push_str(" then");
					self.indent += 1;
					self.out.push('\n');
					self.print_block(b);
				}
				if let Some(b) = elseb {
					self.indent -= 1;
					self.out.push('\n');
					self.out.push_str(&self.ind());
					self.out.push_str("else");
					self.indent += 1;
					self.out.push('\n');
					self.print_block(b);
				}
				self.indent -= 1;
				self.out.push('\n');
				self.out.push_str(&self.ind());
				self.out.push_str("end");
			}
			Stmt::While { cond, body } => {
				self.out.push_str(&self.ind());
				self.out.push_str("while ");
				self.print_expr(cond, Ctx::Top);
				self.out.push_str(" do");
				self.indent += 1;
				self.out.push('\n');
				self.print_block(body);
				self.indent -= 1;
				self.out.push('\n');
				self.out.push_str(&self.ind());
				self.out.push_str("end");
			}
			Stmt::Repeat { body, cond } => {
				self.out.push_str(&self.ind());
				self.out.push_str("repeat");
				self.indent += 1;
				self.out.push('\n');
				self.print_block(body);
				self.indent -= 1;
				self.out.push('\n');
				self.out.push_str(&self.ind());
				self.out.push_str("until ");
				self.print_expr(cond, Ctx::Top);
			}
			Stmt::ForNum { var, var_sym, start, limit, step, body } => {
				self.out.push_str(&self.ind());
				self.out.push_str("for ");
				self.out.push_str(self.table.name_of(*var_sym));
				self.out.push_str(" = ");
				self.print_expr(start, Ctx::Top);
				self.out.push_str(", ");
				self.print_expr(limit, Ctx::Top);
				if let Some(st) = step {
					self.out.push_str(", ");
					self.print_expr(st, Ctx::Top);
				}
				self.out.push_str(" do");
				self.indent += 1;
				self.out.push('\n');
				self.print_block(body);
				self.indent -= 1;
				self.out.push('\n');
				self.out.push_str(&self.ind());
				self.out.push_str("end");
			}
			Stmt::ForGen { vars, syms, iters, body } => {
				self.out.push_str(&self.ind());
				self.out.push_str("for ");
				for (i, id) in syms.iter().enumerate() {
					if i > 0 {
						self.out.push_str(", ");
					}
					let _ = vars;
					self.out.push_str(self.table.name_of(*id));
				}
				self.out.push_str(" in ");
				for (i, it) in iters.iter().enumerate() {
					if i > 0 {
						self.out.push_str(", ");
					}
					self.print_expr(it, Ctx::Top);
				}
				self.out.push_str(" do");
				self.indent += 1;
				self.out.push('\n');
				self.print_block(body);
				self.indent -= 1;
				self.out.push('\n');
				self.out.push_str(&self.ind());
				self.out.push_str("end");
			}
			Stmt::Do(b) => {
				self.out.push_str(&self.ind());
				self.out.push_str("do");
				self.indent += 1;
				self.out.push('\n');
				self.print_block(b);
				self.indent -= 1;
				self.out.push('\n');
				self.out.push_str(&self.ind());
				self.out.push_str("end");
			}
			Stmt::Break => {
				self.out.push_str(&self.ind());
				self.out.push_str("break");
			}
			Stmt::Continue => {
				self.out.push_str(&self.ind());
				self.out.push_str("continue");
			}
			Stmt::Return(es) => {
				self.out.push_str(&self.ind());
				if es.is_empty() {
					self.out.push_str("return");
				} else {
					self.out.push_str("return ");
					for (i, e) in es.iter().enumerate() {
						if i > 0 {
							self.out.push_str(", ");
						}
						self.print_expr(e, Ctx::Top);
					}
				}
			}
		}
	}

	fn print_func_signature(&mut self, f: &FuncDef) {
		self.out.push('(');
		let start = if f.has_self { 1 } else { 0 };
		for i in start..f.params.len() {
			if i > start {
				self.out.push_str(", ");
			}
			let name = if i < f.param_syms.len() {
				self.table.name_of(f.param_syms[i])
			} else {
				f.params[i].as_str()
			};
			self.out.push_str(name);
		}
		if f.vararg {
			if !f.params.is_empty() {
				self.out.push_str(", ");
			}
			self.out.push_str("...");
		}
		self.out.push(')');
		self.out.push(' ');
	}

	fn print_expr(&mut self, e: &Expr, ctx: Ctx) {
		if needs_parens(e, ctx) {
			self.out.push('(');
			self.print_expr_inner(e, Ctx::Top);
			self.out.push(')');
			return;
		}
		self.print_expr_inner(e, ctx);
	}

	fn print_expr_inner(&mut self, e: &Expr, ctx: Ctx) {
		match e {
			Expr::Num { value, isfloat } => self.print_num(*value, *isfloat),
			Expr::Str { bytes, is_binary } => {
				self.out.push('"');
				self.out.push_str(&print_string_bytes(bytes, *is_binary));
				self.out.push('"');
			}
			Expr::LongStr { bytes } => {
				let mut lvl = 0usize;
				loop {
					let closer = format!("]{}]", "=".repeat(lvl));
					let mut joined = bytes.to_vec();
					joined.extend_from_slice(closer.as_bytes());
					let first = joined
						.windows(closer.len())
						.position(|w| w == closer.as_bytes());
					let closer_ok = first == Some(bytes.len());
					let opener_ok = lvl > 0 || !bytes.windows(2).any(|w| w == b"[[");
					if closer_ok && opener_ok {
						break;
					}
					lvl += 1;
				}
				let pad = if bytes.first() == Some(&b'\n') { "\n" } else { "" };
				self.out.push_str(&format!(
					"[{o}[{pad}{c}]{o}]",
					o = "=".repeat(lvl),
					pad = pad,
					c = String::from_utf8_lossy(bytes)
				));
			}
			Expr::SealedStr { .. } => {
				crate::limits::fatal(
					"input exceeds compiler limit: unlowered VM-sealed string literal reached the printer",
				)
			}
			Expr::Bool { value } => {
				self.out.push_str(if *value { "true" } else { "false" });
			}
			Expr::Nil => self.out.push_str("nil"),
			Expr::IfExpr { arms, elseb } => {
				let par = !matches!(ctx, Ctx::Top);
				if par {
					self.out.push('(');
				}
				for (i, (c, v)) in arms.iter().enumerate() {
					self.out.push_str(if i == 0 { "if " } else { " elseif " });
					self.print_expr(c, Ctx::Top);
					self.out.push_str(" then ");
					self.print_expr(v, Ctx::Top);
				}
				self.out.push_str(" else ");
				self.print_expr(elseb, Ctx::Top);
				if par {
					self.out.push(')');
				}
			}
			Expr::Vararg => self.out.push_str("..."),
			Expr::Paren { e } => {
				self.out.push('(');
				self.print_expr(e, Ctx::Top);
				self.out.push(')');
			}
			Expr::Ident { name, sym } => match sym {
				Some(id) => self.out.push_str(self.table.name_of(*id)),
				None => self.out.push_str(name),
			},
			Expr::Dot { obj, name } => {
				self.print_expr(obj, Ctx::Suffix);
				self.out.push('.');
				self.out.push_str(name);
			}
			Expr::Index { obj, idx } => {
				self.print_expr(obj, Ctx::Suffix);
				self.out.push('[');
				self.print_expr(idx, Ctx::Top);
				self.out.push(']');
			}
			Expr::Call { func, args } => {
				self.print_expr(func, Ctx::Base);
				self.print_args(args);
			}
			Expr::Method { obj, name, args } => {
				self.print_expr(obj, Ctx::Suffix);
				self.out.push(':');
				self.out.push_str(name);
				self.print_args(args);
			}
			Expr::Un { op, e } => {
				self.out.push_str(op.text());
				self.out.push(' ');
				self.print_expr(e, Ctx::Unary);
			}
			Expr::Bin { op, l, r } => {
			if *op == BinOp::Idiv && !self.luau_fold {
				self.out.push_str("math.floor(");
				self.print_expr(l, Ctx::Bin(BinOp::Div, false));
				self.out.push_str(" / ");
				self.print_expr(r, Ctx::Bin(BinOp::Div, true));
				self.out.push(')');
			} else {
					self.print_expr(l, Ctx::Bin(*op, false));
					self.out.push(' ');
					self.out.push_str(op.text());
					self.out.push(' ');
					self.print_expr(r, Ctx::Bin(*op, true));
				}
			}
			Expr::Table { fields } => {
				self.out.push('{');
				for (i, f) in fields.iter().enumerate() {
					if i > 0 {
						self.out.push_str(", ");
					}
					match f {
						TableField::Array(v) => self.print_expr(v, Ctx::Top),
						TableField::Key { key, value } => {
							if let Expr::Str { bytes, .. } = key {
								if is_printable_key(bytes) {
									self.out.push_str(&String::from_utf8_lossy(bytes));
									self.out.push_str(" = ");
									self.print_expr(value, Ctx::Top);
								} else {
									self.out.push('[');
									self.print_expr(key, Ctx::Top);
									self.out.push_str("] = ");
									self.print_expr(value, Ctx::Top);
								}
							} else {
								self.out.push('[');
								self.print_expr(key, Ctx::Top);
								self.out.push_str("] = ");
								self.print_expr(value, Ctx::Top);
							}
						}
					}
				}
				self.out.push('}');
			}
			Expr::Function { params, param_syms, vararg, body } => {
				self.out.push_str("function(");
				for (i, p) in params.iter().enumerate() {
					if i > 0 {
						self.out.push_str(", ");
					}
					let name = if i < param_syms.len() {
						self.table.name_of(param_syms[i])
					} else {
						p.as_str()
					};
					self.out.push_str(name);
				}
				if *vararg {
					if !params.is_empty() {
						self.out.push_str(", ");
					}
					self.out.push_str("...");
				}
				self.out.push_str(")");
				self.indent += 1;
				self.out.push('\n');
				self.print_block(body);
				self.indent -= 1;
				self.out.push('\n');
				self.out.push_str(&self.ind());
				self.out.push_str("end");
			}
		}
	}

	fn print_args(&mut self, args: &[Expr]) {
		match args {
			[] => {
				self.out.push_str("()");
			}
			[single] if matches!(single, Expr::Table { .. }) => {
				self.print_expr(single, Ctx::Top);
			}
			_ => {
				self.out.push('(');
				for (i, a) in args.iter().enumerate() {
					if i > 0 {
						self.out.push_str(", ");
					}
					self.print_expr(a, Ctx::Top);
				}
				self.out.push(')');
			}
		}
	}

	fn print_num(&mut self, v: f64, isfloat: bool) {
		if v.is_nan() {
			self.out.push_str("0.0/0.0");
			return;
		}
		if v.is_infinite() {
			if v < 0.0 {
				self.out.push_str("-math.huge");
			} else {
				self.out.push_str("math.huge");
			}
			return;
		}
		let is_int = v.fract() == 0.0 && v.abs() < 1e15;
		if is_int && !isfloat {
			self.out.push_str(&format!("{}", v as i64));
		} else {
			self.out.push_str(&format!("{:?}", v));
		}
	}
}

fn is_printable_key(b: &[u8]) -> bool {
	!b.is_empty()
		&& std::str::from_utf8(b).is_ok()
		&& b.iter().all(|&c| {
			c == b'_'
				|| (c >= b'a' && c <= b'z')
				|| (c >= b'A' && c <= b'Z')
				|| (c >= b'1' && c <= b'9')
		})
		&& b[0] != b'0'
}

pub fn print_string_bytes(bytes: &[u8], is_binary: bool) -> String {
	let mut out = String::new();
	let mut i = 0usize;
	while i < bytes.len() {
		let b = bytes[i];
		let next_is_digit = bytes.get(i + 1).map_or(false, |&n| n >= b'0' && n <= b'9');
		if b < 0x80 {
			match b {
				b'"' => out.push_str("\\\""),
				b'\\' => out.push_str("\\\\"),
				c if (0x20..0x7f).contains(&c) => out.push(c as char),
				_ => {
					if next_is_digit {
						out.push_str(&format!("\\{:03}", b));
					} else {
						out.push_str(&format!("\\{}", b));
					}
				}
			}
			i += 1;
		} else if is_binary {
			if next_is_digit {
				out.push_str(&format!("\\{:03}", b));
			} else {
				out.push_str(&format!("\\{}", b));
			}
			i += 1;
		} else {
			let len = match b {
				0xC0..=0xDF => 2,
				0xE0..=0xEF => 3,
				0xF0..=0xF7 => 4,
				_ => 1,
			};
			if i + len <= bytes.len() && std::str::from_utf8(&bytes[i..i + len]).is_ok() {
				let ch = match std::str::from_utf8(&bytes[i..i + len]).ok().and_then(|s| s.chars().next()) {
					Some(c) => c,
					None => crate::limits::fatal(
						"input exceeds compiler limit: invalid UTF-8 sequence in a string literal",
					),
				};
				if ch.is_control() || ch == '"' || ch == '\\' {
					for k in i..i + len {
						out.push_str(&format!("\\{}", bytes[k]));
					}
				} else {
					out.push(ch);
				}
				i += len;
			} else {
				out.push_str(&format!("\\{}", b));
				i += 1;
			}
		}
	}
	out
}

	fn needs_parens(e: &Expr, ctx: Ctx) -> bool {
	if let Ctx::Base = ctx {
		match e {
			Expr::Ident { .. }
			| Expr::Dot { .. }
			| Expr::Index { .. }
			| Expr::Call { .. }
			| Expr::Method { .. }
			| Expr::Paren { .. }
			| Expr::Table { .. } => return false,
			_ => return true,
		}
	}
	if let Ctx::Suffix = ctx {
		match e {
			Expr::Ident { .. } | Expr::Dot { .. } | Expr::Index { .. } | Expr::Call { .. }
			| Expr::Method { .. } | Expr::Paren { .. } => return false,
			_ => return true,
		}
	}
	match (e, ctx) {
		(Expr::Bin { op: cop, .. }, Ctx::Bin(parent, right)) => {
			let (lp, rp) = cop.prio();
			let eff = lp.min(rp);
			let (plp, prp) = parent.prio();
			let right_assoc = prp < plp;
			if right {
				eff < prp || (eff == prp && !right_assoc)
			} else {
				eff < plp
			}
		}
		(Expr::Bin { op: cop, .. }, Ctx::Unary) => {
			let (lp, rp) = cop.prio();
			lp.min(rp) < 8
		}
		(Expr::Un { .. }, Ctx::Bin(_, right)) => {
			if right {
				false
			} else {
				let (plp, _) = match ctx {
					Ctx::Bin(op, _) => op.prio(),
					_ => (0, 0),
				};
				8 < plp
			}
		}
		(Expr::Un { .. }, Ctx::Unary) => false,
		_ => false,
	}
}

fn compound_sym(op: &BinOp) -> Option<&'static str> {
	match op {
		BinOp::Add => Some("+="),
		BinOp::Sub => Some("-="),
		BinOp::Mul => Some("*="),
		BinOp::Div => Some("/="),
		BinOp::Mod => Some("%="),
		BinOp::Pow => Some("^="),
		_ => None,
	}
}

fn foldable_target(e: &Expr) -> bool {
	match e {
		Expr::Ident { .. } => true,
		Expr::Dot { obj, .. } => foldable_target(obj),
		Expr::Index { obj, idx } => foldable_target(obj) && foldable_target(idx),
		_ => false,
	}
}

fn expr_same(a: &Expr, b: &Expr) -> bool {
	match (a, b) {
		(Expr::Ident { sym: sa, .. }, Expr::Ident { sym: sb, .. }) => sa == sb,
		(Expr::Dot { obj: oa, name: na }, Expr::Dot { obj: ob, name: nb }) => {
			na == nb && expr_same(oa, ob)
		}
		(
			Expr::Index { obj: oa, idx: ia },
			Expr::Index { obj: ob, idx: ib },
		) => expr_same(oa, ob) && expr_same(ia, ib),
		(Expr::Num { value: va, .. }, Expr::Num { value: vb, .. }) => va == vb,
		(Expr::Str { bytes: ba, .. }, Expr::Str { bytes: bb, .. }) => ba == bb,
		_ => false,
	}
}

pub fn print_chunk(table: &SymTable, block: &Block) -> String {
	let mut p = Printer::new(table);
	p.print_block(block);
	p.out.push('\n');
	p.out
}

pub fn print_chunk_luau(table: &SymTable, block: &Block) -> String {
	let mut p = Printer::new_luau(table);
	p.print_block(block);
	p.out.push('\n');
	p.out
}

#[cfg(test)]
mod tests {
	use super::{print_chunk, print_chunk_luau};
	use crate::{parser, symtab};

	#[test]
	fn integer_division_uses_target_appropriate_syntax() {
		let mut block = parser::parse("return x // y", true).unwrap();
		let table = symtab::resolve(&mut block);
		assert!(print_chunk(&table, &block).contains("math.floor(x / y)"));
		assert!(print_chunk_luau(&table, &block).contains("x // y"));
	}
}
