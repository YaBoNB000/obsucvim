local u = 17
local v = 31
local w = u * v - 4
local t = {w, v, u}
local function m(y) return y + t[2] end
local s = m(w)
