
use crate::vmgen::strpool::StrPool;

pub fn code(fmt: u8, p: &mut StrPool) -> String {
	let number = p.lit("number");
	let unm = p.lit("__unm");
	let msg = p.lit("attempt to perform arithmetic on a ");
	let value = p.lit(" value");
	match fmt {
		0 => format!(
			"local x = V[b + 1]; if TYP(x) == {number} then V[a + 1] = -x else local f = mget(x, {unm}); if f then V[a + 1] = f(x) else ERR({msg} .. TYP(x) .. {value}, 0) end end"
		),
		_ => format!(
			"local x = V[b + 1]; local tx = TYP(x); if tx == {number} then V[a + 1] = -x else local f = mget(x, {unm}); if f then V[a + 1] = f(x) else ERR({msg} .. tx .. {value}, 0) end end"
		),
	}
}
