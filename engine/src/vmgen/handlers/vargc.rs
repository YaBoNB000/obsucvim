
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "V[a + 1] = vargc".to_string(),
		_ => "local n = vargc; V[a + 1] = n".to_string(),
	}
}
