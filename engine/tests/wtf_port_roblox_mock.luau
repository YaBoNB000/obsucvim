-- WTF-PORT tier-2 fragment mock: faithful-enough Roblox/executor surface
-- for the ENVFRAG battery, with BREAK hooks for negative testing.
-- Research test fixture; never shipped in artifacts.
local BREAK = "@BREAK@"

local REAL_PRINT = print
local BASE = getfenv(0)
local ENV = {}
for k, v in pairs(BASE) do ENV[k] = v end
ENV._G = ENV

local TAGS = setmetatable({}, { __mode = "k" })
local rtypeof = typeof
ENV.typeof = function(v)
	if type(v) == "table" then
		local t = TAGS[v]
		if t then return t end
	end
	return rtypeof(v)
end
local LOCKED = "The metatable is locked"
local function lock(o)
	setmetatable(o, { __metatable = LOCKED })
	return o
end

-- signals ---------------------------------------------------------------
local function makeSignal()
	local s = {}
	TAGS[s] = "RBXScriptSignal"
	s.Connect = function(_, _fn)
		local c = { Connected = true }
		TAGS[c] = "RBXScriptConnection"
		c.Disconnect = function(cself)
			if BREAK ~= "signal" then cself.Connected = false end
		end
		return c
	end
	return s
end

-- Instance --------------------------------------------------------------
local CHILDREN = {}
local PARENT = {} -- proxy store: "Parent" must NEVER land in the table,
-- or the second assignment would bypass __newindex (rawset makes the key
-- present and Lua only fires __newindex for ABSENT keys)
local function newFolder()
	local obj = { Name = "Folder", ClassName = "Folder" }
	TAGS[obj] = "Instance"
	CHILDREN[obj] = {}
	local mt
	mt = {
		__metatable = LOCKED,
		__index = function(t, k)
			if k == "Parent" then return PARENT[t] end
		end,
		__newindex = function(t, k, v)
			if k == "Parent" then
				if v ~= nil and v._destroyed then
					error("The Parent property of " .. tostring(t.Name) .. " is locked")
				end
				PARENT[t] = v
				if v ~= nil then table.insert(CHILDREN[v], t) end
			else
				rawset(t, k, v)
			end
		end,
	}
	obj.GetChildren = function(self) return CHILDREN[self] end
	obj.Destroy = function(self)
		if BREAK ~= "destroy-lock" then
			self._destroyed = true
		else
			self._destroyed = false
		end
		CHILDREN[self] = nil
	end
	obj.ChildRemoved = makeSignal()
	setmetatable(obj, mt)
	return obj
end
ENV.Instance = { new = function(cls)
	if cls ~= "Folder" then error("unknown class " .. tostring(cls)) end
	return newFolder()
end }

-- Enum -------------------------------------------------------------------
local R6 = lock({ Name = "R6", Value = 0 })
TAGS[R6] = "EnumItem"
local hrt = lock({})
TAGS[hrt] = "Enum"
R6.EnumType = hrt
local function freshR6()
	local f = lock({ Name = "R6", Value = 0, EnumType = hrt })
	TAGS[f] = "EnumItem"
	return f
end
hrt.R6 = R6
hrt.FromName = function(_, n)
	if n ~= "R6" then return nil end
	if BREAK == "enum-interning" then return freshR6() end
	return R6
end
hrt.FromValue = function(_, v)
	if v ~= 0 then return nil end
	return R6
end
local Enum = lock({ HumanoidRigType = hrt })
TAGS[Enum] = "Enums"

-- Random ------------------------------------------------------------------
-- Fixed-seed 51018400 replays the proven Roblox sequence; every other
-- seed uses a per-object LCG so determinism/clone-fork self-consistency
-- holds. BREAK hooks break the value or the fork.
local FIXED = { 0.9598971928030796, 889, 0.027539190730015265 }
local function mkFrom(st, fi, fixed)
	local o = {}
	TAGS[o] = "Random"
	o.NextNumber = function(_)
		if fixed then
			fi = fi + 1
			local v = FIXED[fi]
			if BREAK == "random-value" and fi == 1 then return v + 0.5 end
			return v
		end
		st = (st * 48271 + 11) % 2147483647
		return st / 2147483647
	end
	o.NextInteger = function(_, lo, hi)
		if fixed then
			fi = fi + 1
			return FIXED[fi]
		end
		st = (st * 48271 + 11) % 2147483647
		return lo + st % (hi - lo + 1)
	end
	o.Clone = function(_)
		if BREAK == "random-clone" then
			return mkFrom(4242, 0, false)
		end
		return mkFrom(st, fi, fixed)
	end
	return o
end
ENV.Random = {
	-- accept BOTH call forms: Random.new(seed) dot call passes one
	-- argument, Random:new(seed) colon call passes (self, seed)
	new = function(a, b)
		local seed = b or a
		if seed == 51018400 then
			return mkFrom(0, 0, true)
		end
		local st = (seed or 1) % 2147483647
		if st == 0 then st = 1 end
		return mkFrom(st, 0, false)
	end,
}

-- task ---------------------------------------------------------------------
ENV.task = {
	spawn = function(f)
		if BREAK == "task-spawn" then
			f()
			return nil
		end
		local t = coroutine.create(f)
		coroutine.resume(t)
		return t
	end,
	delay = function(_t, f)
		return coroutine.create(f) -- not resumed
	end,
	cancel = function(t)
		return coroutine.close(t)
	end,
	wait = function(d)
		if type(d) ~= "number" then
			error("invalid argument #1 to 'wait' (number expected, got " .. type(d) .. ")")
		end
	end,
	defer = function(f) return coroutine.create(f) end,
}

-- game ----------------------------------------------------------------------
local RunService = { ClassName = "RunService", Name = "RunService" }
TAGS[RunService] = "Instance"
RunService.IsStudio = function(self)
	if self == nil then
		if BREAK ~= "dotcall" then
			error("Expected ':' not '.' calling member function IsStudio")
		end
		return false
	end
	return false
end
RunService.IsClient = RunService.IsStudio
RunService.IsServer = RunService.IsStudio
local game = { ClassName = "DataModel", Name = "Game" }
TAGS[game] = "Instance"
local gs_calls = 0
game.GetService = function(_self, name)
	if name ~= "RunService" then return nil end
	gs_calls = gs_calls + 1
	-- identity-swap: replace the FUNCTION ITSELF after the first call
	-- (the sweep compares game.GetService == captured reference)
	if BREAK == "identity-swap" then
		game.GetService = function() return RunService end
	end
	return RunService
end
game.GetChildren = function(self)
	if self == nil then
		error("Expected ':' not '.' calling member function GetChildren")
	end
	return {}
end

if BREAK ~= "no-game" then
	ENV.game = game
end
ENV.Enum = Enum
ENV.workspace = game

-- executor surface -----------------------------------------------------------
ENV.clonefunction = function(f)
	return function(...) return f(...) end
end
