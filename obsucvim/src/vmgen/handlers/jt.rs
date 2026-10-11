
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "if V[a + 1] then pc = b end".to_string(),
		_ => "local t = V[a + 1]; if t then pc = b end".to_string(),
	}
}
