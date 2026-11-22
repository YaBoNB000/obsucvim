
use crate::vmgen::strpool::StrPool;

pub fn code(fmt: u8, p: &mut StrPool) -> String {
	let table = p.lit("table");
	let newindex = p.lit("__newindex");
	let function = p.lit("function");
	let msg = p.lit("attempt to index ");
	let with = p.lit(" with '");
	let quote = p.lit("'");
	match fmt {
		0 => format!(
			"local t=V[a+1]; local k=V[b+1]; local v=V[c+1]; if TYP(t)~={table} then local f,hidden=mget(t,{newindex}); if TYP(f)=={function} then f(t,k,v) elseif f~=nil then f[k]=v elseif hidden then t[k]=v else ERR({msg}..TYP(t)..{with}..TSTR(k)..{quote},0) end else if RGET(t,k)==nil then local f,hidden=mget(t,{newindex}); if TYP(f)=={function} then f(t,k,v) elseif f~=nil then f[k]=v elseif hidden then t[k]=v else RSET(t,k,v) end else RSET(t,k,v) end end"
		),
		_ => format!(
			"local o=V[a+1]; local j=V[b+1]; local w=V[c+1]; if TYP(o)~={table} then local f,hidden=mget(o,{newindex}); if TYP(f)=={function} then f(o,j,w) elseif f~=nil then f[j]=w elseif hidden then o[j]=w else ERR({msg}..TYP(o)..{with}..TSTR(j)..{quote},0) end else if RGET(o,j)==nil then local f,hidden=mget(o,{newindex}); if TYP(f)=={function} then f(o,j,w) elseif f~=nil then f[j]=w elseif hidden then o[j]=w else RSET(o,j,w) end else RSET(o,j,w) end end"
		),
	}
}
