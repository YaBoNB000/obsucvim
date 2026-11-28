-- luau_compound.lua: compound assignments (Luau-only)
local a = 10
a += 5
print("add:", a)
local b = 10
b -= 4
print("sub:", b)
local c = 3
c *= 4
print("mul:", c)
local d = 12
d /= 2
print("div:", d)
local e = 17
e %= 5
print("mod:", e)
local f = 2
f ^= 5
print("pow:", f)
-- compound on index
local t = { x = 1, [2] = 10 }
t.x += 1
t[2] -= 3
print("index:", t.x, t[2])
-- compound with expression RHS
local g = 2
g += 3 * 4
print("expr:", g)
local h = 100
h %= 7 + 1
print("expr mod:", h)
local s = "a"
s ..= "b"
print("concat:", s)
local z = 17
z //= 5
print("idiv:", z)
-- dynamic lvalue parts must be evaluated exactly once, in native order:
-- object, key, current-value read, RHS, write.
local calls = 0
local dyn = { value = 1 }
local function nextKey()
	calls += 1
	return "value"
end
dyn[nextKey()] += 5
print("dynamic key:", dyn.value, calls)
local order = {}
local backing = { value = 3 }
local proxy = setmetatable({}, {
	__index = function(_, k)
		order[#order + 1] = "get"
		return backing[k]
	end,
	__newindex = function(_, k, v)
		order[#order + 1] = "set"
		backing[k] = v
	end,
})
local function factory()
	order[#order + 1] = "factory"
	return proxy
end
local function rhs()
	order[#order + 1] = "rhs"
	return 4
end
factory().value += rhs()
print("dynamic order:", backing.value, table.concat(order, ","))
-- multiple statements
local i = 1
while i <= 4 do
	i += 1
end
print("loop:", i)
print("luau compound done")
