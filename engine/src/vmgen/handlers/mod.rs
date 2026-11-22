
use crate::vmgen::strpool::StrPool;

pub mod add;
pub mod call;
pub mod calle;
pub mod callm;
pub mod callt;
pub mod closure;
pub mod concat;
pub mod div;
pub mod eq;
pub mod ge;
pub mod getglobal;
pub mod gettab;
pub mod getup;
pub mod gt;
pub mod idiv;
pub mod jf;
pub mod jmp;
pub mod jt;
pub mod le;
pub mod len;
pub mod loadk;
pub mod loadnil;
pub mod loadstr;
pub mod lt;
pub mod mkstr;
pub mod mod_;
#[path = "move.rs"]
pub mod move_;
pub mod mul;
pub mod ne;
pub mod newtab;
pub mod nop;
pub mod not_;
pub mod pow;
pub mod ret;
pub mod setglobal;
pub mod settab;
pub mod setup;
pub mod sub;
pub mod tabn;
pub mod unm;
pub mod vargc;
pub mod vargtab;
pub mod vargtabn;

pub fn n_formats(name: &str, v15: bool) -> u8 {
	match name {
		"Return" if v15 => 1,
		_ => match name {
			"Add" | "Sub" | "Mul" | "Div" | "Mod" | "Pow" => 3,
			"Lt" | "Le" | "Gt" | "Ge" => 3,
			_ => 2,
		},
	}
}

pub fn gen(name: &str, fmt: u8, v15: bool, p: &mut StrPool, mk: (u16, u16), luau_target: bool) -> String {
	match name {
		"Jmp" => jmp::code(fmt),
		"Jf" => jf::code(fmt),
		"Jt" => jt::code(fmt),
		"LoadNil" => {
			if v15 {
				loadnil::code_v15(fmt)
			} else {
				loadnil::code(fmt)
			}
		}
		"LoadK" => loadk::code(fmt),
		"LoadStr" => loadstr::code(fmt),
		"Move" => move_::code(fmt),
		"Add" => add::code(fmt, p, luau_target),
		"Sub" => sub::code(fmt, p, luau_target),
		"Mul" => mul::code(fmt, p, luau_target),
		"Div" => div::code(fmt, p, luau_target),
		"Mod" => mod_::code(fmt, p, luau_target),
		"Pow" => pow::code(fmt, p, luau_target),
		"Concat" => concat::code(fmt, p),
		"Unm" => unm::code(fmt, p, luau_target),
		"Not" => not_::code(fmt),
		"Len" => len::code(fmt, p),
		"Lt" => lt::code(fmt, p, luau_target),
		"Le" => le::code(fmt, p, luau_target),
		"Gt" => gt::code(fmt, p, luau_target),
		"Ge" => ge::code(fmt, p, luau_target),
		"Eq" => eq::code(fmt, p),
		"Ne" => ne::code(fmt, p),
		"Idiv" => idiv::code(fmt, p, luau_target),
		"NewTab" => newtab::code(fmt),
		"GetTab" => gettab::code(fmt, p),
		"SetTab" => settab::code(fmt, p),
		"TabN" => tabn::code(fmt),
		"CallT" => {
			if v15 {
				callt::code_v15(fmt)
			} else {
				callt::code(fmt)
			}
		}
		"Closure" => {
			if v15 {
				closure::code_v15(fmt)
			} else {
				closure::code(fmt)
			}
		}
		"Call" => {
			if v15 {
				call::code_v15(fmt)
			} else {
				call::code(fmt)
			}
		}
		"VarArgTab" => vargtab::code(fmt),
		"VarArgC" => vargc::code(fmt),
		"VarArgTabN" => vargtabn::code(fmt),
		"GetGlobal" => getglobal::code(fmt),
		"SetGlobal" => setglobal::code(fmt),
		"GetUp" => getup::code(fmt),
		"SetUp" => setup::code(fmt),
		"Return" => {
			if v15 {
				ret::code_v15()
			} else {
				ret::code(fmt)
			}
		}
		"CallE" => {
			if v15 {
				calle::code_v15(fmt)
			} else {
				calle::code(fmt)
			}
		}
		"CallM" => {
			if v15 {
				callm::code_v15(fmt)
			} else {
				callm::code(fmt)
			}
		}
		"MkStr" => mkstr::code(fmt, mk),
		_ => panic!("unknown opcode name {name}"),
	}
}

pub(crate) fn bin_op_body(
	fmt: u8,
	mm: &str,
	expr: &dyn Fn(&str, &str) -> String,
	p: &mut StrPool,
	luau_target: bool,
) -> String {
	let number = p.lit("number");
	let string = p.lit("string");
	let meta = p.lit(mm);
	let msg = p.lit("attempt to perform arithmetic on a ");
	let value = p.lit(" value");
	let (err_xy, err_uw) = if luau_target {
		let op = match mm {
			"__add" => "add",
			"__sub" => "sub",
			"__mul" => "mul",
			"__div" => "div",
			"__mod" => "mod",
			"__pow" => "pow",
			_ => unreachable!("unknown arithmetic metamethod {mm}"),
		};
		let open = p.lit("attempt to perform arithmetic (");
		let op = p.lit(op);
		let on = p.lit(") on ");
		let and = p.lit(" and ");
		let make_error = |left_ty: &str, right_ty: &str| {
			format!(
				"local em={open}..{op}..{on}; if {left_ty}=={right_ty} then ERR(em..{left_ty},0) else ERR(em..{left_ty}..{and}..{right_ty},0) end"
			)
		};
		(make_error("tx", "ty"), make_error("tu", "tw"))
	} else {
		(
			format!("local bad=tx; if TYP(x)=={number} then bad=ty end; ERR({msg}..bad..{value},0)"),
			format!("local bad=tu; if TYP(u)=={number} then bad=tw end; ERR({msg}..bad..{value},0)"),
		)
	};
	match fmt {
		0 => format!(
			"local x=V[b+1]; local y=V[c+1]; local ox,oy=x,y; local tx,ty=TYP(x),TYP(y); if tx=={string} then x=TONUM(x) end; if ty=={string} then y=TONUM(y) end; if TYP(x)=={number} and TYP(y)=={number} then V[a+1]={exy} else local f,h=mget(ox,{meta}); if f==nil and not h then local g,h2=mget(oy,{meta}); f=g; h=h2 end; if f~=nil then V[a+1]=f(ox,oy) elseif h then V[a+1]={native} else {err_xy} end end",
			exy = expr("x", "y"),
			native = expr("ox", "oy"),
			err_xy = err_xy,
		),
		1 => format!(
			"local u=V[b+1]; local w=V[c+1]; local ou,ow=u,w; local tu,tw=TYP(u),TYP(w); if tu=={string} then u=TONUM(u) end; if tw=={string} then w=TONUM(w) end; if TYP(u)=={number} and TYP(w)=={number} then V[a+1]={euw} else local f,h=mget(ou,{meta}); if f==nil and not h then local g,h2=mget(ow,{meta}); f=g; h=h2 end; if f~=nil then V[a+1]=f(ou,ow) elseif h then V[a+1]={native} else {err_uw} end end",
			euw = expr("u", "w"),
			native = expr("ou", "ow"),
			err_uw = err_uw,
		),
		_ => format!(
			"local x=V[b+1]; local y=V[c+1]; local ox,oy=x,y; local tx,ty=TYP(x),TYP(y); if tx=={string} then x=TONUM(x) end; if ty=={string} then y=TONUM(y) end; if TYP(x)=={number} and TYP(y)=={number} then V[a+1]={exy} else local f,h=mget(ox,{meta}); if f==nil and not h then local g,h2=mget(oy,{meta}); f=g; h=h2 end; if f~=nil then V[a+1]=f(ox,oy) elseif h then V[a+1]={native} else {err_xy} end end",
			exy = expr("x", "y"),
			native = expr("ox", "oy"),
			err_xy = err_xy,
		),
	}
}

pub(crate) fn cmp_body(
	fmt: u8,
	native: &str,
	mm: &str,
	swapped: bool,
	p: &mut StrPool,
	luau_target: bool,
) -> String {
	let number = p.lit("number");
	let string = p.lit("string");
	let meta = p.lit(mm);
	let msg = p.lit("attempt to compare ");
	let with = p.lit(" with ");
	let op = native.split_whitespace().nth(1).expect("comparison operator");
	let op = p.lit(op);
	let space = p.lit(" ");
	let call = |x: &str, y: &str| {
		if swapped {
			format!("f({y}, {x})")
		} else {
			format!("f({x}, {y})")
		}
	};
	let error = |left: &str, right: &str| {
		if luau_target {
			format!("ERR({msg}..TYP({left})..{space}..{op}..{space}..TYP({right}),0)")
		} else {
			format!("ERR({msg}..TYP({left})..{with}..TYP({right}),0)")
		}
	};
	let nat = native;
	match fmt {
		0 => format!(
			"local x=V[b+1]; local y=V[c+1]; if TYP(x)=={number} and TYP(y)=={number} then V[a+1]={nat} elseif TYP(x)=={string} and TYP(y)=={string} then V[a+1]={nat} else local f,h=mget(x,{meta}); if f==nil and not h then local g,h2=mget(y,{meta}); f=g; h=h2 end; if f~=nil then V[a+1]={cxy} elseif h then V[a+1]={nat} else {err_xy} end end",
			cxy = call("x", "y"),
			err_xy = error("x", "y"),
		),
		1 => {
			let nat_uw = native.replace('x', "u").replace('y', "w");
			format!(
				"local u=V[b+1]; local w=V[c+1]; if TYP(u)=={number} and TYP(w)=={number} then V[a+1]={nat_uw} elseif TYP(u)=={string} and TYP(w)=={string} then V[a+1]={nat_uw} else local f,h=mget(u,{meta}); if f==nil and not h then local g,h2=mget(w,{meta}); f=g; h=h2 end; if f~=nil then V[a+1]={cuw} elseif h then V[a+1]={nat_uw} else {err_uw} end end",
				cuw = call("u", "w"),
				err_uw = error("u", "w"),
			)
		}
		_ => format!(
			"local x=V[b+1]; local y=V[c+1]; local tx=TYP(x); local ty=TYP(y); if tx=={number} and ty=={number} then V[a+1]={nat} elseif tx=={string} and ty=={string} then V[a+1]={nat} else local f,h=mget(x,{meta}); if f==nil and not h then local g,h2=mget(y,{meta}); f=g; h=h2 end; if f~=nil then V[a+1]={cxy} elseif h then V[a+1]={nat} else {err_xy_typed} end end",
			cxy = call("x", "y"),
			err_xy_typed = if luau_target {
				format!("ERR({msg}..tx..{space}..{op}..{space}..ty,0)")
			} else {
				format!("ERR({msg}..tx..{with}..ty,0)")
			},
		),
	}
}
