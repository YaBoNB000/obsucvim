
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "local t = {}; for i = 1, vargc do t[i] = vargs[i] end; V[a + 1] = t".to_string(),
		_ => "local t = {}; local i = 1; while i <= vargc do t[i] = vargs[i]; i = i + 1 end; V[a + 1] = t"
			.to_string(),
	}
}
