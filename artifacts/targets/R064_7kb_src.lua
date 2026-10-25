-- R064 ~7KB regression source: local VM semantics canaries, deterministic stdout.
-- Intentionally uses only standard Lua/Luau constructs; no Roblox or network APIs.
local MOD = 1000003

local function shift_byte(b, delta, lo, hi)
    if b >= lo and b <= hi then
        return lo + ((b - lo + delta) % 26)
    end
    return b
end

local function rotate_text(text, delta)
    local out = {}
    for i = 1, #text do
        local b = string.byte(text, i)
        b = shift_byte(b, delta, 65, 90)
        b = shift_byte(b, delta, 97, 122)
        out[i] = string.char(b)
    end
    return table.concat(out)
end

local function fold_text(text, seed)
    local h = seed % MOD
    for i = 1, #text do
        h = (h * 257 + string.byte(text, i) + i * 31) % MOD
    end
    return h
end

local function make_accumulator(seed)
    local state = seed % MOD
    local calls = 0
    return function(step, ...)
        calls = calls + 1
        local n = select("#", ...)
        for i = 1, n do
            local value = select(i, ...)
            state = (state + fold_text(tostring(value), step + i * 13)) % MOD
        end
        return state, calls, n
    end
end

local function make_counter(start)
    local value = start % MOD
    local function add(delta)
        value = (value + delta) % MOD
        return value
    end
    local function read()
        return value
    end
    return add, read
end

local function pack_values(...)
    local n = select("#", ...)
    local values = {}
    for i = 1, n do
        values[i] = select(i, ...)
    end
    return values, n
end

local function safe_ratio(a, b)
    if b == 0 then
        return nil, "zero"
    end
    return a / b, "ok"
end

local function make_ring(capacity)
    local data = {}
    local first, last, count = 1, 0, 0
    local function push(value)
        last = last + 1
        data[last] = value
        count = count + 1
        if count > capacity then
            data[first] = nil
            first = first + 1
            count = count - 1
        end
        return count
    end
    local function drain()
        local out = {}
        while first <= last do
            if data[first] ~= nil then
                out[#out + 1] = data[first]
            end
            data[first] = nil
            first = first + 1
        end
        last, count = first - 1, 0
        return out
    end
    return { push = push, drain = drain }
end

local function make_tree(records, lo, hi)
    if lo > hi then
        return nil
    end
    local mid = math.floor((lo + hi) / 2)
    local row = records[mid]
    return {
        key = row[2],
        value = fold_text(row[3], row[5]),
        left = make_tree(records, lo, mid - 1),
        right = make_tree(records, mid + 1, hi),
    }
end

local function visit_tree(node, depth, meter)
    if node == nil then
        return 0
    end
    local left = visit_tree(node.left, depth + 1, meter)
    local right = visit_tree(node.right, depth + 1, meter)
    local state = meter(depth + node.value % 19, node.key, node.value, left, right)
    return (state + left * 3 + right * 5 + node.value) % MOD
end

local function summarize(records)
    local meter = make_accumulator(271828)
    local ring = make_ring(9)
    local encoded = {}
    local total, count = 0, 0
    local histogram = setmetatable({}, {
        __index = function(_, key)
            return 0
        end,
    })
    for i = 1, #records do
        local row = records[i]
        local wire = rotate_text(row[3], row[1] % 26)
        local plain = rotate_text(wire, -(row[1] % 26))
        if plain ~= row[3] then
            error("roundtrip mismatch", 0)
        end
        local h = fold_text(wire, row[5])
        local state, callno, argc = meter(row[4], row[1], row[2], h, row[3])
        total = (total + h + state + callno * argc + i * row[4]) % MOD
        local prefix = row[2]:sub(1, 2)
        histogram[prefix] = histogram[prefix] + 1
        ring.push(h)
        encoded[#encoded + 1] = wire
        count = count + 1
    end
    local queue = ring.drain()
    local hist_sum = 0
    for key, value in pairs(histogram) do
        hist_sum = (hist_sum + fold_text(key, value)) % MOD
    end
    local tree = make_tree(records, 1, #records)
    local tree_value = visit_tree(tree, 0, meter)
    local closure, read = make_counter(total)
    closure(tree_value)
    closure(hist_sum)
    return total, count, tree_value, read(), table.concat(encoded, ":"), queue
end

local RECORDS = {
    {1, "teal-01", "scan:01|teal:r01|w=049|s=00108", 49, 108},
    {2, "cyan-02", "branch:02|cyan:r02|w=086|s=00199", 86, 199},
    {3, "amber-03", "rotate:03|amber:r03|w=123|s=00290", 123, 290},
    {4, "teal-04", "cache:04|teal:r04|w=160|s=00381", 160, 381},
    {5, "cyan-05", "fold:05|cyan:r05|w=197|s=00472", 197, 472},
    {6, "amber-06", "scan:06|amber:r06|w=234|s=00563", 234, 563},
    {7, "teal-07", "branch:07|teal:r07|w=271|s=00654", 271, 654},
    {8, "cyan-08", "rotate:08|cyan:r08|w=308|s=00745", 308, 745},
    {9, "amber-09", "cache:09|amber:r09|w=345|s=00836", 345, 836},
    {10, "teal-10", "fold:10|teal:r10|w=382|s=00927", 382, 927},
    {11, "cyan-11", "scan:11|cyan:r11|w=419|s=01018", 419, 1018},
    {12, "amber-12", "branch:12|amber:r12|w=456|s=01109", 456, 1109},
    {13, "teal-13", "rotate:13|teal:r00|w=493|s=01200", 493, 1200},
    {14, "cyan-14", "cache:14|cyan:r01|w=530|s=01291", 530, 1291},
    {15, "amber-15", "fold:15|amber:r02|w=567|s=01382", 567, 1382},
    {16, "teal-16", "scan:16|teal:r03|w=604|s=01473", 604, 1473},
    {17, "cyan-17", "branch:17|cyan:r04|w=641|s=01564", 641, 1564},
    {18, "amber-18", "rotate:18|amber:r05|w=678|s=01655", 678, 1655},
    {19, "teal-19", "cache:19|teal:r06|w=715|s=01746", 715, 1746},
    {20, "cyan-20", "fold:20|cyan:r07|w=752|s=01837", 752, 1837},
    {21, "amber-21", "scan:21|amber:r08|w=789|s=01928", 789, 1928},
    {22, "teal-22", "branch:22|teal:r09|w=826|s=02019", 826, 2019},
    {23, "cyan-23", "rotate:23|cyan:r10|w=863|s=02110", 863, 2110},
    {24, "amber-24", "cache:24|amber:r11|w=900|s=02201", 900, 2201},
    {25, "teal-25", "fold:25|teal:r12|w=937|s=02292", 937, 2292},
    {26, "cyan-26", "scan:26|cyan:r00|w=974|s=02383", 974, 2383},
    {27, "amber-27", "branch:27|amber:r01|w=014|s=02474", 14, 2474},
    {28, "teal-28", "rotate:28|teal:r02|w=051|s=02565", 51, 2565},
    {29, "cyan-29", "cache:29|cyan:r03|w=088|s=02656", 88, 2656},
    {30, "amber-30", "fold:30|amber:r04|w=125|s=02747", 125, 2747},
    {31, "teal-31", "scan:31|teal:r05|w=162|s=02838", 162, 2838},
    {32, "cyan-32", "branch:32|cyan:r06|w=199|s=02929", 199, 2929},
}

local total, count, tree_value, state, joined, queue = summarize(RECORDS)
local packed, packed_n = pack_values(nil, "tail", false, total)
local ratio, ratio_status = safe_ratio(total % 997, count)
local fallback = setmetatable({ live = 3 }, {
    __index = function(_, key)
        return #tostring(key)
    end,
})
local queue_hash = 0
for i = 1, #queue do
    queue_hash = (queue_hash + queue[i] * i) % MOD
end
print("R064_7KB", count, total, tree_value, state, packed_n, ratio_status,
      fallback.missing, queue_hash, fold_text(joined, 19))
