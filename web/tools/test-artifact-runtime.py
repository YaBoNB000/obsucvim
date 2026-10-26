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
import hashlib
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

# R081（2026-10-25）：发布 wasm 拒绝 --no-bind-executor（绑定层恒开）。
# 本脚本第 1 段要的正是「未绑定产物在裸 Luau 里跑通」这一对照，只能由
# **原生开发构建**产出——该构建保留测试用旗标，产物从不进发布通道。
# 发布件（wasm）留给第 2–4 段的绑定态断言。
NATIVE = pathlib.Path(os.environ.get("ENGINE_BIN")
                      or (TOOLS.parent.parent / "engine" / "target" / "release" / "obsucvim"))

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


def build_status(workdir, flags, seed, name):
    """Returns (ok, artifact, detail). detail carries engine stderr on failure.

    含 `--no-bind-executor` 的构建走原生开发件（R081 后发布 wasm 拒绝该旗标），
    其余仍走发布 wasm —— 这样「绑定态行为」断言测的始终是线上那一份字节。
    """
    out = workdir / f"{name}.lua"
    if "--no-bind-executor" in flags and NATIVE.exists():
        proc = subprocess.run(
            [str(NATIVE), *flags, "--seed", seed,
             str(workdir / "input.lua"), str(out)],
            capture_output=True, text=True, timeout=300,
        )
    else:
        args = flags + ["--seed", seed, "/work/input.lua", f"/work/{name}.lua"]
        cfg = {"wasmPath": str(WASM), "workDir": str(workdir), "args": args}
        cfg_path = workdir / "cfg.json"
        cfg_path.write_text(json.dumps(cfg))
        proc = subprocess.run(
            ["node", str(RUNNER), cfg_path.read_text()],
            capture_output=True, text=True, timeout=300,
        )
    if not out.exists() or out.stat().st_size == 0:
        detail = (proc.stderr or proc.stdout or "").strip()
        return False, None, f"engine rc={proc.returncode} {detail[:240]}"
    return True, out.read_text(), ""


def build(workdir, flags, seed, name):
    ok, artifact, detail = build_status(workdir, flags, seed, name)
    if not ok:
        return None, detail
    return artifact, None


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

        # 4. v16-a --key-host 开关态行为约束：
        #    a. 产物可构建，precheck 仍在转录链之前先行（缺 API 清单与非 keyhost
        #       hard 逐字一致），载荷字符串不在字节中；
        #    b. CLI 校验：未知 profile 拒绝；与 --no-executor-guard 组合拒绝。
        artifact, error = build(
            work, BASE_FLAGS + HARDENING_FLAGS + ["--key-host", "delta"], "20261022", "keyhost")
        if artifact is None:
            check("keyhost hard: engine produced output", False, error)
        else:
            check("keyhost hard: source text is gone", "suda" not in artifact)
            target = work / "prelude-keyhost.luau"
            target.write_text(PRELUDE.read_text() + "\n" + artifact)
            rc, out = run_luau(luau, target)
            missing_ok = ("lacks APIs" in out and "debug.getconstants" in out
                          and "debug.setupvalue" in out and "identifyexecutor" not in out)
            check("keyhost hard refuses with the accurate missing list (precheck first)",
                  rc != 0 and missing_ok, out[:140])

        ok, _, detail = build_status(
            work, BASE_FLAGS + HARDENING_FLAGS + ["--key-host", "delta", "--no-executor-guard"],
            "20261022", "keyhost-noguard")
        check("--key-host with --no-executor-guard is refused",
              not ok and "executor guard" in detail, detail[:140])
        ok, _, detail = build_status(
            work, BASE_FLAGS + HARDENING_FLAGS + ["--key-host", "wave"], "20261022", "keyhost-unknown")
        check("--key-host unknown profile is refused",
              not ok and "unknown" in detail, detail[:140])

        # 5. R069/R070 翻默认+固化等价断言（字节级；锚点 R073 起新口径）：
        #    a. 默认（auto）hard == 显式 --key-host delta hard（同种子同参数）
        #    b. auto 默认件 == R073 锚点（b67a06e3；R073 体积收敛：_mx 工厂化+别名折叠）
        #    c. R070 起 --no-key-host 已移除：使用即报错（固化不可关）
        artifact_auto, error = build(work, BASE_FLAGS + HARDENING_FLAGS, "20261022", "khauto")
        artifact_expl, error = build(
            work, BASE_FLAGS + HARDENING_FLAGS + ["--key-host", "delta"], "20261022", "khexpl")
        check("keyhost auto default == explicit --key-host delta (byte-identical)",
              artifact_auto is not None and artifact_auto == artifact_expl,
              (error or "")[:120])

        R073A = "b67a06e3f4ba620daaac0b81a3fb266cfb9f78fac9dda614052cbc7738024b9b"  # 历史锚点：R073 及更早形态（冻结于 R075 系列之前的提交）
        R075PB = "a968a510b50a623b538c8a4814724feb39eecc87d79ac0583ad243e6354fabc5"  # 历史锚点：R075 终锚（M1 齐），R076 起被 M1-bb 改写
        R076PB = "20ff221c64e34abafa857c96bbd32d1e85b286923de431a905ed769a5091b728"  # 历史锚点：R076 终锚（M1-bb），R077 起被 M2-R1 改写
        R077PB = "fdcd689ea9868cd4ca3310e8f92b66d50ee083286dfc89bfa8e18cbe39d5e5a3"  # 历史锚点：R077（三车道 NL 像在真 Delta 定罪，R078 判定性回退）；V19 §11/§12
        R078PB = "04572e910c7f7bb18da864bb2a51bf161fccc05e15ef0791dd95d9d4ca2654fe"  # 历史锚点：R078（三车道全回退），R079 起被 P1 车道解剖定案改写
        R079PB = "ccf5d0839fef962a8a0c031d745d869dff7a861b6590fe8e267d93f5cf63c045"  # R079 终锚（bx _sv 车道折叠独罪→恒等、ct/km _svk/_svq 折叠无罪复辟）；V19 §11.5/§13
        ANCHOR_FLAGS = ["--preset", "v15", "--env-check", "--bind-executor"]
        auto5, error = build(work, ANCHOR_FLAGS, "67067", "khauto5")
        check("auto default artifact == R079 final anchor (M2: P1 lane verdict)",
              auto5 is not None and hashlib.sha256(auto5.encode()).hexdigest() == R079PB,
              (error or "")[:120])
        ok, _, detail = build_status(
            work, ANCHOR_FLAGS + ["--no-key-host"], "1", "khconflict")
        check("--no-key-host is refused (removed in R070, key is permanent)",
              not ok and "removed" in detail, detail[:140])
    finally:
        shutil.rmtree(work, ignore_errors=True)

    print()
    print(f"{'OK' if not failures else 'FAILED'}: {len(failures)} failing check(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
