-- C5 round-2 Luau stress: error/cancel paths, 32-coroutine random schedule,
-- master immutability, and cost scaling by block size. Research only.
local MOD = 268435456
local LANES = 5

local function lcg(seed)
    local s = seed
    return function()
        s = (s * 48271 + 11) % MOD
        return s
    end
end

local function newplain(rng, words)
    local lanes = table.create(LANES)
    for l = 1, LANES do
        lanes[l] = table.create(words)
        for w = 1, words do lanes[l][w] = rng() % MOD end
    end
    return lanes
end

local function newframe(secret, plain, key)
    local f = {secret = secret, digest = 11, chain = key, touched = false,
               cipher = table.create(LANES), words = #plain[1]}
    for l = 1, LANES do
        f.cipher[l] = table.create(f.words)
        for w = 1, f.words do f.cipher[l][w] = (plain[l][w] + key) % MOD end
    end
    return f
end

local function entrykey(f)
    return (f.chain * 48271 + f.secret) % MOD
end

local function visit(f, exec)
    -- 'touched' flips only AFTER a successful re-encode: an interrupted visit
    -- (error or yield mid-execute) must leave the frame in its pre-visit
    -- state so a retry decodes with the same (literal build) key again.
    local key = f.touched and entrykey(f) or f.chain
    local words = f.words
    local acc = 0
    for l = 1, LANES do
        local src = f.cipher[l]
        for w = 1, words do
            local p = (src[w] - key) % MOD
            if l == 1 then acc += p end
        end
    end
    if exec then exec() end -- may yield or error mid-visit
    f.digest = (f.digest * 48271 + acc % 131) % MOD
    f.chain = (f.chain * 48271 + f.digest * 31 + f.secret) % MOD
    local outkey = entrykey(f)
    for l = 1, LANES do
        local src = f.cipher[l]
        for w = 1, words do src[w] = ((src[w] - key) + outkey) % MOD end
    end
    f.touched = true
    return acc
end

local checks = 0
local function check(name, ok)
    assert(ok, name)
    checks += 1
    print("PASS", name)
end

local rng = lcg(20261010)
local plain = newplain(rng, 32)
local key = 1234567
local truth = 0
for w = 1, 32 do truth += plain[1][w] end

-- 1) error mid-execute: no re-encode, next visit still correct
local fe = newframe(777, plain, key)
local boom = {reason = "mid-execute"}
local ok1, e1 = pcall(visit, fe, function() error(boom, 0) end)
check("mid-execute-error-propagates", not ok1 and rawequal(e1, boom))
check("post-error-revisit-correct", visit(fe) == truth)
check("revisit-after-revisit-correct", visit(fe) == truth)

-- 2) coroutine.close mid-visit: frame abandoned; others unaffected; fresh frame fine
local fx = newframe(31337, plain, key)
local co = coroutine.create(function() visit(fx, function() coroutine.yield("paused") end) end)
local r = table.pack(coroutine.resume(co))
check("yield-inside-visit-pauses", r[1] and r[2] == "paused")
check("close-mid-visit-succeeds", coroutine.close(co))
local fy = newframe(2718, plain, key)
check("other-frame-unaffected", visit(fy) == truth)
local fz = newframe(99991, plain, key)
check("fresh-frame-after-cancel", visit(fz) == truth)

-- 3) master ciphertext never mutated by any frame (first-touch copy semantics)
local master = newframe(4242, plain, key)
local snapshot = table.create(LANES)
for l = 1, LANES do
    snapshot[l] = table.create(master.words)
    for w = 1, master.words do snapshot[l][w] = master.cipher[l][w] end
end
local fm = newframe(555, plain, key)
visit(fm); visit(fm); visit(fm)
local unchanged = true
for l = 1, LANES do
    for w = 1, master.words do
        if master.cipher[l][w] ~= snapshot[l][w] then unchanged = false end
    end
end
check("master-cipher-never-mutated", unchanged)

-- 4) 32 coroutines, random scheduling, independent frames on one block
local frames, cos = {}, {}
for i = 1, 32 do
    frames[i] = newframe(1000 + i * 7919, plain, key)
    cos[i] = coroutine.create(function()
        for round = 1, 60 do
            local acc = visit(frames[i])
            assert(acc == truth, "coroutine frame corrupted")
            coroutine.yield(acc)
        end
        return truth -- final return: the draining resume also carries a value
    end)
end
local all_ok, total_resumes, remaining = true, 0, 32
-- multiplication reduction over the FULL LCG value (rng.rs discipline):
-- `% 32` on a Park-Miller stream only cycles through 4 residues because
-- 48271 ≡ 15 (mod 32) and x -> 15x+11 (mod 32) has period 4.
local pick = lcg(987654)
local function pick32()
    return 1 + math.floor(pick() * 32 / 268435456)
end
while remaining > 0 do
    local i = pick32()
    if coroutine.status(cos[i]) == "suspended" then
        local out = table.pack(coroutine.resume(cos[i]))
        all_ok = all_ok and out[1] and out[2] == truth
        total_resumes += 1
        if coroutine.status(cos[i]) == "dead" then remaining -= 1 end
    end
end
check("32-coroutines-random-schedule", all_ok and total_resumes == 32 * 61)

-- 5) cost scaling by block size: baseline re-mask vs candidate decode+re-encode
local function bench(mode, words, iters)
    local lanes = table.create(LANES)
    local block = table.create(LANES)
    for l = 1, LANES do
        lanes[l] = table.create(words)
        block[l] = table.create(words)
        for w = 1, words do
            block[l][w] = (w * 2654435761 + l) % MOD
            lanes[l][w] = mode == 0 and block[l][w] or (block[l][w] + 424243) % MOD
        end
    end
    local mask, ckey, chain, digest, secret = 424243, 424243, 424243, 11, 777
    local total = 0
    local t0 = os.clock()
    for iter = 1, iters do
        if mode == 0 then
            local nm = (mask + iter) % MOD
            for l = 1, LANES do
                local lane = lanes[l]
                for w = 1, words do lane[w] = (lane[w] - mask + nm) % MOD end
            end
            mask = nm
            total += lanes[1][1 + iter % words]
        else
            digest = (digest * 48271 + iter % 131) % MOD
            chain = (chain * 48271 + digest * 31 + secret) % MOD
            local kout = (chain * 48271 + secret) % MOD
            for l = 1, LANES do
                local lane = lanes[l]
                for w = 1, words do
                    local p = (lane[w] - ckey) % MOD
                    lane[w] = (p + kout) % MOD
                end
            end
            ckey = kout
            total += 1
        end
    end
    return os.clock() - t0, total
end

for _, words in {8, 32, 128} do
    local iters = words == 128 and 4000 or 12000
    local ratios = {}
    for round = 1, 9 do
        local t0 = bench(0, words, iters)
        local t1 = bench(1, words, iters)
        ratios[round] = t1 / t0
    end
    table.sort(ratios)
    print("SCALE", words, ratios[5], ratios[1], ratios[9])
end
print("SUMMARY", checks)
