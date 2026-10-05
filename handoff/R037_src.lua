local a = 23
local b = 31
local m = a * b + 7
local t = {b, m, a}
local function f(v) return (v + t[1] - t[3]) .. ":" .. tostring(v % 17) end
local r = f(m)
