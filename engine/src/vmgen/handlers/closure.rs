
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "V[a + 1] = makefn(b + 1, V, ups)".to_string(),
		_ => "local cf = makefn(b + 1, V, ups); V[a + 1] = cf".to_string(),
	}
}

pub fn code_v15(fmt: u8) -> String {
	match fmt {
		0 => "V[a + 1] = makefn(E.k[b + 1], V, ups, S)".to_string(),
		_ => "local cf = makefn(E.k[b + 1], V, ups, S); V[a + 1] = cf".to_string(),
	}
}
