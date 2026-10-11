
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "V[a + 1] = {}".to_string(),
		_ => "local t = {}; V[a + 1] = t".to_string(),
	}
}
