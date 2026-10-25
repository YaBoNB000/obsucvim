local a = 67
local b = 71
local function f(n) local s = 0; for i = 1, n do s = (s + i * b) % 9973 end; return s end
local t = {f(5), f(7), a + b}
local m = t[1] + t[2] + t[3]
local r = (m * 3 + a) % 10000
