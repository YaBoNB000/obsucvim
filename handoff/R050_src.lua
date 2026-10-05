local a = 151
local b = 157
local function f(n) local x = a; for i = 1, n do x = (x * 29 + i) % 65521 end; return x end
local function g(n) local x = b; for i = 1, n do x = (x * 31 + i * 3) % 65521 end; return x end
local m = f(8) + g(10)
local r = (m * 17 + a + b) % 10000
