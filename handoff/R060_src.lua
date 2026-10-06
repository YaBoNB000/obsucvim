local a = 239
local function mk(x) return function(n, ...) local s = "beryl"; local c = select("#", ...); for i = 1, n do x = (x * a + i * 7 + c + #s) % 100057 end; return x end end
local f, g = mk(43), mk(61)
local t = {f(10, "jade"), g(13, 2, 3), f(8), g(7, false)}
local m = t[1] + t[2] + t[3] + t[4]
local r = (m * 59 + a) % 10000
