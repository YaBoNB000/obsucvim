local a = 193
local b = 197
local function f(n) local x = 0; for i = 1, n do if i % 3 == 0 then x = x + i * a else x = x + i * b end end; return x end
local t = {f(12), f(15), f(18)}
local m = t[1] + t[2] + t[3]
local r = (m * 31 + a + b) % 10000
