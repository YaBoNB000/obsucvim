local a = 131
local b = 137
local function f(n) local x = a; for i = 1, n do x = (x * 17 + i * b) % 65521 end; return x end
local t = {f(4), f(6), f(8)}
local m = t[1] + t[2] + t[3]
local r = (m * 11 + b) % 10000
