
use crate::vmgen::strpool::StrPool;

pub fn code(fmt: u8, p: &mut StrPool, luau_target: bool) -> String {
	let number = p.lit("number");
	let string = p.lit("string");
	let unm = p.lit("__unm");
	let error = if luau_target {
		let msg = p.lit("attempt to perform arithmetic (unm) on ");
		format!("ERR({msg}..tx,0)")
	} else {
		let msg = p.lit("attempt to perform arithmetic on a ");
		let value = p.lit(" value");
		format!("ERR({msg}..tx..{value},0)")
	};
	match fmt {
		0 => format!(
			"local x=V[b+1]; local ox=x; local tx=TYP(x); if tx=={string} then x=TONUM(x) end; if TYP(x)=={number} then V[a+1]=-x else local f,hidden=mget(ox,{unm}); if f~=nil then V[a+1]=f(ox) elseif hidden then V[a+1]=-ox else {error} end end"
		),
		_ => format!(
			"local x=V[b+1]; local ox=x; local tx=TYP(x); if tx=={string} then x=TONUM(x) end; if TYP(x)=={number} then V[a+1]=-x else local f,hidden=mget(ox,{unm}); if f~=nil then V[a+1]=f(ox) elseif hidden then V[a+1]=-ox else {error} end end"
		),
	}
}
