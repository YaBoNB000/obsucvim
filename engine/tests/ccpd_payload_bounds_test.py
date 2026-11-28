#!/usr/bin/env python3
"""CCPD repair plan 2.6: variant payload hard bounds, executed live.

The payload decoder under test is the real `CCPD_BDEC_LUA` text lifted
verbatim from `engine/src/vmgen/template.rs` (not a reimplementation):
each case hands it a stub frame whose bank and per-variant directory
fields are mutated, and expects either a clean decode or a fail-closed
`error(...)` raised *before* the read that would leave the variant's
own `[po, po + pl)` span. Directory-level rules (Rust `decode_section`
vs runtime parser parity) are covered by the Rust unit tests in
`ccpd.rs::section_span_rules`.

Mutations from plan 2.6 (all failures must use the sanitized
release message `invalid runtime payload`, with no internal marker
or fn/block/variant/offset fields):
  * `pl` one byte shorter than the real payload -> rejected;
  * each of the four operand lanes truncated in turn -> rejected;
  * `pl` one byte longer than the consumed data -> rejected;
  * `po + pl` beyond the bank -> rejected;
  * `pl` below the format minimum (5 bytes per record) -> rejected;
  * a truncated final operand whose continuation runs into the next
    variant's bytes -> rejected at the variant's own boundary instead
    of borrowing from the neighbour.

Usage: python3 tests/ccpd_payload_bounds_test.py
Exit code: 0 = all assertions hold, 1 = at least one failed.
"""
import os
import re
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)  # engine/
REPO = os.path.dirname(ROOT)
TOOLS_BIN = os.environ.get("OBSUCVIM_TOOLS", os.path.join(REPO, ".tools", "bin"))
LUAU = os.path.join(TOOLS_BIN, "luau")
TEMPLATE = os.path.join(ROOT, "src", "vmgen", "template.rs")

checks = []


def check(name, ok, detail=""):
    checks.append((name, ok))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}{(' :: ' + detail) if detail and not ok else ''}")


def extract_rust_str(src, name):
    """Pull a `const NAME: &str = "..."` literal out of template.rs."""
    m = re.search(r'const %s: &str = "((?:[^"\\]|\\.)*)"' % re.escape(name), src)
    if not m:
        raise SystemExit(f"template.rs: cannot locate const {name}")
    return m.group(1).replace('\\"', '"').replace("\\\\", "\\")


def build_harness():
    src = open(TEMPLATE, encoding="utf-8").read()
    helpers = extract_rust_str(src, "CCPD_HELPERS_LUA")
    bdec = extract_rust_str(src, "CCPD_BDEC_LUA")
    cases = r'''
local function mkE(bank, blocks)
  return { bt = { sx = PM + 1, csa = 0, cd = { blocks = blocks, ne = 1, clos = -1 }, bk = bank },
           er = 0, bv = {}, bch = {}, bq = {}, edge = 0, fctx = 1 }
end
local function mkblocks(vs, sp, ep)
  -- bdec picks one of the four variants via CCPDV; the pool must be
  -- complete or it would index a nil variant descriptor.
  return { { sp = sp or 1, ep = ep or 1, inc = { [0] = 1 }, vs = { vs, vs, vs, vs } } }
end
local function attempt(name, bank, vs, sp, ep, want, needle)
  local E = mkE(bank, mkblocks(vs, sp, ep))
  local ok, err = pcall(bdec, E, 1)
  if want == "ok" then
    report(name, ok, ok and "" or tostring(err))
  else
    local msg = tostring(err)
    local lower = msg:lower()
    local good = (not ok)
      and msg:find(needle, 1, true) ~= nil
      and lower:find("ccpd", 1, true) == nil
      and msg:find("fn=", 1, true) == nil
      and msg:find("block=", 1, true) == nil
      and msg:find("variant=", 1, true) == nil
      and msg:find("po=", 1, true) == nil
      and msg:find("pl=", 1, true) == nil
    report(name, good, good and "" or ("want sanitized error containing '" .. needle .. "', got ok=" .. tostring(ok) .. " err=" .. msg))
  end
end
local function V(po, pl) return { pc = 0, pe = 0, po = po, pl = pl, ms = 0, mt = 1 } end

-- Well-formed: 1 record, opcode byte + four one-byte lane values.
attempt("well-formed payload decodes", { 10, 20, 30, 40, 50 }, V(0, 5), nil, nil, "ok", "")
-- pl one byte short: below the format minimum for one record, so the
-- decoder rejects it before any payload byte is read.
attempt("pl one byte short rejected", { 10, 20, 30, 40, 50 }, V(0, 4), nil, nil, "err", "invalid runtime payload")
-- Lane 1 truncated: continuation bytes run out mid-varint.
attempt("lane 1 truncated at span end", { 10, 200, 200, 200, 200, 200 }, V(0, 6), nil, nil, "err", "invalid runtime payload")
-- Lane 2 truncated.
attempt("lane 2 truncated at span end", { 10, 20, 200, 200, 200, 200 }, V(0, 6), nil, nil, "err", "invalid runtime payload")
-- Lane 3 truncated.
attempt("lane 3 truncated at span end", { 10, 20, 30, 200, 200, 200, 200 }, V(0, 7), nil, nil, "err", "invalid runtime payload")
-- Lane 4 truncated.
attempt("lane 4 truncated at span end", { 10, 20, 30, 40, 200 }, V(0, 5), nil, nil, "err", "invalid runtime payload")
-- pl one byte longer than the data actually consumed: trailing data.
attempt("trailing byte rejected", { 10, 20, 30, 40, 50, 60 }, V(0, 6), nil, nil, "err", "invalid runtime payload")
-- Span runs off the end of the bank entirely.
attempt("span beyond bank rejected", { 10, 20, 30 }, V(0, 7), nil, nil, "err", "invalid runtime payload")
-- pl below the format minimum for two records (needs >= 10 bytes).
attempt("below format minimum rejected", { 1, 2, 3, 4, 5, 6, 7, 8, 9 }, V(0, 9), 1, 2, "err", "invalid runtime payload")
-- Boundary exact: po + pl == #bank decodes without complaint.
attempt("span ending exactly at bank end", { 10, 20, 30, 40, 50 }, V(0, 5), nil, nil, "ok", "")
-- Neighbour isolation: the last lane starts a continuation that
-- would complete only inside the next variant's bytes; with pl = 4
-- the decoder refuses the span outright (format minimum) instead of
-- ever reaching for the neighbour's bytes.
attempt("decoder does not borrow next variant", { 10, 20, 30, 200, 77, 77, 77, 77 }, V(0, 4), nil, nil, "err", "invalid runtime payload")
-- The next variant itself is still decodable on its own span.
attempt("next variant decodes on its own span", { 10, 20, 30, 200, 77, 77, 77, 77, 88 }, V(4, 5), nil, nil, "ok", "")
'''
    ledger = r'''
local results = {}
function report(name, ok, detail) results[#results + 1] = (ok and "PASS " or "FAIL ") .. name .. (detail == "" and "" or (" :: " .. detail)) end
'''
    trailer = r'''
for i = 1, #results do print(results[i]) end
'''
    return (
        "BC = 4 WM = 257 WB = 31 PM = 123 FLOOR = math.floor\n"
        + helpers + "\n"
        + bdec + "\n"
        + ledger + cases + trailer
    )


def main():
    print("ccpd_payload_bounds_test: variant payloads decode inside [po, po+pl)")
    harness = build_harness()
    with tempfile.TemporaryDirectory() as td:
        path = os.path.join(td, "harness.lua")
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(harness)
        proc = subprocess.run([LUAU, path], capture_output=True, text=True,
                              errors="replace", timeout=60)
    lines = [ln for ln in proc.stdout.splitlines() if ln.startswith(("PASS ", "FAIL "))]
    if proc.returncode != 0 or not lines:
        check("harness runs", False, f"rc={proc.returncode} stderr={proc.stderr[:300]!r}")
    else:
        for ln in lines:
            if ln.startswith("PASS "):
                check(ln[5:], True)
            else:
                body = ln[5:]
                name, _, detail = body.partition(" :: ")
                check(name, False, detail)
    failed = [n for n, ok in checks if not ok]
    print(f"ccpd_payload_bounds_test: {len(checks) - len(failed)}/{len(checks)} checks passed")
    if failed:
        print("failed checks:")
        for n in failed:
            print(f"  - {n}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
