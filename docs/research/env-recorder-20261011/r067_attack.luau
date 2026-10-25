-- R067 attack harness: recorder/replay + executor-surface fakes against the
-- bound-state canary (--env-check --bind-executor, seed 67067).
--
-- Threat model (same as the R066 campaign): "real Luau runtime + attacker-
-- controlled env layer". The attacker setfenv's the artifact into a shadow
-- env they fully control. NEW for R067: the exguard reads its environment
-- via getfenv(0), which in Luau CLI is the (readonly) real _G -- so the
-- harness replaces shadow.getfenv with a wrapper that redirects ONLY
-- level 0 to a FAKE_G table the attacker controls (getgenv -> CAP, debug
-- -> introspection fakes). getfenv()/getfenv(n>0) keep real semantics
-- (level-shifted) so tier-1/tier-2/VM env lookups stay correct.
--
-- Executor surface tiers:
--   half  : E3-style stub (getconstants -> {} always, setupvalue -> true)
--   smart : probe-aware fake -- debug.info-based C/Lua classification,
--           getconstants by CALLING the probe closure (guard probes are
--           `return CONST` shaped), no-op setupvalue, empty protos.
--
-- Modes:
--   direct : single run, everything real or deterministic-fake.
--   replay : variant-D minimal recording -- only buffer.* / unpack / os.* /
--            assert / vector.* / utf8.* (the non-passthrough real globals)
--            are recorded in phase 1 and served from the log in phase 2.
--            Mock + executor fakes stay real (deterministic) in both phases.
--
-- Usage: driven by run_r067_attack.py, which fills the four placeholders
-- below (target literal, tier, mode, and the embedded Roblox mock).

local TARGET = @TARGET@
local TIER = "@TIER@"
local MODE = "@MODE@"

local RP, RPC, RE, RGF, RCLK = print, pcall, error, getfenv, os.clock

-- ---- shared instrumentation ------------------------------------------------
local XLOG = {}
local PHASE_TAG = "D" -- D=direct, R=record, P=replay
local function shape(v)
	local t = type(v)
	if t == "function" then
		if debug.info(v, "s") == "[C]" then
			return "Cfn"
		end
		return "lfn@" .. tostring(debug.info(v, "s")) .. ":" .. tostring(debug.info(v, "l"))
	end
	if t == "number" or t == "boolean" then
		return tostring(v)
	end
	if t == "string" then
		return "s:" .. v:sub(1, 10)
	end
	return t
end
local function xlog(name, ...)
	local n = select("#", ...)
	local parts = {}
	for i = 1, n do
		parts[i] = shape(select(i, ...))
	end
	XLOG[#XLOG + 1] = PHASE_TAG .. " " .. name .. "(" .. table.concat(parts, ",") .. ")"
end

-- ---- record/replay machinery (variant D, verbatim semantics) ---------------
local LOG = {}
local CALL_BUDGET = 300000
local calls_made = 0
local function record_entry(name, fv, ...)
	calls_made = calls_made + 1
	if calls_made > CALL_BUDGET then
		RP("BUDGET\texceeded at call " .. calls_made .. " current=" .. name)
		for n, q in pairs(LOG) do
			RP("LOG\t" .. n .. "\t" .. #q)
		end
		RE("budget-exceeded", 0)
	end
	local r = table.pack(RPC(fv, ...))
	local q = LOG[name]
	if not q then
		q = {}
		LOG[name] = q
	end
	if r[1] then
		local out = { n = r.n - 1 }
		for i = 2, r.n do
			out[i - 1] = r[i]
		end
		q[#q + 1] = { raised = false, results = out }
		return table.unpack(out, 1, out.n)
	else
		q[#q + 1] = { raised = true, val = r[2] }
		RE(r[2], 0)
	end
end
local function replay_entry(name)
	local q = LOG[name]
	local entry = q and table.remove(q, 1)
	if entry == nil then
		RP("DIVERGE-EMPTY\t" .. name)
		RE("replay-diverge:" .. name, 0)
	end
	if entry.raised then
		RE(entry.val, 0)
	end
	return table.unpack(entry.results, 1, entry.results.n)
end

-- ---- env builder -----------------------------------------------------------
local function BUILD(phase)
	--@MOCK@
	-- (mock chunk above defined ENV / TAGS / game / RunService as locals and
	-- populated ENV with the faithful Roblox surface; BREAK is "none")

	-- -- extensions the exguard host-shape gate needs ----------------------
	local Players = { ClassName = "Players", Name = "Players" }
	TAGS[Players] = "Instance"
	do
		local origGS = game.GetService
		game.GetService = function(self, name)
			if name == "Players" then
				return Players
			end
			return origGS(self, name)
		end
	end
	ENV.Vector3 = {
		new = function(x, y, z)
			local v = { X = x, Y = y, Z = z }
			TAGS[v] = "Vector3"
			return v
		end,
	}

	-- -- executor surface (CAP = what getgenv() returns) --------------------
	local CAP = {}
	CAP.getgenv = function()
		xlog("getgenv")
		return CAP
	end
	CAP.identifyexecutor = function()
		xlog("identifyexecutor")
		return "SmartShim", "1.0"
	end
	CAP.checkcaller = function()
		xlog("checkcaller")
		return true
	end
	local WRAPPED = setmetatable({}, { __mode = "k" })
	local function iscf(f)
		if type(f) ~= "function" then
			return false
		end
		if WRAPPED[f] then
			return true
		end
		return debug.info(f, "s") == "[C]"
	end
	if TIER == "half" then
		CAP.iscclosure = function(f)
			xlog("iscclosure", f)
			return debug.info(f, "s") == "[C]"
		end
		CAP.islclosure = function(f)
			xlog("islclosure", f)
			return debug.info(f, "s") ~= "[C]"
		end
		CAP.newcclosure = function(f)
			xlog("newcclosure", f)
			return f
		end
		CAP.getconstants = function(f)
			xlog("getconstants", f)
			return {}
		end
		CAP.getupvalues = function(f)
			xlog("getupvalues", f)
			return {}
		end
		CAP.setupvalue = function(f)
			xlog("setupvalue", f)
			return true
		end
		CAP.getproto = function(f)
			xlog("getproto", f)
			return {}
		end
		CAP.getprotos = function(f)
			xlog("getprotos", f)
			return {}
		end
	else
		CAP.iscclosure = function(f)
			xlog("iscclosure", f)
			return iscf(f)
		end
		CAP.islclosure = function(f)
			xlog("islclosure", f)
			return not iscf(f)
		end
		CAP.newcclosure = function(f)
			xlog("newcclosure", f)
			local w = function(...)
				return f(...)
			end
			WRAPPED[w] = true
			return w
		end
		CAP.getconstants = function(f)
			xlog("getconstants", f)
			if iscf(f) then
				error("cannot get constants of a C closure", 2)
			end
			-- guard probes are `return CONST` shaped: calling reveals the
			-- constant the check expects to find in the table
			local ok, a, b, c = RPC(f)
			if not ok then
				return {}
			end
			return { a, b, c }
		end
		CAP.getupvalues = function(f)
			xlog("getupvalues", f)
			if iscf(f) then
				error("cannot get upvalues of a C closure", 2)
			end
			return {}
		end
		CAP.setupvalue = function(f)
			xlog("setupvalue", f)
			if iscf(f) then
				error("cannot setupvalue a C closure", 2)
			end
			return true
		end
		CAP.getproto = function(f)
			xlog("getproto", f)
			if iscf(f) then
				error("cannot get proto of a C closure", 2)
			end
			return {}
		end
		CAP.getprotos = function(f)
			xlog("getprotos", f)
			if iscf(f) then
				error("cannot get protos of a C closure", 2)
			end
			return {}
		end
	end
	CAP.loadstring = loadstring
	CAP.typeof = ENV.typeof
	CAP.game = ENV.game
	CAP.Instance = ENV.Instance
	CAP.Vector3 = ENV.Vector3
	CAP.task = ENV.task
	CAP.Enum = ENV.Enum
	CAP.Random = ENV.Random
	CAP.clonefunction = ENV.clonefunction

	-- -- FAKE_G: what getfenv(0) returns inside the artifact ----------------
	local REALG = RGF(0)
	local FAKE_G = {}
	for k, v in pairs(REALG) do
		FAKE_G[k] = v
	end
	FAKE_G._G = FAKE_G
	local DBG = {}
	for k, v in pairs(REALG.debug) do
		DBG[k] = v
	end
	FAKE_G.debug = DBG
	FAKE_G.getgenv = CAP.getgenv
	-- exguard reads introspection from the debug TABLE first
	DBG.getconstants = CAP.getconstants
	DBG.getupvalues = CAP.getupvalues
	DBG.getproto = CAP.getproto
	DBG.getprotos = CAP.getprotos
	DBG.setupvalue = CAP.setupvalue

	-- -- shadow env: getgenv + level-0 getfenv redirect ----------------------
	ENV.getgenv = CAP.getgenv
	do
		local realgetfenv = ENV.getfenv
		ENV.getfenv = function(l)
			if l == 0 then
				xlog("getfenv(0)->FAKE_G")
				return FAKE_G
			end
			if l == nil then
				return realgetfenv(2)
			end
			if type(l) == "number" then
				return realgetfenv(l + 1)
			end
			return realgetfenv(l)
		end
	end

	-- -- variant-D recording on the non-passthrough real globals ------------
	if phase then
		local PASSTH = {
			math = true, bit32 = true, string = true, table = true,
			setmetatable = true, getmetatable = true, rawset = true,
			rawget = true, rawequal = true, rawlen = true, newproxy = true,
			debug = true, tonumber = true, tostring = true, type = true,
			select = true, print = true, pcall = true, xpcall = true,
			setfenv = true, coroutine = true, pairs = true, ipairs = true,
			next = true, error = true, loadstring = true,
			-- attacker-owned deterministic surface (NOT recorded):
			getfenv = true, getgenv = true,
			game = true, Enum = true, workspace = true, Random = true,
			task = true, Instance = true, Vector3 = true, clonefunction = true,
			typeof = true, _G = true, _VERSION = true,
		}
		for k, v in pairs(ENV) do
			if not PASSTH[k] then
				if type(v) == "function" then
					ENV[k] = phase == "record"
						and function(...) return record_entry(k, v, ...) end
						or function(...) return replay_entry(k) end
				elseif type(v) == "table" then
					local proxy = {}
					for fk, fv in pairs(v) do
						local name = k .. "." .. fk
						if type(fv) == "function" then
							proxy[fk] = phase == "record"
								and function(...) return record_entry(name, fv, ...) end
								or function(...) return replay_entry(name) end
						else
							proxy[fk] = fv
						end
					end
					ENV[k] = proxy
				end
			end
		end
	end
	return ENV
end

-- ---- runner -----------------------------------------------------------------
local function dump_xlog()
	RP("XLOG\tcount=" .. #XLOG)
	for i = 1, #XLOG do
		RP("X\t" .. XLOG[i])
	end
end

if MODE == "direct" then
	local ENV = BUILD(nil)
	local art = assert(loadstring(TARGET, "attack-target"))
	setfenv(art, ENV)
	local t0 = RCLK()
	local ok, err = RPC(art)
	RP("RUN\trc=" .. tostring(ok) .. "\t" .. tostring(err):sub(1, 220) .. "\tsec=" .. (RCLK() - t0))
	dump_xlog()
else
	PHASE_TAG = "R"
	local env1 = BUILD("record")
	local art1 = assert(loadstring(TARGET, "attack-target"))
	setfenv(art1, env1)
	local t1 = RCLK()
	local ok1, err1 = RPC(art1)
	RP("PHASE\trecord\trc=" .. tostring(ok1) .. "\t" .. tostring(err1):sub(1, 160) .. "\tsec=" .. (RCLK() - t1))
	local total, names = 0, {}
	for name, q in pairs(LOG) do
		total = total + #q
		names[#names + 1] = name
	end
	table.sort(names)
	RP("LOG\tentries=" .. total)
	for i = 1, #names do
		RP("LOG\t" .. names[i] .. "\t" .. #LOG[names[i]])
	end
	dump_xlog()
	for i = 1, #XLOG do
		XLOG[i] = nil
	end

	PHASE_TAG = "P"
	local env2 = BUILD("replay")
	local art2 = assert(loadstring(TARGET, "attack-target"))
	setfenv(art2, env2)
	local t2 = RCLK()
	local ok2, err2 = RPC(art2)
	RP("PHASE\treplay\trc=" .. tostring(ok2) .. "\t" .. tostring(err2):sub(1, 220) .. "\tsec=" .. (RCLK() - t2))
	local leftover = 0
	for _, q in pairs(LOG) do
		leftover = leftover + #q
	end
	RP("LOG\tleftover=" .. leftover)
	dump_xlog()
	RP("VERDICT\tsee-payload-output-above")
end
