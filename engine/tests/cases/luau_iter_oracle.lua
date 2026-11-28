-- luau_iter_oracle.lua: generalized iteration, numeric-for boundaries,
-- vararg/closure/metamethod oracle coverage (Luau-only)
-- __iter metamethod: generalized iteration must call it for non-table values
local range = setmetatable({ lo = 1, hi = 4 }, {
	__iter = function(self)
		local i = self.lo - 1
		return function()
			i += 1
			if i <= self.hi then
				return i, i * i
			end
			return nil
		end
	end,
})
local isum = 0
for k, v in range do
	isum += v
end
print("iter:", isum)
-- iterator factory returning (iter, state, control) triple
local function triples(n)
	return function(_, c)
		if c < n then
			return c + 1
		end
		return nil
	end, nil, 0
end
local tsum = 0
for v in triples(5) do
	tsum += v
end
print("triple:", tsum)
-- numeric for: float step hitting the boundary exactly and overshooting
local f1 = 0
for x = 1, 2, 0.5 do
	f1 += x
end
print("fstep:", f1)
local f2 = 0
for x = 0, 1, 0.3 do
	f2 += 1
end
print("fcount:", f2)
local f3 = 0
for x = 5, 1, -1.5 do
	f3 += x
end
print("fneg:", f3)
-- numeric for: limit evaluated once even if a closure mutates the source
local lim = 3
local function bump()
	lim = 100
	return lim
end
local once = 0
for i = 1, bump() do
	once += 1
	if i >= 3 then
		break
	end
end
print("limitonce:", once, lim)
-- vararg: count, select, pack round-trip
local function vargs(...)
	local n = select("#", ...)
	local p = table.pack(...)
	local s = 0
	for i = 1, n do
		s += p[i] or 0
	end
	return n, s
end
print("vargs:", vargs(1, nil, 3, nil, 5))
-- closure upvalues shared and mutated across calls
local function counter(start)
	local n = start
	return function(d)
		n += d
		return n
	end, function()
		return n
	end
end
local add, get = counter(10)
add(1)
add(2)
print("closure:", get(), add(100))
-- string interpolation with nested expressions and method calls
local name = "world"
print(`hello {name} {1 + 2} {("x"):rep(3)}`)
-- repeat/while interplay with compound assignment
local w = 0
while w < 3 do
	w += 1
end
local r = 0
repeat
	r += 2
until r >= 5
print("loops:", w, r)
print("iter oracle done")
