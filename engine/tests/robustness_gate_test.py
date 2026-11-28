#!/usr/bin/env python3
"""Phase 18 P0-3: Robustness regression gate (C1-C5 resource limits + edge inputs).

For every calibrated dimension in engine/src/limits.rs the gate runs two
specimens (plus scenario E = C5 and the P4-audit findings, see CASES):

  threshold      -> must compile cleanly (rc 0)
  threshold + 1  -> must be rejected with a single-line `error:` diagnostic and
                    a non-zero exit code, with no `panicked at` / SIGABRT /
                    SIGSEGV / RUST_BACKTRACE anywhere in the output

The thresholds are parsed out of limits.rs so this gate can never drift from the
implementation (no duplicated magic numbers).
"""
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.environ.get("OBSUCVIM_BIN", os.path.join(ROOT, "target/release/obsucvim"))
LIMITS_RS = os.path.join(ROOT, "src", "limits.rs")


def const_of(name):
    src = open(LIMITS_RS, "r", encoding="utf-8").read()
    m = re.search(r"^pub const %s: usize = ([0-9_]+);" % re.escape(name), src, re.M)
    if not m:
        raise SystemExit(f"cannot read {name} from {LIMITS_RS}")
    return int(m.group(1).replace("_", ""))


MAX_FUNCS = const_of("MAX_TOPLEVEL_FUNC_DECLS")
MAX_STMTS = const_of("MAX_TOPLEVEL_STMTS")
MAX_CLOSURE = const_of("MAX_CLOSURE_NEST_DEPTH")
MAX_EXPR = const_of("MAX_EXPR_PARSE_DEPTH")
MAX_BLOCK = const_of("MAX_BLOCK_NEST_DEPTH")
MAX_CSEC = const_of("MAX_CONSTS_SECTION_BYTES")


def funcs(n):
    return "\n".join(f"local function f{i}() return {i} end" for i in range(n)) + "\nprint(f0())\n"


def stmts(n):
    return "\n".join(f"local v{i} = {i}" for i in range(n)) + "\n"


def closures(n):
    body = "local function f0()\n"
    for i in range(1, n):
        body += "  " * i + f"local function f{i}()\n"
    body += "  " * n + "return 1\n"
    for i in range(n - 1, 0, -1):
        body += "  " * i + "end\n" + "  " * i + f"return f{i}()\n"
    body += "end\nprint(f0())\n"
    return body


def parens(n):
    # The outermost expression already counts as one level, so a specimen of
    # "depth n" needs n-1 nested parentheses.
    n -= 1
    return "local x = " + ("(" * n) + "1" + (")" * n) + "\n"


def blocks(n):
    # The chunk itself is a block, so "depth n" needs n-1 nested `do` blocks.
    n -= 1
    return ("do\n" * n) + "print(1)\n" + ("end\n" * n)


def invalid_assignment():
    # Input-reachable clean rejection (a call used as an assignment target).
    # Before the P4 audit this case was mislabelled "C5"; it is not C5 -- see
    # closure_positions() below for the real C5 scenario.
    return "f() = 1\n"


def closure_positions():
    # Scenario E = C5 (`child_index >= fns.len()`): the trigger condition traced
    # through compiler.rs compile_function is "a closure in an AST position that
    # count_stmt / count_expr does not enumerate" (the slot table is sized from
    # that count). The specimen puts a closure in EVERY position the two
    # counters enumerate, so a future AST node that forgets to update them
    # surfaces here as the clean `slot overflow` rejection instead of a panic.
    # The Rust unit test `c5_count_fns_matches_compiled_closure_slots` pins the
    # count == compiled-slots equality directly; this gate pins the end-to-end
    # behaviour (clean compile + correct execution, see robustness_test.sh).
    # 40 closures on purpose: the v15 preset's S table (48 slots, 6 fixed + 1
    # per carrier chunk) caps a program at 42 chunks = main + 41 closures
    # (HANDOFF §18 P0-1 boundary note), so the specimen must stay under it.
    return """local function lf() return 1 end
local a = function() return 2 end
local obj = {}
function obj:m(x) return function() return x end end
function obj.f() return 3 end
local t = { function() return 4 end, k = function() return 5 end, [1] = function() return 6 end }
local key = (function() return 9 end)()
t[key] = 9
print(function() return 7 end)
local r = (function() return 8 end)()
local x = true and function() return 10 end or 1
local y = not function() return false end
local i = t[function() return 1 end]
local z = a and (function() return 1 end) or function() return 2 end
local acc = 0
if (function() return true end)() then acc = acc + (function() return 1 end)()
elseif (function() return false end)() then acc = acc + (function() return 2 end)()
else acc = acc + (function() return 3 end)() end
while (function() return false end)() do local w = function() end end
local u
repeat u = function() acc = acc + 4 end until (function() return true end)()
u()
for i = (function() return 1 end)(), (function() return 2 end)(), (function() return 1 end)() do local b = function() return i end end
for k, v in (function() return pairs({}) end)() do local c = function() end end
do local d = function() acc = acc + 5 end d() end
local e = ("x"):rep(1, (function() return "" end)())
local m = ("abc"):sub((function() return 1 end)(), 2)
local n = t.a
local q = if x then function() return 11 end else function() return 12 end
local up = 6
local function o() return function() return function() return up end end end
local s = "x" .. (function() return "y" end)()
print(lf(), a(), obj.f(), obj:m(7)(), r, i, z, acc, e, m, q(), o()()(), s)
"""


def break_in_fn_in_loop():
    # P4 audit: input-reachable bare panic that P0-2 missed. The parser's loop
    # counter used to leak into nested function bodies, so this reached vmgen
    # with an empty loop stack and hit `expect("break outside loop")` (rc 101).
    # Lua 5.1 / Luau reject it with "break statement must be inside a loop".
    return "while true do local function f() break end end\n"


def continue_in_fn_in_loop():
    # Same family as break_in_fn_in_loop (Luau `continue`).
    return "for i=1,2 do local f = function() continue end end\n"


def consts_section(n):
    # P4 audit: the per-function constants section stores its byte length as a
    # u16 varint. A single sealed literal of n bytes used to wrap that length
    # and ship an artifact that fails at load time (no diagnostic). Now it is
    # a clean `input exceeds compiler limit: constant section = ...` rejection.
    return "local s = \"" + "a" * n + "\"\nprint(#s)\n"


# (name, generator, size, expect_ok)
CASES = [
    ("C1_funcs_threshold", funcs, MAX_FUNCS, True),
    ("C1_funcs_threshold_plus_1", funcs, MAX_FUNCS + 1, False),
    ("C2_stmts_threshold", stmts, MAX_STMTS, True),
    ("C2_stmts_threshold_plus_1", stmts, MAX_STMTS + 1, False),
    ("C3_closure_threshold", closures, MAX_CLOSURE, True),
    ("C3_closure_threshold_plus_1", closures, MAX_CLOSURE + 1, False),
    ("C4_expr_depth_threshold", parens, MAX_EXPR, True),
    ("C4_expr_depth_threshold_plus_1", parens, MAX_EXPR + 1, False),
    ("C6_block_depth_threshold", blocks, MAX_BLOCK, True),
    ("C6_block_depth_threshold_plus_1", blocks, MAX_BLOCK + 1, False),
    ("E_c5_closure_positions", closure_positions, 0, True),
    ("G_invalid_assignment_target", invalid_assignment, 0, False),
    ("G_break_inside_function_in_loop", break_in_fn_in_loop, 0, False),
    ("G_continue_inside_function_in_loop", continue_in_fn_in_loop, 0, False),
    # Structural bound (not bisected): the specimen sizes straddle the limit
    # with a margin because the exact section size also depends on the
    # seed-dependent noise constants (seed is fixed to 42 here).
    ("H_consts_section_under_limit", consts_section, MAX_CSEC - 64, True),
    ("H_consts_section_over_limit", consts_section, MAX_CSEC + 1, False),
]


def run_case(name, gen, size, expect_ok):
    src = gen(size) if size else gen()
    with tempfile.NamedTemporaryFile("w", suffix=".lua", delete=False, encoding="utf-8") as f:
        f.write(src)
        inp = f.name
    out = inp + ".out.lua"
    try:
        p = subprocess.run(
            [BIN, "--preset", "v15", "--dialect", "luau", "--seed", "42", inp, out],
            capture_output=True,
            text=True,
            timeout=120,
        )
        err = (p.stderr or "").strip()
        crash = any(
            tok in err for tok in ("panicked at", "SIGABRT", "SIGSEGV", "RUST_BACKTRACE")
        )
        if crash:
            print(f"  [FAIL] {name}: compiler crashed :: {err!r}")
            return False
        if expect_ok:
            if p.returncode != 0:
                print(f"  [FAIL] {name}: expected clean compile, rc={p.returncode} :: {err!r}")
                return False
            print(f"  [PASS] {name}: compiled cleanly at threshold {size}")
            return True
        if p.returncode == 0:
            print(f"  [FAIL] {name}: expected rejection at {size}, got rc=0")
            return False
        lines = [ln for ln in err.splitlines() if ln.strip()]
        if len(lines) != 1 or not lines[0].startswith("error:"):
            print(f"  [FAIL] {name}: diagnostic must be exactly one `error:` line :: {err!r}")
            return False
        print(f"  [PASS] {name}: rejected cleanly :: {lines[0]}")
        return True
    finally:
        for path in (inp, out):
            if os.path.exists(path):
                os.remove(path)


def main():
    if not os.path.exists(BIN):
        print(f"FAIL: missing binary {BIN}", file=sys.stderr)
        return 2
    print(
        f"thresholds: funcs={MAX_FUNCS} stmts={MAX_STMTS} closure={MAX_CLOSURE} "
        f"expr={MAX_EXPR} block={MAX_BLOCK} consts_section={MAX_CSEC}"
    )
    passed = sum(1 for case in CASES if run_case(*case))
    print(f"ROBUSTNESS GATE PASS: {passed}/{len(CASES)}")
    return 0 if passed == len(CASES) else 1


if __name__ == "__main__":
    sys.exit(main())
