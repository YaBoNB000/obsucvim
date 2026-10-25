local a = 12
local b = 34
local c = a + b * 2
local t = {c, b, a}
local function f(x) return x + t[1] end
local r = f(c)
