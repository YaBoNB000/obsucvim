#!/usr/bin/env python3
"""Phase 18 P1-1/P1-2 harvest simulation: what a generic harvester sees.

The fixture (tests/fixtures/string_cache_harvester.lua) plays two roles:

  * "off-the-shelf harvester": globally replaces `string.char` with a
    recorder and rebuilds every decoded string afterwards with
    `table.concat` + pattern matching. It knows nothing about the VM.
  * "cache dumper": reports what the VM actually stored in its string cache
    (audit-only channels, compiled out of production artifacts).

What is asserted (and why):

  P1-1 core claim -- a constant with a single static reference is decoded
  inline and never written into the persistent cache table:
      * 10 SINGLE_USE_MARKER_* constants (one static reference each) must not
        appear in the cache dump.
      * negative control: all 4 MULTI_USE_MARKER_* constants (three static
        references each) must appear, otherwise the dumper itself is broken
        and the first assertion would be vacuous.
  P1-2 scope -- clear-after-last-use:
      * the 4 multi-use markers must appear in the clear log;
      * a marker referenced inside a loop body and a marker referenced from
        two different closures must never be cleared.
  Honest boundary (reported, not asserted as protection):
      * a global `string.char` hook still observes every decoded constant,
        including the single-use ones, because decoding a constant has to
        build its bytes one way or another. P1-1 removes the *persistent*
        copy, not the transient decode. The count is printed as INFO so the
        HANDOFF threat model can quote the real number.
  Production isolation:
      * the production artifact must contain no audit channel text.

Usage: python3 tests/string_cache_dump_test.py
Exit code: 0 = all assertions hold, 1 = at least one failed.
"""
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
PROD_BIN = os.environ.get("OBSUCVIM_BIN", os.path.join(ROOT, "target/release/obsucvim"))
AUDIT_BIN = os.environ.get(
    "OBSUCVIM_AUDIT_BIN", os.path.join(ROOT, "target/audit/release/obsucvim")
)
FIXTURE = os.path.join(HERE, "fixtures", "string_cache_harvester.lua")
TOOLS_BIN = os.environ.get("OBSUCVIM_TOOLS", os.path.join(os.path.dirname(ROOT), ".tools", "bin"))
LUAU = os.path.join(TOOLS_BIN, "luau")

SINGLE = [f"SINGLE_USE_MARKER_{i:02d}" for i in range(1, 11)]
MULTI = [f"MULTI_USE_MARKER_{i:02d}" for i in range(1, 5)]
LOOP_MARK = "LOOP_BODY_MARKER"
CLOSURE_MARK = "CLOSURE_CAPTURED_MARKER"

checks = []


def check(name, ok, detail=""):
    checks.append((name, ok, detail))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}{(' :: ' + detail) if detail and not ok else ''}")


def info(name, detail):
    print(f"  [INFO] {name} :: {detail}")


def build_source():
    parts = []
    for m in SINGLE:
        parts.append(f'local s = "{m}"\nprint(s)\n')
    for m in MULTI:
        parts.append(f'print("{m}", "{m}", "{m}")\n')
    # loop: same plaintext referenced twice, but inside a loop body. The loop
    # lives in its own function so the top-level body stays loop-free (a loop in
    # the top-level body would veto clearing for every top-level constant).
    parts.append(
        f'local function withloop() for i = 1, 2 do print("{LOOP_MARK}", "{LOOP_MARK}") end end\n'
        "withloop()\n"
    )
    # closure: same plaintext referenced in two different frames
    parts.append(f'local function outer() return "{CLOSURE_MARK}" end\n')
    parts.append(f'local function inner() return "{CLOSURE_MARK}" end\n')
    parts.append("print(outer(), inner())\n")
    return "".join(parts)


def gen_artifact(binary, src_path, out_path, tag):
    env = dict(os.environ)
    if tag == "audit":
        env["OBSUCVIM_STR_CACHE_DUMP"] = "1"
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
        env=env,
    )
    if proc.returncode != 0:
        raise SystemExit(f"artifact generation failed ({tag}): {proc.stderr}")


def make_harness(artifact_path, harness_path):
    art = open(artifact_path, "r", encoding="utf-8", errors="surrogateescape").read()
    wrap = None
    for lvl in range(1, 8):
        closer = "]" + "=" * lvl + "]"
        if closer not in art:
            wrap = f'local ART = [{"=" * lvl}[\n{art}\n]{"=" * lvl}]\n'
            break
    if wrap is None:
        raise SystemExit("cannot wrap artifact in a long string")
    body = open(FIXTURE).read()
    with open(harness_path, "w", encoding="utf-8", errors="surrogateescape") as fh:
        fh.write(wrap + body)


def run_harvest(harness_path):
    # The harvester rebuilds raw byte strings, so the transcript is not
    # guaranteed to be valid UTF-8.
    proc = subprocess.run(
        [LUAU, harness_path],
        capture_output=True,
        text=True,
        errors="replace",
        timeout=180,
    )
    harvested, dumped, cleared = [], [], []
    section = None
    run_ok = None
    for line in proc.stdout.splitlines():
        if line == "HARVEST_BEGIN":
            section = "harvest"
        elif line == "HARVEST_END":
            section = None
        elif line == "DUMP_BEGIN":
            section = "dump"
        elif line == "DUMP_END":
            section = None
        elif line == "CLR_BEGIN":
            section = "clr"
        elif line == "CLR_END":
            section = None
        elif line.startswith("RUN_OK:"):
            run_ok = line.split(":", 1)[1] == "true"
        elif line.startswith("H:"):
            harvested.append(line[2:])
        elif line.startswith("D:"):
            dumped.append(line[2:])
        elif line.startswith("C:"):
            cleared.append(line[2:])
    return run_ok, harvested, dumped, cleared, proc.stdout + proc.stderr


def main():
    if not os.path.exists(AUDIT_BIN):
        raise SystemExit(
            f"missing audit binary {AUDIT_BIN}; build it with "
            "`cargo build --release --features audit-hooks --target-dir target/audit`"
        )
    tmp = tempfile.mkdtemp(prefix="scdump_")
    src = os.path.join(tmp, "harvest.lua")
    with open(src, "w") as fh:
        fh.write(build_source())

    prod_out = os.path.join(tmp, "prod.lua")
    audit_out = os.path.join(tmp, "audit.lua")
    gen_artifact(PROD_BIN, src, prod_out, "prod")
    gen_artifact(AUDIT_BIN, src, audit_out, "audit")

    print("== P1-1: single-use constants must not be cached ==")
    harness = os.path.join(tmp, "harvester.lua")
    make_harness(audit_out, harness)
    run_ok, harvested, dumped, cleared, raw = run_harvest(harness)
    check("artifact runs under the harvester", run_ok, raw[-400:])

    missing_multi = [m for m in MULTI if not any(m in d for d in dumped)]
    check(
        "negative control: all 4 multi-use markers reach the cache",
        not missing_multi,
        f"missing={missing_multi}",
    )
    leaked_single = [m for m in SINGLE if any(m in d for d in dumped)]
    check(
        "10 single-use markers never reach the cache",
        not leaked_single,
        f"leaked={leaked_single}",
    )

    print("== P1-2: clear-after-last-use scope ==")
    cleared_multi = [m for m in MULTI if any(m in c for c in cleared)]
    check(
        "multi-use markers are cleared after their last use",
        len(cleared_multi) == len(MULTI),
        f"cleared={cleared_multi}",
    )
    check(
        "loop-body marker is never cleared",
        not any(LOOP_MARK in c for c in cleared),
        "loop back-edge must veto clearing",
    )
    check(
        "closure-captured marker is never cleared",
        not any(CLOSURE_MARK in c for c in cleared),
        "cross-frame reference must veto clearing",
    )

    print("== documented boundary (not a protection claim) ==")
    hook_single = [m for m in SINGLE if any(m in h for h in harvested)]
    hook_multi = [m for m in MULTI if any(m in h for h in harvested)]
    info(
        "global string.char hook sees decoded constants",
        f"single-use {len(hook_single)}/{len(SINGLE)}, multi-use {len(hook_multi)}/{len(MULTI)}"
        " -> P1-1 removes the persistent copy, not the transient decode",
    )

    print("== production isolation ==")
    prod_text = open(prod_out, "r", encoding="utf-8", errors="surrogateescape").read()
    check(
        "production artifact carries no audit channel",
        "OBSUCVIM_SC_DUMP" not in prod_text and "OBSUCVIM_SC_CLR" not in prod_text,
    )

    failed = [c for c in checks if not c[1]]
    print("==================================================")
    print(f"STRING CACHE DUMP PASS: {len(checks) - len(failed)}   FAIL: {len(failed)}")
    if failed:
        for name, _, detail in failed:
            print(f"  failed: {name} :: {detail}")
        return 1
    print("ALL STRING CACHE DUMP GREEN")
    return 0


if __name__ == "__main__":
    sys.exit(main())
