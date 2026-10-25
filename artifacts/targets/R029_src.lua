local a = 7
local b = 13
local c = a * b + 5
local t = {c, b, a}
local function g(x) return x - t[2] end
local r = g(c)
