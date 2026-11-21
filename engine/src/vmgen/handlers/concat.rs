
use crate::vmgen::strpool::StrPool;

pub fn code(fmt: u8, p: &mut StrPool) -> String {
	let number = p.lit("number");
	let string = p.lit("string");
	let concat = p.lit("__concat");
	let msg = p.lit("attempt to concatenate ");
	let with = p.lit(" with ");
	match fmt {
		0 => format!(
			"local x=V[b+1]; local y=V[c+1]; local tx=TYP(x); local ty=TYP(y); if (tx=={number} or tx=={string}) and (ty=={number} or ty=={string}) then V[a+1]=x..y else local f,h=mget(x,{concat}); if f==nil and not h then local g,h2=mget(y,{concat}); f=g; h=h2 end; if f~=nil then V[a+1]=f(x,y) elseif h then V[a+1]=x..y else ERR({msg}..tx..{with}..ty,0) end end"
		),
		_ => format!(
			"local u=V[b+1]; local w=V[c+1]; local tu=TYP(u); local tw=TYP(w); if (tu=={number} or tu=={string}) and (tw=={number} or tw=={string}) then V[a+1]=u..w else local f,h=mget(u,{concat}); if f==nil and not h then local g,h2=mget(w,{concat}); f=g; h=h2 end; if f~=nil then V[a+1]=f(u,w) elseif h then V[a+1]=u..w else ERR({msg}..tu..{with}..tw,0) end end"
		),
	}
}
