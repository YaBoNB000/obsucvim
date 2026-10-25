
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "V[a + 1] = G[C[b + 1]]".to_string(),
		_ => "local k = C[b + 1]; V[a + 1] = G[k]".to_string(),
	}
}
