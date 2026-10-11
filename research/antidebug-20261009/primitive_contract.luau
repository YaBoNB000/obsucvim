-- Project-shape counterexamples for CX's existing E.tp(f(E[21](...))) form.
-- No real target rewriting; deliberate capability injected into toy frames.
local pack, unpack, equal = table.pack, table.unpack, rawequal
local function frame(fn, x)
    return {fn = fn, argv = pack(x), n = 1, tp = pack, unp = unpack,
            pc = 5, family = 1, seq = 0}
end
local function cx(E)
    local f, a, n, pc, family = E.fn, E.argv, E.n, E.pc, E.family
    local seq = E.seq + 1
    local t = {}
    E.seq, E.ticket = seq, t
    -- Evaluation order here intentionally follows the reviewed emitter.
    local r = E.tp(E.fn(E.unp(E.argv, 1, E.n)))
    assert(E.seq == seq and equal(E.ticket, t))
    assert(equal(E.fn, f) and equal(E.argv, a) and E.n == n and E.pc == pc and E.family == family)
    E.ticket = nil
    return unpack(r, 1, r.n)
end
local calls = 0
local function double(x) calls += 1; return x * 2, nil end
local clean = pack(cx(frame(double, 21)))
assert(clean.n == 2 and clean[1] == 42 and calls == 1)
print("PASS", "CX-shaped-clean-case")

local E = frame(double, 21)
E.tp = function(...) return pack(999, nil) end
local result = pack(cx(E))
assert(result.n == 2 and result[1] == 999 and calls == 2)
print("COUNTEREXAMPLE", "unchecked-pack-substitution-forges-results")

local E2 = frame(double, 21)
E2.unp = function(...) return 111 end
local result2 = pack(cx(E2))
assert(result2.n == 2 and result2[1] == 222 and calls == 3 and E2.argv[1] == 21)
print("COUNTEREXAMPLE", "unchecked-unpack-substitution-forges-call-arguments")

local E3
E3 = frame(function(x)
    E3.tp = function(...) return pack(777, nil) end
    return x * 2, nil
end, 21)
local first = pack(cx(E3))
-- Lua captures E.tp before evaluating its argument call, so that change
-- does not replace THIS invocation's already-selected outer function.
assert(first.n == 2 and first[1] == 42)
print("PASS", "outer-pack-callee-selected-before-user-call")
local second = pack(cx(E3))
assert(second.n == 2 and second[1] == 777)
print("COUNTEREXAMPLE", "unchecked-pack-change-persists-to-next-call")
print("SUMMARY", 2, 3)
