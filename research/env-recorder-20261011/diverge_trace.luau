-- Full-artifact replay with per-call arg verification + trace.
-- Value args (number/string/bool/nil) are compared exactly; object args
-- are compared by type only (wrapper identities legitimately differ).
-- The trace prints every call so the pipe buffer flushes continuously;
-- the first mismatch line marks the divergence point.
local REAL_PRINT = print
local REAL_PCALL = pcall
local REAL_ERROR = error
local REAL_GETFENV = getfenv
local TARGET = @TARGET@

local PASSTHROUGH = {
	print = true, getfenv = true, pcall = true, xpcall = true,
	setfenv = true, coroutine = true, pairs = true, ipairs = true,
	next = true, error = true, loadstring = true,
}

local log = {}

local function record_entry(name, fv, ...)
	local args = table.pack(...)
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
		q[#q + 1] = { raised = false, results = out, args = args }
		return table.unpack(out, 1, out.n)
	else
		q[#q + 1] = { raised = true, val = r[2], args = args }
		REAL_ERROR(r[2], 0)
	end
end

local cursors = {}
local RP = 0
local FIRST_DIV = nil

local function arg_matches(want, got)
	local tw = type(want)
	if tw ~= type(got) then
		return false
	end
	if tw == "number" or tw == "string" or tw == "boolean" or want == nil then
		return want == got
	end
	-- objects: type equality is all we can check across phases
	return true
end

local function replay_entry(name, ...)
	RP = RP + 1
	local q = log[name]
	local i = (cursors[name] or 1)
	local entry = q and q[i]
	if entry == nil then
		REAL_PRINT("DIVERGE-EMPTY\t" .. name .. "\tat\t" .. RP)
		REAL_ERROR("replay-diverge:" .. name, 0)
	end
	cursors[name] = i + 1
	local actual = table.pack(...)
	local bad = nil
	if actual.n ~= entry.args.n then
		bad = "ARGCOUNT"
	else
		for k = 1, actual.n do
			if not arg_matches(entry.args[k], actual[k]) then
				bad = "ARGVAL#" .. k
					.. "\twant\t" .. tostring(entry.args[k])
					.. "\tgot\t" .. tostring(actual[k])
				break
			end
		end
	end
	if bad and FIRST_DIV == nil then
		FIRST_DIV = RP
		REAL_PRINT("FIRST-DIVERGENCE\tcall\t" .. RP .. "\t" .. name .. "\t" .. bad)
	end
	if RP % 50 == 0 then
		REAL_PRINT("RP\t" .. RP .. "\t" .. name)
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
				and function(...) return replay_entry(k, ...) end
				or function(...) return record_entry(k, v, ...) end
		elseif type(v) == "table" then
			local proxy = {}
			for fk, fv in pairs(v) do
				local name = k .. "." .. fk
				if type(fv) == "function" then
					proxy[fk] = replay
						and function(...) return replay_entry(name, ...) end
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

-- Phase 1
local art1 = loadstring(TARGET, "t")
setfenv(art1, make_env(false))
local ok1, err1 = REAL_PCALL(art1)
REAL_PRINT("PHASE1\tok=" .. tostring(ok1) .. "\t" .. tostring(err1):sub(1, 60))

-- Phase 2
local art2 = loadstring(TARGET, "t")
setfenv(art2, make_env(true))
local ok2, err2 = REAL_PCALL(art2)
REAL_PRINT("PHASE2\tok=" .. tostring(ok2) .. "\t" .. tostring(err2):sub(1, 60))
REAL_PRINT("SUMMARY\treplay-calls=" .. RP .. "\tfirst-div=" .. tostring(FIRST_DIV))
REAL_ERROR("DONE", 0)
