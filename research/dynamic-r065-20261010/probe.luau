-- Probe the Luau CLI's exposed attack surface for dynamic methods.
local names = {}
for k in pairs(debug) do table.insert(names, k) end
table.sort(names)
print("DEBUG_LIB", table.concat(names, ","))
print("string.dump", type(string.dump))
local ok, len = pcall(function()
    local d = string.dump(function() return 1 end)
    return #d
end)
print("string.dump(f)", ok, len)
print("getfenv", type(getfenv), "setfenv", type(setfenv))
print("debug.sethook", type(debug.sethook))
print("debug.getlocal", type(debug.getlocal))
print("debug.info", type(debug.info))
