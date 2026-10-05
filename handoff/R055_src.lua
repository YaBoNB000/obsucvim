local a = 199
local b = 211
local function f(n) local x = 0; for i = 1, n do if i % 4 == 0 then x = x + i * a else x = x + i * b end end; return x end
local t = {f(13), f(17), f(21)}
local m = t[1] + t[2] + t[3]
local r = (m * 37 + a + b) % 10000
