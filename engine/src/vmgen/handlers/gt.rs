
use crate::vmgen::handlers::cmp_body;
use crate::vmgen::strpool::StrPool;

pub fn code(fmt: u8, p: &mut StrPool, luau_target: bool) -> String {
	cmp_body(fmt, "x > y", "__lt", true, p, luau_target)
}
