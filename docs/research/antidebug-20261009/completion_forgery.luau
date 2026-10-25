-- Research-only falsification: does a valid interval receipt prove a call
-- executed? This is NOT a patcher for production targets.
local real_calls = 0
local function user(x)
    real_calls += 1
    return x * 2, nil
end
local E = {f = user, args = table.pack(21), pc = 9, family = 1, seq = 0, phase = "ready"}
local function perform(mode)
    local f, args, n, pc, family = E.f, E.args, E.args.n, E.pc, E.family
    local expected = E.seq + 1
    local ticket = {}
    E.seq, E.ticket, E.phase = expected, ticket, "calling"
    local r
    if mode == "skip" then
        -- Attack-model action: keep all bookkeeping, replace the semantic
        -- call with a fabricated result of the correct arity.
        r = table.pack(999, nil)
    else
        r = table.pack(f(table.unpack(args, 1, n)))
        if mode == "replace-result" then r[1] = 999 end
    end
    local accepted = rawequal(E.ticket, ticket) and E.seq == expected and E.phase == "calling"
        and rawequal(E.f, f) and rawequal(E.args, args) and E.args.n == n
        and E.pc == pc and E.family == family
    assert(accepted)
    E.ticket, E.phase = nil, "ready"
    return table.unpack(r, 1, r.n)
end
local original = table.pack(perform("clean"))
assert(original.n == 2 and original[1] == 42 and real_calls == 1)
print("PASS", "clean-completion")
local skipped = table.pack(perform("skip"))
assert(skipped.n == 2 and skipped[1] == 999 and real_calls == 1)
print("COUNTEREXAMPLE", "all-receipt-checks-pass-without-calling-user")
local altered = table.pack(perform("replace-result"))
assert(altered.n == 2 and altered[1] == 999 and real_calls == 2)
print("COUNTEREXAMPLE", "all-receipt-checks-pass-after-result-replacement")
print("SUMMARY", 1, 2)
