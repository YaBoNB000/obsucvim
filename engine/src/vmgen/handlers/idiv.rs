
use crate::vmgen::strpool::StrPool;

pub fn code(fmt: u8, p: &mut StrPool, luau_target: bool) -> String {
	let number = p.lit("number");
	let string = p.lit("string");
	let idiv = p.lit("__idiv");
	let (err_xy, err_uw) = if luau_target {
		let open = p.lit("attempt to perform arithmetic (");
		let op = p.lit("idiv");
		let on = p.lit(") on ");
		let and = p.lit(" and ");
		let make_error = |left_ty: &str, right_ty: &str| {
			format!(
				"local em={open}..{op}..{on}; if {left_ty}=={right_ty} then ERR(em..{left_ty},0) else ERR(em..{left_ty}..{and}..{right_ty},0) end"
			)
		};
		(make_error("tx", "ty"), make_error("tu", "tw"))
	} else {
		let msg = p.lit("attempt to perform arithmetic on a ");
		let value = p.lit(" value");
		(
			format!("local bad=tx; if TYP(x)=={number} then bad=ty end; ERR({msg}..bad..{value},0)"),
			format!("local bad=tu; if TYP(u)=={number} then bad=tw end; ERR({msg}..bad..{value},0)"),
		)
	};
	let fallback_xy = if luau_target { "ox // oy" } else { "FLOOR(ox/oy)" };
	let fallback_uw = if luau_target { "ou // ow" } else { "FLOOR(ou/ow)" };
	match fmt {
		0 => format!(
			"local x=V[b+1]; local y=V[c+1]; local ox,oy=x,y; local tx,ty=TYP(x),TYP(y); if tx=={string} then x=TONUM(x) end; if ty=={string} then y=TONUM(y) end; if TYP(x)=={number} and TYP(y)=={number} then V[a+1]=FLOOR(x/y) else local f,h=mget(ox,{idiv}); if f==nil and not h then local g,h2=mget(oy,{idiv}); f=g; h=h2 end; if f~=nil then V[a+1]=f(ox,oy) elseif h then V[a+1]={fallback_xy} else {err_xy} end end",
			fallback_xy = fallback_xy,
			err_xy = err_xy,
		),
		_ => format!(
			"local u=V[b+1]; local w=V[c+1]; local ou,ow=u,w; local tu,tw=TYP(u),TYP(w); if tu=={string} then u=TONUM(u) end; if tw=={string} then w=TONUM(w) end; if TYP(u)=={number} and TYP(w)=={number} then V[a+1]=FLOOR(u/w) else local f,h=mget(ou,{idiv}); if f==nil and not h then local g,h2=mget(ow,{idiv}); f=g; h=h2 end; if f~=nil then V[a+1]=f(ou,ow) elseif h then V[a+1]={fallback_uw} else {err_uw} end end",
			fallback_uw = fallback_uw,
			err_uw = err_uw,
		),
	}
}
