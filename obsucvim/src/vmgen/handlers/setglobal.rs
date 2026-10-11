
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "G[C[b + 1]] = V[a + 1]".to_string(),
		_ => "local k = C[b + 1]; local val = V[a + 1]; G[k] = val".to_string(),
	}
}
