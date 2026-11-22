
pub const ANTI_CORE: &str = r###"	local function stage_core()
		if
			type(type) ~= "function"
			or type(pcall) ~= "function"
			or type(xpcall) ~= "function"
			or type(error) ~= "function"
			or type(rawget) ~= "function"
			or type(rawset) ~= "function"
			or type(getmetatable) ~= "function"
			or type(setmetatable) ~= "function"
		then
			abort()
		end
	end
"###;

pub const ANTI_ENV: &str = r###"	local function stage_env()
		local gfe = (EV and EV["getfenv"]) or getfenv
		if type(gfe) == "function" then
			local ok, value = pcall(gfe)

			if ok then
				env = value
			end
		end

		if env == nil then
			env = (EV and EV["_G"]) or _G
		end

		if type(env) ~= "table" or type(print) ~= "function" or (warn ~= nil and type(warn) ~= "function") then
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
		-- Prefer protected source checks over stack-line/error-message parsing.
		-- Some Luau executors implement debug.info without the same line semantics.
		if type(debugInfo) ~= "function" then
			return
		end

		local function sourceOf(f)
			local ok, src = pcall(debugInfo, f, "s")
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

		-- Only enforce loader provenance when debug.info can identify a known
		-- native function; otherwise fail open on executors with partial metadata.
		if sourceOf(pcall) ~= "[C]" then
			return
		end

		local envTable = (EV and EV["_G"]) or _G
		if type(envTable) ~= "table" then
			return
		end

		local loaderLS = rawget(envTable, "loadstring")
		local loaderL = rawget(envTable, "load")
		local loaderSource = sourceOf(loaderLS)
		local loadSource = sourceOf(loaderL)
		if type(loaderLS) == "function" and loaderSource ~= nil and loaderSource ~= "[C]" then
			failed = true
		end
		if type(loaderL) == "function" and loadSource ~= nil and loadSource ~= "[C]" then
			failed = true
		end
	end
"###;

pub const ANTI_CANARY: &str = r###"	local function stage_canaries()
		if type(newproxy) == "function" then
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
		if type(unpack) == "function" then
			local ok = pcall(unpack, {}, 0, 64)

			if not ok then
				failed = true
			end
		end

		if type(env) == "table" then
			if type(print) == "function" and env.print ~= print then
				failed = true
			end

			if type(warn) == "function" and env.warn ~= warn then
				failed = true
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
