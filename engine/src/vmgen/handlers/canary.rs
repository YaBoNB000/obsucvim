/// Phase 18 P1-3: legacy (non-v15) canary handler.
///
/// The v15 pipeline implements the canary as the `M_CANARY` micro-op and calls
/// the guard's bounded fail-closed `PD()`; the legacy `--preset vm` template has
/// no `PD` binding, so the same bounded abort (65,536 LCG steps followed by a
/// boolean-index fault) is inlined here instead. The four checks are identical
/// to the v15 ones.
///
/// Scope note: the legacy dispatch tree only binds `CHAR`, `BYTE` and `TYP`
/// (there is no `PD`/`STBL`/`RAWEQ` in that scope), so the environment-resolved
/// `string` table is read through the global here. That is deliberate: the
/// canary exists to catch a *mid-execution* swap of `string.char`, and a
/// captured upvalue would keep pointing at the pre-swap function. The load-time
/// guard stage (`strcanary`) still validates the table before the payload runs.
pub fn code(_fmt: u8) -> String {
	String::from(
		"do local cid = a; local ok = false; \
		if cid == 0 then ok = CHAR(65) == \"A\" \
		elseif cid == 1 then ok = #(\"abc\") == 3 \
		elseif cid == 2 then ok = TYP(string) == \"table\" and TYP(string.char) == \"function\" \
		else ok = CHAR(BYTE(\"Z\")) == \"Z\" end; \
		if not ok then local z0, z1 = 0, 1; while z0 < 65536 do z0 = z0 + 1; z1 = (z1 * 80659 + 95050511 + z0) % 268435456 end; local zf = false; zf = zf[zf]; local zn; return zn[z1] end end",
	)
}
