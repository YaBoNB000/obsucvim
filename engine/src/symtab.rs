
use crate::ast::*;

pub struct Sym {
	pub name: String,
	pub is_param: bool,
	pub keep_name: bool,
}

pub struct SymTable {
	pub syms: Vec<Sym>,
	pub globals: Vec<String>,
}

impl SymTable {
	pub fn new() -> SymTable {
		SymTable {
			syms: Vec::new(),
			globals: Vec::new(),
		}
	}
	pub fn name_of(&self, id: SymId) -> &str {
		&self.syms[id as usize].name
	}
}

struct Resolver<'a> {
	table: &'a mut SymTable,
	scopes: Vec<Vec<(String, u32)>>,
}

impl<'a> Resolver<'a> {
	fn new(table: &'a mut SymTable) -> Resolver<'a> {
		Resolver {
			table,
			scopes: vec![Vec::new()],
		}
	}

	fn new_scope(&mut self) {
		self.scopes.push(Vec::new());
	}

	fn pop_scope(&mut self) {
		self.scopes.pop();
	}

	fn declare(&mut self, name: &str, is_param: bool) -> SymId {
		self.table.syms.push(Sym {
			name: name.to_string(),
			is_param,
			keep_name: false,
		});
		let id = (self.table.syms.len() - 1) as SymId;
		// invariant: push_scope() installs the root scope in SymbolTable::new and pop_scope never removes it
		self.scopes.last_mut().unwrap().push((name.to_string(), id));
		id
	}

	fn lookup(&self, name: &str) -> Option<SymId> {
		for scope in self.scopes.iter().rev() {
			let mut found = None;
			for (n, id) in scope.iter() {
				if n == name {
					found = Some(*id);
				}
			}
			if found.is_some() {
				return found;
			}
		}
		None
	}

	fn note_global(&mut self, name: &str) {
		if !self.table.globals.iter().any(|g| g == name) {
			self.table.globals.push(name.to_string());
		}
	}

	fn resolve_expr(&mut self, e: &mut Expr) {
		match e {
			Expr::Ident { name, sym } => match self.lookup(name) {
				Some(id) => *sym = Some(id),
				None => {
					*sym = None;
					self.note_global(name);
				}
			},
			Expr::Dot { obj, .. } => self.resolve_expr(obj),
			Expr::Index { obj, idx } => {
				self.resolve_expr(obj);
				self.resolve_expr(idx);
			}
			Expr::Call { func, args } => {
				self.resolve_expr(func);
				for a in args {
					self.resolve_expr(a);
				}
			}
			Expr::Method { obj, args, .. } => {
				self.resolve_expr(obj);
				for a in args {
					self.resolve_expr(a);
				}
			}
			Expr::Paren { e } | Expr::Un { e, .. } => self.resolve_expr(e),
			Expr::Bin { l, r, .. } => {
				self.resolve_expr(l);
				self.resolve_expr(r);
			}
			Expr::IfExpr { arms, elseb } => {
				for (c, v) in arms {
					self.resolve_expr(c);
					self.resolve_expr(v);
				}
				self.resolve_expr(elseb);
			}
			Expr::Table { fields } => {
				for f in fields {
					match f {
						TableField::Array(e) => self.resolve_expr(e),
						TableField::Key { key, value } => {
							self.resolve_expr(key);
							self.resolve_expr(value);
						}
					}
				}
			}
			Expr::Function { params, param_syms, body, .. } => {
				self.resolve_func_body(body, params, param_syms, false);
			}
			_ => {}
		}
	}

	fn resolve_func_body(
		&mut self,
		body: &mut Block,
		params: &[String],
		param_syms: &mut Vec<u32>,
		self_param: bool,
	) {
		self.new_scope();
		param_syms.clear();
		for p in params {
			param_syms.push(self.declare(p, true));
		}
		if self_param && !param_syms.is_empty() {
			let id = param_syms[0];
			self.table.syms[id as usize].keep_name = true;
		}
		self.resolve_block(body);
		self.pop_scope();
	}

	fn resolve_block(&mut self, b: &mut Block) {
		for s in b.stmts.iter_mut() {
			self.resolve_stmt(s);
		}
	}

	fn resolve_stmt(&mut self, s: &mut Stmt) {
		match s {
			Stmt::Local { names, syms, values } => {
				for v in values.iter_mut() {
					if let Some(e) = v {
						self.resolve_expr(e);
					}
				}
				for i in 0..names.len() {
					syms[i] = self.declare(&names[i], false);
				}
			}
			Stmt::LocalFunc { name, sym, func } => {
				*sym = self.declare(name, false);
				self.resolve_func_body(
					&mut func.body,
					&func.params,
					&mut func.param_syms,
					false,
				);
			}
			Stmt::FuncDecl { obj, func, .. } => {
				if let Some(o) = obj {
					self.resolve_expr(o);
				}
				self.resolve_func_body(
					&mut func.body,
					&func.params,
					&mut func.param_syms,
					func.has_self,
				);
			}
			Stmt::Assign { targets, values } => {
				for v in values.iter_mut() {
					self.resolve_expr(v);
				}
				for t in targets.iter_mut() {
					self.resolve_expr(t);
				}
			}
			Stmt::ExprStmt(e) => self.resolve_expr(e),
			Stmt::If { cond, thenb, elsifs, elseb } => {
				self.resolve_expr(cond);
				self.new_scope();
				self.resolve_block(thenb);
				self.pop_scope();
				for (c, b) in elsifs.iter_mut() {
					self.resolve_expr(c);
					self.new_scope();
					self.resolve_block(b);
					self.pop_scope();
				}
				if let Some(b) = elseb {
					self.new_scope();
					self.resolve_block(b);
					self.pop_scope();
				}
			}
			Stmt::While { cond, body } => {
				self.resolve_expr(cond);
				self.new_scope();
				self.resolve_block(body);
				self.pop_scope();
			}
			Stmt::Repeat { body, cond } => {
				self.new_scope();
				self.resolve_block(body);
				self.resolve_expr(cond);
				self.pop_scope();
			}
			Stmt::ForNum {
				var,
				var_sym,
				start,
				limit,
				step,
				body,
			} => {
				self.resolve_expr(start);
				self.resolve_expr(limit);
				if let Some(st) = step {
					self.resolve_expr(st);
				}
				self.new_scope();
				*var_sym = self.declare(var, false);
				self.resolve_block(body);
				self.pop_scope();
			}
			Stmt::ForGen { vars, syms, iters, body } => {
				for it in iters.iter_mut() {
					self.resolve_expr(it);
				}
				self.new_scope();
				for i in 0..vars.len() {
					syms[i] = self.declare(&vars[i], false);
				}
				self.resolve_block(body);
				self.pop_scope();
			}
			Stmt::Do(b) => {
				self.new_scope();
				self.resolve_block(b);
				self.pop_scope();
			}
			Stmt::Break | Stmt::Continue => {}
			Stmt::Return(es) => {
				for e in es.iter_mut() {
					self.resolve_expr(e);
				}
			}
		}
	}
}

pub fn resolve(block: &mut Block) -> SymTable {
	let mut table = SymTable::new();
	let mut r = Resolver::new(&mut table);
	r.resolve_block(block);
	table
}
