local a = 13
local b = 29
local c = a * b - 7
local t = {b, c, a}
local function q(x) return x + t[1] - t[3] end
local r = q(c)
