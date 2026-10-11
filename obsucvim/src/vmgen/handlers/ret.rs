
pub fn code(fmt: u8) -> String {
	match fmt {
		0 => "local out = {}; local n = b; local total; if n == 255 then local pre = d; for i = 1, pre do out[i] = V[a + i] end; if c == 1 then for i = 1, vargc do out[pre + i] = vargs[i] end; total = pre + vargc else for i = 1, lastn do out[pre + i] = V[lastbase + i] end; total = pre + lastn end else for i = 1, n do out[i] = V[a + i] end; total = n end; return U(out, 1, total)".to_string(),
		_ => "local ov = {}; local n = b; local tot; if n == 255 then local pre = d; for i = 1, pre do ov[i] = V[a + i] end; if c == 1 then for i = 1, vargc do ov[pre + i] = vargs[i] end; tot = pre + vargc else for i = 1, lastn do ov[pre + i] = V[lastbase + i] end; tot = pre + lastn end else for i = 1, n do ov[i] = V[a + i] end; tot = n end; return U(ov, 1, tot)".to_string(),
	}
}

pub fn code_v15() -> String {
	"local out = {}; local n = b; local total; if n == 255 then local pre = d; for i = 1, pre do out[i] = V[S[a + i]] end; if c == 1 then for i = 1, vargc do out[pre + i] = vargs[i] end; total = pre + vargc else for i = 1, lastn do local s = S[lastbase + i]; if s then out[pre + i] = V[s] else out[pre + i] = O[lastbase + i] end end; total = pre + lastn end else for i = 1, n do out[i] = V[S[a + i]] end; total = n end; return U(out, 1, total)"
		.to_string()
}
