-- C4: research-only session micro-record reseal model, NOT actual VM encoding.
-- Rotation below is intentionally simple to falsify the claim that more
-- generations alone imply stronger anti-debugging.
local BYTE, CHAR = ("").byte, ("").char
local function crypt(s, k)
    local out = table.create(#s)
    for i = 1, #s do
        out[i] = CHAR((BYTE(s, i) + k) % 256)
    end
    return table.concat(out)
end
local function plain_blob(n)
    local out = table.create(n)
    for i = 1, n do out[i] = CHAR((i * 17 + 11) % 256) end
    return table.concat(out)
end
local function ck(name, cond)
    assert(cond, name)
    print("PASS", name)
end
local function counter(name, cond)
    assert(cond, name)
    print("COUNTEREXAMPLE", name)
end
local original = plain_blob(4096)
local key, step = 19, 37
local blob = crypt(original, key)
local captured_blob, captured_key = blob, key
for i = 1, 256 do
    local next_key = (key + step) % 256
    blob = crypt(blob, (next_key - key) % 256)
    key = next_key
    ck("rotation-correct-" .. i, crypt(blob, (-key) % 256) == original)
    if i == 1 then
        ck("old-key-fails-on-new-blob", crypt(blob, (-captured_key) % 256) ~= original)
        local predicted = (captured_key + step) % 256
        counter("known-update-predicts-next-key", crypt(blob, (-predicted) % 256) == original)
    end
end
counter("8bit-rotation-repeats-after-256", blob == captured_blob and key == captured_key)
counter("whole-live-snapshot-decodes-every-record", crypt(blob, (-key) % 256) == original)

-- Mixed-generation read around an allowed user callback. With >1 record
-- per descriptor, lazy iteration must not retain the old key while another
-- invocation rotates the same shard. Current R064 one-record recipes reduce
-- this case, but a design must enforce it or support safe reentry explicitly.
local k, s = 7, crypt("AB", 7)
local first = (BYTE(s, 1) - k) % 256
local saved_key = k
local function reentrant_callback()
    s = crypt(s, 3)
    k = (k + 3) % 256
end
reentrant_callback()
local second = (BYTE(s, 2) - saved_key) % 256
counter("lazy-multirecord-reader-mixes-generations", CHAR(first, second) ~= "AB")

-- Materializing both bytes before callback is consistent, but keeps the
-- complete plain record in local state across that callback: observation
-- window tradeoff, not a free concurrency fix.
k, s = 7, crypt("AB", 7)
local p1, p2 = (BYTE(s, 1) - k) % 256, (BYTE(s, 2) - k) % 256
reentrant_callback()
ck("predecode-before-callback-is-consistent", CHAR(p1, p2) == "AB")

-- 4KB / 16KB shard benchmarks. Immutable strings require copying the full
-- shard for a global-key rotation. Times include equivalent four-byte reads.
for _, size in {4096, 16384} do
    local plain = plain_blob(size)
    local stable = crypt(plain, 17)
    local checksum, t = 0, os.clock()
    for i = 1, 1000 do
        local offset = ((i * 4) % (size - 4)) + 1
        for j = 0, 3 do checksum += (BYTE(stable, offset + j) - 17) % 256 end
    end
    local stable_time = os.clock() - t
    local rolling, rk, checksum2 = stable, 17, 0
    t = os.clock()
    for i = 1, 1000 do
        local offset = ((i * 4) % (size - 4)) + 1
        for j = 0, 3 do checksum2 += (BYTE(rolling, offset + j) - rk) % 256 end
        rolling = crypt(rolling, 37)
        rk = (rk + 37) % 256
    end
    local rolling_time = os.clock() - t
    ck("benchmark-checksum-" .. size, checksum == checksum2)
    print("BENCH", size, stable_time, rolling_time, rolling_time / stable_time, size * 1000)
end
print("SUMMARY", "C4-not-integrated;no-strong-anti-debug-claim")
