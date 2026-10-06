local a = 233
local function mk(x) return function(n, ...) local s = "agate"; local c = select("#", ...); for i = 1, n do x = (x * a + i * 5 + c + #s) % 100043 end; return x end end
local f, g = mk(41), mk(59)
local t = {f(9, "jade"), g(12, 2, 3), f(7), g(6, false)}
local m = t[1] + t[2] + t[3] + t[4]
local r = (m * 53 + a) % 10000
