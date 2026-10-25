local a = 101
local b = 103
local function f(n) local x = a; for i = 1, n do x = (x * b + i) % 10007 end; return x end
local t = {f(3), f(5), a + b}
local m = t[1] + t[2] + t[3]
local r = (m * 7 + b) % 10000
