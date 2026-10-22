-- Roblox-ish surface for a stock Luau runtime, used to see whether the
-- obfuscator's environment checks accept a faithful (if minimal) Roblox host.
local TAGGED = setmetatable({}, { __mode = "k" })

local function tag(obj, kind)
	TAGGED[obj] = kind
	return obj
end

function typeof(v)
	local kind = TAGGED[v]
	if kind then return kind end
	if type(v) == "table" then return "table" end
	return type(v)
end

task = task or {
	wait = function() end,
	spawn = function(f, ...) f(...) end,
	defer = function(f, ...) f(...) end,
	delay = function(_, f, ...) f(...) end,
}
Vector3 = Vector3 or {
	new = function(x, y, z) return tag({ X = x or 0, Y = y or 0, Z = z or 0 }, "Vector3") end,
}
Vector2 = Vector2 or {
	new = function(x, y) return tag({ X = x or 0, Y = y or 0 }, "Vector2") end,
}
CFrame = CFrame or { new = function() return tag({}, "CFrame") end }
local function instance(class)
	local obj = { ClassName = class }
	obj.GetService = function(_, name) return instance("Service:" .. name) end
	obj.IsLoaded = function() return true end
	return tag(obj, "Instance")
end
Instance = Instance or { new = function(class) return instance(class) end }
game = game or instance("DataModel")
workspace = workspace or instance("Workspace")
Players = Players or instance("Players")

function warn(...)
	local parts = {}
	for i = 1, select("#", ...) do parts[#parts + 1] = tostring(select(i, ...)) end
	print("warn: " .. table.concat(parts, " "))
end
function getgenv() return _G end
function gethwid() return "delta-hwid-0123456789abcdef" end
function identifyexecutor() return "Delta", "1.0.0" end
function checkcaller() return true end
function iscclosure() return false end
function islclosure() return true end
function newcclosure(f) return f end
function hookfunction(a) return a end
function clonefunction(f) return f end
function getrawmetatable(o) return getmetatable(o) end
function setreadonly() return true end
function isreadonly() return false end
function getconnections() return {} end
function getfenv() return _G end
function setfenv() return _G end
-- No debug.getconstants / getupvalues / setupvalue: this is the Delta case.
