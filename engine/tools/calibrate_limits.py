#!/usr/bin/env python3
"""Mechanical threshold calibration for engine/src/limits.rs (Phase 18 P0-1, §0.4).

For every limited dimension D the tool runs the §0.4 binary search:

    low = good_floor, high = bad
    while high - low > 1:
        mid = (low + high) // 2
        generate an input whose dimension D == mid (all other dims minimal)
        run the release binary with the default v15 preset
        crash / hang / signal  -> high = mid
        clean run (rc 0 + non-empty artifact) -> low = mid
    threshold = floor(low * 0.8)   (0 -> 1, noted as a narrow margin)

The binary MUST be built with the limits disabled (`--prepare` rewrites the
constants to usize::MAX and rebuilds), otherwise the search only finds the
already-configured threshold instead of the real failure boundary.

Usage:
    python3 tools/calibrate_limits.py --prepare          # disable + rebuild
    python3 tools/calibrate_limits.py --bin target/release/obsucvim
    python3 tools/calibrate_limits.py --restore          # put the constants back
"""

import argparse
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LIMITS_RS = os.path.join(ROOT, "src", "limits.rs")
BIN_DEFAULT = os.path.join(ROOT, "target", "release", "obsucvim")
TOOLS_BIN = os.path.join(os.path.dirname(ROOT), ".tools", "bin")

CONSTS = [
    "MAX_TOPLEVEL_FUNC_DECLS",
    "MAX_TOPLEVEL_STMTS",
    "MAX_CLOSURE_NEST_DEPTH",
    "MAX_EXPR_PARSE_DEPTH",
    "MAX_BLOCK_NEST_DEPTH",
]

DISABLE_SNIPPET = "\n".join(
    f"pub const {c}: usize = usize::MAX;" for c in CONSTS
)


def read_limits():
    with open(LIMITS_RS) as fh:
        return fh.read()


def write_limits(text):
    with open(LIMITS_RS, "w") as fh:
        fh.write(text)


def strip_consts(text):
    """Remove the pub const lines, keeping the rest of the file."""
    out = []
    for line in text.splitlines():
        if any(line.startswith(f"pub const {c}:") for c in CONSTS):
            continue
        out.append(line)
    return "\n".join(out)


def disable_limits():
    text = read_limits()
    body = strip_consts(text)
    # Re-insert right after the module doc comment.
    lines = body.splitlines()
    insert_at = 0
    for i, line in enumerate(lines):
        if line.startswith("//!"):
            insert_at = i + 1
        else:
            break
    lines.insert(insert_at, DISABLE_SNIPPET)
    write_limits("\n".join(lines) + "\n")


def restore_limits(values):
    text = read_limits()
    body = strip_consts(text)
    lines = body.splitlines()
    insert_at = 0
    for i, line in enumerate(lines):
        if line.startswith("//!"):
            insert_at = i + 1
        else:
            break
    block = []
    for name in CONSTS:
        if name in values:
            block.append(f"pub const {name}: usize = {values[name]};")
    lines.insert(insert_at, "\n".join(block))
    write_limits("\n".join(lines) + "\n")


def build():
    env = dict(os.environ)
    env["PATH"] = TOOLS_BIN + os.pathsep + env.get("PATH", "")
    env["CARGO_NET_OFFLINE"] = "true"
    subprocess.run(
        ["cargo", "build", "--release", "--quiet"],
        cwd=ROOT,
        env=env,
        check=True,
    )


# ---------------------------------------------------------------- generators

def gen_toplevel_funcs(n):
    body = "\n".join(f"local function f{i}() return {i} end" for i in range(n))
    return body + "\nprint(f0())\n"


def gen_toplevel_stmts(n):
    return "\n".join(f"a{i} = {i}" for i in range(n)) + "\nprint(a0)\n"


def gen_closure_nest(n):
    out = "return 1"
    for i in range(n):
        out = f"local function f{i}() {out} end"
    return out + "\nprint(f0())\n"


def gen_expr_depth(n):
    return "local x = " + "(" * n + "1" + ")" * n + "\nprint(x)\n"


def gen_block_depth(n):
    return "do " * n + "x=1" + " end" * n + "\nprint(x)\n"


DIMS = [
    ("MAX_TOPLEVEL_FUNC_DECLS", gen_toplevel_funcs, 0, 100),
    ("MAX_TOPLEVEL_STMTS", gen_toplevel_stmts, 0, 3000),
    ("MAX_CLOSURE_NEST_DEPTH", gen_closure_nest, 1, 20),
    ("MAX_EXPR_PARSE_DEPTH", gen_expr_depth, 0, 20000),
    ("MAX_BLOCK_NEST_DEPTH", gen_block_depth, 0, 20000),
]


def run_case(binary, src, tmpdir, tag):
    src_path = os.path.join(tmpdir, f"cal_{tag}.lua")
    out_path = os.path.join(tmpdir, f"cal_{tag}_out.lua")
    with open(src_path, "w") as fh:
        fh.write(src)
    if os.path.exists(out_path):
        os.remove(out_path)
    try:
        proc = subprocess.run(
            [
                binary,
                "--preset",
                "v15",
                "--dialect",
                "luau",
                "--seed",
                "42",
                src_path,
                out_path,
            ],
            capture_output=True,
            text=True,
            timeout=60,
        )
    except subprocess.TimeoutExpired:
        return "hang", "timeout after 60s"
    err = (proc.stderr or "") + (proc.stdout or "")
    bad_markers = ("panicked at", "RUST_BACKTRACE", "stack overflow", "SIGABRT", "SIGSEGV")
    if any(m in err for m in bad_markers) or proc.returncode < 0 or proc.returncode >= 128:
        return "crash", f"rc={proc.returncode} {err.strip().splitlines()[:2]}"
    if proc.returncode != 0:
        return "error", f"rc={proc.returncode} {err.strip().splitlines()[:1]}"
    size = os.path.getsize(out_path) if os.path.exists(out_path) else 0
    if size == 0:
        return "error", "rc=0 but empty artifact"
    return "ok", f"rc=0 size={size}"


def bisect(binary, name, gen, good_floor, bad, tmpdir):
    low, high = good_floor, bad
    # sanity: bad must actually fail
    kind, detail = run_case(binary, gen(bad), tmpdir, f"{name}_bad")
    print(f"  [{name}] bad={bad} -> {kind} ({detail})")
    if kind == "ok":
        print(f"  [{name}] WARNING: bad value {bad} no longer fails; "
              f"cannot calibrate (already clean).")
        return None
    kind, detail = run_case(binary, gen(good_floor) if good_floor else "print(1)\n",
                            tmpdir, f"{name}_floor")
    print(f"  [{name}] floor={good_floor} -> {kind} ({detail})")
    if kind != "ok":
        print(f"  [{name}] WARNING: floor value {good_floor} does not run cleanly.")
        return None
    steps = 0
    while high - low > 1:
        mid = (low + high) // 2
        kind, detail = run_case(binary, gen(mid), tmpdir, f"{name}_mid")
        steps += 1
        print(f"    mid={mid} -> {kind} ({detail})")
        if kind == "ok":
            low = mid
        else:
            high = mid
    threshold = (low * 4) // 5
    narrow = False
    if threshold == 0:
        threshold = 1
        narrow = True
    print(f"  [{name}] low={low} high={high} threshold=floor({low}*0.8)={threshold}"
          + ("  [NARROW MARGIN: minimum usable value taken]" if narrow else ""))
    return {
        "name": name,
        "low": low,
        "high": high,
        "threshold": threshold,
        "narrow": narrow,
        "steps": steps,
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=BIN_DEFAULT)
    ap.add_argument("--prepare", action="store_true")
    ap.add_argument("--restore", action="store_true")
    ap.add_argument("--only", default=None,
                    help="comma separated constant names to calibrate")
    ap.add_argument("--out", default=None, help="write results as JSON")
    args = ap.parse_args()

    if args.prepare:
        disable_limits()
        print("limits disabled, rebuilding ...")
        build()
        return 0
    if args.restore:
        restore_limits({})
        print("limits.rs stripped of const block; re-add final values")
        return 0

    os.makedirs("/tmp/obsucvim_cal", exist_ok=True)
    results = []
    only = set(args.only.split(",")) if args.only else None
    for name, gen, floor, bad in DIMS:
        if only and name not in only:
            continue
        r = bisect(args.bin, name, gen, floor, bad, "/tmp/obsucvim_cal")
        if r:
            results.append(r)
    print("\n==== calibration summary ====")
    for r in results:
        print(f"{r['name']}: low={r['low']} high={r['high']} "
              f"threshold={r['threshold']} (bisect steps={r['steps']})")
    if args.out:
        import json
        with open(args.out, "w") as fh:
            json.dump(results, fh, indent=2)
    return 0


if __name__ == "__main__":
    sys.exit(main())
