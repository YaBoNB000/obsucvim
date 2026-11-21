
use crate::vmgen::strpool::StrPool;

pub fn code(fmt: u8, p: &mut StrPool) -> String {
	let table = p.lit("table");
	let string = p.lit("string");
	let index = p.lit("__index");
	let function = p.lit("function");
	let msg = p.lit("attempt to index ");
	let with = p.lit(" with '");
	let quote = p.lit("'");
	match fmt {
		0 => format!(
			"local t=V[b+1]; local k=V[c+1]; local r; if TYP(t)=={string} then r=t[k] elseif TYP(t)=={table} then r=RGET(t,k); if r==nil then local f,hidden=mget(t,{index}); if TYP(f)=={function} then r=f(t,k) elseif f~=nil then r=f[k] elseif hidden then r=t[k] end end else local f,hidden=mget(t,{index}); if TYP(f)=={function} then r=f(t,k) elseif f~=nil then r=f[k] elseif hidden then r=t[k] else ERR({msg}..TYP(t)..{with}..TSTR(k)..{quote},0) end end; V[a+1]=r"
		),
		_ => format!(
			"local o=V[b+1]; local j=V[c+1]; local r; if TYP(o)=={string} then r=o[j] elseif TYP(o)=={table} then r=RGET(o,j); if r==nil then local f,hidden=mget(o,{index}); if TYP(f)=={function} then r=f(o,j) elseif f~=nil then r=f[j] elseif hidden then r=o[j] end end else local f,hidden=mget(o,{index}); if TYP(f)=={function} then r=f(o,j) elseif f~=nil then r=f[j] elseif hidden then r=o[j] else ERR({msg}..TYP(o)..{with}..TSTR(j)..{quote},0) end end; V[a+1]=r"
		),
	}
}
