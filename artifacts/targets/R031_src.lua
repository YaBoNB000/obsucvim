local x = 5
local y = 9
local z = x * y + 3
local t = {z, y, x}
local function k(v) return v + t[1] end
local r = k(z)
