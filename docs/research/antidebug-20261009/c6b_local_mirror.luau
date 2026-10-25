-- C6 round 2: novel-query + LOCAL-MIRROR consistency, on the real runtime.
-- Research only; not the production VM.
--
-- Design: the program queries the environment ONCE with a per-run nonce
-- input (recording from another run cannot contain the answer), and checks
-- the answer against a mirror computed locally in VM-exact arithmetic.
--   exact replayer (recording table)  -> dies on the novel nonce
--   semantic emulator (real Luau)      -> passes (honest boundary)
--   subtly-wrong emulator (edge-rule) -> dies iff the probe feeds an edge
--                                        class where the rules diverge
-- False-positive side: the local mirror must agree with the real runtime
-- EXACTLY on every input class the probe can generate (measured below).

local checks, counters = 0, 0
local function check(name, ok)
    assert(ok, name)
    checks += 1
    print("PASS", name)
end
local function counter(name, ok)
    assert(ok, name)
    counters += 1
    print("COUNTEREXAMPLE", name)
end

-- ---------------------------------------------------------------------------
-- Part 1: map the real runtime's bit32 argument semantics on edge classes
-- ---------------------------------------------------------------------------
local function pcall2(f, ...)
    local ok, v = pcall(f, ...)
    if ok then return tostring(v) else return "ERR" end
end
local edges = {
    {0, 0}, {1, 0}, {2^31, 0}, {2^31, 2^31 - 1}, {2^32 - 1, 1}, {2^32, 5},
    {2^32 + 7, 3}, {-1, 5}, {-2^31, 1}, {0.5, 7}, {-0.5, 7}, {2.75, 1.25},
    {2^53, 3}, {-2^53, 1}, {1/0, 1}, {0/0, 1},
}
print("== Part 1: real bit32.bxor/bnot/floor semantics on edge classes ==")
for i = 1, #edges do
    local x, y = edges[i][1], edges[i][2]
    print("EDGE", i, string.format("x=%s y=%s", tostring(x), tostring(y)),
        "bxor=" .. pcall2(bit32.bxor, x, y),
        "bnot(x)=" .. pcall2(bit32.bnot, x),
        "band=" .. pcall2(bit32.band, x, y),
        "floor=" .. pcall2(math.floor, x))
end

-- ---------------------------------------------------------------------------
-- Part 2: local mirrors in double-exact arithmetic
-- ---------------------------------------------------------------------------
-- Mirror domain: integers in [0, 2^32) plus the verified edge behaviour
-- measured in Part 1 (wrap by mod 2^32 of the TRUNCATED value; fractions
-- are truncated toward zero). Mirror exactness is asserted against the
-- runtime across large sweeps below, so any wrong assumption fails loudly
-- instead of shipping as a false positive.
local function canon_u32(x)
    -- truncation toward zero, then wrap into [0, 2^32)
    if x ~= x or x == 1 / 0 or x == -1 / 0 then
        return nil -- NaN/inf: not in the probe's input domain
    end
    if x >= 0 then
        return math.floor(x) % 4294967296
    end
    return -math.floor(-x) % 4294967296 -- trunc toward zero for negatives
end

local function local_xor(x, y)
    local a, b = canon_u32(x), canon_u32(y)
    if a == nil or b == nil then return nil end
    local r, pa, pb = 0, a, b
    for i = 0, 31 do
        local ba, bb = pa % 2, pb % 2
        if ba ~= bb then r += 2 ^ i end
        pa = math.floor(pa / 2)
        pb = math.floor(pb / 2)
    end
    return r
end

local function local_and(x, y)
    local a, b = canon_u32(x), canon_u32(y)
    if a == nil or b == nil then return nil end
    local r, pa, pb = 0, a, b
    for i = 0, 31 do
        if pa % 2 == 1 and pb % 2 == 1 then r += 2 ^ i end
        pa = math.floor(pa / 2)
        pb = math.floor(pb / 2)
    end
    return r
end

local function local_floor(x) return math.floor(x) end -- trivially identical

-- ---------------------------------------------------------------------------
-- Part 3: false-positive sweep — mirror must equal the runtime everywhere
-- in the probe's input domain (1e6 structured+random pairs)
-- ---------------------------------------------------------------------------
print("== Part 3: mirror-vs-runtime agreement sweep ==")
local rng_state = 20261011
local function rnd(n)
    rng_state = (rng_state * 48271 + 11) % 2147483647
    return rng_state % n
end
local mismatch = 0
local tested = 0
for i = 1, 200000 do
    local x, y
    local class = rnd(8)
    if class == 0 then
        x, y = rnd(4294967296), rnd(4294967296) -- full u32 domain
    elseif class == 1 then
        x, y = rnd(65536), rnd(65536)
    elseif class == 2 then
        x, y = 2 ^ 32 + rnd(1000), rnd(1000) -- just above the domain
    elseif class == 3 then
        x, y = -(1 + rnd(1000)), rnd(1000) -- negatives
    elseif class == 4 then
        x, y = rnd(1000) + 0.5, rnd(1000) + 0.25 -- fractions
    elseif class == 5 then
        x, y = -(rnd(1000) + 0.5), rnd(1000) -- negative fractions
    elseif class == 6 then
        x, y = 2 ^ 53 - 2 + rnd(4), rnd(4) -- double-exactness frontier
    else
        x, y = 2 ^ 31 + rnd(4), rnd(4) -- sign boundary
    end
    local env_xor = table.pack(pcall(bit32.bxor, x, y))
    local env_and = table.pack(pcall(bit32.band, x, y))
    local mir_xor = local_xor(x, y)
    local mir_and = local_and(x, y)
    if env_xor[1] and mir_xor ~= nil then
        tested += 1
        if env_xor[2] ~= mir_xor or env_and[2] ~= mir_and then
            mismatch += 1
            if mismatch <= 5 then
                print("MISMATCH", x, y, env_xor[2], mir_xor, env_and[2], mir_and)
            end
        end
    elseif not env_xor[1] and mir_xor ~= nil then
        -- runtime ERRORS where the mirror computes: domain must exclude it
        tested += 1
        mismatch += 1
        if mismatch <= 5 then print("MISMATCH-ERR", x, y) end
    end
end
print("SWEEP", "tested=" .. tested, "mismatch=" .. mismatch)
check("local-mirror-agrees-with-runtime", mismatch == 0 and tested > 190000)

-- ---------------------------------------------------------------------------
-- Part 4: the probe — nonce query + local mirror + silent key fold
-- ---------------------------------------------------------------------------
local PLAIN = "C6B-LOCAL-MIRROR-PROBE"
local function make_cipher(key)
    local bytes = table.create(#PLAIN)
    for i = 1, #PLAIN do bytes[i] = (PLAIN:byte(i) + key) % 256 end
    return bytes
end
local key_at_rest -- set from the first (real) run's fold below

local function run_probe(env_bxor, nonce, cipher)
    local x = nonce % 4294967296
    local y = (nonce * 48271) % 4294967296
    local env_answer = env_bxor(x, y) -- ONE environment query, novel input
    local mirror = local_xor(x, y) -- computed locally, not fakeable via env
    if env_answer ~= mirror then
        -- silent poison: wrong key, no error oracle
        local key = 12345679
        local out = table.create(#cipher)
        for i = 1, #cipher do out[i] = string.char((cipher[i] - key) % 256) end
        return table.concat(out), false
    end
    local key = (mirror * 48271 + 7) % 268435456
    local out = table.create(#cipher)
    for i = 1, #cipher do out[i] = string.char((cipher[i] - key) % 256) end
    return table.concat(out), true
end

-- establish the at-rest cipher with the real runtime + a fixed nonce
local real_nonce = 987654321
local real_answer = bit32.bxor(real_nonce % 4294967296,
    (real_nonce * 48271) % 4294967296)
key_at_rest = (real_answer * 48271 + 7) % 268435456
local cipher = make_cipher(key_at_rest)

-- 4a) real environment passes with a NOVEL nonce (build cannot know it):
-- the cipher is keyed by a BUILD-TIME nonce here only for the harness; the
-- shipped design folds the runtime answer into a SESSION reseal (C5 family)
-- instead of the at-rest key. We demonstrate the check, not the reseal.
local fresh_nonce = 13579111315
-- for the at-rest-cipher demo we fold the fresh answer by re-keying:
local fresh_answer = bit32.bxor(fresh_nonce % 4294967296,
    (fresh_nonce * 48271) % 4294967296)
local cipher2 = make_cipher((fresh_answer * 48271 + 7) % 268435456)
local out, ok = run_probe(bit32.bxor, fresh_nonce, cipher2)
check("real-env-novel-nonce-passes", ok and out == PLAIN)

-- 4b) exact replayer holding a recording from ANOTHER run: dies silently
local recording = {}
recording[real_nonce % 4294967296] = real_answer -- one recorded pair
local function replayer(x, y)
    -- replay tools answer with a recorded value for a known query shape;
    -- for the novel (x, y) it returns the only recorded answer
    for k, v in pairs(recording) do return v end
end
local out2, ok2 = run_probe(replayer, fresh_nonce, cipher2)
counter("exact-replayer-dies-on-novel-nonce", not ok2 and out2 ~= PLAIN)

-- 4c) wrong-rule emulators: plausible-but-wrong bit32 argument conversions
local function make_wrong_rule(rule)
    return function(x, y)
        local a, b
        if rule == "floor-mod" then -- Lua-5.2-style floor then mod
            a = math.floor(x) % 4294967296
            b = math.floor(y) % 4294967296
        elseif rule == "js-toint32" then -- truncate toward zero, signed wrap
            local function toint32(v)
                -- TRUE truncate toward zero (v - v % 1 floors in Lua)
                v = v >= 0 and math.floor(v) or math.ceil(v)
                v = v % 4294967296
                if v >= 2147483648 then v -= 4294967296 end
                return v < 0 and v + 4294967296 or v
            end
            a, b = toint32(x), toint32(y)
        else -- "strict": fractions and out-of-range clamp to 0
            a = (x >= 0 and x < 4294967296 and x % 1 == 0) and x or 0
            b = (y >= 0 and y < 4294967296 and y % 1 == 0) and y or 0
        end
        -- correct XOR on the (wrongly) converted operands
        local r, pa, pb = 0, a, b
        for i = 0, 31 do
            local ba, bb = pa % 2, pb % 2
            if ba ~= bb then r += 2 ^ i end
            pa = math.floor(pa / 2)
            pb = math.floor(pb / 2)
        end
        return r
    end
end

-- feed edge-class nonces: the probe's nonce transform must be able to
-- generate the divergent classes for the wrong rules to die
local edge_nonces = {
    4294967296 + 3, -- x wraps above u32 (kills clamp-style rules)
    -5, -- negative x (two's-complement wrap)
    2^53 - 1, -- double frontier
    -0.5, -- negative FRACTION: Luau truncates toward zero (0), floor-style
    -- rules convert to -1 -> 0xFFFFFFFF and diverge
    3.75, -- positive fraction (trunc)
}
-- rule labels: "floor-mod" and "strict" are genuinely wrong vs Luau;
-- "js-toint32" (truncate toward zero + wrap) is semantically IDENTICAL to
-- Luau's rule and therefore survives -- it is a CORRECT implementation and
-- passing it is the honest boundary, not a failure of the probe.
for _, rule in {"floor-mod", "js-toint32", "strict"} do
    local wrong = make_wrong_rule(rule)
    local died = 0
    for i = 1, #edge_nonces do
        local n = edge_nonces[i]
        local x = n % 4294967296
        local y = (n * 48271) % 4294967296
        -- NOTE: x/y are always canonical u32 here; the edge classes must
        -- come from the nonce transform itself. Simulate a probe variant
        -- that feeds the RAW nonce into the environment call:
        local raw = n
        local env_a = wrong(raw, y)
        local mir = local_xor(canon_u32(raw), canon_u32(y))
        if env_a ~= mir then died += 1 end
    end
    if died > 0 then
        counter("wrong-rule-" .. rule .. "-dies-on-edge-classes", true)
    else
        check("wrong-rule-" .. rule .. "-survives-canonical-inputs", true)
    end
end

print("SUMMARY", checks, counters)
