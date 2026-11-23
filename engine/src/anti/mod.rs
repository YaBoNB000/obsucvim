/// Structured classification for each anti-tamper guard stage (P4).
///
/// Every check explicitly distinguishes:
/// - **Required foundational Luau/Lua 5.1 semantics** (`type`, `pcall`, `error`,
///   `rawget`, `rawset`, `getmetatable`, `setmetatable`, table metatable lock):
///   absence or behavioral corruption is treated as active tamper (`abort()`).
/// - **Optional host/sandbox capabilities** (`xpcall`, `getfenv`, `print`, `warn`,
///   `tostring`, `tonumber`, `newproxy`, `unpack`, `debug`, `debug.info`, `os`,
///   `os.clock`, `loadstring`, `load`): `nil` (absent) is allowed without false
///   positives, whereas non-nil wrong-type values or spoofed provenance/semantics
///   trigger fail-closed termination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct GuardStageMeta {
	pub id: &'static str,
	pub target: &'static str,
	pub required_apis: &'static [&'static str],
	pub optional_apis: &'static [&'static str],
	pub luau_guaranteed: bool,
	pub fail_action: &'static str,
	pub test_ids: &'static [&'static str],
}

#[allow(dead_code)]
pub const STAGE_MANIFEST: [GuardStageMeta; 5] = [
	GuardStageMeta {
		id: "core",
		target: "Essential base primitives callable + optional xpcall type/handler roundtrip",
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
		luau_guaranteed: true,
		fail_action: "immediate abort() (fail-closed hang)",
		test_ids: &[
			"clean_stock_luau",
			"xpcall_nil_allowed",
			"pcall_tampered_nonfn",
			"xpcall_tampered_nonfn",
			"xpcall_stub_broken",
		],
	},
	GuardStageMeta {
		id: "env",
		target: "Environment table resolution, rawset/rawget negative-key roundtrip, pcall(error) failure semantics, optional I/O & conversion global type sanity",
		required_apis: &["rawget", "rawset", "pcall", "error"],
		optional_apis: &["getfenv", "_G", "print", "warn", "tostring", "tonumber"],
		luau_guaranteed: true,
		fail_action: "failed = true -> abort()",
		test_ids: &[
			"readonly_globals",
			"getfenv_nil_allowed",
			"print_warn_nil_allowed",
			"print_redirect_allowed",
			"warn_tampered_nonfn",
			"rawget_tampered",
			"pcall_error_swallowed",
		],
	},
	GuardStageMeta {
		id: "debug",
		target: "Optional debug.info closure-vs-[C] provenance & native loadstring/load verification",
		required_apis: &[],
		optional_apis: &["debug", "debug.info", "loadstring", "load"],
		luau_guaranteed: false,
		fail_action: "failed = true -> abort()",
		test_ids: &[
			"debug_nil_allowed",
			"debug_info_nil_allowed",
			"debug_non_table_tamper",
			"debug_info_nonfn_tamper",
			"debug_info_spoof_c",
			"loadstring_lua_hook",
			"loadstring_hook_with_env_debug_nil",
		],
	},
	GuardStageMeta {
		id: "canary",
		target: "Locked metatable honeypot canaries + optional newproxy(true) userdata canary",
		required_apis: &["setmetatable", "getmetatable"],
		optional_apis: &["newproxy"],
		luau_guaranteed: true,
		fail_action: "failed = true -> abort() + runtime tripwire() on probe",
		test_ids: &[
			"newproxy_nil_allowed",
			"newproxy_tampered_nonfn",
			"metatable_lock_tampered",
		],
	},
	GuardStageMeta {
		id: "misc",
		target: "Optional unpack/os/os.clock sanity + explicit 1..7 core-primitive shadow consistency",
		required_apis: &[],
		optional_apis: &["unpack", "table.unpack", "os", "os.clock"],
		luau_guaranteed: true,
		fail_action: "failed = true -> abort()",
		test_ids: &[
			"os_nil_allowed",
			"os_clock_nil_allowed",
			"os_non_table_tamper",
			"os_clock_nonfn_tamper",
			"unpack_nil_allowed",
			"unpack_broken_tamper",
			"shadow_pcall_false_tamper",
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
		local gfe = pick("getfenv", getfenv)
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

		local infoFn = debugInfo
		if type(infoFn) ~= "function" then
			local gfe = pick("getfenv", getfenv)
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
		end

		if sourceOf(pcall) ~= "[C]" then
			return
		end
		if sourceOf(type) ~= "[C]" or sourceOf(error) ~= "[C]" or sourceOf(rawget) ~= "[C]" or sourceOf(rawset) ~= "[C]" then
			failed = true
			return
		end

		local loaderLS = pick("loadstring", nil)
		local loaderL = pick("load", nil)
		if loaderLS ~= nil and type(loaderLS) ~= "function" then
			failed = true
		elseif type(loaderLS) == "function" then
			local loaderSource = sourceOf(loaderLS)
			if loaderSource ~= nil and loaderSource ~= "[C]" then
				failed = true
			end
		end
		if loaderL ~= nil and type(loaderL) ~= "function" then
			failed = true
		elseif type(loaderL) == "function" then
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
		if unpack ~= nil and type(unpack) ~= "function" then
			failed = true
		elseif type(unpack) == "function" then
			local ok = pcall(unpack, {}, 0, 64)
			if not ok then
				failed = true
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

pub const STAGES: [(&str, &str, &str); 5] = [
	("core", ANTI_CORE, "stage_core()"),
	("env", ANTI_ENV, "stage_env()"),
	("debug", ANTI_DEBUG, "stage_debug()"),
	("canary", ANTI_CANARY, "stage_canaries()"),
	("misc", ANTI_MISC, "stage_misc()"),
];

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn stage_manifest_matches_stages_and_separates_optional_apis() {
		assert_eq!(STAGE_MANIFEST.len(), STAGES.len());
		for (meta, (id, src, call)) in STAGE_MANIFEST.iter().zip(STAGES.iter()) {
			assert_eq!(meta.id, *id);
			assert!(!meta.target.is_empty());
			assert!(!meta.fail_action.is_empty());
			assert!(!meta.test_ids.is_empty());
			assert!(src.contains(call.trim_end_matches("()")));
			for req in meta.required_apis {
				assert!(
					!meta.optional_apis.contains(req),
					"stage {} lists {} as both required and optional",
					meta.id,
					req
				);
			}
		}
	}
}
