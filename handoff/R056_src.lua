local a = 223
local function mk(x) return function(n, ...) local c = select("#", ...); x = x + n * a + c; return x end end
local f = mk(317)
local t = {f(5, "jade"), f(7, 2, 3), f(11)}
local m = t[1] + t[2] + t[3]
local r = (m * 41 + a) % 10000
