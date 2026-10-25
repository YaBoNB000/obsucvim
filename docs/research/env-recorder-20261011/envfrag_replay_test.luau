-- ENVFRAG isolation: record phase A, replay phase B, compare EVD + report
-- the first divergence with args. Top-level errors are visible on exit.
local REAL_PRINT = print
local REAL_PCALL = pcall
local REAL_ERROR = error
local REAL_GETFENV = getfenv

local PASSTHROUGH = {
	print = true, getfenv = true, pcall = true, xpcall = true,
	setfenv = true, coroutine = true, pairs = true, ipairs = true,
	next = true, error = true, loadstring = true,
}

local log = {}

local function same_arg(a, b)
	if a == b then
		return true
	end
	-- values equal or same object; strings/numbers/booleans/nil covered by ==
	return false
end

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

local diverged = nil

local function replay_entry(name, ...)
	local q = log[name]
	local entry = q and table.remove(q, 1)
	if entry == nil then
		REAL_ERROR("DIVERGE-EMPTY:" .. name, 0)
	end
	local actual = table.pack(...)
	if actual.n ~= entry.args.n then
		diverged = "ARGCOUNT:" .. name .. " want " .. entry.args.n .. " got " .. actual.n
	elseif diverged == nil then
		for i = 1, actual.n do
			if not same_arg(entry.args[i], actual[i]) then
				diverged = "ARGVAL:" .. name .. "#" .. i
					.. " want " .. type(entry.args[i])
					.. " got " .. type(actual[i])
				break
			end
		end
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

local SRC = @ENVFRAG@

-- Phase A: record
local loaderA = loadstring("return " .. SRC, "fragA")
setfenv(loaderA, make_env(false))
local fragA = loaderA()
local okA, dA = REAL_PCALL(fragA)
REAL_PRINT("PHASE-A", "ok=" .. tostring(okA), "d=" .. tostring(dA))

local total = 0
local names = {}
for n, q in pairs(log) do
	total = total + #q
	names[#names + 1] = n .. "=" .. #q
end
table.sort(names)
REAL_PRINT("LOG", total, table.concat(names, " "))

-- Phase B: replay a fresh copy of the factory
local loaderB = loadstring("return " .. SRC, "fragB")
setfenv(loaderB, make_env(true))
local fragB = loaderB()
local okB, dB = REAL_PCALL(fragB)
REAL_PRINT("PHASE-B", "ok=" .. tostring(okB), "d=" .. tostring(dB))
if diverged then
	REAL_PRINT("DIVERGED", diverged)
else
	REAL_PRINT("DIVERGED", "none")
end
local leftover = 0
for _, q in pairs(log) do
	leftover = leftover + #q
end
REAL_PRINT("LEFTOVER", leftover)
REAL_ERROR("DONE", 0)
