local p = 19
local q = 28
local m = p * q + 11
local t = {q, m, p}
local function k(v) if not false then return v + t[1] - t[3] end end
local r = k(m)
