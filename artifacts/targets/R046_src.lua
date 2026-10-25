local a = 107
local b = 109
local function f(n) local x = 0; for i = 1, n do if i % 2 == 0 then x = x + i * a else x = x + i * b end end; return x end
local t = {f(8), f(5)}
local m = t[1] + t[2] + a
local r = (m * 3 + b) % 10000
