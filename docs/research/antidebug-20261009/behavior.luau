-- Research only. Never imported by the production obfuscator.
local pack = table.pack
local unpack = table.unpack
local RAW_PCALL = pcall
local RAW_ERROR = error
local checks = 0
local negatives = 0
local function check(name, value)
    checks += 1
    if not value then error("CHECK FAILED: " .. name, 0) end
    print("PASS", name)
end
local function counterexample(name, value)
    negatives += 1
    if not value then error("COUNTEREXAMPLE NOT REPRODUCED: " .. name, 0) end
    print("COUNTEREXAMPLE", name)
end

-- Private one-call owner. There is no global pending table. Not a VM patch.
local function guarded(f, ...)
    local owner = {}
    local ticket = {}
    local receipt = {owner, ticket, "postcall"}
    local pending = ticket
    local result = pack(RAW_PCALL(f, ...))
    local valid = receipt[1] == owner and receipt[2] == pending and receipt[3] == "postcall"
    pending = nil
    receipt[1], receipt[2], receipt[3] = nil, nil, nil
    if not valid then error("research receipt mismatch", 0) end
    if not result[1] then RAW_ERROR(result[2], 0) end
    return unpack(result, 2, result.n)
end

local function values(...)
    return ...
end
local r = pack(guarded(values, nil, 1, nil, false, nil))
check("trailing-nil-and-arity", r.n == 5 and r[1] == nil and r[2] == 1 and r[4] == false and r[5] == nil)
check("no-return-values", pack(guarded(function() end)).n == 0)
check("one-nil-result", pack(guarded(function() return nil end)).n == 1)
local marker = {}
local ok, reason = pcall(function() guarded(function() error(marker, 0) end) end)
check("error-object-identity", not ok and rawequal(marker, reason))
local nilerr = pack(pcall(function() guarded(function() error(nil, 0) end) end))
check("nil-error-object", nilerr.n == 2 and not nilerr[1] and nilerr[2] == nil)

local function recurse(n)
    if n == 0 then return 0, nil end
    local v, tail = guarded(recurse, n - 1)
    return v + 1, tail
end
local v = pack(guarded(recurse, 40))
check("nested-reentry", v.n == 2 and v[1] == 40 and v[2] == nil)

local cos = {}
local total_yields = 0
for i = 1, 16 do
    cos[i] = coroutine.create(function()
        return guarded(function()
            for j = 1, 32 do
                local echo = pack(coroutine.yield(i, j, nil))
                assert(echo.n == 2 and echo[1] == j and echo[2] == nil)
            end
            return i, nil
        end)
    end)
end
for j = 1, 33 do
    for i = 16, 1, -1 do
        local out
        if j == 1 then out = pack(coroutine.resume(cos[i]))
        else out = pack(coroutine.resume(cos[i], j - 1, nil)) end
        assert(out[1], tostring(out[2]))
        if j <= 32 then
            assert(out.n == 4 and out[2] == i and out[3] == j and out[4] == nil)
            total_yields += 1
        else
            assert(out.n == 3 and out[2] == i and out[3] == nil)
        end
    end
end
check("16-coroutines-512-yields", total_yields == 512)

local calls = 0
local callable = setmetatable({}, {__call = function(_, x) calls += 1 return x, nil end, __metatable = "locked"})
local result = pack(guarded(callable, 9))
check("protected-call-metamethod", calls == 1 and result.n == 2 and result[1] == 9)

-- False-positive witness: the program deliberately and legally checks its
-- immediate caller. The extra wrapper changes that caller, even without a
-- debugger or hostile host.
local parent
local function caller_sensitive()
    return debug.info(2, "f") == parent
end
parent = function(use_guard)
    if use_guard then return guarded(caller_sensitive) end
    return caller_sensitive()
end
counterexample("extra-adapter-changes-debug-info-caller", parent(false) and not parent(true))

local function caller_error()
    error("caller-sensitive-error", 2)
end
local function parent_error(use_guard)
    if use_guard then return guarded(caller_error) end
    return caller_error()
end
local ok1, e1 = pcall(parent_error, false)
local ok2, e2 = pcall(parent_error, true)
counterexample("error-level-location-changes", not ok1 and not ok2 and e1 ~= e2)
print("OBSERVATION", "error-direct", e1)
print("OBSERVATION", "error-adapter", e2)

-- A singleton phase guard is incompatible with ordinary interleaving.
local global_pending = false
local function bad_global_guard(f)
    if global_pending then return "busy" end
    global_pending = true
    local x = f()
    global_pending = false
    return x
end
local a = coroutine.create(function() return bad_global_guard(function() coroutine.yield("a") return "done" end) end)
assert(coroutine.resume(a))
local b = bad_global_guard(function() return "legal-b" end)
assert(coroutine.resume(a))
counterexample("global-singleton-false-positive-on-yield", b == "busy")

-- Error cleanup does not run on coroutine cancellation. A global ownership
-- registry would keep the frame alive unless explicitly designed otherwise.
if coroutine.close then
    local outstanding = 0
    local abandoned = coroutine.create(function()
        outstanding += 1
        guarded(function() coroutine.yield("pause") end)
        outstanding -= 1
    end)
    assert(coroutine.resume(abandoned))
    assert(coroutine.close(abandoned))
    counterexample("cancellation-skips-postcall-cleanup", outstanding == 1)
else
    print("UNTESTED", "coroutine.close-not-available")
end

local meta_calls = 0
local mt = {__add = function() meta_calls += 1 return 7 end}
local x = setmetatable({}, mt)
local direct = x + x
local before = meta_calls
local redundant = x + x
local redundant_check = x + x
counterexample("redundant-evaluation-duplicates-user-effects", meta_calls - before == 2 and redundant == redundant_check and direct == 7)
local nan = 0 / 0
counterexample("self-equality-rejects-legal-nan", not (nan == nan))
counterexample("subtract-self-rejects-legal-infinity", not (math.huge - math.huge == 0))
counterexample("add-subtract-one-not-identity", (2^53 + 1) - 1 ~= 2^53)

-- Verify arithmetic against a bit32 implementation for deterministic pairs.
local function splitmul(a, b)
    local a0, a1 = a % 65536, math.floor(a / 65536)
    local b0, b1 = b % 65536, math.floor(b / 65536)
    return (a0 * b0 + ((a0 * b1 + a1 * b0) % 65536) * 65536) % 2^32
end
local function shiftmul(a, b)
    local out = 0
    for _ = 1, 32 do
        if bit32.band(b, 1) == 1 then out = (out + a) % 2^32 end
        a = bit32.lshift(a, 1)
        b = bit32.rshift(b, 1)
    end
    return out
end
local state, mismatches, exact_errors = 17, 0, 0
for i = 1, 10000 do
    state = (state * 48271) % 2147483647
    local aa = state * 2
    state = (state * 48271) % 2147483647
    local bb = state * 2 + 1
    local exact = shiftmul(aa, bb)
    if (aa * bb) % 2^32 ~= exact then mismatches += 1 end
    if splitmul(aa, bb) ~= exact then exact_errors += 1 end
end
check("split16-multiplication-10000-pairs", exact_errors == 0)
counterexample("naive-double-mod32-multiplication", mismatches > 0)
print("MEASURE", "naive-multiply-mismatches", mismatches, 10000)

-- Microbenchmark alternates order, includes the same result-packing cost for
-- the receipt-free baseline. Not representative of full VM throughput.
local function plain_adapter(f, ...)
    local result = pack(RAW_PCALL(f, ...))
    if not result[1] then RAW_ERROR(result[2], 0) end
    return unpack(result, 2, result.n)
end
local function target(x) return x + 1, nil end
local function bench(adapter)
    local start, acc = os.clock(), 0
    for i = 1, 50000 do
        acc += adapter(target, i)
    end
    assert(acc == 1250075000)
    return os.clock() - start
end
for i = 1, 9 do
    local plain, guarded_time
    if i % 2 == 0 then guarded_time = bench(guarded); plain = bench(plain_adapter)
    else plain = bench(plain_adapter); guarded_time = bench(guarded) end
    print("BENCH", i, plain, guarded_time, guarded_time / plain)
end
print("SUMMARY", checks, negatives)
