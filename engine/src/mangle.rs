
use crate::ast::{Block, Expr, Stmt, SymId, TableField};
use crate::rng::Rng;
use crate::symtab::SymTable;
use std::collections::HashSet;

pub const RESERVED: &[&str] = &[
	"and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in",
	"local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
	"continue", "goto",
	"self",
];

pub fn reserved_set(extra: &HashSet<String>) -> HashSet<String> {
	let mut s: HashSet<String> = RESERVED.iter().map(|x| x.to_string()).collect();
	s.extend(extra.iter().cloned());
	s
}

pub fn gen_name(rng: &mut Rng, reserved: &HashSet<String>) -> String {
	loop {
		let short = rng.int(0, 99);
		let len = if short < 30 {
			rng.int(2, 4)
		} else if short < 70 {
			rng.int(5, 8)
		} else {
			rng.int(9, 15)
		};
		let first = if rng.int(0, 1) == 0 {
			rng.int(b'a' as i64, b'z' as i64)
		} else {
			rng.int(b'A' as i64, b'Z' as i64)
		};
		let mut s = String::new();
		s.push(first as u8 as char);
		for _ in 1..len {
			let r = rng.int(0, 99);
			let c = if r < 55 {
				rng.int(b'a' as i64, b'z' as i64) as u8 as char
			} else if r < 85 {
				rng.int(b'0' as i64, b'9' as i64) as u8 as char
			} else if r < 95 {
				rng.int(b'A' as i64, b'Z' as i64) as u8 as char
			} else {
				'_'
			};
			s.push(c);
		}
		if !reserved.contains(&s) {
			return s;
		}
	}
}

pub fn mangle(table: &mut SymTable, rng: &mut Rng) {
	let mut reserved =
		reserved_set(&table.globals.iter().cloned().collect::<HashSet<_>>());
	let n = table.syms.len();
	let mut names: Vec<Option<String>> = Vec::with_capacity(n);
	for sym in table.syms.iter() {
		if sym.keep_name {
names.push(None);
		} else {
			let name = gen_name(rng, &reserved);
			reserved.insert(name.clone());
			names.push(Some(name));
		}
	}
	for (i, name) in names.into_iter().enumerate() {
		if let Some(name) = name {
			table.syms[i].name = name;
		}
	}
}

struct CompactNames {
	pool: Vec<String>,
	active: HashSet<String>,
	scopes: Vec<Vec<String>>,
}

impl CompactNames {
	fn new(rng: &mut Rng, reserved: &HashSet<String>) -> Self {
		const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
		let mut singles = Vec::new();
		let mut pairs = Vec::new();
		for &a in ALPHA {
			let name = (a as char).to_string();
			if !reserved.contains(&name) {
				singles.push(name);
			}
			for &b in ALPHA {
				let name = format!("{}{}", a as char, b as char);
				if !reserved.contains(&name) {
					pairs.push(name);
				}
			}
		}
		rng.shuffle(&mut singles);
		rng.shuffle(&mut pairs);
		pairs.extend(singles);
		Self {
			pool: pairs,
			active: reserved.clone(),
			scopes: vec![Vec::new()],
		}
	}

	fn enter_scope(&mut self) {
		self.scopes.push(Vec::new());
	}

	fn leave_scope(&mut self) {
		if let Some(names) = self.scopes.pop() {
			for name in names {
				self.active.remove(&name);
				self.pool.push(name);
			}
		}
	}

	fn bind(&mut self, table: &mut SymTable, id: SymId) -> Result<(), String> {
		let sym = table
			.syms
			.get(id as usize)
			.ok_or_else(|| format!("invalid local symbol id {id}"))?;
		if sym.keep_name {
			return Ok(());
		}
		let name = self.pool.pop().ok_or_else(|| {
			format!(
				"two-letter name pool exhausted at lexical depth {}",
				self.scopes.len()
			)
		})?;
		self.active.insert(name.clone());
		self.scopes.last_mut().expect("root name scope").push(name.clone());
		table.syms[id as usize].name = name;
		Ok(())
	}
}

struct CompactMangler<'a> {
	table: &'a mut SymTable,
	names: CompactNames,
}

impl<'a> CompactMangler<'a> {
	fn walk_function(&mut self, params: &[SymId], body: &Block) -> Result<(), String> {
		self.names.enter_scope();
		for &id in params {
			self.names.bind(self.table, id)?;
		}
		self.walk_block(body)?;
		self.names.leave_scope();
		Ok(())
	}

	fn walk_block(&mut self, block: &Block) -> Result<(), String> {
		for stmt in &block.stmts {
			self.walk_stmt(stmt)?;
		}
		Ok(())
	}

	fn walk_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
		match stmt {
			Stmt::Local { syms, values, .. } => {
				for value in values.iter().flatten() {
					self.walk_expr(value)?;
				}
				for &id in syms {
					self.names.bind(self.table, id)?;
				}
			}
			Stmt::LocalFunc { sym, func, .. } => {
				self.names.bind(self.table, *sym)?;
				self.walk_function(&func.param_syms, &func.body)?;
			}
			Stmt::FuncDecl { obj, func, .. } => {
				if let Some(obj) = obj {
					self.walk_expr(obj)?;
				}
				self.walk_function(&func.param_syms, &func.body)?;
			}
			Stmt::Assign { targets, values } => {
				for target in targets {
					self.walk_expr(target)?;
				}
				for value in values {
					self.walk_expr(value)?;
				}
			}
			Stmt::ExprStmt(expr) => self.walk_expr(expr)?,
			Stmt::If { cond, thenb, elsifs, elseb } => {
				self.walk_expr(cond)?;
				self.walk_scoped_block(thenb)?;
				for (cond, body) in elsifs {
					self.walk_expr(cond)?;
					self.walk_scoped_block(body)?;
				}
				if let Some(body) = elseb {
					self.walk_scoped_block(body)?;
				}
			}
			Stmt::While { cond, body } => {
				self.walk_expr(cond)?;
				self.walk_scoped_block(body)?;
			}
			Stmt::Repeat { body, cond } => {
				self.names.enter_scope();
				self.walk_block(body)?;
				self.walk_expr(cond)?;
				self.names.leave_scope();
			}
			Stmt::ForNum { var_sym, start, limit, step, body, .. } => {
				self.walk_expr(start)?;
				self.walk_expr(limit)?;
				if let Some(step) = step {
					self.walk_expr(step)?;
				}
				self.names.enter_scope();
				self.names.bind(self.table, *var_sym)?;
				self.walk_block(body)?;
				self.names.leave_scope();
			}
			Stmt::ForGen { vars: _, syms, iters, body } => {
				for iter in iters {
					self.walk_expr(iter)?;
				}
				self.names.enter_scope();
				for &id in syms {
					self.names.bind(self.table, id)?;
				}
				self.walk_block(body)?;
				self.names.leave_scope();
			}
			Stmt::Do(body) => self.walk_scoped_block(body)?,
			Stmt::Return(values) => {
				for value in values {
					self.walk_expr(value)?;
				}
			}
			Stmt::Break | Stmt::Continue => {}
		}
		Ok(())
	}

	fn walk_scoped_block(&mut self, block: &Block) -> Result<(), String> {
		self.names.enter_scope();
		self.walk_block(block)?;
		self.names.leave_scope();
		Ok(())
	}

	fn walk_expr(&mut self, expr: &Expr) -> Result<(), String> {
		match expr {
			Expr::Paren { e } => self.walk_expr(e)?,
			Expr::Dot { obj, .. } => self.walk_expr(obj)?,
			Expr::Index { obj, idx } => {
				self.walk_expr(obj)?;
				self.walk_expr(idx)?;
			}
			Expr::Call { func, args } => {
				self.walk_expr(func)?;
				for arg in args {
					self.walk_expr(arg)?;
				}
			}
			Expr::Method { obj, args, .. } => {
				self.walk_expr(obj)?;
				for arg in args {
					self.walk_expr(arg)?;
				}
			}
			Expr::Un { e, .. } => self.walk_expr(e)?,
			Expr::Bin { l, r, .. } => {
				self.walk_expr(l)?;
				self.walk_expr(r)?;
			}
			Expr::IfExpr { arms, elseb } => {
				for (cond, value) in arms {
					self.walk_expr(cond)?;
					self.walk_expr(value)?;
				}
				self.walk_expr(elseb)?;
			}
			Expr::Table { fields } => {
				for field in fields {
					match field {
						TableField::Array(value) => self.walk_expr(value)?,
						TableField::Key { key, value } => {
							self.walk_expr(key)?;
							self.walk_expr(value)?;
						}
					}
				}
			}
			Expr::Function { param_syms, body, .. } => {
				self.walk_function(param_syms, body)?;
			}
			_ => {}
		}
		Ok(())
	}
}

pub fn mangle_v15_compact(
	block: &Block,
	table: &mut SymTable,
	rng: &mut Rng,
) -> Result<(), String> {
	let globals: HashSet<String> = table.globals.iter().cloned().collect();
	let mut reserved = reserved_set(&globals);
	reserved.extend(["b", "C", "p1", "p2", "p3", "E", "V"].map(str::to_string));
	let names = CompactNames::new(rng, &reserved);
	let mut mangler = CompactMangler { table, names };
	mangler.walk_block(block)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::parser;

	#[test]
	fn v15_compact_mangler_limits_local_symbols_to_two_letters() {
		let src = r#"
			local top = 7
			local function outer(arg)
				local x = arg + top
				do local y = x + 1; x = y end
				do local z = x + 2; x = z end
				local function child(q)
					local r = q + x + top
					return r
				end
				return child
			end
			local f = outer(1)
			return f(2)
		"#;
		let mut block = parser::parse(src, true).expect("test chunk parses");
		let mut table = crate::symtab::resolve(&mut block);
		let mut rng = Rng::new(0x5230_3634_4e41_4d45);
		mangle_v15_compact(&block, &mut table, &mut rng).expect("compact names fit");
		for sym in &table.syms {
			if sym.keep_name {
				assert_eq!(sym.name, "self");
				continue;
			}
			assert!((1..=2).contains(&sym.name.len()), "long local name: {}", sym.name);
			assert!(sym.name.bytes().all(|b| b.is_ascii_alphabetic()), "non-letter local name: {}", sym.name);
		}
	}
}
