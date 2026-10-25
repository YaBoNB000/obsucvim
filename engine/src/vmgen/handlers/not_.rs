
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "V[a + 1] = not V[b + 1]".to_string(),
		_ => "local t = V[b + 1]; V[a + 1] = not t".to_string(),
	}
}
