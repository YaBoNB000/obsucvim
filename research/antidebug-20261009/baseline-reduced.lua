local function m3(a, b, c) return a, b, c end
local function m1(x) return x end
local function mnone() end
local function sum(...)
	return s
end
local c3 = (m3(1, 2, 3) and "T" or "F")
local function f1(...) return ... end
local function f2(...) return f1(...) end
print("fwd1:", f2(1, 2, 3))
local function mixed(a, ...)
end
local function genpairs()
	return function()
	end
end
for k, v in genpairs() do
end
local function exact_n(...)
end
local function forward_n(...)
end
local function forward_all(...)
end
local function fixed_n(head, ...)
end
local function closure_n(...)
	local function inner(...) return select("#", ...) end
end
print("tailnil:", exact_n(nil, "tail", false, 7, nil),
	closure_n(nil, 9, nil))
