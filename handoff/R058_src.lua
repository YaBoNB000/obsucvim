local a = 229
local function mk(x) return function(n, ...) local c = select("#", ...); for i = 1, n do x = (x * a + i * 3 + c) % 100019 end; return x end end
local f, g = mk(37), mk(53)
local t = {f(8, "onyx"), g(10, 2, 3), f(6), g(5, false)}
local m = t[1] + t[2] + t[3] + t[4]
local r = (m * 47 + a) % 10000
