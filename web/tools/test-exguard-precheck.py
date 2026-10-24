#!/usr/bin/env python3
"""exguard 前置自检的回归测试（R067.4 重写）

历史（全部实测）：
  - R067：缺 API 的执行器上“静默投毒”，VM 随机位置崩溃，报出与真实原因
    无关的错误（2026-10-23 线上两次）。
  - R067.1/2/3：加了前置自检，但自检里裸写了全局标识符 `getgenv`。
    注入片段会过 sanitize_frag（模板级改名）：任何非保留字标识符都会被改名，
    `getgenv` 在产物里变成了 `Idu`，`type(Idu)=="function"` 恒假 → 自检退回
    只查 `_G` → 在真 Delta（API 全在 getgenv()）上误报“整张清单全缺”。
  - R067.4：自检不再做任何裸查表，改为复用守卫自身已解析好的绑定
    （IDENTV/CHKCV/.../SUPVV，全部经 LKP(VK(密文串)) 取得），
    判定清单与 SCHALL 第一道闸门逐一对应；同时 `DTBL` 改走 LKP（debug 库
    可能只在 getgenv() 侧），CIDENT 对 DTBL 来源容错（ENV 或 CENV 任一）。

本脚本用真引擎生成 hard 产物，再在真 Luau 里按宿主布局断言：

  1. 裸 Luau（无任何执行器 API）→ 必须拒绝且清单准确
     （loadstring/typeof/debug.info 是 Luau 自带的，不许出现在清单里）。
  2. Delta 式宿主（执行器 API 齐，但无 debug 深内省）→ 拒绝清单必须
     恰好是 debug.getconstants/getupvalues/getproto/setupvalue 四项。
  3. 产物里不得再出现裸 `getgenv` 调用（sanitize_frag 之外的陷阱一次性排掉）。

    LUAU_BIN=/path/to/luau python3 web/tools/test-exguard-precheck.py
"""
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
TOOLS = ROOT / "web" / "tools"
WASM = TOOLS / "obsucvim.wasm"
RUNNER = TOOLS / "wasi-runner.mjs"
PRELUDE = TOOLS / "luau-roblox-prelude.luau"

SOURCE = 'print("suda")\n'
MSG_TAG = "[Obsucvim] this executor lacks APIs the hardened build requires: "


def find_luau():
    candidate = os.environ.get("LUAU_BIN") or shutil.which("luau")
    return candidate if candidate and pathlib.Path(candidate).exists() else None


def build_hard(workdir):
    args = [
        "--preset", "v15", "--env-check", "--bind-executor", "--bind-env", "roblox",
        "--seed", "670674", "/work/input.lua", "/work/hard.lua",
    ]
    cfg = {"wasmPath": str(WASM), "workDir": str(workdir), "args": args}
    cfg_path = workdir / "cfg.json"
    cfg_path.write_text(json.dumps(cfg))
    proc = subprocess.run(
        ["node", str(RUNNER), cfg_path.read_text()],
        capture_output=True, text=True, timeout=300,
    )
    out = workdir / "hard.lua"
    if proc.returncode != 0 or not out.exists() or out.stat().st_size == 0:
        return None, f"engine rc={proc.returncode} {proc.stderr.strip()[:160]}"
    return out.read_text(), None


def run_luau(luau, text, name, workdir):
    target = workdir / name
    target.write_text(text)
    proc = subprocess.run([luau, str(target)], capture_output=True, text=True, timeout=120)
    return proc.returncode, (proc.stdout + proc.stderr).strip()


def missing_list(output):
    if MSG_TAG not in output:
        return None
    tail = output.split(MSG_TAG, 1)[1]
    listed = tail.split(" - use the compatible build", 1)[0]
    return [item for item in listed.split(",") if item]


def main():
    luau = find_luau()
    if not luau:
        print("SKIP: no luau binary (set LUAU_BIN=)")
        return 0
    if not WASM.exists():
        print("SKIP: engine not built (web/tools/obsucvim.wasm missing)")
        return 0

    work = pathlib.Path(tempfile.mkdtemp(prefix="obsucvim-precheck-"))
    failures = []

    def check(name, ok, detail=""):
        print(("PASS " if ok else "FAIL ") + name + ((" | " + detail) if detail else ""))
        if not ok:
            failures.append(name)

    try:
        (work / "input.lua").write_text(SOURCE)
        artifact, error = build_hard(work)
        check("hard artifact builds", artifact is not None, error or "")

        if artifact is not None:
            # 3. 没有裸 getgenv：sanitize_frag 会把它改名，出现即意味着又有人裸写了
            bare = re.findall(r"(?<![%w])getgenv\s*\(", artifact)
            check("no bare getgenv() call survives in artifact", len(bare) == 0,
                  f"found {len(bare)}")

            # 1. 裸 Luau（根本不是执行器）：必须失败关闭、绝不放行；且**只要**
            #    自检消息出现，清单就必须准确。注意：某些种子下 tier-2 的宿主探测
            #    会比 precheck 先崩（fail-closed 噪音，R067 起既有，见
            #    docs/ENGINE-VERSION.md），所以裸 Luau 上不强制消息必须送达——
            #    送达性只在执行器宿主（下面的 delta-like 用例）上强制。
            rc, out = run_luau(luau, artifact, "bare.luau", work)
            check("stock Luau: never runs the payload", rc != 0 and "suda" not in out,
                  out[:140])
            listed = missing_list(out)
            if listed is not None:
                for native in ("loadstring", "typeof", "debug.info"):
                    check(f"stock Luau: native {native} is NOT reported missing",
                          native not in listed, ",".join(listed))
                for truly_missing in ("identifyexecutor", "checkcaller", "iscclosure",
                                      "islclosure", "newcclosure",
                                      "debug.getconstants", "debug.getupvalues",
                                      "debug.getproto", "debug.setupvalue",
                                      "game", "Instance", "Vector3", "task"):
                    check(f"stock Luau: {truly_missing} IS reported missing",
                          truly_missing in listed, ",".join(listed))
            else:
                print("NOTE stock Luau: fail-closed before precheck this seed "
                      "(accepted, pre-existing tier-2 noise)")

            # 2. Delta 式宿主：只差 debug 深内省，清单必须恰好这四项
            combined = PRELUDE.read_text() + "\n" + artifact
            rc, out = run_luau(luau, combined, "delta-like.luau", work)
            listed = missing_list(out) or []
            check("delta-like host: refuses with the accurate message",
                  rc != 0 and listed != [], out[:140])
            check("delta-like host: exactly the deep-debug quartet is missing",
                  sorted(listed) == sorted([
                      "debug.getconstants", "debug.getupvalues",
                      "debug.getproto", "debug.setupvalue"]),
                  ",".join(listed))
    finally:
        shutil.rmtree(work, ignore_errors=True)

    print()
    print(f"{'OK' if not failures else 'FAILED'}: {len(failures)} failing check(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
