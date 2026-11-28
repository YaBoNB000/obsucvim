/// Structured, auditable security manifest for each anti-tamper guard stage (Phase 17 §3.1).
///
/// Every check explicitly documents and enforces:
/// - **Required foundational Luau/Lua 5.1 semantics** (`type`, `pcall`, `error`,
///   `rawget`, `rawset`, `getmetatable`, `setmetatable`, table metatable lock):
///   absence or behavioral corruption is treated as active tamper (`abort()`).
/// - **Optional host/sandbox capabilities** (`xpcall`, `getfenv`, `_G`, `print`,
///   `warn`, `tostring`, `tonumber`, `select`, `newproxy`, `unpack`, `table`,
///   `table.unpack`, `next`, `debug`, `debug.info`, `os`, `os.clock`,
///   `loadstring`, `load`): legitimate omission (`nil`) is allowed without false
///   positives, whereas non-nil wrong-type values, behavioral corruption, or
///   spoofed provenance trigger bounded fail-closed termination before user
///   payload execution.
/// - **`Unpack` Contract (Scheme A)**: `VmPrim::Unpack` binds
///   `unpack -> table.unpack -> pure-Lua slice fallback (_uf)`. Consequently,
///   `unpack == nil and table.unpack == nil` is explicitly allowed, while any
///   non-nil `unpack` or `table.unpack` is independently verified for function
///   type, intermediate/trailing `nil` preservation, `(i, j)` slice bounds,
///   empty interval `i > j` zero-return count, and `select("#", ...)` arity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct GuardStageMeta {
	pub id: &'static str,
	pub target: &'static str,
	pub threat_model: &'static str,
	pub required_apis: &'static [&'static str],
	pub optional_apis: &'static [&'static str],
	pub allowed_missing: &'static str,
	pub tamper_triggers: &'static str,
	pub luau_guaranteed: bool,
	pub fail_action: &'static str,
	pub trigger_timing: &'static str,
	pub resource_bound: &'static str,
	pub positive_test_ids: &'static [&'static str],
	pub negative_test_ids: &'static [&'static str],
}

#[allow(dead_code)]
impl GuardStageMeta {
	pub fn all_test_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
		self.positive_test_ids
			.iter()
			.copied()
			.chain(self.negative_test_ids.iter().copied())
	}
}

#[allow(dead_code)]
pub const STAGE_MANIFEST: [GuardStageMeta; 6] = [
	GuardStageMeta {
		id: "core",
		target: "Essential base primitives callable + optional xpcall type/handler roundtrip",
		threat_model: "Detects replacement of foundational Lua/Luau runtime primitives with non-functions or broken xpcall error-recovery stubs used to neutralize protected checks.",
		required_apis: &[
			"type",
			"pcall",
			"error",
			"rawget",
			"rawset",
			"getmetatable",
			"setmetatable",
		],
		optional_apis: &["xpcall"],
		allowed_missing: "xpcall == nil is permitted for stripped Lua/Luau sandboxes; all 7 core primitives must exist as functions.",
		tamper_triggers: "Any of the 7 core primitives ~= function, xpcall ~= nil and ~= function, or xpcall(error_fn, handler) failing to invoke handler / returning true.",
		luau_guaranteed: true,
		fail_action: "Immediate bounded abort() (65,536 LCG steps + C-VM boolean/nil index fault; pre-HBOOT PCAL non-function also triggers PD()).",
		trigger_timing: "Pre-payload VM bootstrap (inside HBOOT / guard_prelude before bytecode decode or user chunk execution).",
		resource_bound: "O(1) stack/heap (4 locals), deterministic 65,536 arithmetic iterations (<1 ms on stock Luau), zero unbounded spin.",
		positive_test_ids: &[
			"clean_stock_luau",
			"clean_stock_luau_guard_on",
			"clean_stock_luau_guard_off",
			"xpcall_nil_allowed",
		],
		negative_test_ids: &[
			"pcall_tampered_nonfn",
			"xpcall_tampered_nonfn",
			"xpcall_stub_broken",
			"error_tampered_nonfn",
			"rawset_tampered_nonfn",
			"setmetatable_tampered_nonfn",
		],
	},
	GuardStageMeta {
		id: "env",
		target: "Environment table resolution, rawset/rawget negative-key roundtrip, pcall(error) failure semantics, optional I/O & conversion global type sanity",
		threat_model: "Detects global/setfenv environment table corruption, rawget/rawset interception that drops negative sentinel keys, error-swallowing pcall shims, and wrong-type I/O globals.",
		required_apis: &["rawget", "rawset", "pcall", "error"],
		optional_apis: &["getfenv", "_G", "print", "warn", "tostring", "tonumber"],
		allowed_missing: "getfenv == nil, _G == nil, print == nil, warn == nil, and legitimate function redirection of print/warn (including setfenv __index proxy tables and LUAU_READONLY_GLOBALS=1) are permitted.",
		tamper_triggers: "Non-nil getfenv/print/warn/tostring/tonumber ~= function, resolved env ~= table, rawset/rawget sentinel roundtrip mismatch, or pcall(error) returning true.",
		luau_guaranteed: true,
		fail_action: "Sets failed = true -> GUARD_EPILOGUE bounded abort() before HBOOT phase decode.",
		trigger_timing: "Pre-payload VM bootstrap (stage shuffle inside _guard before user VM entry).",
		resource_bound: "O(1) temporary 1-entry table + closure allocation, bounded abort() on failure.",
		positive_test_ids: &[
			"readonly_globals",
			"readonly_globals_guard_on",
			"readonly_globals_guard_off",
			"getfenv_nil_allowed",
			"print_warn_nil_allowed",
			"warn_nil_allowed",
			"print_redirect_allowed",
			"setfenv_index_proxy_allowed",
			"differential_noguard_warn_nonfn",
		],
		negative_test_ids: &[
			"getfenv_tampered_nonfn",
			"warn_tampered_nonfn",
			"print_tampered_nonfn",
			"tostring_tampered_nonfn",
			"tonumber_tampered_nonfn",
			"rawget_tampered",
			"pcall_error_swallowed",
		],
	},
	GuardStageMeta {
		id: "debug",
		target: "Optional debug.info closure-vs-[C] provenance, uniform 7-core-primitive [C] consistency, & native loadstring/load verification",
		threat_model: "Detects naive Lua-layer function hooks on core primitives or loadstring/load and blanket debug.info('[C]') spoofing, while acknowledging that C-level host hooks or selective wrapped-set debug.info spoofing inside a replaced thread env cannot be guaranteed detectable from pure Lua alone.",
		required_apis: &[],
		optional_apis: &["debug", "debug.info", "loadstring", "load"],
		allowed_missing: "debug == nil, debug.info == nil, loadstring == nil, load == nil, or runtimes whose debug.info(..., 's') does not report '[C]' for native functions are permitted.",
		tamper_triggers: "debug ~= nil and ~= table, debug.info ~= nil and ~= function (checked on both chunk env and getfenv(0) fallback), fresh Lua closure reporting '[C]', selective Lua wrapping among the 7 core primitives when any reports '[C]', or non-[C] / non-function loadstring or load.",
		luau_guaranteed: false,
		fail_action: "Sets failed = true -> GUARD_EPILOGUE bounded abort() before HBOOT phase decode.",
		trigger_timing: "Pre-payload VM bootstrap (stage shuffle inside _guard before user VM entry).",
		resource_bound: "O(1) (at most 12 protected debug.info('s') probes + bounded abort() on failure).",
		positive_test_ids: &[
			"debug_nil_allowed",
			"debug_info_nil_allowed",
			"loadstring_load_nil_allowed",
		],
		negative_test_ids: &[
			"debug_non_table_tamper",
			"debug_info_nonfn_tamper",
			"debug_info_spoof_c",
			"pcall_lua_hook_detected",
			"shadow_rawget_hook_with_xpcall_nil",
			"loadstring_nonfn_tamper",
			"loadstring_lua_hook",
			"load_lua_hook",
			"loadstring_hook_with_env_debug_nil",
		],
	},
	GuardStageMeta {
		id: "canary",
		target: "Locked metatable honeypot canaries + optional newproxy(true) userdata canary",
		threat_model: "Detects blanket metatable unlocking/stripping shims (broken __metatable lock), corrupted newproxy(true) shims, and post-boot blind table/userdata value dumps that invoke tostring/concat/call/iter metamethods on stashed HW2 canary slots.",
		required_apis: &["setmetatable", "getmetatable"],
		optional_apis: &["newproxy"],
		allowed_missing: "newproxy == nil is permitted.",
		tamper_triggers: "newproxy ~= nil and ~= function, newproxy(true) failing or lacking a table metatable, setmetatable failing, getmetatable ignoring __metatable lock string, or runtime invocation of canary metamethods.",
		luau_guaranteed: true,
		fail_action: "Boot failure sets failed = true -> bounded abort(); runtime canary metamethod probe invokes tripwire() -> bounded abort().",
		trigger_timing: "Boot lock verification pre-payload + passive tripwire stashed in HW2[257..456] during VM lifetime.",
		resource_bound: "O(1) (3 small locked tables + at most 1 userdata proxy; bounded abort() on trigger).",
		positive_test_ids: &[
			"newproxy_nil_allowed",
			"differential_noguard_newproxy_nonfn",
		],
		negative_test_ids: &[
			"newproxy_tampered_nonfn",
			"newproxy_broken_no_metatable",
			"metatable_lock_tampered",
		],
	},
	GuardStageMeta {
		id: "misc",
		target: "Scheme A unpack/table.unpack behavioral contract, optional select/next/os/os.clock sanity, & TOCTOU core-primitive shadow consistency",
		threat_model: "Detects corrupted or type-poisoned unpack/table.unpack/select/next/os.clock shims (slice-bound drops, trailing-nil truncation, non-empty i>j return, broken next iteration) and stateful/shadowed EV globals diverging from bound core primitives.",
		required_apis: &[],
		optional_apis: &["unpack", "table", "table.unpack", "select", "next", "os", "os.clock"],
		allowed_missing: "unpack == nil, table.unpack == nil, BOTH unpack == nil and table.unpack == nil (Scheme A pure-Lua _uf fallback), next == nil, os == nil, and os.clock == nil are all permitted.",
		tamper_triggers: "select ~= nil and ~= function, table ~= nil and ~= table, either non-nil unpack or non-nil table.unpack failing multi-return/nil/slice/empty-interval/count probes, non-nil next failing empty/single-key probes, os ~= table, os.clock ~= nil and ~= function, or stateful/boolean-shadowed EV[core_prim] mismatch.",
		luau_guaranteed: true,
		fail_action: "Sets failed = true -> GUARD_EPILOGUE bounded abort() (pre-HBOOT PCAL boolean shadow hits PD()).",
		trigger_timing: "Pre-payload VM bootstrap (stage shuffle inside _guard before user VM entry).",
		resource_bound: "O(1) (small 2..4 element probe tables + 7-slot core check loop; bounded abort() on failure).",
		positive_test_ids: &[
			"os_nil_allowed",
			"os_clock_nil_allowed",
			"unpack_nil_allowed",
			"table_unpack_nil_allowed",
			"both_unpack_nil_allowed",
			"next_nil_allowed",
			"differential_noguard_next_nonfn",
			"differential_noguard_both_unpack_tampered",
		],
		negative_test_ids: &[
			"os_non_table_tamper",
			"os_clock_nonfn_tamper",
			"select_tampered_nonfn",
			"unpack_tampered_nonfn",
			"table_unpack_tampered_nonfn",
			"table_unpack_tampered_when_unpack_nil",
			"unpack_broken_tamper",
			"unpack_slice_bounds_tamper",
			"unpack_trailing_nil_drop_tamper",
			"unpack_empty_range_tamper",
			"next_tampered_nonfn",
			"next_broken_tamper",
			"shadow_pcall_false_tamper",
			"env_stateful_getter_shadow_tamper",
		],
	},
	GuardStageMeta {
		id: "strcanary",
		target: "String-library primitive integrity for the Phase 18 distributed string-cache canaries: environment string table shape, string.char / string.byte type, and value-level self-consistency of the resolved library (char(65), char(65,66), char(97,98,99), byte(90)), plus the four mid-execution canaries themselves",
		threat_model: "Detects off-the-shelf harvester scripts that globally replace string.char / string.byte (or the whole string table) with stubs that drop or shorten return values before the payload runs, and a string table swapped for a non-table. The four mid-execution canaries (rawequal(string.char(65), \"A\"), rawequal(#\"abc\", 3), type(string.char) == \"function\", rawequal(string.char(string.byte(\"Z\")), \"Z\")) re-check the same invariants at four shuffled split points of every compiled instruction stream. Explicitly NOT covered: a transparent forwarding wrapper (behaviourally identical, so undetectable by value), and a fake installed only in a copied environment string table, which the VM never resolves through (it binds char/byte from the string metatable, R021).",
		required_apis: &[],
		optional_apis: &["string", "string.char", "string.byte"],
		allowed_missing: "string == nil, string.char == nil and string.byte == nil are all permitted (stripped sandboxes); only a present-but-wrong value is tamper.",
		tamper_triggers: "string ~= nil and ~= table, string.char ~= nil and ~= function, string.byte ~= nil and ~= function, resolved library char(65) not returning a 1-byte \"A\", char(65,66) not concatenating to char(65) .. char(66), char(97,98,99) not returning a 3-byte string, byte(\"Z\") not returning 90, or any of the four mid-execution canaries observing a violated invariant.",
		luau_guaranteed: true,
		fail_action: "Boot failure sets failed = true -> GUARD_EPILOGUE bounded abort(); a mid-execution canary failure calls the same bounded PD() fail-closed path from inside the VM.",
		trigger_timing: "Pre-payload VM bootstrap (stage shuffle inside _guard) plus four shuffled split points of every compiled instruction stream (floor(n/5)*k, k = 1..4).",
		resource_bound: "O(1) boot probes (5 pcall + 6 type checks) plus 4 constant-time canary checks per compiled function body; bounded abort() on failure.",
		positive_test_ids: &[
			"canary1_char_value_clean",
			"canary2_len_operator_clean",
			"canary3_char_type_clean",
			"canary4_char_byte_roundtrip_clean",
		],
		negative_test_ids: &[
			"canary1_char_value_tamper",
			"canary2_len_operator_tamper",
			"canary3_char_type_tamper",
			"canary4_char_byte_roundtrip_tamper",
		],
	},
];

pub const ANTI_CORE: &str = r###"	local function stage_core()
		if
			type(type) ~= "function"
			or type(pcall) ~= "function"
			or type(error) ~= "function"
			or type(rawget) ~= "function"
			or type(rawset) ~= "function"
			or type(getmetatable) ~= "function"
			or type(setmetatable) ~= "function"
		then
			abort()
		end

		if xpcall ~= nil then
			if type(xpcall) ~= "function" then
				abort()
			end
			local xok, xv = xpcall(function()
				error(71)
			end, function()
				return 73
			end)
			if xok or xv ~= 73 then
				abort()
			end
		end
	end
"###;

pub const ANTI_ENV: &str = r###"	local function stage_env()
		local gfe = pick("getfenv", nil)
		if gfe ~= nil then
			if type(gfe) ~= "function" then
				failed = true
			else
				local ok, value = pcall(function() return gfe(1) end)
				if ok and type(value) == "table" then
					env = value
				end
			end
		end

		if env == nil and type(_G) == "table" then
			env = _G
		end

		if
			(env ~= nil and type(env) ~= "table")
			or (print ~= nil and type(print) ~= "function")
			or (warn ~= nil and type(warn) ~= "function")
			or (tostring ~= nil and type(tostring) ~= "function")
			or (tonumber ~= nil and type(tonumber) ~= "function")
		then
			failed = true
		end

		local slot = {}
		local marker = function() end
		local writeOk = pcall(rawset, slot, -271823, marker)

		if not writeOk or rawget(slot, -271823) ~= marker then
			failed = true
		end

		rawset(slot, -271823, nil)

		if rawget(slot, -271823) ~= nil or pcall(error) then
			failed = true
		end
	end
"###;

pub const ANTI_DEBUG: &str = r###"	local function stage_debug()
		if dbgTable ~= nil and type(dbgTable) ~= "table" then
			failed = true
			return
		end
		if debugInfo ~= nil and type(debugInfo) ~= "function" then
			failed = true
			return
		end

		local loaderLS = pick("loadstring", nil)
		local loaderL = pick("load", nil)
		if (loaderLS ~= nil and type(loaderLS) ~= "function") or (loaderL ~= nil and type(loaderL) ~= "function") then
			failed = true
			return
		end

		local infoFn = debugInfo
		if type(infoFn) ~= "function" then
			local gfe = pick("getfenv", nil)
			if type(gfe) == "function" then
				local ok0, g0 = pcall(gfe, 0)
				if ok0 and type(g0) == "table" then
					local gd = g0["debug"]
					if gd ~= nil and type(gd) ~= "table" then
						failed = true
						return
					end
					if type(gd) == "table" then
						local gi = gd["info"]
						if gi ~= nil and type(gi) ~= "function" then
							failed = true
							return
						end
						infoFn = gi
					end
				end
			end
		end

		if type(infoFn) ~= "function" then
			return
		end

		local function sourceOf(f)
			local ok, src = pcall(infoFn, f, "s")
			if ok and type(src) == "string" then
				return src
			end
			return nil
		end

		local probe = function() return true end
		local probeSource = sourceOf(probe)
		if probeSource == "[C]" then
			failed = true
			return
		end

		local _cf = { type, pcall, error, rawget, rawset, getmetatable, setmetatable }
		local anyC = false
		local allC = true
		for _di = 1, 7 do
			if sourceOf(_cf[_di]) == "[C]" then
				anyC = true
			else
				allC = false
			end
		end
		if not anyC then
			return
		end
		if not allC then
			failed = true
			return
		end

		if type(loaderLS) == "function" then
			local loaderSource = sourceOf(loaderLS)
			if loaderSource ~= nil and loaderSource ~= "[C]" then
				failed = true
			end
		end
		if type(loaderL) == "function" then
			local loadSource = sourceOf(loaderL)
			if loadSource ~= nil and loadSource ~= "[C]" then
				failed = true
			end
		end
	end
"###;

pub const ANTI_CANARY: &str = r###"	local function stage_canaries()
		if newproxy ~= nil and type(newproxy) ~= "function" then
			failed = true
		elseif type(newproxy) == "function" then
			local proxyOk, proxy = pcall(newproxy, true)

			if not proxyOk or proxy == nil then
				failed = true
			else
				local metatableOk, metatable = pcall(getmetatable, proxy)

				if not metatableOk or type(metatable) ~= "table" then
					failed = true
				else
					metatable.__tostring = tripwire
					metatable.__concat = tripwire
					metatable.__call = tripwire
					canaries[#canaries + 1] = proxy
				end
			end
		end

		local function addCanary(lock)
			local value = {}
			local metatable = {
				__metatable = lock,
				__tostring = tripwire,
				__concat = tripwire,
				__iter = tripwire,
			}
			local ok, result = pcall(setmetatable, value, metatable)

			if not ok or result ~= value then
				failed = true
				return
			end

			local readOk, visibleMetatable = pcall(getmetatable, value)

			if not readOk or visibleMetatable ~= lock then
				failed = true
			end

			canaries[#canaries + 1] = value
		end

		addCanary("tddxhpfcpi")
		addCanary("lsphhfstdm")
		addCanary("vymgglkhqr")
	end
"###;

pub const ANTI_MISC: &str = r###"	local function stage_misc()
		if selectFn ~= nil and type(selectFn) ~= "function" then
			failed = true
		end
		if tblTable ~= nil and type(tblTable) ~= "table" then
			failed = true
		end

		local function checkUnpack(fn)
			if type(fn) ~= "function" then
				return false
			end
			local ok0 = pcall(fn, {}, 1, 0)
			local ok1, a1, a2, a3, a4 = pcall(fn, { 11, nil, 33, nil }, 1, 4)
			local ok2, b1, b2, b3 = pcall(fn, { 10, 20, 30, 40 }, 2, 3)
			local ok3, e1 = pcall(fn, { 10, 20 }, 3, 2)
			if not ok0 or not ok1 or a1 ~= 11 or a2 ~= nil or a3 ~= 33 or a4 ~= nil then
				return false
			end
			if not ok2 or b1 ~= 20 or b2 ~= 30 or b3 ~= nil then
				return false
			end
			if not ok3 or e1 ~= nil then
				return false
			end
			if type(selectFn) == "function" then
				local okc1, c1 = pcall(function()
					return selectFn("#", fn({ 11, nil, 33, nil }, 1, 4))
				end)
				local okc2, c2 = pcall(function()
					return selectFn("#", fn({ 10, 20, 30, 40 }, 2, 3))
				end)
				local okc3, c3 = pcall(function()
					return selectFn("#", fn({ 10, 20 }, 3, 2))
				end)
				if not okc1 or c1 ~= 4 or not okc2 or c2 ~= 2 or not okc3 or c3 ~= 0 then
					return false
				end
			end
			return true
		end

		if unpackFn ~= nil and not checkUnpack(unpackFn) then
			failed = true
		end
		if tableUnpackFn ~= nil and not checkUnpack(tableUnpackFn) then
			failed = true
		end

		if nextFn ~= nil then
			if type(nextFn) ~= "function" then
				failed = true
			else
				local _nt = { [1] = 7 }
				local ok1, nk1, nv1 = pcall(nextFn, _nt, nil)
				local ok2, nk2 = pcall(nextFn, {}, nil)
				if not ok1 or nk1 ~= 1 or nv1 ~= 7 or not ok2 or nk2 ~= nil then
					failed = true
				end
			end
		end

		if osTable ~= nil then
			if type(osTable) ~= "table" then
				failed = true
			else
				local okc, clk = pcall(function() return osTable["clock"] end)
				if not okc or (clk ~= nil and type(clk) ~= "function") then
					failed = true
				end
			end
		end

		local _cn = { "type", "pcall", "error", "rawget", "rawset", "getmetatable", "setmetatable" }
		local _cv = { type, pcall, error, rawget, rawset, getmetatable, setmetatable }
		local chkEnv = EV
		if type(chkEnv) ~= "table" then chkEnv = env end
		if type(chkEnv) == "table" then
			for _ci = 1, 7 do
				local okv, evv = pcall(function() return chkEnv[_cn[_ci]] end)
				if not okv or (evv ~= nil and evv ~= _cv[_ci]) then
					failed = true
				end
			end
		end
	end
"###;

/// Boot half of the Phase 18 P1-3 string-cache canary policy.
///
/// The distributed half lives in the VM (`Op::Canary` at four shuffled split
/// points of every compiled instruction stream), so this stage only pins the
/// primitives that half depends on before the payload can run. No `--` comments
/// inside the constant: the guard source is minified onto a single line.
pub const ANTI_STRCANARY: &str = r###"	local function stage_strcanaries()
		local TF, TT = "function", "table"
		local st = pick("string", nil)
		if st ~= nil then
			local ch, by = st["char"], st["byte"]
			if type(st) ~= TT or (ch ~= nil and type(ch) ~= TF) or (by ~= nil and type(by) ~= TF) then
				failed = true
			end
		end
		local mt = getmetatable(tostring(nil))
		local real = nil
		if type(mt) == TT then
			real = mt.__index
		end
		local rc, rb = nil, nil
		if type(real) == TT then
			rc, rb = real["char"], real["byte"]
		end
		if type(rc) ~= TF or (rb ~= nil and type(rb) ~= TF) then
			failed = true
			return
		end
		local ok, v = pcall(rc, 65)
		local ok2, v2 = pcall(rc, 65, 66)
		local ok3, v3 = pcall(rc, 66)
		local ok4, v4 = pcall(rc, 97, 98, 99)
		if not ok or v ~= "A" or #v ~= 1 or not ok2 or not ok3 or v2 ~= v .. v3 or v == v3 or not ok4 or #v4 ~= 3 then
			failed = true
		end
		if rb ~= nil then
			local okb, vb = pcall(rb, "Z")
			if not okb or vb ~= 90 then
				failed = true
			end
		end
	end
"###;

pub const STAGES: [(&str, &str, &str); 6] = [
	("core", ANTI_CORE, "stage_core()"),
	("env", ANTI_ENV, "stage_env()"),
	("debug", ANTI_DEBUG, "stage_debug()"),
	("canary", ANTI_CANARY, "stage_canaries()"),
	("misc", ANTI_MISC, "stage_misc()"),
	("strcanary", ANTI_STRCANARY, "stage_strcanaries()"),
];

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::BTreeSet;

	#[test]
	fn stage_manifest_matches_stages_and_enforces_complete_metadata() {
		assert_eq!(STAGE_MANIFEST.len(), STAGES.len());
		let mut seen_stage_ids = BTreeSet::new();
		let mut seen_test_ids = BTreeSet::new();
		for (meta, (id, src, call)) in STAGE_MANIFEST.iter().zip(STAGES.iter()) {
			assert_eq!(meta.id, *id);
			assert!(
				seen_stage_ids.insert(meta.id),
				"duplicate stage id {} in STAGE_MANIFEST",
				meta.id
			);
			assert!(!meta.target.is_empty(), "empty target in {}", meta.id);
			assert!(!meta.threat_model.is_empty(), "empty threat_model in {}", meta.id);
			assert!(!meta.allowed_missing.is_empty(), "empty allowed_missing in {}", meta.id);
			assert!(!meta.tamper_triggers.is_empty(), "empty tamper_triggers in {}", meta.id);
			assert!(!meta.fail_action.is_empty(), "empty fail_action in {}", meta.id);
			assert!(!meta.trigger_timing.is_empty(), "empty trigger_timing in {}", meta.id);
			assert!(!meta.resource_bound.is_empty(), "empty resource_bound in {}", meta.id);
			assert!(
				!meta.positive_test_ids.is_empty(),
				"stage {} must declare at least one positive test_id",
				meta.id
			);
			assert!(
				!meta.negative_test_ids.is_empty(),
				"stage {} must declare at least one negative test_id",
				meta.id
			);
			assert!(src.contains(call.trim_end_matches("()")));
			for req in meta.required_apis {
				assert!(
					!meta.optional_apis.contains(req),
					"stage {} lists {} as both required and optional",
					meta.id,
					req
				);
			}
			for tid in meta.all_test_ids() {
				assert!(
					seen_test_ids.insert(tid),
					"duplicate test_id {} across STAGE_MANIFEST",
					tid
				);
			}
		}
	}

	#[test]
	fn stage_manifest_test_ids_match_guard_matrix_test_py_bidirectionally() {
		let py_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("tests")
			.join("guard_matrix_test.py");
		let py_src = std::fs::read_to_string(&py_path)
			.unwrap_or_else(|e| panic!("failed to read {}: {}", py_path.display(), e));

		let pos_ids: BTreeSet<&str> = STAGE_MANIFEST
			.iter()
			.flat_map(|s| s.positive_test_ids.iter().copied())
			.collect();
		let neg_ids: BTreeSet<&str> = STAGE_MANIFEST
			.iter()
			.flat_map(|s| s.negative_test_ids.iter().copied())
			.collect();
		let manifest_ids: BTreeSet<&str> = pos_ids.union(&neg_ids).copied().collect();

		let mut py_all = BTreeSet::new();
		let mut py_pos = BTreeSet::new();
		let mut py_neg = BTreeSet::new();
		let mut in_cases = false;
		let mut cur_tuple: Vec<String> = Vec::new();
		let mut depth: i32 = 0;

		for line in py_src.lines() {
			let trimmed = line.trim();
			if trimmed == "cases = [" {
				in_cases = true;
				continue;
			}
			if in_cases && line.starts_with("        ]") {
				break;
			}
			if !in_cases || trimmed.starts_with('#') || trimmed.is_empty() {
				continue;
			}
			for ch in trimmed.chars() {
				if ch == '(' {
					depth += 1;
				} else if ch == ')' {
					depth -= 1;
				}
			}
			cur_tuple.push(trimmed.to_string());
			if depth == 0 && !cur_tuple.is_empty() {
				let joined = cur_tuple.join(" ");
				cur_tuple.clear();
				// Extract first string literal (`name`) and `expect_class` (`"OK"` vs `"GuardRejected"`).
				let mut strs = Vec::new();
				let bytes = joined.as_bytes();
				let mut i = 0;
				let mut paren_d = 0i32;
				while i < bytes.len() {
					let b = bytes[i];
					if b == b'(' {
						paren_d += 1;
						i += 1;
					} else if b == b')' {
						paren_d -= 1;
						i += 1;
					} else if paren_d == 1 && b == b'"' {
						let start = i + 1;
						let mut j = start;
						while j < bytes.len() {
							if bytes[j] == b'\\' {
								j += 2;
								continue;
							}
							if bytes[j] == b'"' {
								break;
							}
							j += 1;
						}
						if j < bytes.len() {
							strs.push(joined[start..j].to_string());
							i = j + 1;
						} else {
							break;
						}
					} else {
						i += 1;
					}
				}
				assert!(
					strs.len() >= 3,
					"failed to parse top-level case tuple in guard_matrix_test.py: {}",
					joined
				);
				let cid = strs[0].clone();
				let expect_cls = strs[2].as_str();
				assert!(
					py_all.insert(cid.clone()),
					"duplicate case id {} in guard_matrix_test.py",
					cid
				);
				match expect_cls {
					"OK" => {
						py_pos.insert(cid);
					}
					"GuardRejected" => {
						py_neg.insert(cid);
					}
					other => panic!(
						"unexpected top-level expect_class {:?} for case {} in guard_matrix_test.py",
						other, cid
					),
				}
			}
		}
		assert!(
			!py_all.is_empty(),
			"parsed 0 cases from guard_matrix_test.py — parser broken"
		);

		let py_ref: BTreeSet<&str> = py_all.iter().map(String::as_str).collect();
		let missing_in_py: Vec<&&str> = manifest_ids.difference(&py_ref).collect();
		let missing_in_manifest: Vec<&&str> = py_ref.difference(&manifest_ids).collect();
		assert!(
			missing_in_py.is_empty() && missing_in_manifest.is_empty(),
			"STAGE_MANIFEST <-> guard_matrix_test.py drift!\n  missing in guard_matrix_test.py: {:?}\n  missing in STAGE_MANIFEST: {:?}",
			missing_in_py,
			missing_in_manifest
		);

		let py_pos_ref: BTreeSet<&str> = py_pos.iter().map(String::as_str).collect();
		let py_neg_ref: BTreeSet<&str> = py_neg.iter().map(String::as_str).collect();
		assert_eq!(
			pos_ids, py_pos_ref,
			"positive_test_ids polarity mismatch between STAGE_MANIFEST and guard_matrix_test.py"
		);
		assert_eq!(
			neg_ids, py_neg_ref,
			"negative_test_ids polarity mismatch between STAGE_MANIFEST and guard_matrix_test.py"
		);
	}
}
