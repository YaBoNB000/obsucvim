-- Round 2 independent CALL/CX interval experiment.
-- No production code is imported or patched. The functions here model a
-- single existing shallow call boundary; no extra pcall/wrapper is inserted
-- between boundary() and the user callback when checking is enabled.
local PACK, UNPACK = table.pack, table.unpack
local EQ = rawequal
local RERR = error
local checks, counterexamples = 0, 0
local function check(name, value)
    assert(value, name)
    checks += 1
    print("PASS", name)
end
local function negative(name, value)
    assert(value, name)
    counterexamples += 1
    print("COUNTEREXAMPLE", name)
end
local function frame(f, ...)
    return {fn = f, args = PACK(...), pc = 7, family = 1, phase = "ready", seq = 0}
end

-- expected sequence in E is NOT independent of E. This candidate claims
-- interval mutation detection only, not authentication of pre-entry state.
local function boundary(E, enabled)
    local fn, args, n, pc, family = E.fn, E.args, E.args.n, E.pc, E.family
    local ticket, generation
    if enabled then
        if E.phase ~= "ready" then RERR("interval-phase", 0) end
        generation = E.seq + 1
        ticket = {}
        E.seq, E.ticket, E.phase = generation, ticket, "calling"
    end
    -- The actual callback call site is the SAME line/function in both modes.
    local results = PACK(fn(UNPACK(args, 1, n)))
    if enabled then
        if not EQ(E.ticket, ticket) or E.seq ~= generation or E.phase ~= "calling"
            or not EQ(E.fn, fn) or not EQ(E.args, args) or E.args.n ~= n
            or E.pc ~= pc or E.family ~= family then
            RERR("interval-mismatch", 0)
        end
        E.ticket, E.phase = nil, "ready"
    end
    return UNPACK(results, 1, results.n)
end

-- Keeping the same existing call boundary, unlike Round 1 wrapping it,
-- preserves immediate caller identity and the error-callsite in this model.
local function caller_probe()
    return debug.info(2, "f") == boundary, debug.info(2, "l")
end
local a, la = boundary(frame(caller_probe), false)
local b, lb = boundary(frame(caller_probe), true)
check("same-boundary-caller-and-line", a and b and la == lb)
local function throw_to_caller() error("location-sensitive", 2) end
local oa, ea = pcall(boundary, frame(throw_to_caller), false)
local ob, eb = pcall(boundary, frame(throw_to_caller), true)
check("same-callsite-error-location", not oa and not ob and ea == eb)

local t = PACK(boundary(frame(function(...) return ... end, nil, 7, nil, false, nil), true))
check("exact-arity-five-with-nils", t.n == 5 and t[1] == nil and t[2] == 7 and t[4] == false)
check("zero-result", PACK(boundary(frame(function() end), true)).n == 0)
local err = {}
local ok, val = pcall(boundary, frame(function() error(err, 0) end), true)
check("error-object-identity", not ok and EQ(val, err))
local error_n = PACK(pcall(boundary, frame(function() error(nil, 0) end), true))
check("nil-error-arity", error_n.n == 2 and error_n[1] == false and error_n[2] == nil)

-- Do NOT authenticate transitive user state. Tables and upvalues may be
-- legitimately mutated inside a callback, including via nested callbacks.
local shared, up = {count = 0}, 0
local value = boundary(frame(function(x)
    x.count += 1
    up += 1
    return boundary(frame(function() x.count += 2; up += 2; return x.count end), true)
end, shared), true)
check("legal-table-and-upvalue-mutation", value == 3 and shared.count == 3 and up == 3)
local calls = 0
local callable = setmetatable({}, {__call = function(_, x) calls += 1 return x, nil end, __metatable = "protected"})
local result = PACK(boundary(frame(callable, 11), true))
check("protected-callable-single-effect", calls == 1 and result.n == 2 and result[1] == 11)

local function rec(n)
    if n == 0 then return nil, 0, nil end
    local _, count = boundary(frame(rec, n - 1), true)
    return nil, count + 1, nil
end
local r = PACK(boundary(frame(rec, 64), true))
check("64-level-recursion", r.n == 3 and r[2] == 64)

local cos, fs = {}, {}
for i = 1, 32 do
    fs[i] = frame(function()
        for j = 1, 20 do
            local resumed = PACK(coroutine.yield(i, j, nil))
            assert(resumed.n == 2 and resumed[1] == j and resumed[2] == nil)
        end
        return i, nil
    end)
    cos[i] = coroutine.create(function() return boundary(fs[i], true) end)
end
for j = 1, 21 do
    for i = 32, 1, -1 do
        local out
        if j == 1 then out = PACK(coroutine.resume(cos[i]))
        else out = PACK(coroutine.resume(cos[i], j - 1, nil)) end
        assert(out[1], tostring(out[2]))
        if j <= 20 then
            assert(out.n == 4 and out[2] == i and out[3] == j)
            assert(fs[i].phase == "calling")
        else
            assert(out.n == 3 and out[2] == i and fs[i].phase == "ready")
        end
    end
end
check("32-coroutines-640-yields", true)

-- Single-field active mutation during callback. These are research-only
-- injections with direct E access; ordinary user programs do not receive E.
local mutations = {
    ticket = function(E) E.ticket = {} end,
    sequence = function(E) E.seq += 1 end,
    phase = function(E) E.phase = "ready" end,
    fn = function(E) E.fn = function() end end,
    args_identity = function(E) E.args = {n = 0} end,
    args_arity = function(E) E.args.n += 1 end,
    pc = function(E) E.pc += 1 end,
    family = function(E) E.family = 2 end,
}
for name, mutate in pairs(mutations) do
    local E
    E = frame(function() mutate(E); return 42 end)
    local success, why = pcall(boundary, E, true)
    check("reject-single-field-" .. name, not success and why == "interval-mismatch")
end

-- Attack before snapshot: both expected and observed values use that same
-- already-corrupted frame. The interval check cannot authenticate them.
local E = frame(function() return "original" end)
E.fn = function() return "replaced" end
negative("pre-entry-function-substitution-accepted", boundary(E, true) == "replaced")

-- Argument contents are intentionally NOT mirrored; mirroring arbitrary
-- values increases live copies and may invoke user equality metamethods.
local A
A = frame(function(...)
    A.args[1] = "tampered-buffer"
    return ...
end, "original")
local answer = boundary(A, true)
negative("argument-buffer-content-write-not-detected", answer == "original" and A.args[1] == "tampered-buffer")

-- Temporary observation/mutation followed by restoration leaves no evidence
-- in a pure boundary-state comparison. The trace really was recorded.
local captured, F = {}, nil
F = frame(function(x)
    captured.fn, captured.args = F.fn, PACK(UNPACK(F.args, 1, F.args.n))
    local oldpc = F.pc
    F.pc = 999
    F.pc = oldpc
    return x * 2
end, 21)
negative("read-observe-restore-accepted", boundary(F, true) == 42 and captured.args[1] == 21 and captured.fn ~= nil)

-- A valid completion does not prove that a returned number is the value
-- intended by the original function. No reference execution is available.
local R = frame(function() return "forged-result" end)
negative("receipt-does-not-authenticate-user-result", boundary(R, true) == "forged-result")

-- A thrown error prevents the postcall check from executing, without extra
-- pcall/handler frames. Propagation matches baseline, but phase cleanup is
-- not available here. Production may abandon that frame; do not claim leak.
local X
X = frame(function() X.pc = 100; error(err, 0) end)
local success, why = pcall(boundary, X, true)
negative("throw-skips-postcall-validation", not success and EQ(why, err) and X.phase == "calling" and X.ticket ~= nil)
if coroutine.close then
    local C = frame(function() coroutine.yield(); return 7 end)
    local co = coroutine.create(function() boundary(C, true) end)
    assert(coroutine.resume(co))
    assert(coroutine.close(co))
    negative("cancel-skips-postcall-validation", C.phase == "calling" and C.ticket ~= nil)
end

-- Stored references use raw identity, not arbitrary user __eq. The guard
-- must not compare mutable user values as a shortcut for result integrity.
local eq_calls = 0
local mt = {__eq = function() eq_calls += 1; return true end}
local q1, q2 = setmetatable({}, mt), setmetatable({}, mt)
check("raw-identity-does-not-invoke-user-equality", not EQ(q1, q2) and eq_calls == 0)
local ordinary = q1 == q2
negative("ordinary-equality-has-user-side-effect", ordinary and eq_calls == 1)

-- Microbench: reuse one private frame to separate per-call ticket overhead
-- from allocating a fresh test frame. Both modes preserve the same callsite.
local function f(x) return x + 1, nil end
local BF = frame(f, 10)
local function bench(enabled)
    local start, total = os.clock(), 0
    for i = 1, 50000 do total += boundary(BF, enabled) end
    assert(total == 550000)
    return os.clock() - start
end
for i = 1, 9 do
    local plain, guarded
    if i % 2 == 0 then guarded = bench(true); plain = bench(false)
    else plain = bench(false); guarded = bench(true) end
    print("BENCH", i, plain, guarded, guarded / plain)
end
print("SUMMARY", checks, counterexamples)
