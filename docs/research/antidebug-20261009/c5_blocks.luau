-- C5 Luau model: per-frame self-resealing block windows (research only).
-- NOT the production VM; no production file is loaded or modified.
local MOD = 268435456 -- 2^28 (all intermediates stay < 2^53)
local LANES, WORDS = 5, 32

local function newblock(rng)
    local lanes = {}
    for l = 1, LANES do
        lanes[l] = table.create(WORDS)
        for w = 1, WORDS do lanes[l][w] = rng() % MOD end
    end
    return lanes
end

local function newframe(secret, blocks, keys)
    local f = {secret = secret, digest = 11, cipher = {}, chain = {}}
    for b, plain in pairs(blocks) do
        -- at-rest encoding with the per-block build key
        local lanes = table.create(LANES)
        for l = 1, LANES do
            lanes[l] = table.create(WORDS)
            for w = 1, WORDS do lanes[l][w] = (plain[l][w] + keys[b]) % MOD end
        end
        f.cipher[b] = lanes
        f.chain[b] = keys[b]
        f.touched = f.touched or {}
        f.touched[b] = false
    end
    return f
end

local function entrykey(f, b)
    return (f.chain[b] * 48271 + f.secret) % MOD
end

-- decode -> execute -> re-encode with a chain folded from (digest, secret)
-- touched[b] flips only after a successful re-encode (idempotent retry).
local function visit(f, b, exec)
    local key
    if f.touched[b] then
        key = entrykey(f, b)
    else
        key = f.chain[b] -- literal build key on first touch
    end
    local lanes = f.cipher[b]
    local scratch = table.create(LANES)
    for l = 1, LANES do
        scratch[l] = table.create(WORDS)
        local src, dst = lanes[l], scratch[l]
        for w = 1, WORDS do dst[w] = (src[w] - key) % MOD end
    end
    local acc = exec and exec(scratch) or 0
    f.digest = (f.digest * 48271 + b * 131 + acc) % MOD
    f.chain[b] = (f.chain[b] * 48271 + f.digest * 31 + f.secret) % MOD
    local outkey = entrykey(f, b)
    for l = 1, LANES do
        local src, dst = scratch[l], lanes[l]
        for w = 1, WORDS do dst[w] = (src[w] + outkey) % MOD end
    end
    f.touched[b] = true
    return acc, scratch
end

-- deterministic rng (LCG) so the model is reproducible
local function lcg(seed)
    local s = seed
    return function()
        s = (s * 48271 + 11) % MOD
        return s
    end
end

local checks, counterexamples = 0, 0
local function check(name, ok)
    assert(ok, name)
    checks += 1
    print("PASS", name)
end
local function counter(name, ok)
    assert(ok, name)
    counterexamples += 1
    print("COUNTEREXAMPLE", name)
end

local rng = lcg(20261009)
local plainblocks = {}
for b = 1, 4 do plainblocks[b] = newblock(rng) end
local keys = {[1] = 1234567, [2] = 7654321, [3] = 112233445 % MOD, [4] = 998877665 % MOD}

-- ground truth executor: sum of a fixed lane
local function execsum(scratch) local t = scratch[1] local s = 0 for w = 1, WORDS do s += t[w] end return s end
local truth = {}
for b = 1, 4 do truth[b] = execsum(plainblocks[b]) end

-- 1) loop revisit round-trip: same plaintext every visit
local f = newframe(777, plainblocks, keys)
local a1 = select(1, visit(f, 2, execsum))
local a2 = select(1, visit(f, 2, execsum))
local a3 = select(1, visit(f, 2, execsum))
check("loop-revisit-round-trip", a1 == truth[2] and a2 == truth[2] and a3 == truth[2])

-- 2) two coroutines, independent frames, interleaved over the same blocks
local coA = coroutine.create(function()
    for i = 1, 40 do
        local acc = select(1, visit(f, 3, execsum))
        assert(acc == truth[3], "coA decode corrupted")
        coroutine.yield(acc)
    end
end)
local fB = newframe(31415, plainblocks, keys)
local coB = coroutine.create(function()
    for i = 1, 40 do
        local acc = select(1, visit(fB, 3, execsum))
        assert(acc == truth[3], "coB decode corrupted")
        coroutine.yield(acc)
    end
end)
local oka, okb = true, true
for i = 1, 40 do
    local r1 = table.pack(coroutine.resume(coA)); oka = oka and r1[1] and r1[2] == truth[3]
    local r2 = table.pack(coroutine.resume(coB)); okb = okb and r2[1] and r2[2] == truth[3]
end
check("coroutine-interleave-both-frames-stable", oka and okb)

-- 3) stale cipher replay: snapshot before a visit, decode after the chain moved
local stale = table.create(LANES)
for l = 1, LANES do
    stale[l] = table.create(WORDS)
    for w = 1, WORDS do stale[l][w] = f.cipher[2][l][w] end
end
visit(f, 2, execsum) -- chain advanced, cipher re-encoded
local k = entrykey(f, 2)
local stale_acc = 0
for w = 1, WORDS do stale_acc += (stale[1][w] - k) % MOD end
counter("stale-cipher-replay-is-garbage", stale_acc ~= truth[2])

-- 4) cross-frame pair copy without the frame secret
local fD = {secret = 271828, digest = 5, chain = {[2] = f.chain[2]}, touched = {[2] = true},
            cipher = {[2] = f.cipher[2]}}
local kd = entrykey(fD, 2)
local dacc = 0
for w = 1, WORDS do dacc += (f.cipher[2][1][w] - kd) % MOD end
local racc = 0
local kr = entrykey(f, 2)
for w = 1, WORDS do racc += (f.cipher[2][1][w] - kr) % MOD end
counter("cross-frame-pair-copy-is-garbage", dacc ~= racc)

-- 5) full-state copy (cipher + chain + secret) still decodes: physical limit
local fE = {secret = f.secret, digest = 5, chain = {[2] = f.chain[2]}, touched = {[2] = true},
            cipher = {[2] = f.cipher[2]}}
local ke = entrykey(fE, 2)
local eacc = 0
for w = 1, WORDS do eacc += (f.cipher[2][1][w] - ke) % MOD end
counter("full-state-copy-still-decodes", eacc == racc)

-- 6) microbenchmark: existing re-mask-per-visit vs candidate decode+re-encode
-- Both keep a decoded window; baseline re-masks in place (H4-6B shape),
-- candidate decodes from and re-encodes to the per-frame cipher each visit.
local function bench(mode)
    local block = plainblocks[1]
    local mask, key = 424242, keys[1]
    -- vault lanes: baseline starts decoded; candidate starts encoded
    local lanes = table.create(LANES)
    for l = 1, LANES do
        lanes[l] = table.create(WORDS)
        for w = 1, WORDS do
            lanes[l][w] = mode == 0 and block[l][w] or (block[l][w] + key) % MOD
        end
    end
    local chain, digest, secret = key, 11, 777
    local total = 0
    local t0 = os.clock()
    for iter = 1, 20000 do
        for b = 1, 3 do -- hot loop body of three blocks
            if mode == 0 then
                local nm = (mask + iter) % MOD
                for l = 1, LANES do
                    local lane = lanes[l]
                    for w = 1, WORDS do lane[w] = (lane[w] - mask + nm) % MOD end
                end
                mask = nm
                total += lanes[1][b] -- same "execute" work
            else
                digest = (digest * 48271 + b * 131) % MOD
                chain = (chain * 48271 + digest * 31 + secret) % MOD
                local kout = (chain * 48271 + secret) % MOD
                for l = 1, LANES do
                    local lane = lanes[l]
                    for w = 1, WORDS do
                        local p = (lane[w] - key) % MOD
                        lane[w] = (p + kout) % MOD
                        if l == 1 and w == b then total += p end
                    end
                end
                key = kout
            end
        end
    end
    return os.clock() - t0, total
end

local results = {}
for round = 1, 9 do
    local t0, s0 = bench(0)
    local t1, s1 = bench(1)
    assert(s0 > 0 and s1 > 0)
    results[round] = {t0, t1, t1 / t0}
    print("BENCH", round, t0, t1, t1 / t0)
end
table.sort(results, function(a, b) return a[3] < b[3] end)
print("MEDIAN_RATIO", results[5][3])
print("SUMMARY", checks, counterexamples)
