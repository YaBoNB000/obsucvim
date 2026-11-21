
use crate::vmgen::handlers::bin_op_body;
use crate::vmgen::strpool::StrPool;

pub fn code(fmt: u8, p: &mut StrPool, luau_target: bool) -> String {
	bin_op_body(fmt, "__sub", &|x, y| format!("{x} - {y}"), p, luau_target)
}
