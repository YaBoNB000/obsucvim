local a = 241
local function mk(x) return function(n, ...) local s = "spinel"; local c = select("#", ...); for i = 1, n do x = (x * a + i * 11 + c + #s + #"脚本") % 100069 end; return x end end
local f, g = mk(47), mk(67)
local t = {["suda"] = f(11, "jade"), g(14, 2, 3), f(9), g(8, false)}
local m = t["suda"] + t[1] + t[2] + t[3]
return (m * 61 + a + #"脚本") % 10000
