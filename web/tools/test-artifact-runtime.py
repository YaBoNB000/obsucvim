#!/usr/bin/env python3
"""Prove that pipeline artifacts actually *run*, not just compile.

The 2026-10-23 incident: every published artifact died inside a real executor
with `attempt to call a nil value`, because the engine was asked for
`--bind-executor` / `--bind-env roblox`. Those layers verify exact host API
shapes at startup and self-poison when anything differs, so the whole script
dies. A compile check would never catch that — only running it does.

This script:
  1. runs the real WASI engine (web/tools/obsucvim.wasm) with the flags the
     server now builds (and optionally the opt-in hardening flags),
  2. executes each artifact in a real Luau runtime, twice:
       a. bare (stock Luau),
       b. after web/tools/luau-roblox-prelude.luau, which fakes the executor /
          Roblox surface a Delta-like host provides,
  3. asserts the program prints exactly what the source prints.

Requires a `luau` binary: set LUAU_BIN, or have one on PATH.
Without it the script exits 0 with a SKIP line (CI without Luau stays green,
but you should run this before shipping a release).

    LUAU_BIN=/tmp/luau-src/luau-master/build/release/luau \
      python3 web/tools/test-artifact-runtime.py
"""
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

TOOLS = pathlib.Path(__file__).resolve().parent
WASM = TOOLS / "obsucvim.wasm"
RUNNER = TOOLS / "wasi-runner.mjs"
PRELUDE = TOOLS / "luau-roblox-prelude.luau"

SOURCE = 'print("suda")\n'
EXPECTED = "suda"
SEEDS = ["1", "2", "3", "20261022", "424242", "987654"]

BASE_FLAGS = ["--preset", "v15", "--env-check", "--minify", "--vm-seal-strings"]
HARDENING_FLAGS = ["--bind-executor", "--bind-env", "roblox"]


def find_luau():
    candidate = os.environ.get("LUAU_BIN") or shutil.which("luau")
    if candidate and pathlib.Path(candidate).exists():
        return candidate
    return None


def build(workdir, flags, seed, name):
    args = flags + ["--seed", seed, "/work/input.lua", f"/work/{name}.lua"]
    cfg = {"wasmPath": str(WASM), "workDir": str(workdir), "args": args}
    cfg_path = workdir / "cfg.json"
    cfg_path.write_text(json.dumps(cfg))
    proc = subprocess.run(
        ["node", str(RUNNER), cfg_path.read_text()],
        capture_output=True, text=True, timeout=300,
    )
    out = workdir / f"{name}.lua"
    if not out.exists() or out.stat().st_size == 0:
        return None, f"engine rc={proc.returncode} {proc.stderr.strip()[:160]}"
    return out.read_text(), None


def run_luau(luau, path):
    proc = subprocess.run([luau, str(path)], capture_output=True, text=True, timeout=120)
    return proc.returncode, (proc.stdout + proc.stderr).strip()


def main():
    luau = find_luau()
    if not luau:
        print("SKIP artifact runtime test: no luau binary (set LUAU_BIN)")
        return 0

    work = pathlib.Path(tempfile.mkdtemp(prefix="obsucvim-artifact-"))
    (work / "input.lua").write_text(SOURCE)
    failures = []

    def check(name, ok, detail=""):
        print(("PASS " if ok else "FAIL ") + name + ((" | " + detail) if detail else ""))
        if not ok:
            failures.append(name)

    try:
        # 1. shipping defaults must run everywhere
        for seed in SEEDS:
            artifact, error = build(work, BASE_FLAGS + ["--no-bind-executor"], seed, f"base{seed}")
            if artifact is None:
                check(f"default config seed {seed}: engine produced output", False, error)
                continue
            bare = work / f"bare-b{seed}.luau"
            bare.write_text(artifact)
            rc, out = run_luau(luau, bare)
            check(f"default config seed {seed}: runs in stock Luau", rc == 0 and out == EXPECTED, out[:120])

            with_prelude = work / f"prelude-b{seed}.luau"
            with_prelude.write_text(PRELUDE.read_text() + "\n" + artifact)
            rc, out = run_luau(luau, with_prelude)
            check(f"default config seed {seed}: runs on a Roblox/executor host",
                  rc == 0 and out == EXPECTED, out[:120])

            check(f"default config seed {seed}: source text is gone", "suda" not in artifact)

        # 2. bind-env 分层（tier-1+tier-2+Roblox 绑定，无 exguard）在高保真宿主上必须
        #    跑通。R067.3/4 时代这层从未在任何宿主上正向跑通过（`:IsStudio()` 被
        #    sanitize_frag 改名导致恒投毒），R067.5 起此为固定回归。
        artifact, error = build(
            work, BASE_FLAGS + ["--bind-executor", "--bind-env", "roblox", "--no-executor-guard"],
            "20261022", "tier2")
        if artifact is None:
            check("tier-2/bind-env config: engine produced output", False, error)
        else:
            target = work / "prelude-tier2.luau"
            target.write_text(PRELUDE.read_text() + "\n" + artifact)
            rc, out = run_luau(luau, target)
            check("tier-2/bind-env config runs on a faithful Roblox host",
                  rc == 0 and out == EXPECTED, out[:120])

        # 3. exguard：本 prelude 故意不带 debug 深内省，hard 必须给出精确缺失清单
        #    （而不是静默投毒随机崩）。
        artifact, error = build(work, BASE_FLAGS + HARDENING_FLAGS, "20261022", "hard")
        if artifact is None:
            check("hardening config: engine produced output", False, error)
        else:
            target = work / "prelude-hard.luau"
            target.write_text(PRELUDE.read_text() + "\n" + artifact)
            rc, out = run_luau(luau, target)
            missing_ok = ("lacks APIs" in out and "debug.getconstants" in out
                          and "debug.setupvalue" in out and "identifyexecutor" not in out)
            check("hardening config refuses with the accurate missing list",
                  rc != 0 and missing_ok, out[:140])
    finally:
        shutil.rmtree(work, ignore_errors=True)

    print()
    print(f"{'OK' if not failures else 'FAILED'}: {len(failures)} failing check(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
