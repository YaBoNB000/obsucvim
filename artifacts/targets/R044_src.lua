local a = 89
local b = 97
local function f(x, ...) local t = {...}; return x * b + #t, x + a end
local p, q = f(7, 1, 2, 3, 4)
local m = p + q
local r = (m * 5 + b) % 10000
