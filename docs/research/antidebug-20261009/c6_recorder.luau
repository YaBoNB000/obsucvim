-- C6 Luau confirmation: recorder/replayer matrix against REAL runtime
-- functions (bit32 / string / math). Research only; not the production VM.
local MOD = 268435456

local function fold(...)
    local out = 0
    for i = 1, select("#", ...) do
        out = (out * 48271 + select(i, ...)) % MOD
    end
    return out
end

local PLAIN = "C6-RESEARCH-PAYLOAD"
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

-- environments ------------------------------------------------------------
local function RealEnv()
    local state = {}
    return {
        bxor = function(a, b) return bit32.bxor(a, b) end,
        band = function(a, b) return bit32.band(a, b) end,
        replen = function(s, n) return #string.rep(s, n) end,
        floor = function(x) return math.floor(x) end,
        put = function(k, v) state[k] = v end,
        get = function(k) return state[k] end,
    }
end

local function record_from(env)
    local rec = {}
    local wrapper = {}
    for name, fn in pairs(env) do
        wrapper[name] = function(...)
            local answer = fn(...)
            rec[name] = answer -- single-slot recording (last answer per name)
            return answer
        end
    end
    return wrapper, rec
end

local function ExactReplayer(rec)
    return setmetatable({}, {__index = function(_, name)
        return function(...) return rec[name] end
    end})
end

local function CombinedFake(rec)
    -- constants answered from the table; novel bxor/band queries answered
    -- with the same recorded value (attacker reversed the pairing)
    return setmetatable({}, {__index = function(_, name)
        return function(...) return rec[name] end
    end})
end

local function StatefulFake(rec)
    local state = {}
    return setmetatable({}, {__index = function(_, name)
        if name == "put" then return function(k, v) state[k] = v end end
        if name == "get" then return function(k) return state[k] end end
        return function(...) return rec[name] end
    end})
end

-- program ------------------------------------------------------------------
-- constant probes (build-time mirrorable): the at-rest cipher is keyed with
-- the REAL answers; runtime must reproduce them to decode.
local function make_cipher(key)
    local bytes = table.create(#PLAIN)
    for i = 1, #PLAIN do bytes[i] = (PLAIN:byte(i) + key) % 256 end
    return bytes
end

local function decode(env, cipher, extra_gate)
    -- missing answers poison the key SILENTLY (no error oracle)
    local a1 = (env.bxor(0xDEADBEEF, 0x12345678) or 0) % MOD
    local a2 = env.replen("ab", 7) or 0
    local a3 = env.floor(2.5) or 0
    local nonce = 0
    do -- run-random nonce from a fresh table address (SALT-like)
        local s = tostring({})
        for i = 1, #s do nonce = (nonce * 131 + s:byte(i)) % 2147483647 end
    end
    local poison = false
    if extra_gate then
        poison = extra_gate(env, nonce)
    end
    local key = fold(a1, a2, a3)
    if poison then key = (key + 12345679) % MOD end
    local out = table.create(#cipher)
    for i = 1, #cipher do out[i] = string.char((cipher[i] - key) % 256) end
    return table.concat(out)
end

local real = RealEnv()
local recorded_env, rec = record_from(real)
local key_at_rest = fold(
    bit32.bxor(0xDEADBEEF, 0x12345678) % MOD,
    real.replen("ab", 7),
    real.floor(2.5))
local cipher = make_cipher(key_at_rest)

-- 1) exact single-slot replay happens to answer all three constants right
-- (they are queried once, in this order) -> recording defeats verification
check("p1_recorded_replay_succeeds", decode(recorded_env, cipher) == PLAIN)

-- 2) one missing constant -> silent garbage
local rec_missing = {bxor = rec.bxor, replen = rec.replen}
check("p4_missing_probe_garbles", decode(ExactReplayer(rec_missing), cipher) ~= PLAIN)

-- 3) dual-path consistency on a NOVEL nonce: two real routes must agree
local function dual_gate(env, nonce)
    local x = math.floor(nonce / 2) % 65536
    local y = (nonce % 65536)
    local r1 = env.bxor(x, y)                     -- route 1: bit32.bxor
    local b = env.band(x, y)                      -- route 2: x+y-2*(x&y)
    if r1 == nil or b == nil then return true end
    return r1 ~= (x + y) - 2 * b
end
check("p2_novel_dual_path_in_real_env_ok",
      decode(RealEnv(), cipher, dual_gate) == PLAIN)
check("p2_exact_replay_fails_novel_query",
      decode(ExactReplayer(rec), cipher, dual_gate) ~= PLAIN)
-- combined fake: recorded constants + same value for bxor and band routes.
-- The identity x^y == x+y-2*(x&y) fails with the same substituted value on
-- the right side unless the fake is genuinely consistent; give the fake the
-- real recorded pair (rec.bxor, rec.band from the same real run) and verify.
local rec_pair = record_from(RealEnv())
local _, pair = record_from(RealEnv())
local fake2 = CombinedFake({bxor = pair.bxor, band = nil, replen = rec.replen,
                            floor = rec.floor})
-- band missing -> route 2 errors on nil arithmetic -> poison (coverage)
check("p2_fake_missing_band_fails",
      decode(fake2, cipher, dual_gate) ~= PLAIN)

-- an attacker who IMPLEMENTS both routes consistently (emulator) passes:
local function EmulatorEnv()
    local state = {}
    return {
        bxor = function(a, b)
            local r, x, y = 0, a, b
            for i = 0, 31 do
                local xb, yb = x % 2, y % 2
                if xb ~= yb then r += 2 ^ i end
                x = math.floor(x / 2); y = math.floor(y / 2)
            end
            return r
        end,
        band = function(a, b)
            local r, x, y = 0, a, b
            for i = 0, 31 do
                if x % 2 == 1 and y % 2 == 1 then r += 2 ^ i end
                x = math.floor(x / 2); y = math.floor(y / 2)
            end
            return r
        end,
        replen = function(s, n) return #s * n end,
        floor = function(x) return math.floor(x) end,
        put = function(k, v) state[k] = v end,
        get = function(k) return state[k] end,
    }
end
check("p2_semantic_emulator_passes",
      decode(EmulatorEnv(), cipher, dual_gate) == PLAIN)

-- 4) stateful probe kills stateless replay
local function state_gate(env, nonce)
    local marker = nonce % 1000000
    env.put("m", marker)
    return env.get("m") ~= marker
end
check("p5_stateless_replay_fails",
      decode(ExactReplayer(rec), cipher, state_gate) ~= PLAIN)
check("p5_stateful_fake_passes",
      decode(StatefulFake(rec), cipher, state_gate) == PLAIN)
check("p5_real_env_passes",
      decode(RealEnv(), cipher, state_gate) == PLAIN)

-- 5) false-positive demo: a "variant" environment whose floor differs by a
-- legitimately-possible epsilon (e.g. platform rounding) garbles a REAL user
local function VariantEnv()
    local base = RealEnv()
    base.floor = function(x) return math.floor(x + 0.5) end -- round-half-up
    return base
end
counter("p3_variant_env_legit_user_garbled", decode(VariantEnv(), cipher) ~= PLAIN)

print("SUMMARY", checks, counters)
