local a = 113
local b = 127
local s = "qf-key"
local function f(v) return v and (a * b + #s) or (b - a) end
local m = f(true) + f(false)
local r = (m * 3 + b) % 10000
