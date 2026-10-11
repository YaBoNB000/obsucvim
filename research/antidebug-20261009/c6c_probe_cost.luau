-- C6 plan appendix: boot-time probe cost measurement (research only).
local function canon(x)
    if x ~= x or x == 1/0 or x == -1/0 then return nil end
    if x >= 0 then return math.floor(x) % 4294967296 end
    return -math.floor(-x) % 4294967296
end
local function mirror_xor(x, y)
    local a, b = canon(x), canon(y)
    local r, pa, pb = 0, a, b
    for i = 0, 31 do
        if pa % 2 ~= pb % 2 then r += 2 ^ i end
        pa = math.floor(pa / 2); pb = math.floor(pb / 2)
    end
    return r
end
-- one boot = 3 probes (bxor + band + bnot families), each: 1 env call + 1 mirror
local function boot_probes(nonce)
    local x = (nonce % 4294967296) - 0.5      -- deliberate negative/positive fraction class
    local y = ((nonce * 48271) % 4294967296) + 4294967296 -- deliberate >2^32 class
    local ok = bit32.bxor(x, y) == mirror_xor(x, y)
    ok = ok and bit32.band(x, y) == (function()
        local a, b = canon(x), canon(y)
        local r, pa, pb = 0, a, b
        for i = 0, 31 do
            if pa % 2 == 1 and pb % 2 == 1 then r += 2 ^ i end
            pa = math.floor(pa / 2); pb = math.floor(pb / 2)
        end
        return r
    end)()
    return ok
end
local t = os.clock()
local n = 0
for i = 1, 100000 do if boot_probes(i * 7919) then n += 1 end end
local dt = os.clock() - t
print("BOOTS", 100000, "all_ok=" .. tostring(n == 100000))
print("PER_BOOT", dt / 100000, "seconds")
print("TOTAL", dt, "seconds")
