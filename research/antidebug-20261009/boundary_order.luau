-- Ordering model derived from template.rs: trampoline clears callx before
-- CX; CX does user call -> postcall -> slot cleanup -> chain epilogue.
-- Synthetic model only; never loaded by production.
local function newframe()
    return {pc = 10, callx = 1, phase = "pending", seq = 1, ticket = {}}
end
local function chain(E)
    E.pc += 1
    -- A legitimate next CALL can be prepared before the current CX returns.
    E.callx, E.phase, E.seq, E.ticket = 2, "pending", E.seq + 1, {}
end
local function CX_bad(E)
    local results = table.pack((function() return 42, nil end)())
    E.lastn = results.n
    chain(E)
end
local E = newframe()
local before_pc, before_ticket = E.pc, E.ticket
E.callx = nil -- same order as existing trampoline
CX_bad(E)
assert(E.pc ~= before_pc and E.ticket ~= before_ticket and E.lastn == 2)
print("COUNTEREXAMPLE", "checking-after-entire-CX-flags-legitimate-next-CALL")
assert(E.phase == "pending" and E.callx == 2)
-- Clearing old state here would erase the new pending call, not the old one.
E.ticket, E.phase = nil, nil
assert(E.callx == 2 and E.ticket == nil)
print("COUNTEREXAMPLE", "late-cleanup-erases-next-generation-ticket")

local function CX_interval(E)
    assert(E.callx == nil)
    local ticket, seq, pc = E.ticket, E.seq, E.pc
    E.phase = "calling"
    local results = table.pack((function() return 42, nil end)())
    assert(E.ticket == ticket and E.seq == seq and E.pc == pc and E.phase == "calling")
    E.ticket, E.phase = nil, "consumed"
    E.lastn = results.n
    -- Consume BEFORE post-tail can generate another pending CALL.
    chain(E)
end
local F = newframe()
local first = F.ticket
F.callx = nil
CX_interval(F)
assert(F.pc == 11 and F.callx == 2 and F.ticket ~= first and F.seq == 2)
print("PASS", "consume-inside-CX-before-tail-keeps-next-generation")

-- Merely checking callx still equals the producer's value at CX entry is
-- also wrong: the existing trampoline has already intentionally cleared it.
local G = newframe()
local producer_phase = G.callx
G.callx = nil
assert(G.callx ~= producer_phase)
print("COUNTEREXAMPLE", "expecting-producer-callx-at-CX-entry-false-positive")
print("SUMMARY", 1, 3)
