local a = 29
local b = 37
local m = a * b + 13
local t = {b, m, a}
local function f(v, ...) local x = {...}; return tostring(v + t[1] - t[3]) .. ":" .. #x end
local r = f(m, 4, 5, 6)
