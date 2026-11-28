#!/usr/bin/env python3
"""CCPD repair plan 1.4: EXIT edge runtime semantics, end to end.

The compiler emits edges whose target PC is `n + 1` (one past the
last record) for branch merges at a function tail. Those edges have
no owning block; the runtime epilogue must route them to the frame's
implicit zero-value return instead of the block decoder (repair plan
P1). Every case here runs against the plain Luau interpreter as the
oracle: stdout, exit code and observable return behaviour must match
exactly, or the check fails.

Coverage:
  * tail merge reached by a non-returning branch (Jump -> EXIT);
  * tail merge present in the CFG while both branches return;
  * the implicit zero-value return through `select('#', ...)`;
  * explicit multi-return / nil-hole shapes kept intact next to an
    EXIT-taking helper;
  * if/elseif chains and a trailing while loop landing on the tail;
  * nested frames where an inner function exits through the tail.

Each case runs under `--ccpd`, `--ccpd --super-insn` and
`--ccpd --no-guard` at seeds {7, 42, 123}, plus a CFG ground-truth
assertion (via the audit-channel dump) that the first case really
does contain an EXIT edge - otherwise the test would prove nothing.

Usage: python3 tests/ccpd_exit_edge_test.py
Exit code: 0 = all assertions hold, 1 = at least one failed.
"""
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)  # engine/
REPO = os.path.dirname(ROOT)
BIN = os.environ.get("OBSUCVIM_BIN", os.path.join(ROOT, "target", "release", "obsucvim"))
AUDIT_BIN = os.environ.get(
    "OBSUCVIM_AUDIT_BIN", os.path.join(ROOT, "target", "audit", "release", "obsucvim"))
TOOLS_BIN = os.environ.get("OBSUCVIM_TOOLS", os.path.join(REPO, ".tools", "bin"))
LUAU = os.path.join(TOOLS_BIN, "luau")

SEEDS = (7, 42, 123)
FLAG_SETS = (
    ("--ccpd",),
    ("--ccpd", "--super-insn"),
    ("--ccpd", "--no-guard"),
)

CASES = {
    # Non-returning branch falls through to the tail merge jump.
    "merge_fallthrough": r"""
local function f(a) if a then print("then") else return 99 end end
print(f(true))
print("mid")
print(f(false))
print("end")
""",
    # EXIT edge exists in the CFG; both branches return explicitly.
    "merge_both_return": r"""
local function f(a) if a then return 1 else return 2 end end
print(f(true), f(false))
""",
    # The implicit tail return produces zero values.
    "exit_return_count": r"""
local function f(a) if a then print("x") else return 1, 2 end end
print(select("#", f(true)))
print(select("#", f(false)))
""",
    # Explicit multi-return with a nil hole stays exact, and a tail
    # exit in a helper does not disturb the caller's frame.
    "multi_return_preserved": r"""
local function g(a) if a then print("g") else return -1 end end
local function f() return 1, nil, 3 end
g(true)
local a, b, c = f()
print(a, b, c, select("#", f()))
""",
    # An if/elseif chain whose final branch does not return.
    "elseif_tail_merge": r"""
local function f(n)
  if n == 1 then return "one"
  elseif n == 2 then return "two"
  else print("other") end
end
print(f(1))
print(f(2))
print(f(3))
print("done")
""",
    # A trailing while loop: loop exit lands on the function tail.
    "while_tail": r"""
local function f(n)
  local s = 0
  while n > 0 do s = s + n n = n - 1 end
  return s
end
local function g(n)
  while n > 0 do
    if n == 2 then return "hit" end
    n = n - 1
  end
end
print(f(4))
print(g(3), g(1))
""",
    # Inner frame exits through the tail between outer operations.
    "nested_call_chain": r"""
local function inner(a) if a then print("in") else return 7 end end
local function outer(a)
  local x = inner(a)
  return (x or 0) + 1
end
print(outer(true))
print(outer(false))
""",
}

checks = []


def check(name, ok, detail=""):
    checks.append((name, ok))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}{(' :: ' + detail) if detail and not ok else ''}")


def run_luau(path):
    proc = subprocess.run([LUAU, path], capture_output=True, text=True,
                          errors="replace", timeout=60)
    return proc.returncode, proc.stdout, proc.stderr


def oracle(src_text):
    with tempfile.NamedTemporaryFile("w", suffix=".lua", delete=False) as fh:
        fh.write(src_text)
        path = fh.name
    try:
        return run_luau(path)[:2]
    finally:
        os.unlink(path)


def gen(binary, src_path, out_path, seed, extra, audit_path=None):
    env = dict(os.environ)
    if audit_path:
        env["OBSUCVIM_CCPD_AUDIT"] = audit_path
    proc = subprocess.run(
        [binary, src_path, out_path, "--preset", "v15", "--dialect", "luau",
         "--ccpd", "--seed", str(seed)] + list(extra),
        capture_output=True, text=True, env=env, timeout=120)
    return proc.returncode == 0, proc.stderr.strip()


def diff_case(name, src_text, cases_dir):
    ref = oracle(src_text)
    src_path = os.path.join(cases_dir, name + ".lua")
    with open(src_path, "w", encoding="utf-8") as fh:
        fh.write(src_text)
    for seed in SEEDS:
        for flags in FLAG_SETS:
            out_path = os.path.join(cases_dir, name + ".out.lua")
            ok, err = gen(BIN, src_path, out_path, seed, flags)
            tag = f"{name} seed={seed} flags={' '.join(flags)}"
            if not ok:
                check(tag, False, f"generation failed: {err[:200]}")
                continue
            rc, stdout, stderr = run_luau(out_path)
            check(tag, (rc, stdout) == ref,
                  f"rc={rc} want={ref[0]}; stderr={stderr[:200]!r}")


def cfg_has_exit(binary, name, src_text, cases_dir):
    """Compiler ground truth: the case really carries an EXIT edge."""
    if not os.path.exists(binary):
        check(f"cfg {name}: audit binary present", False, binary)
        return
    src_path = os.path.join(cases_dir, name + ".cfg.lua")
    out_path = os.path.join(cases_dir, name + ".cfg.out.lua")
    dump_path = os.path.join(cases_dir, name + ".dump")
    with open(src_path, "w", encoding="utf-8") as fh:
        fh.write(src_text)
    ok, err = gen(binary, src_path, out_path, 42, (), audit_path=dump_path)
    if not ok:
        check(f"cfg {name} exits", False, f"audit generation failed: {err[:200]}")
        return
    with open(dump_path, encoding="utf-8") as fh:
        dump = fh.read()
    has_exit = any(line.rstrip().endswith(" EXIT") for line in dump.splitlines())
    check(f"cfg {name} declares an EXIT edge", has_exit)


def main():
    print("ccpd_exit_edge_test: EXIT edges take the frame return path")
    with tempfile.TemporaryDirectory() as td:
        cfg_has_exit(AUDIT_BIN, "merge_fallthrough",
                     CASES["merge_fallthrough"], td)
        for name, src in CASES.items():
            diff_case(name, src, td)
    failed = [n for n, ok in checks if not ok]
    print(f"ccpd_exit_edge_test: {len(checks) - len(failed)}/{len(checks)} checks passed")
    if failed:
        print("failed checks:")
        for n in failed:
            print(f"  - {n}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
