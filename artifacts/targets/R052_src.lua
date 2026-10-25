local a = 173
local b = 179
local s = "tag-dialect"
local function f(v) if v then return a * b + #s, 3.5 else return b - a, 2.25 end end
local x, p = f(true); local y, q = f(false); local m = x + y + p + q
local r = math.floor((m * 23 + b) % 10000)
