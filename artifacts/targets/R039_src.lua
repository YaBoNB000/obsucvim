local a = 31
local b = 41
local m = a * b + 9
local t = {b, m, a}
local function f(n) local s = 0; for i = 1, n do s = s + i end; return s + t[2] + t[1] - t[3] end
local r = f(5)
