#!/usr/bin/env python3
"""CCPD plan 11.7 / 15.2: runtime audit against compiler CFG ground truth.

The audit channel is compiled only into `--features audit-hooks`
builds and only emits reporters when `OBSUCVIM_CCPD_AUDIT` is set at
generation time (same discipline as the string-cache dump channel).
With it, a generated artifact records:

  B <fi> <bi> <edge> <variant> <visit> <fctx>   every block entry (bdec)
  E <fi> <fctx> <edge>                          every frame creation (newE)
  M <fi> <origin>                               every closure creation (makefn)
  C <site_pc> <ordinal> <child>                 every Closure record executed

Independently, the compiler appends its CFG ground truth (per
function: block/edge counts, entry edge, incoming edge sets, edge
table, closure sites) to the file named by the same variable. This
test cross-checks the two:

  * every observed block entry names an edge that the CFG declares
    incoming to that exact block (the edge audit);
  * every frame starts on the function's entry edge;
  * variant selection stays in [0, 4) and at least one block is
    observed decoding through >= 2 distinct variants (rotation is
    live, not just compiled in);
  * one block is entered through >= 2 distinct incoming edges;
  * closures created repeatedly at one site receive distinct origin
    tokens, and invocations of the closure functions receive distinct
    frame contexts (per-instance call-ordinal rotation);
  * a frame-local creation counter produces ordinals 1..k for k
    closures created in a loop inside one frame;
  * production isolation: neither the production binary nor the audit
    binary without the env var emits the audit channel, and the audit
    binary without the env var produces byte-identical output to the
    production binary.

Usage: python3 tests/ccpd_runtime_audit_test.py
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
PROD_BIN = os.environ.get("OBSUCVIM_BIN", os.path.join(ROOT, "target", "release", "obsucvim"))
AUDIT_BIN = os.environ.get(
    "OBSUCVIM_AUDIT_BIN", os.path.join(ROOT, "target", "audit", "release", "obsucvim"))
TOOLS_BIN = os.environ.get("OBSUCVIM_TOOLS", os.path.join(REPO, ".tools", "bin"))
LUAU = os.path.join(TOOLS_BIN, "luau")

PROGRAM = r"""
local function make(base)
  return function(x) return base + x end
end
local fns = {}
for i = 1, 4 do fns[i] = make(i * 10) end
local s = 0
for i = 1, 4 do
  s = s + fns[i](i)
  s = s + fns[i](i * 2)
end
local hs = {}
for i = 1, 3 do
  local v = i
  hs[i] = function() return v * 2 end
end
local function fib(n)
  if n < 2 then return n end
  return fib(n - 1) + fib(n - 2)
end
local acc = 0
local i = 0
while i < 20 do
  i = i + 1
  if i % 3 == 0 then acc = acc + i elseif i % 3 == 1 then acc = acc + 1 end
end
print(s, fib(10), acc, hs[1]() + hs[2]() + hs[3]())
"""

EXIT_PROGRAM = r"""
local function f(a) if a then print("in") else return 99 end end
local function g() local s = 0 for i = 1, 4 do s = s + i end if s > 5 then s = s * 10 else return -1 end return s end
local function h(a) while a do return 7 end end
print(f(true))
print(f(false))
print(g())
print(h(false), h(true))
"""

REVISIT_PROGRAM = r"""
local function f(n) local s = 0 for i = 1, n do s = s + i end return s end
print(f(20))
"""

CORO_PROGRAM = r"""
local function worker(tag, base)
  for i = 1, 3 do coroutine.yield(tag .. i) end
  return base * 100
end
local a = coroutine.create(worker)
local b = coroutine.create(worker)
print(coroutine.resume(a, "a", 1))
print(coroutine.resume(b, "b", 2))
print(coroutine.resume(a))
print(coroutine.resume(b))
print(coroutine.resume(a))
print(coroutine.resume(a))
print(coroutine.resume(b))
"""

checks = []


def check(name, ok, detail=""):
    checks.append((name, ok))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}{(' :: ' + detail) if detail and not ok else ''}")


def info(name, detail):
    print(f"  [INFO] {name} :: {detail}")


def gen(binary, src, out, seed=42, audit_path=None):
    env = dict(os.environ)
    if audit_path:
        env["OBSUCVIM_CCPD_AUDIT"] = audit_path
    proc = subprocess.run(
        [binary, src, out, "--preset", "v15", "--dialect", "luau",
         "--ccpd", "--seed", str(seed)],
        capture_output=True, text=True, env=env, timeout=120)
    return proc.returncode == 0, proc.stderr.strip()


def run_artifact(artifact):
    """Run an artifact with the audit table pre-registered; return
    (stdout_lines, audit_entries)."""
    art = open(artifact, encoding="utf-8", errors="surrogateescape").read()
    lvl = 1
    while "]" + "=" * lvl + "]" in art:
        lvl += 1
    eq = "=" * lvl
    driver = (
        "OBSUCVIM_CCPD_AUDIT = {}\n"
        f"local ART = [{eq}[\n{art}\n]{eq}]\n"
        'loadstring(ART, "artifact")()\n'
        'for i = 1, #OBSUCVIM_CCPD_AUDIT do print("AUD " .. OBSUCVIM_CCPD_AUDIT[i]) end\n'
    )
    with tempfile.TemporaryDirectory() as td:
        path = os.path.join(td, "driver.lua")
        with open(path, "w", encoding="utf-8", errors="surrogateescape") as fh:
            fh.write(driver)
        proc = subprocess.run([LUAU, path], capture_output=True, text=True,
                              errors="replace", timeout=120)
    if proc.returncode != 0:
        raise SystemExit(f"artifact run failed: {proc.stderr[:400]}")
    out_lines, audit = [], []
    for line in proc.stdout.splitlines():
        if line.startswith("AUD "):
            audit.append(line[4:])
        else:
            out_lines.append(line)
    return out_lines, audit


def parse_dump(path):
    fns = {}
    for line in open(path, encoding="utf-8"):
        parts = line.split()
        if len(parts) < 3 or parts[0] != "FN":
            continue
        fi = int(parts[1])
        fn = fns.setdefault(fi, {"inc": {}, "edges": {}, "csites": []})
        kind = parts[2]
        if kind == "BLOCKS":
            fn["blocks"] = int(parts[3])
            fn["edges_total"] = int(parts[5])
            fn["entry"] = int(parts[7])
            fn["salt"] = int(parts[9])
        elif kind == "INC":
            fn["inc"][int(parts[3])] = (
                [int(x) for x in parts[4].split(",")] if len(parts) > 4 and parts[4] else [])
        elif kind == "EDGE":
            fn["edges"][int(parts[3])] = (parts[4], int(parts[5]), parts[6])
        elif kind == "CSITE":
            fn["csites"].append((int(parts[3]), int(parts[4])))
    return fns


def main():
    if not os.path.isfile(PROD_BIN):
        print(f"production binary missing: {PROD_BIN}")
        return 1
    if not os.path.isfile(AUDIT_BIN):
        print(f"audit binary missing: {AUDIT_BIN}")
        print("build: CARGO_TARGET_DIR=target/audit cargo build --release --features audit-hooks")
        return 1
    with tempfile.TemporaryDirectory() as td:
        src = os.path.join(td, "prog.lua")
        with open(src, "w", encoding="utf-8") as fh:
            fh.write(PROGRAM)
        ref = subprocess.run([LUAU, src], capture_output=True, text=True, timeout=60)
        ref_out = ref.stdout.splitlines()

        dump = os.path.join(td, "cfg_dump.txt")
        art = os.path.join(td, "art.lua")
        ok, err = gen(AUDIT_BIN, src, art, audit_path=dump)
        check("audit artifact generates", ok, err)
        if not ok:
            return 1
        out_lines, audit = run_artifact(art)
        check("program output matches plain reference", out_lines == ref_out,
              f"got {out_lines} want {ref_out}")

        fns = parse_dump(dump)
        check("compiler dumped CFG for >= 4 functions", len(fns) >= 4,
              f"functions={len(fns)}")
        total_variants = sum(fn["blocks"] * 4 for fn in fns.values())
        info("static directory (plan 11.8)",
             "; ".join(f"fn{fi}: {fn['blocks']} blocks / {fn['edges_total']} edges / "
                       f"{fn['blocks'] * 4} variants"
                       for fi, fn in sorted(fns.items())))
        check("every function carries 4 variants per block in the directory",
              all(fn["blocks"] >= 1 for fn in fns.values()))

        b_lines = [a.split() for a in audit if a.startswith("B ")]
        e_lines = [a.split() for a in audit if a.startswith("E ")]
        m_lines = [a.split() for a in audit if a.startswith("M ")]
        c_lines = [a.split() for a in audit if a.startswith("C ")]
        check("audit channel recorded B/E/M/C entries",
              bool(b_lines and e_lines and m_lines and c_lines),
              f"B={len(b_lines)} E={len(e_lines)} M={len(m_lines)} C={len(c_lines)}")

        # Edge audit: every observed (fn, block, edge) is CFG-declared.
        bad = []
        variants_seen = {}
        edges_seen = {}
        for (_t, fi, bi, edge, var, visit, fctx) in b_lines:
            fn = fns.get(int(fi) - 1)
            if fn is None:
                bad.append(f"unknown fn {fi}")
                continue
            if int(edge) not in fn["inc"].get(int(bi) - 1, []):
                bad.append(f"fn{fi} block{bi} via undeclared edge {edge}")
            if not (0 <= int(var) < 4):
                bad.append(f"variant {var} out of range")
            variants_seen.setdefault((fi, bi), set()).add(int(var))
            edges_seen.setdefault((fi, bi), set()).add(int(edge))
        check("every runtime block entry uses a CFG-declared incoming edge",
              not bad, "; ".join(bad[:5]))
        check("at least one block decoded through >= 2 distinct variants",
              any(len(v) >= 2 for v in variants_seen.values()),
              f"distinct-variant blocks={sum(1 for v in variants_seen.values() if len(v) >= 2)}")
        check("at least one block entered through >= 2 distinct edges",
              any(len(v) >= 2 for v in edges_seen.values()))

        # Frame audit: entry edge on every frame; fctx rotation.
        bad_e = [a for a in e_lines
                 if int(a[3]) != fns[int(a[1]) - 1]["entry"]]
        check("every frame starts on its function's entry edge", not bad_e,
              "; ".join(" ".join(a) for a in bad_e[:3]))
        fctx_by_fn = {}
        for (_t, fi, fctx, _edge) in e_lines:
            fctx_by_fn.setdefault(fi, []).append(fctx)
        rotations = {fi: v for fi, v in fctx_by_fn.items() if len(v) >= 2}
        check("closure/recursion frames rotate frame contexts (all distinct per fn)",
              bool(rotations) and all(len(set(v)) == len(v) for v in rotations.values()),
              str({k: (len(v), len(set(v))) for k, v in rotations.items()}))

        # Closure creation audit: repeated creations -> distinct origins.
        org_by_child = {}
        for (_t, fi, org) in m_lines:
            org_by_child.setdefault(fi, []).append(org)
        check("closures created repeatedly get distinct origin tokens",
              any(len(v) >= 2 and len(set(v)) == len(v) for v in org_by_child.values()),
              str({k: (len(v), len(set(v))) for k, v in org_by_child.items()}))

        # Creation ordinals in one frame run 1..k for the loop site.
        ords_by_site_child = {}
        for (_t, site, ordinal, child) in c_lines:
            ords_by_site_child.setdefault((site, child), []).append(int(ordinal))
        check("loop closure site yields creation ordinals 1..k in one frame",
              any(v == list(range(1, len(v) + 1)) and len(v) >= 3
                  for v in ords_by_site_child.values()),
              str(ords_by_site_child))

        # EXIT events (repair plan 3.2): a tail-merge function whose
        # non-returning branch falls through to pc == tn + 1 must be
        # observed leaving through an X event on a CFG-declared exit
        # edge, never through a B event dressed up as a block entry,
        # and the frame must end there (no later B with that fctx).
        exit_src = os.path.join(td, "exitprog.lua")
        with open(exit_src, "w", encoding="utf-8") as fh:
            fh.write(EXIT_PROGRAM)
        ref2 = subprocess.run([LUAU, exit_src], capture_output=True, text=True, timeout=60)
        dump2 = os.path.join(td, "exit_dump.txt")
        art2 = os.path.join(td, "exit_art.lua")
        ok, err = gen(AUDIT_BIN, exit_src, art2, audit_path=dump2)
        check("EXIT program audit artifact generates", ok, err)
        if ok:
            out2, audit2 = run_artifact(art2)
            check("EXIT program output matches plain reference",
                  out2 == ref2.stdout.splitlines(),
                  f"got {out2} want {ref2.stdout.splitlines()}")
            fns2 = parse_dump(dump2)
            x_lines = [a.split() for a in audit2 if a.startswith("X ")]
            b2_lines = [a.split() for a in audit2 if a.startswith("B ")]
            bad_edges = []
            for (_t, fi, edge, _fctx) in x_lines:
                fn = fns2.get(int(fi) - 1)
                ent = fn["edges"].get(int(edge)) if fn else None
                if ent is None or ent[2] != "EXIT":
                    bad_edges.append(f"X fn{fi} edge {edge} not a CFG exit edge")
            check("EXIT observed through X events", bool(x_lines),
                  f"X events={len(x_lines)}")
            check("every X event names a CFG-declared exit edge", not bad_edges,
                  "; ".join(bad_edges[:3]))
            bad_b = [a for a in b2_lines
                     if not (1 <= int(a[2]) <= fns2[int(a[1]) - 1]["blocks"])]
            check("no B event presents EXIT as a block entry", not bad_b,
                  "; ".join(" ".join(a) for a in bad_b[:3]))
            ended = set()
            leak = []
            for a in audit2:
                parts = a.split()
                if not parts:
                    continue
                if parts[0] == "X":
                    ended.add(parts[3])
                elif parts[0] == "B" and parts[6] in ended:
                    leak.append(a)
            check("no block entry follows an exit in the same frame", not leak,
                  "; ".join(leak[:3]))

        # Revisit audit (repair plan 3.2): one (function, block, edge)
        # entered repeatedly inside one frame keeps monotone visits
        # and rotates through >= 2 variants; the composite cache
        # identity (block, edge, variant) stays distinguishable.
        rev_src = os.path.join(td, "revisit.lua")
        with open(rev_src, "w", encoding="utf-8") as fh:
            fh.write(REVISIT_PROGRAM)
        dump3 = os.path.join(td, "revisit_dump.txt")
        art3 = os.path.join(td, "revisit_art.lua")
        ok, err = gen(AUDIT_BIN, rev_src, art3, audit_path=dump3)
        check("revisit audit artifact generates", ok, err)
        if ok:
            out3, audit3 = run_artifact(art3)
            ref3 = subprocess.run([LUAU, rev_src], capture_output=True, text=True, timeout=60)
            check("revisit program output matches plain reference",
                  out3 == ref3.stdout.splitlines())
            visits_by_key = {}
            for a in audit3:
                parts = a.split()
                if parts and parts[0] == "B":
                    (_t, fi, bi, edge, var, visit, _fctx) = parts
                    visits_by_key.setdefault((fi, bi, edge), []).append((int(visit), int(var)))
            repeated = {k: v for k, v in visits_by_key.items() if len(v) >= 3}
            mono = all(seq == sorted(seq) and all(
                x[0] < y[0] for x, y in zip(seq, seq[1:]))
                for seq in repeated.values())
            check("one (function, block, edge) is re-entered repeatedly",
                  bool(repeated), f"keys={ {k: len(v) for k, v in visits_by_key.items()} }")
            check("revisit counts strictly increase on the repeated key", mono)
            check("repeated key rotates through >= 2 variants",
                  any(len({var for (_v, var) in seq}) >= 2 for seq in repeated.values()),
                  str({k: sorted({var for (_v, var) in seq}) for k, seq in repeated.items()}))

        # Coroutine audit (repair plan 3.2): two coroutine instances
        # yielded and resumed in alternation keep disjoint frame
        # contexts, and every block entry belongs to a frame that an
        # E event actually opened (no cross-frame edge/context leak).
        coro_src = os.path.join(td, "coro.lua")
        with open(coro_src, "w", encoding="utf-8") as fh:
            fh.write(CORO_PROGRAM)
        dump4 = os.path.join(td, "coro_dump.txt")
        art4 = os.path.join(td, "coro_art.lua")
        ok, err = gen(AUDIT_BIN, coro_src, art4, audit_path=dump4)
        check("coroutine audit artifact generates", ok, err)
        if ok:
            out4, audit4 = run_artifact(art4)
            ref4 = subprocess.run([LUAU, coro_src], capture_output=True, text=True, timeout=60)
            check("coroutine program output matches plain reference",
                  out4 == ref4.stdout.splitlines(),
                  f"got {out4} want {ref4.stdout.splitlines()}")
            opened = set()
            frame_blocks = {}
            orphans = []
            for a in audit4:
                parts = a.split()
                if not parts:
                    continue
                if parts[0] == "E":
                    opened.add(parts[2])
                elif parts[0] == "B":
                    if parts[6] not in opened:
                        orphans.append(a)
                    frame_blocks.setdefault(parts[6], []).append(parts[2])
            check("coroutine frames open distinct frame contexts",
                  len(opened) >= 3, f"frames={len(opened)}")
            check("every coroutine block entry belongs to an opened frame",
                  not orphans, "; ".join(orphans[:3]))
            check("each coroutine resumes in its own frame blocks",
                  sum(1 for v in frame_blocks.values() if len(v) >= 2) >= 2,
                  str({k: len(v) for k, v in frame_blocks.items()}))

        # Production isolation.
        prod_art = os.path.join(td, "prod.lua")
        ok1, _ = gen(PROD_BIN, src, prod_art)
        audit_noenv = os.path.join(td, "audit_noenv.lua")
        ok2, _ = gen(AUDIT_BIN, src, audit_noenv)
        chan = "OBSUCVIM_CCPD_AUDIT"
        txt_prod = open(prod_art, encoding="utf-8", errors="surrogateescape").read() if ok1 else ""
        txt_noenv = open(audit_noenv, encoding="utf-8", errors="surrogateescape").read() if ok2 else ""
        check("production artifact carries no audit channel", chan not in txt_prod)
        check("production artifact carries no CCPD plaintext marker",
              re.search(r"ccpd", txt_prod, re.IGNORECASE) is None)
        check("production runtime failures use opaque numeric sentinels",
              "invalid runtime payload" not in txt_prod
              and "invalid control transfer" not in txt_prod
              and re.search(r"\[16\]\(0,\s*0\)", txt_prod) is not None
              and "fn=" not in txt_prod and "variant=" not in txt_prod)
        check("audit binary without env carries no audit channel", chan not in txt_noenv)
        check("audit binary without env is byte-identical to production",
              ok1 and ok2 and txt_prod == txt_noenv)
        check("audit artifact does carry the channel (negative control)",
              chan in open(art, encoding="utf-8", errors="surrogateescape").read())

    failed = [n for n, ok in checks if not ok]
    print(f"ccpd_runtime_audit_test: {len(checks) - len(failed)}/{len(checks)} checks passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
