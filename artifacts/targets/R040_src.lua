local a = 113
local b = 127
local function f(x, ...) local q = {...}; return x + #q, x * 2, nil, x + b end
local t = {f(a, 1, 2, 3)}
local m = t[1] + t[2] + (t[4] or 0)
local r = (select("#", f(m, 9, 8))) .. ":" .. m
