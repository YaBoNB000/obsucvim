local a = 227
local function mk(x) return function(n, ...) local c = select("#", ...); for i = 1, n do x = (x * a + i + c) % 100003 end; return x end end
local f, g = mk(31), mk(47)
local t = {f(9, "opal"), g(11, 2, 3), f(5), g(7, true)}
local m = t[1] + t[2] + t[3] + t[4]
local r = (m * 43 + a) % 10000
