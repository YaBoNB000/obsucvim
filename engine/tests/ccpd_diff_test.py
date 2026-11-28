#!/usr/bin/env python3
"""CCPD plan 10.4 / 11.4 / 11.5 / 11.6: semantic differential + CLI matrix.

For every program the ground truth is the plain Luau interpreter
running the original source. A CCPD-enabled artifact must reproduce
stdout and exit code exactly; so must the CCPD-off artifact. CCPD is a
representation change, never a semantics change, so any divergence is
a failure - it is never "fixed" by regenerating the sample.

Sections:
  A. All shared cases (tests/cases/*.lua) x seeds {1, 42, 777},
     CCPD on; seed 42 also CCPD off.
  B. CCPD-focused programs: closure creation/invocation matrix
     (plan 11.5), cache-pressure shapes (11.4: many blocks, backedges,
     same-block re-entry), metamethods, coroutines, tail calls,
     multi-return nil holes, error unwinding. Each runs CCPD on,
     CCPD on + --super-insn, and CCPD off at seeds {7, 42}.
  C. CLI matrix (10.4): flag combinations, argument-order
     independence, --no-ccpd byte equality with the off artifact,
     default-preset equality, and fail-closed rejection of legacy
     presets / the 5.1 dialect.
  D. Final-artifact plaintext scan (CCPD plaintext leak repair):
     generated Lua files must not contain the internal CCPD marker
     in any case, across seeds and minify/no-minify outputs.

Usage: python3 tests/ccpd_diff_test.py
Exit code: 0 = all assertions hold, 1 = at least one failed.
"""
import glob
import os
import re
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)  # engine/
REPO = os.path.dirname(ROOT)
BIN = os.environ.get("OBSUCVIM_BIN", os.path.join(ROOT, "target", "release", "obsucvim"))
TOOLS_BIN = os.environ.get("OBSUCVIM_TOOLS", os.path.join(REPO, ".tools", "bin"))
LUAU = os.path.join(TOOLS_BIN, "luau")
CASES_DIR = os.path.join(HERE, "cases")

checks = []


def check(name, ok, detail=""):
    checks.append((name, ok))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}{(' :: ' + detail) if detail and not ok else ''}")


def run_luau(path):
    proc = subprocess.run([LUAU, path], capture_output=True, text=True,
                          errors="replace", timeout=60)
    return proc.returncode, proc.stdout


def gen(src_path, out_path, seed, extra):
    proc = subprocess.run(
        [BIN, src_path, out_path, "--preset", "v15", "--dialect", "luau",
         "--seed", str(seed)] + extra,
        capture_output=True, text=True, timeout=120)
    return proc.returncode == 0, proc.stderr.strip()


def diff_case(name, src_path, seed, extra, ref):
    """Generate one artifact and diff its behaviour against the reference."""
    with tempfile.TemporaryDirectory() as td:
        out = os.path.join(td, "art.lua")
        ok, err = gen(src_path, out, seed, extra)
        if not ok:
            return False, f"generation failed: {err[:200]}"
        rc, stdout = run_luau(out)
    return (rc, stdout) == ref, f"rc={rc} want={ref[0]}"


# ---------------------------------------------------------------------------
# Section B programs (plan 11.5 closure/invocation + 11.4 cache shapes).
# ---------------------------------------------------------------------------

B_CASES = {
    "loop_created_instances": """
local fns = {}
for i = 1, 5 do
  local base = i * 100
  fns[i] = function(x) return base + x end
end
local s = 0
for i = 1, 5 do s = s + fns[i](i) end
print(s)
""",
    "repeat_calls_one_instance": """
local c = 0
local f = function() c = c + 1 return c end
print(f(), f(), f(), f())
""",
    "recursive_closure": """
local fib
fib = function(n) if n < 2 then return n end return fib(n - 1) + fib(n - 2) end
print(fib(15))
""",
    "reentrant_via_callback": """
local depth = 0
local hook
local f
f = function(n)
  depth = depth + 1
  local d = depth
  if n > 0 then hook(n - 1) end
  depth = depth - 1
  return d
end
hook = function(m) return f(m) end
print(f(4), depth)
""",
    "alternating_instances": """
local function mk(tag)
  local n = 0
  return function() n = n + 1 return tag .. n end
end
local a, b = mk("a"), mk("b")
print(a(), b(), a(), b(), a())
""",
    "upvalue_cells": """
local function counter()
  local n = 0
  local inc = function() n = n + 1 return n end
  local get = function() return n end
  return inc, get
end
local i1, g1 = counter()
local i2, g2 = counter()
i1() i1() i2()
print(g1(), g2())
local function outer()
  local x = 10
  local mid = function()
    local inner = function() x = x + 5 return x end
    return inner
  end
  return mid()
end
local inn = outer()
print(inn(), inn())
""",
    "vararg_closure": """
local f = function(...) local a = {...} return #a, a[2] end
print(f(10, 20, 30))
print(f())
local g = function(a, ...) return a + select("#", ...) end
print(g(1, 2, 3, 4))
""",
    "host_callbacks": """
local t = {5, 2, 8, 1}
table.sort(t, function(a, b) return a > b end)
print(table.concat(t, ","))
local s = string.gsub("a1b2c3", "%d", function(d) return tostring(tonumber(d) * 2) end)
print(s)
local ok, v = pcall(function() return 40 + 2 end)
print(ok, v)
""",
    "coroutine_interleave": """
local log = {}
local function worker(n)
  for i = 1, n do
    log[#log + 1] = "w" .. i
    coroutine.yield(i)
    log[#log + 1] = "r" .. i
  end
  return "done"
end
local co = coroutine.create(worker)
local cb = function(x) log[#log + 1] = "cb" .. x end
while coroutine.status(co) ~= "dead" do
  local ok, v = coroutine.resume(co, 3)
  if ok and v then cb(v) end
end
print(table.concat(log, " "))
""",
    "tail_call_deep": """
local function sum(n, acc)
  if n == 0 then return acc end
  return sum(n - 1, acc + n)
end
print(sum(1000, 0))
""",
    "metamethods": """
local mt = {}
mt.__add = function(a, b) return setmetatable({v = a.v + b.v}, mt) end
mt.__len = function(a) return a.v * 2 end
mt.__index = function(a, k) return "ix:" .. k end
local a = setmetatable({v = 3}, mt)
local b = setmetatable({v = 4}, mt)
local c = a + b
print(c.v, #a, a.missing)
local eq = setmetatable({}, {__eq = function() return true end})
print(eq == setmetatable({}, getmetatable(eq)))
""",
    "multi_return_holes": """
local function f() return nil, 7, nil end
print(select("#", f()))
local a, b, c = f()
print(a, b, c)
local function g() return f() end
print(select("#", g()))
""",
    "same_target_merge": """
local function pick(c, x)
  local r
  if c then r = x + 1 else r = x + 1 end
  if c and x > 2 then r = r * 10 end
  return r
end
print(pick(true, 1), pick(false, 1), pick(true, 5), pick(false, 5))
local y = (true and 3) or 9
local z = (false and 3) or 9
print(y, z)
""",
    "nested_loops_break_continue": """
local s = 0
for i = 1, 6 do
  if i == 3 then continue end
  local j = 0
  while true do
    j = j + 1
    if j > i then break end
    s = s + j
  end
  if i == 5 then break end
end
print(s)
local k = 0
repeat k = k + 3 until k >= 10
print(k)
for name, v in pairs({a = 1}) do print(name, v) end
for i, v in ipairs({10, 20}) do print(i, v) end
""",
    "error_unwind": """
local function inner() error("boom", 0) end
local function mid() return inner() end
local ok, err = pcall(mid)
print(ok, err)
local ok2, err2 = pcall(function() error({code = 5}, 0) end)
print(ok2, err2.code)
print(pcall(function() return "fine" end))
""",
    "cache_pressure_branches": None,  # generated below
    "cache_pressure_loops": """
local total = 0
for i = 1, 300 do
  if i % 7 == 0 then total = total + i
  elseif i % 5 == 0 then total = total - 1
  elseif i % 3 == 0 then total = total + 2
  else total = total + 1 end
  local j = i
  while j > 100 do j = j - 37 total = total + 1 end
end
print(total)
""",
}


def cache_pressure_branches_src():
    parts = ["local function classify(n)"]
    for i in range(48):
        kw = "if" if i == 0 else "elseif"
        parts.append(f"  {kw} n == {i} then return {(i * 37) % 91}")
    parts.append("  else return -1 end")
    parts.append("end")
    parts.append("local s = 0")
    parts.append("for i = 0, 60 do s = s + classify(i) end")
    parts.append("print(s)")
    return "\n".join(parts) + "\n"


def section_a():
    print("ccpd_diff_test A: shared cases differential")
    sources = sorted(glob.glob(os.path.join(CASES_DIR, "*.lua")))
    for src in sources:
        name = os.path.basename(src)
        ref = run_luau(src)
        for seed in (1, 42, 777):
            ok, detail = diff_case(name, src, seed, ["--ccpd"], ref)
            check(f"A {name} seed={seed} ccpd-on", ok, detail)
        ok, detail = diff_case(name, src, 42, [], ref)
        check(f"A {name} seed=42 ccpd-off", ok, detail)


def section_b():
    print("ccpd_diff_test B: closure/cache focused differential")
    cases = dict(B_CASES)
    cases["cache_pressure_branches"] = cache_pressure_branches_src()
    with tempfile.TemporaryDirectory() as td:
        for name, body in cases.items():
            src = os.path.join(td, name + ".lua")
            with open(src, "w", encoding="utf-8") as fh:
                fh.write(body)
            ref = run_luau(src)
            for seed in (7, 42):
                for label, extra in (("on", ["--ccpd"]),
                                     ("on+super", ["--ccpd", "--super-insn"]),
                                     ("off", [])):
                    ok, detail = diff_case(name, src, seed, extra, ref)
                    check(f"B {name} seed={seed} {label}", ok, detail)


def section_c():
    print("ccpd_diff_test C: CLI matrix")
    with tempfile.TemporaryDirectory() as td:
        src = os.path.join(td, "prog.lua")
        with open(src, "w", encoding="utf-8") as fh:
            fh.write(B_CASES["alternating_instances"])
        ref = run_luau(src)
        combos = [
            ("ccpd", ["--ccpd"]),
            ("ccpd+super", ["--ccpd", "--super-insn"]),
            ("ccpd+no-proto-layout", ["--ccpd", "--no-proto-layout"]),
            ("ccpd+no-guard", ["--ccpd", "--no-guard"]),
            ("ccpd+guard", ["--ccpd", "--guard"]),
            ("ccpd+no-minify", ["--ccpd", "--no-minify"]),
        ]
        for label, extra in combos:
            ok, detail = diff_case(label, src, 42, extra, ref)
            check(f"C combo {label}", ok, detail)

        def gen_bytes(args, out_name):
            out = os.path.join(td, out_name)
            proc = subprocess.run([BIN, src, out] + args,
                                  capture_output=True, text=True, timeout=120)
            if proc.returncode != 0:
                return None, proc.stderr.strip()
            with open(out, "rb") as fh:
                return fh.read(), ""

        on, _ = gen_bytes(["--preset", "v15", "--ccpd", "--seed", "42"], "on.lua")
        default, _ = gen_bytes(["--ccpd", "--seed", "42"], "default.lua")
        swapped, _ = gen_bytes(["--ccpd", "--seed", "42", "--preset", "v15"], "swap.lua")
        check("C --ccpd alone equals --preset v15 --ccpd",
              on is not None and on == default)
        check("C argument order independent", on is not None and on == swapped)
        off, _ = gen_bytes(["--preset", "v15", "--seed", "42"], "off.lua")
        noccpd, _ = gen_bytes(["--preset", "v15", "--seed", "42", "--ccpd", "--no-ccpd"],
                              "noccpd.lua")
        check("C --ccpd --no-ccpd byte-identical to off",
              off is not None and off == noccpd)
        check("C on artifact differs from off artifact", on is not None and on != off)

        illegal = [
            ("legacy preset vm", ["--preset", "vm", "--ccpd"]),
            ("legacy preset low", ["--preset", "low", "--ccpd"]),
            ("legacy --vm flag", ["--vm", "--ccpd"]),
            ("dialect 5.1", ["--preset", "v15", "--ccpd", "--dialect", "5.1"]),
        ]
        for label, args in illegal:
            out = os.path.join(td, "illegal.lua")
            proc = subprocess.run([BIN, src, out] + args,
                                  capture_output=True, text=True, timeout=120)
            check(f"C illegal combo rejected: {label}",
                  proc.returncode != 0 and proc.stderr.strip() != "",
                  f"rc={proc.returncode} stderr={proc.stderr.strip()[:120]!r}")

        # CCPD-off byte identity anchor against the shipped example
        # (generated with --seed 42; see gen_examples.sh conventions).
        basics_src = os.path.join(CASES_DIR, "basics.lua")
        shipped = os.path.join(ROOT, "examples", "basics.v15.luau.lua")
        out = os.path.join(td, "basics_off.lua")
        ok, err = gen(basics_src, out, 42, [])
        same = False
        if ok and os.path.isfile(shipped):
            with open(out, "rb") as fh:
                mine = fh.read()
            with open(shipped, "rb") as fh:
                same = mine == fh.read()
        check("C off seed42 basics byte-identical to shipped example", same, err)


def section_d():
    print("ccpd_diff_test D: final artifacts carry no CCPD plaintext markers")
    body = r"""
local function f(a) if a then print("then") else return 99 end end
local mt = {}
mt.__add = function(a, b) return setmetatable({v = a.v + b.v}, mt) end
local a = setmetatable({v = 3}, mt)
local b = setmetatable({v = 4}, mt)
print(f(true), f(false), (a + b).v)
"""
    configs = [
        ("ccpd seed=7 minify", 7, ["--ccpd"]),
        ("ccpd seed=42 minify", 42, ["--ccpd"]),
        ("ccpd seed=123 minify", 123, ["--ccpd"]),
        ("ccpd seed=42 no-minify", 42, ["--ccpd", "--no-minify"]),
        ("ccpd off control seed=42", 42, []),
    ]
    with tempfile.TemporaryDirectory() as td:
        src = os.path.join(td, "marker.lua")
        with open(src, "w", encoding="utf-8") as fh:
            fh.write(body)
        for label, seed, extra in configs:
            out = os.path.join(td, "marker.out.lua")
            ok, err = gen(src, out, seed, extra)
            if not ok:
                check(f"D marker scan {label}", False, f"generation failed: {err[:200]}")
                continue
            with open(out, encoding="utf-8", errors="surrogateescape") as fh:
                text = fh.read()
            hit = re.search(r"ccpd", text, re.IGNORECASE)
            detail = ""
            if hit:
                lo = max(0, hit.start() - 40)
                hi = min(len(text), hit.end() + 40)
                detail = f"hit at {hit.start()}: {text[lo:hi]!r}"
            check(f"D marker scan {label}", hit is None, detail)


def main():
    if not os.path.isfile(BIN):
        print(f"obsucvim binary not found at {BIN}; build with cargo build --release")
        return 1
    if not os.path.isfile(LUAU):
        print(f"luau not found at {LUAU}; run engine/tools/rebuild_tools.sh first")
        return 1
    section_a()
    section_b()
    section_c()
    section_d()
    failed = [n for n, ok in checks if not ok]
    print(f"ccpd_diff_test: {len(checks) - len(failed)}/{len(checks)} checks passed")
    if failed:
        print("failed checks:")
        for n in failed:
            print(f"  - {n}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
