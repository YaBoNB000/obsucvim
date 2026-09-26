local x = 17
local y = 26
local z = x * y + 9
local t = {y, x, z}
local function h(v) return v - t[2] + t[1] end
local r = h(z)
