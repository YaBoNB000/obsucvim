-- Cost/ABA tradeoff for research-only interval guards. Never deployed.
local PACK, UNPACK, EQ = table.pack, table.unpack, rawequal
local function invoke(E, mode)
    local f, args, n, pc, family = E.f, E.args, E.args.n, E.pc, E.family
    local expected, ticket
    if mode > 0 then
        assert(E.phase == 0)
        expected = (E.seq + 1) % 65536
        E.seq, E.phase = expected, 1
        if mode == 2 then ticket = {}; E.ticket = ticket end
    end
    local r = PACK(f(UNPACK(args, 1, n)))
    if mode > 0 then
        assert(E.seq == expected and E.phase == 1 and E.pc == pc and E.family == family)
        assert(E.args.n == n and EQ(E.args, args) and EQ(E.f, f))
        if mode == 2 then assert(EQ(E.ticket, ticket)); E.ticket = nil end
        E.phase = 0
    end
    return UNPACK(r, 1, r.n)
end
local function f(x) return x + 1, nil end
local function frame()
    return {f = f, args = PACK(7), pc = 11, family = 1, seq = 0, phase = 0}
end
for mode = 0, 2 do
    local E = frame()
    local values = PACK(invoke(E, mode))
    assert(values.n == 2 and values[1] == 8 and values[2] == nil)
    for _, field in {"seq", "pc", "family"} do
        if mode > 0 then
            local X = frame()
            X.f = function() X[field] += 1; return 8, nil end
            assert(not pcall(invoke, X, mode))
        end
    end
    print("PASS", "basic-and-persistent-mutation-mode-" .. mode)
end

-- Replay only the visible completion state, NOT the receiver's private
-- expected copy. A finite sequence repeats; an old retained object does not
-- compare equal to the fresh object issued to the new invocation.
local old_seq, old_ticket = 1, {}
local current_seq = 0
for _ = 1, 65537 do current_seq = (current_seq + 1) % 65536 end
local current_ticket = {}
assert(old_seq == current_seq)
assert(not EQ(old_ticket, current_ticket))
print("COUNTEREXAMPLE", "16-bit-generation-ABA-after-65536-more-calls")
print("PASS", "retained-old-object-distinct-from-current-ticket")
local max = 2^53
assert(max + 1 == max)
print("COUNTEREXAMPLE", "unbounded-double-generation-eventually-stops-incrementing")
-- Wider counters postpone ABA. They are not secret entropy or a solution
-- to whole-state rollback, and numeric formulas may have precision hazards.

local function bench(mode)
    local E = frame()
    local t, sum = os.clock(), 0
    for _ = 1, 80000 do sum += invoke(E, mode) end
    assert(sum == 640000)
    return os.clock() - t
end
for round = 1, 12 do
    local elapsed = {}
    -- Rotate modes to reduce systematic warmup/order bias.
    for delta = 0, 2 do
        local mode = (round + delta) % 3
        elapsed[mode + 1] = bench(mode)
    end
    print("BENCH", round, elapsed[1], elapsed[2], elapsed[3], elapsed[2]/elapsed[1], elapsed[3]/elapsed[1])
end
print("SUMMARY", "microbenchmark only; full-VM security and memory remain unverified")
