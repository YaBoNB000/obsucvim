-- Environment recorder + replayer (research attack harness).
-- Phase 1 runs the artifact under a RECORDING environment (every env
-- function call logged with exact multi-returns / raised errors, in call
-- order, per function). Phase 2 loads a FRESH copy of the artifact and
-- runs it with answers served ONLY from the log (plus a minimal
-- passthrough set). Success = the payload's observable behavior
-- (print) still happens in phase 2 with zero real library semantics
-- beyond the log.
--
-- This models the "real Luau runtime + recorded env" architecture of the
-- public deobfuscators (their envlog.luau stage) against OUR artifact.
--
-- Passthrough set (NOT recorded; real in both phases):
--   print      - we WANT to observe the payload
--   getfenv    - GFE(0) must reach the real _G (platform fact: setfenv
--                shadows are invisible to getfenv(0))
--   pcall/xpcall - must really execute their inner closures; the inner
--                calls still go through the (recording/replaying)
--                wrappers, so pcall results stay consistent
--   setfenv    - harness/VM environment management
--   coroutine  - real threads run artifact closures deterministically
--   pairs/ipairs/next - iterator-returning: replaying a recorded iterator
--                would iterate the RECORDING run's tables (per-run
--                handlers) and corrupt the phase-2 boot; real iteration
--                over the phase-2 run's own tables is deterministic
-- Everything else (bit32.*, buffer.*, string.*, table.*, math.*,
-- select, loadstring, error, type, ...) is wrapped and logged.

local TARGET = @TARGET@

local REAL_PRINT = print
local REAL_PCALL = pcall
local REAL_ERROR = error
local REAL_GETFENV = getfenv

local PASSTHROUGH = {
	print = true,
	getfenv = true,
	pcall = true,
	xpcall = true,
	setfenv = true,
	coroutine = true,
	pairs = true,
	ipairs = true,
	next = true,
	-- error/loadstring MUST stay real: the D9 anti-hook gate identity-checks
	-- them (a Lua-closure replacement is detected and silently poisoned --
	-- empirically confirmed by the bisect: wrapping either one hangs the
	-- artifact in the PD death spiral). A real tool keeps them real too.
	error = true,
	loadstring = true,
}

local log = {}
local CALL_BUDGET = tonumber("@BUDGET@") or 100000
local calls_made = 0

local function record_entry(name, fv, ...)
	calls_made = calls_made + 1
	if calls_made > CALL_BUDGET then
		REAL_PRINT("BUDGET	exceeded at call " .. calls_made .. " current=" .. name)
		for n, q in pairs(log) do
			REAL_PRINT("LOG	" .. n .. "\t" .. #q)
		end
		REAL_ERROR("budget-exceeded", 0)
	end
	local r = table.pack(REAL_PCALL(fv, ...))
	local q = log[name]
	if not q then
		q = {}
		log[name] = q
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
		REAL_ERROR(r[2], 0)
	end
end

local function replay_entry(name)
	local q = log[name]
	local entry = q and table.remove(q, 1)
	if entry == nil then
		REAL_PRINT("DIVERGE-EMPTY\t" .. name)
		REAL_ERROR("replay-diverge:" .. name, 0)
	end
	if entry.raised then
		REAL_ERROR(entry.val, 0)
	end
	return table.unpack(entry.results, 1, entry.results.n)
end

local function make_env(replay)
	local real = REAL_GETFENV(0)
	local env = {}
	for k, v in pairs(real) do
		if PASSTHROUGH[k] then
			env[k] = v
		elseif type(v) == "function" then
			env[k] = replay
				and function(...) return replay_entry(k) end
				or function(...) return record_entry(k, v, ...) end
		elseif type(v) == "table" then
			local proxy = {}
			for fk, fv in pairs(v) do
				local name = k .. "." .. fk
				if type(fv) == "function" then
					proxy[fk] = replay
						and function(...) return replay_entry(name) end
						or function(...) return record_entry(name, fv, ...) end
				else
					proxy[fk] = fv
				end
			end
			env[k] = proxy
		else
			env[k] = v
		end
	end
	env._G = env
	return env
end

-- ---- Phase 1: RECORD ------------------------------------------------------
REAL_PRINT("PHASE\trecord\tstart")
local art1 = loadstring(TARGET, "attack-target")
assert(art1, "artifact must load")
local env1 = make_env(false)
setfenv(art1, env1)
local t1 = os.clock()
local ok1, err1 = REAL_PCALL(art1)
REAL_PRINT("PHASE\trecord\trc=" .. tostring(ok1) .. "\t" .. tostring(err1):sub(1, 100) .. "\tseconds=" .. (os.clock() - t1))

local total = 0
local names = {}
for name, q in pairs(log) do
	total = total + #q
	names[#names + 1] = name
end
table.sort(names)
REAL_PRINT("LOG\tentries=" .. total)
for i = 1, #names do
	REAL_PRINT("LOG\t" .. names[i] .. "\t" .. #log[names[i]])
end

-- ---- Phase 2: REPLAY ------------------------------------------------------
REAL_PRINT("PHASE\treplay\tstart")
local art2 = loadstring(TARGET, "attack-target")
local env2 = make_env(true)
setfenv(art2, env2)
local t2 = os.clock()
local ok2, err2 = REAL_PCALL(art2)
REAL_PRINT("PHASE\treplay\trc=" .. tostring(ok2) .. "\t" .. tostring(err2):sub(1, 200) .. "\tseconds=" .. (os.clock() - t2))

local leftover = 0
for _, q in pairs(log) do
	leftover = leftover + #q
end
REAL_PRINT("LOG\tleftover=" .. leftover)
REAL_PRINT("VERDICT\tsee-payload-output-above")
