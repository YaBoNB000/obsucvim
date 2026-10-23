--[[ Faithful-ish executor surface for a stock Luau runtime.

  Two constraints shaped this file:

  1. executors hand back a *writable* env table from getgenv(); the Luau CLI's
     _G is a read-only view, so the fake environment is a real table.
  2. globals written at chunk level in the Luau CLI do NOT show up in _G, so
     every fake API is registered into that table explicitly.

  Nothing here shadows Luau builtins (that broke control flow in earlier runs).
]]

local API = {}
local function expose(name, value)
	API[name] = value
	return value
end

task = expose("task", {
	wait = function() end,
	spawn = function(fn, ...) fn(...) end,
	defer = function(fn, ...) fn(...) end,
	delay = function(_, fn, ...) fn(...) end,
	cancel = function() end,
	synchronize = function() end,
})
-- Like Roblox: Vector3/Vector2 instances answer typeof() with their own type.
local TYPED = setmetatable({}, { __mode = "k" })
local function typedVector(kind, x, y, z)
	local vector = { X = x or 0, Y = y or 0, Z = z or 0 }
	TYPED[vector] = kind
	return vector
end
Vector3 = expose("Vector3", {
	new = function(x, y, z) return typedVector("Vector3", x, y, z) end,
})
Vector2 = expose("Vector2", {
	new = function(x, y) return typedVector("Vector2", x, y, 0) end,
})
typeof = expose("typeof", function(value)
	return TYPED[value] or type(value)
end)
CFrame = expose("CFrame", { new = function() return {} end })

local function makeInstance(class)
	local object = { ClassName = class }
	object.GetService = function(_, name) return makeInstance("Service:" .. name) end
	object.IsLoaded = function() return true end
	return object
end
Instance = expose("Instance", { new = function(class) return makeInstance(class) end })
game = expose("game", makeInstance("DataModel"))
workspace = expose("workspace", makeInstance("Workspace"))
Players = expose("Players", makeInstance("Players"))

warn = expose("warn", function(...)
	local parts = {}
	for index = 1, select("#", ...) do parts[#parts + 1] = tostring(select(index, ...)) end
	print("warn: " .. table.concat(parts, " "))
end)
gethwid = expose("gethwid", function() return "delta-hwid-0123456789abcdef" end)
identifyexecutor = expose("identifyexecutor", function() return "Delta", "1.0.0" end)
checkcaller = expose("checkcaller", function() return true end)
-- Truthful closure classification: a C function reports source "[C]".
iscclosure = expose("iscclosure", function(fn)
	if type(fn) ~= "function" then return false end
	local ok, source = pcall(debug.info, fn, "s")
	return ok and source == "[C]"
end)
islclosure = expose("islclosure", function(fn)
	if type(fn) ~= "function" then return false end
	local ok, source = pcall(debug.info, fn, "s")
	return ok and source ~= "[C]"
end)
newcclosure = expose("newcclosure", function(fn) return fn end)
hookfunction = expose("hookfunction", function(a) return a end)
clonefunction = expose("clonefunction", function(fn) return fn end)
getrawmetatable = expose("getrawmetatable", function(object) return getmetatable(object) end)
setreadonly = expose("setreadonly", function() return true end)
isreadonly = expose("isreadonly", function() return false end)
getconnections = expose("getconnections", function() return {} end)

-- Keep a complete Luau debug library (the VM itself uses debug.info), but do not
-- add the executor-only extras (getconstants/getupvalues/setupvalue) — that is
-- what "an executor without deep introspection" looks like.
local REAL_DEBUG = debug
local debugCopy = {}
for key, value in pairs(REAL_DEBUG) do debugCopy[key] = value end
debug = expose("debug", debugCopy)

getgenv = expose("getgenv", function() return API end)
