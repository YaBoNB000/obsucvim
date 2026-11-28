#!/usr/bin/env python3
"""Phase 18 P1-3: distributed runtime string-cache canary.

`guard_matrix_test.py` proves the *boot* half (a tampered `string` library is
rejected before the payload runs). This suite proves the *distributed* half:
the four `Op::Canary` checks that the compiler sprinkles over the instruction
stream at `floor(n / 5) * k` for `k = 1..4`.

The payload itself replaces `string.char` with a non-function. That happens
after the boot guard has already passed, so only a mid-execution canary can
catch it.

Asserted:
  * guard build: the program dies on the guard's bounded fail-closed fault
    (`attempt to index boolean with boolean`) and never reaches the payload's
    final print.
  * `--no-guard` build: the canaries are not emitted, so the same payload runs
    to completion (this is the differential control that proves the rejection
    comes from the canary and not from a broken artifact).
  * the four canary ids really are distributed: the compiled artifact differs
    between seeds (the id -> split-point assignment is `Rng::shuffle`d).

Usage: python3 tests/canary_runtime_test.py
Exit code: 0 = all assertions hold, 1 = at least one failed.
"""
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
TOOL = os.environ.get("OBSUCVIM_BIN", os.path.join(ROOT, "target/release/obsucvim"))
TOOLS_BIN = os.environ.get("OBSUCVIM_TOOLS", os.path.join(os.path.dirname(ROOT), ".tools", "bin"))
LUAU = os.path.join(TOOLS_BIN, "luau")

PAYLOAD = 'string.char = 123\nprint("CANARY_RAN")\n'
HARNESS = r'''
local ART = %s
local env = {}
for k, v in pairs(_G) do env[k] = v end
env._G = env
local chunk = assert(loadstring(ART, "art"))
setfenv(chunk, env)
local ok, err = pcall(chunk)
print("RUN_OK:" .. tostring(ok))
print("ERR:" .. tostring(err))
'''

checks = []


def check(name, ok, detail=""):
    checks.append((name, ok, detail))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}{(' :: ' + detail) if detail and not ok else ''}")


def long_str(s: str) -> str:
    for lvl in range(1, 32):
        eq = "=" * lvl
        if f"]{eq}]" not in s:
            return f"[{eq}[\n{s}]{eq}]"
    raise RuntimeError("cannot wrap long string")


def build(src_path, out_path, guard=True, seed=42):
    args = [TOOL, "--preset", "v15", "--dialect", "luau", "--seed", str(seed)]
    if not guard:
        args.append("--no-guard")
    args += [src_path, out_path]
    proc = subprocess.run(args, capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit(f"artifact generation failed (guard={guard}): {proc.stderr}")


def run(artifact_path):
    harness_path = artifact_path + ".harness.lua"
    with open(harness_path, "w", encoding="utf-8", errors="surrogateescape") as fh:
        fh.write(HARNESS % long_str(open(artifact_path, encoding="utf-8", errors="surrogateescape").read()))
    proc = subprocess.run(
        [LUAU, harness_path], capture_output=True, text=True, errors="replace", timeout=120
    )
    return proc.stdout + proc.stderr


def main():
    tmp = tempfile.mkdtemp(prefix="canary_")
    src = os.path.join(tmp, "payload.lua")
    with open(src, "w") as fh:
        fh.write(PAYLOAD)

    guard_art = os.path.join(tmp, "guard.lua")
    noguard_art = os.path.join(tmp, "noguard.lua")
    seed_art = os.path.join(tmp, "seed7.lua")
    build(src, guard_art, guard=True)
    build(src, noguard_art, guard=False)
    build(src, seed_art, guard=True, seed=7)

    print("== distributed canary: mid-execution string.char tamper ==")
    out = run(guard_art)
    tripped = "attempt to index boolean with boolean" in out
    check("guard build trips the bounded fail-closed path", tripped, out[-300:])
    check("payload never reaches its final print", "CANARY_RAN" not in out, out[-300:])

    print("== differential control ==")
    out_ng = run(noguard_art)
    check(
        "--no-guard runs the same payload to completion",
        "CANARY_RAN" in out_ng and "RUN_OK:true" in out_ng,
        out_ng[-300:],
    )

    print("== canary distribution is seed-dependent ==")
    a = open(guard_art, "rb").read()
    b = open(seed_art, "rb").read()
    check("different seeds produce different artifacts", a != b)

    failed = [c for c in checks if not c[1]]
    print("==================================================")
    print(f"CANARY RUNTIME PASS: {len(checks) - len(failed)}   FAIL: {len(failed)}")
    if failed:
        for name, _, detail in failed:
            print(f"  failed: {name} :: {detail}")
        return 1
    print("ALL CANARY RUNTIME GREEN")
    return 0


if __name__ == "__main__":
    sys.exit(main())
