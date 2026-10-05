local a = 181
local b = 191
local function f(n) local x = a; for i = 1, n do x = (x * 43 + i) % 65521 end; return x end
local function g(n) local x = b; for i = 1, n do x = (x * 47 + i * 5) % 65521 end; return x end
local m = f(10) + g(12)
local r = (m * 29 + a + b) % 10000
