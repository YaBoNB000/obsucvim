local a = 139
local b = 149
local function f(n) local x = a; for i = 1, n do x = (x * 19 + i) % 65521 end; return x end
local function g(n) local x = b; for i = 1, n do x = (x * 23 + i * 2) % 65521 end; return x end
local m = f(7) + g(9)
local r = (m * 13 + a + b) % 10000
