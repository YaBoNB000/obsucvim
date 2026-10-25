
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "local t = V[a + 1]; local n = V[b + 1]; t[n + 1] = V[c + 1]; V[b + 1] = n + 1"
			.to_string(),
		_ => "local t = V[a + 1]; local n = V[b + 1]; local val = V[c + 1]; t[n + 1] = val; V[b + 1] = n + 1"
			.to_string(),
	}
}
