#!/usr/bin/env python3
"""Phase 18 P0-3: Robustness regression gate (C1-C5 resource limits + edge inputs).

For every calibrated dimension in engine/src/limits.rs the gate runs two
specimens:

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
    m = re.search(r"^pub const %s: usize = (\d+);" % re.escape(name), src, re.M)
    if not m:
        raise SystemExit(f"cannot read {name} from {LIMITS_RS}")
    return int(m.group(1))


MAX_FUNCS = const_of("MAX_TOPLEVEL_FUNC_DECLS")
MAX_STMTS = const_of("MAX_TOPLEVEL_STMTS")
MAX_CLOSURE = const_of("MAX_CLOSURE_NEST_DEPTH")
MAX_EXPR = const_of("MAX_EXPR_PARSE_DEPTH")
MAX_BLOCK = const_of("MAX_BLOCK_NEST_DEPTH")


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
    # C5-class: input-reachable compiler limit (invalid assignment target).
    # `f() = 1` is a call used as an assignment target.
    return "f() = 1\n"


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
    ("C5_invalid_assignment_target", invalid_assignment, 0, False),
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
        f"expr={MAX_EXPR} block={MAX_BLOCK}"
    )
    passed = sum(1 for case in CASES if run_case(*case))
    print(f"ROBUSTNESS GATE PASS: {passed}/{len(CASES)}")
    return 0 if passed == len(CASES) else 1


if __name__ == "__main__":
    sys.exit(main())
