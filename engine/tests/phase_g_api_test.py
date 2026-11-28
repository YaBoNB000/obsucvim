#!/usr/bin/env python3
"""Phase G (obf.lua tech-extraction spec section 6 / G4) runtime checks.

Covers the G4 acceptance list against the shipped v15 pipeline:

  * entry closure called multiple times;
  * entry closure invoked after its parent function has returned
    (closure saved into a table, called later);
  * coroutine resume continues execution across yields;
  * API slot remapping between builds (different seeds) keeps behavior
    identical while the artifact bytes change;
  * a host environment missing a required API (`string`) fails
    deterministically: non-zero exit, no payload output, same fault on
    repeated runs.

The API-slot layout itself is unit-tested in
`engine/src/vmgen/api_slots.rs`; this script only asserts the
externally observable contract.
"""
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
TOOL = ROOT / "target" / "release" / "obsucvim"
TOOLS_BIN = ROOT.parent / ".tools" / "bin"
LUAU = TOOLS_BIN / "luau"

CASES = {
    # Closure factory: the returned entry closures are called repeatedly,
    # stored in a table, and invoked after `make` has returned.
    "multi_call": (
        "local function make()\n"
        "  local n = 0\n"
        "  return function(d) n = n + d return n end\n"
        "end\n"
        "local a, b = make(), make()\n"
        "local saved = { a, b }\n"
        "print(a(1), a(2), b(10))\n"
        "print(saved[1](100), saved[2](100))\n"
    ),
    # Coroutine: resume continues the VM where it suspended, with a
    # closure created inside the coroutine body.
    "coroutine": (
        "local co = coroutine.wrap(function(x)\n"
        "  local add = function(d) return x + d end\n"
        "  local y = coroutine.yield(add(1))\n"
        "  return add(y)\n"
        "end)\n"
        "print(co(10))\n"
        "print(co(100))\n"
    ),
    # Deferred calls through a table, invoked after definition site.
    "deferred_table": (
        "local t = {}\n"
        "for i = 1, 3 do t[i] = function() return i * i end end\n"
        "local s = 0\n"
        "for i = 3, 1, -1 do s = s + t[i]() end\n"
        "print(s)\n"
    ),
}

MISSING_API_SRC = (
    'local s = ("ab" .. "cd"):upper()\n'
    'print("PAYLOAD_RAN", s, ("x"):rep(3))\n'
)


def run(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, timeout=120, **kw)


def build(src_path, out_path, seed):
    proc = run(
        [
            str(TOOL),
            "--preset",
            "v15",
            "--dialect",
            "luau",
            "--seed",
            str(seed),
            str(src_path),
            str(out_path),
        ]
    )
    if proc.returncode != 0:
        raise RuntimeError(f"build failed seed={seed}: {proc.stderr.decode()[:200]}")


def harness_missing_string(artifact: str) -> str:
    lit = artifact.replace("\\", "\\\\").replace("]", "\\]")
    level = 1
    while "]" + "=" * level + "]" in artifact:
        level += 1
    eq = "=" * level
    return (
        "local REAL_G = getfenv(0)\n"
        "local REAL_LOADSTRING = REAL_G.loadstring\n"
        "local REAL_SETFENV = REAL_G.setfenv\n"
        f"local ART = [{eq}[{artifact}]{eq}]\n"
        "local ENV = {}\n"
        "for k, v in pairs(REAL_G) do ENV[k] = v end\n"
        "ENV._G = ENV\n"
        "ENV.string = nil\n"
        "local chunk = assert(REAL_LOADSTRING(ART, '=phaseg'))\n"
        "REAL_SETFENV(chunk, ENV)\n"
        "chunk()\n"
    )


def main() -> int:
    failures = []

    def check(name, ok, detail=""):
        print(f"  [{'PASS' if ok else 'FAIL'}] {name}" + (f" :: {detail}" if detail and not ok else ""))
        if not ok:
            failures.append(name)

    if not TOOL.exists() or not LUAU.exists():
        print("missing release binary or luau toolchain", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory(prefix="phase_g_") as td:
        tmp = pathlib.Path(td)

        for name, src in CASES.items():
            src_path = tmp / f"{name}.lua"
            src_path.write_text(src)
            expected = run([str(LUAU), str(src_path)]).stdout
            for seed in (42, 7, 99):
                out_path = tmp / f"{name}_{seed}.lua"
                build(src_path, out_path, seed)
                got = run([str(LUAU), str(out_path)])
                check(
                    f"{name} seed={seed}",
                    got.returncode == 0 and got.stdout == expected,
                    f"rc={got.returncode} out={got.stdout!r} err={got.stderr.decode()[:120]!r}",
                )

        # Slot remapping: same program, different seeds -> different
        # bytes (layout reshuffled), identical behavior.
        src_path = tmp / "multi_call.lua"
        expected = run([str(LUAU), str(src_path)]).stdout
        arts = []
        for seed in (1, 2, 3):
            out_path = tmp / f"remap_{seed}.lua"
            build(src_path, out_path, seed)
            arts.append(out_path.read_bytes())
            got = run([str(LUAU), str(out_path)])
            check(f"slot-remap behavior seed={seed}", got.returncode == 0 and got.stdout == expected)
        check(
            "slot-remap bytes differ across builds",
            len(set(arts)) == len(arts),
            "artifacts byte-identical across seeds; slot layout not reshuffled",
        )

        # Missing required API: stable, deterministic failure.
        src_path = tmp / "missing_api.lua"
        src_path.write_text(MISSING_API_SRC)
        out_path = tmp / "missing_api_obf.lua"
        build(src_path, out_path, 42)
        harness_path = tmp / "missing_api_harness.lua"
        harness_path.write_text(harness_missing_string(out_path.read_text()))
        runs = [run([str(LUAU), str(harness_path)]) for _ in range(2)]
        first_errs = [r.stderr.decode().splitlines()[:1] for r in runs]
        check(
            "missing string API fails closed",
            all(r.returncode != 0 for r in runs)
            and all(b"PAYLOAD_RAN" not in r.stdout for r in runs),
            f"rcs={[r.returncode for r in runs]} outs={[r.stdout[:60] for r in runs]}",
        )
        check(
            "missing string API failure is deterministic",
            first_errs[0] == first_errs[1] and len(first_errs[0]) == 1,
            f"stderr first lines differ: {first_errs}",
        )

    if failures:
        print(f"phase_g_api_test: {len(failures)} FAIL")
        return 1
    print("phase_g_api_test: ALL PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
