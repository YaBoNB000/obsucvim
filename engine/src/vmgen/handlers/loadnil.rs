
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "for i = 1, b do V[a + i] = nil end".to_string(),
		_ => "local i = 1; while i <= b do V[a + i] = nil; i = i + 1 end".to_string(),
	}
}

pub fn code_v15(fmt: u8) -> String {
	match fmt {
		0 => "for i = 1, b do V[S[a + i]] = nil end".to_string(),
		_ => "local i = 1; while i <= b do V[S[a + i]] = nil; i = i + 1 end".to_string(),
	}
}
