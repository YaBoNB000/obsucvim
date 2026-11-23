
use crate::ast::*;
use crate::mangle::{gen_name, reserved_set};
use crate::rng::Rng;
use crate::strings::{build_loader, encrypt, split_into_parts, KEY_LEN};
use crate::symtab::{Sym, SymTable};
use std::collections::HashSet;

pub fn apply_body(
	block: &mut Block,
	table: &mut SymTable,
	rng: &mut Rng,
	reserved: &mut HashSet<String>,
	luau: bool,
) {
	let tim = crate::audit_gate::audit_enabled("OBSUCVIM_TIMING");
	let mut tmark = std::time::Instant::now();
	let mut tlog = |name: &str| {
		if tim {
			eprintln!("[time] body/{} = {:?}", name, tmark.elapsed());
			tmark = std::time::Instant::now();
		}
	};
	let text = if luau {
		crate::printer::print_chunk_luau(table, block)
	} else {
		crate::printer::print_chunk(table, block)
	};
	tlog("print_chunk");
	let compact = crate::minify::minify(&text, luau).unwrap_or(text);
	tlog("minify");

	let key: Vec<u8> = (0..KEY_LEN).map(|_| rng.int(0, 255) as u8).collect();
	let ct = encrypt(compact.as_bytes(), &key);
	tlog("encrypt");

	let n_chunks = (rng.int(3, 5) as usize).min(ct.len().max(1));
	let chunks = split_into_parts(&ct, n_chunks);

	let mut rsv = reserved_set(reserved);
	rsv.extend(table.globals.iter().cloned());
	rsv.extend(table.syms.iter().map(|s| s.name.clone()));
	rsv.insert("loadstring".to_string());
	let (loader, dec_sym, dec_name) = build_loader(table, rng, &mut rsv, &key, false);
	tlog("build_loader");

	let mut chunk_syms: Vec<SymId> = Vec::new();
	let mut chunk_stmts: Vec<Stmt> = Vec::new();
	for bytes in &chunks {
		let id = table.syms.len() as SymId;
		let name = gen_name(rng, &mut rsv);
		rsv.insert(name.clone());
		table.syms.push(Sym {
			name: name.clone(),
			is_param: false,
			keep_name: false,
		});
		chunk_syms.push(id);
		chunk_stmts.push(Stmt::Local {
			names: vec![name],
			syms: vec![id],
			values: vec![Some(Expr::Str {
				bytes: bytes.clone(),
				is_binary: true,
			})],
		});
	}

	let dec_call = Expr::Call {
		func: Box::new(Expr::Ident {
			name: dec_name,
			sym: Some(dec_sym),
		}),
		args: chunk_syms
			.iter()
			.map(|s| Expr::Ident {
				name: table.name_of(*s).to_string(),
				sym: Some(*s),
			})
			.collect(),
	};
	let final_call = Expr::Call {
		func: Box::new(Expr::Call {
			func: Box::new(Expr::Ident {
				name: "loadstring".to_string(),
				sym: None,
			}),
			args: vec![dec_call],
		}),
		args: Vec::new(),
	};

	let mut stmts = Vec::new();
	stmts.extend(loader);
	stmts.extend(chunk_stmts);
	stmts.push(Stmt::ExprStmt(final_call));
	block.stmts = stmts;
}
