
pub fn code(fmt: u8, mk: (u16, u16)) -> String {
	let (mk1, mk2) = (mk.0 as u32, mk.1 as u32);
	match fmt {
		0 => format!(
			"local msk = ({mk1} * a + {mk2}) % 65536; local b0 = (b - msk) % 65536; local c0 = (c - msk) % 65536; local d0 = (d - msk) % 65536; local n = FLOOR(d0 / 256); local t = {{}}; t[1] = b0 % 256; t[2] = FLOOR(b0 / 256); t[3] = c0 % 256; t[4] = FLOOR(c0 / 256); t[5] = d0 % 256; local _mc = (\"\")[\"char\"]; local s = \"\"; local i = 1; while i <= n do s = s .. _mc(t[i]); i = i + 1 end; V[a + 1] = s"
		),
		_ => format!(
			"local ms = ({mk1} * a + {mk2}) % 65536; local q1 = (b - ms) % 65536; local q2 = (c - ms) % 65536; local q3 = (d - ms) % 65536; local n = FLOOR(q3 / 256); local t = {{}}; t[1] = q1 % 256; t[2] = FLOOR(q1 / 256); t[3] = q2 % 256; t[4] = FLOOR(q2 / 256); t[5] = q3 % 256; local _mc = (\"\")[\"char\"]; local s = \"\"; local i = 1; while i <= n do s = s .. _mc(t[i]); i = i + 1 end; V[a + 1] = s"
		),
	}
}
