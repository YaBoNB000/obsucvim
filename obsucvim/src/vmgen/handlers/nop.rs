//! Nop — polymorphic dead instruction (padding + self-modification alias target).
//!
//! H3: one build emits four semantics-neutral runtime shapes behind a selector
//! derived from that site's noise operands. Different Nop sites therefore take
//! arithmetic / comparison / fake-jump / dead-table paths, while branch order
//! and selector lane are build-seeded.

use crate::rng::Rng;

pub fn body(rng: &mut Rng) -> String {
	let selectors = ["a", "b", "c", "d"];
	let selector = selectors[rng.int(0, 3) as usize];
	let mut order: Vec<u8> = vec![0, 1, 2, 3];
	rng.shuffle(&mut order);
	let shape = |kind: u8| -> &'static str {
		match kind {
			0 => "local _n = (a + b) * (c % 17 + 1) - d",
			1 => "local _n if a < b then _n = c else _n = d end",
			2 => "local _p = a if b % 2 == 0 then _p = _p + c % 3 else _p = _p - d % 3 end",
			_ => "local _n = {a, c} _n[b % 2 + 1] = d",
		}
	};
	let mut out = String::new();
	for (i, &kind) in order.iter().enumerate() {
		if i == 0 {
			out.push_str(&format!("if {selector} % 4 == {kind} then "));
		} else if i + 1 == order.len() {
			out.push_str("else ");
		} else {
			out.push_str(&format!("elseif {selector} % 4 == {kind} then "));
		}
		out.push_str(shape(kind));
		out.push(' ');
	}
	out.push_str("end");
	out
}
