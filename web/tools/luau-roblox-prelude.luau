--[[ Faithful-ish executor surface for a stock Luau runtime (R067.5 高保真版).

  在 prelude4 基础上：Random(LCG)/Enum/RunService.IsStudio/Instance 生命周期/
  Vector2/Vector3 __tostring/task.spawn,delay 返回 thread。用途：
  1. 让 tier-1/tier-2/bind-env 分层产物能在沙箱里被正向验证（L2 应打印 Suda）；
  2. 同时**故意不提供** debug 深内省（getconstants/getupvalues/setupvalue/getproto），
     --bind-executor 的 hard 产物应给出精确的缺失清单。
  注意：不遮蔽 Luau 内建（遮蔽内建早年炸过控制流），chunk 级赋值即执行器环境，
  同名 API 同时注册进 getgenv() 返回表。
]]

-- prelude4: 高保真 Roblox/executor 宿主（仅供沙箱正向验证 tier-2/bind-env）
-- 在 prelude1 基础上补齐：Random(LCG)、Enum、RunService.IsStudio、Instance 生命周期、
-- Vector3 __tostring、task.spawn/delay 返回 thread、ExecuteScript 之外的 loadstring。
local API = {}
local TYPED = setmetatable({}, { __mode = "k" })
local function tag(v, kind) TYPED[v] = kind return v end
local function expose(name, value) API[name] = value return value end

-- ============ task（spawn/delay 必须返回 thread）============
task = expose("task", {
	wait = function(t) if type(t) ~= "number" then error("invalid argument #1 (number expected)", 2) end end,
	spawn = function(fn, ...) local co = coroutine.create(fn) coroutine.resume(co, ...) return co end,
	defer = function(fn, ...) local co = coroutine.create(fn) coroutine.resume(co, ...) return co end,
	delay = function(_, fn, ...) local co = coroutine.create(fn) return co end, -- 永不 resume：用于 cancel 语义
	cancel = function(co) if coroutine.status(co) ~= "dead" then pcall(coroutine.close, co) end end,
})

-- ============ Vector3 / CFrame（__tostring 高保真 "{x}, {y}, {z}"）============
local function fmtnum(n)
	if math.floor(n) == n then return string.format("%d", n) end
	return tostring(n)
end
Vector3 = expose("Vector3", {
	new = function(x, y, z)
		local v = { X = x or 0, Y = y or 0, Z = z or 0 }
		return tag(setmetatable(v, { __tostring = function(s)
			return fmtnum(s.X) .. ", " .. fmtnum(s.Y) .. ", " .. fmtnum(s.Z)
		end }), "Vector3")
	end,
})
Vector2 = expose("Vector2", {
	new = function(x, y)
		local v = { X = x or 0, Y = y or 0 }
		return tag(setmetatable(v, { __tostring = function(s)
			return fmtnum(s.X) .. ", " .. fmtnum(s.Y)
		end }), "Vector2")
	end,
})
CFrame = expose("CFrame", { new = function(...) return tag({}, "CFrame") end })
typeof = expose("typeof", function(v) return TYPED[v] or type(v) end)

-- ============ Instance 生命周期 ============
local InstanceMT
InstanceMT = {
	__index = function(self, k)
		if k == "ClassName" then return rawget(self, "_class") end
		if k == "Name" then return rawget(self, "_name") end
		if k == "Parent" then return rawget(self, "_parent") end
		if k == "ChildRemoved" then
			local ev = rawget(self, "_childRemoved")
			if not ev then
				ev = { _conns = {} }
				rawset(self, "_childRemoved", ev)
				ev.Connect = function(_, fn)
					local conn = { Connected = true }
					function conn.Disconnect(c) c.Connected = false end
					return conn
				end
			end
			return ev
		end
		if k == "GetChildren" then
			return function(s)
				local out = {}
				for _, c in pairs(rawget(s, "_kids")) do out[#out + 1] = c end
				return out
			end
		end
		if k == "Destroy" then
			return function(s)
				if rawget(s, "_dead") then return end
				rawset(s, "_dead", true)
				local p = rawget(s, "_parent")
				if p then
					local kids = rawget(p, "_kids")
					for i, c in ipairs(kids) do
						if c == s then table.remove(kids, i) break end
					end
				end
				rawset(s, "_parent", nil)
			end
		end
		return nil
	end,
	__newindex = function(self, k, v)
		if k == "Parent" then
			if v ~= nil and rawget(v, "_dead") then
				error("The Parent property of " .. tostring(rawget(self, "_class")) .. " is locked, current parent is NULL", 2)
			end
			local p = rawget(self, "_parent")
			if p then
				for i, c in ipairs(rawget(p, "_kids")) do
					if c == self then table.remove(rawget(p, "_kids"), i) break end
				end
			end
			rawset(self, "_parent", v)
			if v then rawget(v, "_kids")[#rawget(v, "_kids") + 1] = self end
			return
		end
		rawset(self, k, v)
	end,
}
local function newInstance(class)
	return tag(setmetatable({ _class = class, _name = class, _kids = {}, _dead = false, _parent = nil }, InstanceMT), "Instance")
end
Instance = expose("Instance", { new = function(class) return newInstance(class) end })

-- ============ game / services ============
local RunService = newInstance("RunService")
RunService.IsStudio = function(self)
	if self ~= RunService then error("Expected ':' not '.' calling member function IsStudio", 2) end
	return true
end
local SRVS = { RunService = RunService }
for _, n in ipairs({ "Players", "Workspace", "Lighting", "ReplicatedStorage" }) do SRVS[n] = newInstance(n) end
game = tag(setmetatable({
	ClassName = "DataModel",
	GetService = function(self, name)
		if self ~= game then error("Expected ':' not '.' calling member function GetService", 2) end
		return SRVS[name] or newInstance(name)
	end,
	GetChildren = function(self)
		if self ~= game then error("Expected ':' not '.' calling member function GetChildren", 2) end
		return {}
	end,
}, {}), "Instance")
workspace = expose("workspace", SRVS.Workspace)

-- ============ Enum（HumanoidRigType.R6 全等 + locked metatable 文案）============
local LOCKED = { __metatable = "The metatable is locked" }
local EnumItemR6 = tag(setmetatable({}, LOCKED), "EnumItem")
local HRig = tag(setmetatable({}, LOCKED), "Enum")
EnumItemR6.Name = "R6"
EnumItemR6.Value = 0
EnumItemR6.EnumType = HRig
HRig.FromName = function(self, n) if self ~= HRig or n ~= "R6" then error("invalid enum name", 2) end return EnumItemR6 end
HRig.FromValue = function(self, v) if self ~= HRig or v ~= 0 then error("invalid enum value", 2) end return EnumItemR6 end
HRig.R6 = EnumItemR6
Enum = tag(setmetatable({ HumanoidRigType = HRig }, LOCKED), "Enums")

-- ============ Random（结构忠实 LCG：同种子同序列/Clone 独立/种子敏感）============
local RandomMT = { __index = {} }
local function rngNext(self)
	-- 29 位 LCG，输出 [0,1)
	self._s = (self._s * 636413622 + 1442695041) % 536870912
	return self._s / 536870912
end
RandomMT.__index.NextNumber = function(self) return rngNext(self) end
RandomMT.__index.NextInteger = function(self, lo, hi)
	local span = hi - lo + 1
	return lo + math.floor(rngNext(self) * span)
end
RandomMT.__index.Clone = function(self)
	return setmetatable({ _s = self._s }, RandomMT)
end
Random = expose("Random", {
	new = function(seed) return setmetatable({ _s = (seed or 1) % 536870912 }, RandomMT) end,
})

-- ============ executor API 全套 ============
gethwid = expose("gethwid", function() return "delta-hwid-0123456789abcdef" end)
identifyexecutor = expose("identifyexecutor", function() return "Delta", "1.1.743.1136" end)
getexecutorname = expose("getexecutorname", function() return "Delta" end)
checkcaller = expose("checkcaller", function() return true end)
iscclosure = expose("iscclosure", function(fn)
	if type(fn) ~= "function" then return false end
	local ok, src = pcall(debug.info, fn, "s")
	return ok and src == "[C]"
end)
islclosure = expose("islclosure", function(fn)
	if type(fn) ~= "function" then return false end
	local ok, src = pcall(debug.info, fn, "s")
	return ok and src ~= "[C]"
end)
newcclosure = expose("newcclosure", function(fn) return fn end)
hookfunction = expose("hookfunction", function(a) return a end)
clonefunction = expose("clonefunction", function(fn) return fn end)
getrawmetatable = expose("getrawmetatable", function(o) return getmetatable(o) end)
setreadonly = expose("setreadonly", function() return true end)
isreadonly = expose("isreadonly", function() return false end)
getconnections = expose("getconnections", function() return {} end)
gethui = expose("gethui", function() return {} end)
gethiddenproperty = expose("gethiddenproperty", function() return nil end)
sethiddenproperty = expose("sethiddenproperty", function() end)
warn = expose("warn", function(...)
	local parts = {}
	for i = 1, select("#", ...) do parts[#parts + 1] = tostring(select(i, ...)) end
	print("warn: " .. table.concat(parts, " "))
end)

-- 保持 Luau 原生 debug（不加深内省——hard 产物应因此报精确缺失清单）
local REAL_DEBUG = debug
local debugCopy = {}
for k, v in pairs(REAL_DEBUG) do debugCopy[k] = v end
debug = expose("debug", debugCopy)

getgenv = expose("getgenv", function() return API end)
