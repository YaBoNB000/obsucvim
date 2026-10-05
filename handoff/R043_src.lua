local a = 79
local b = 83
local function f(x) return function(y) return x * y + b end end
local g = f(a)
local m = g(9) + g(4)
local r = (m + b) % 10000
