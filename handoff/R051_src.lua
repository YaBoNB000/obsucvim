local a = 163
local b = 167
local function f(n) local x = a; for i = 1, n do x = (x * 37 + i) % 65521 end; return x end
local function g(n) local x = b; for i = 1, n do x = (x * 41 + i * 3) % 65521 end; return x end
local m = f(9) + g(11)
local r = (m * 19 + a + b) % 10000
