local p = 11
local q = 23
local r = p * q + 7
local t = {r, q, p}
local function h(x) return x - t[2] end
local s = h(r)
