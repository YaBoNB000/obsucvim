#!/usr/bin/env python3
"""exguard 前置自检的回归测试（R067.2）

背景：`--bind-executor` 产物在缺 API 的执行器上会“静默投毒”，然后在 VM 随机位置
崩溃，报出与真实原因无关的错误（2026-10-23 线上两次：`attempt to call a nil value`、
`attempt to perform concatenation on a nil value`）。

修复分两步：
  1. 在投毒之前插入只读自检，把缺失项直接报出来；
  2. 自检必须按**执行器真实布局**查 API —— 先 `getgenv()`，再回退 `_G`。
     第一版只查 `_G`，于是 Delta 上把所有 API（连 `typeof`）都报成缺失，依然无法诊断。

本脚本直接从 `obsucvim/src/vmgen/exguard.rs` 里把生成的 Lua 片段抽出来，
在真 Luau 里跑三种宿主布局，断言缺失清单准确。

    LUAU_BIN=/path/to/luau python3 web/tools/test-exguard-precheck.py
"""
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
EXGUARD = ROOT / "obsucvim" / "src" / "vmgen" / "exguard.rs"


def extract_snippet() -> str:
    src = EXGUARD.read_text(encoding="utf-8")
    marker = "this executor lacks APIs"
    i = src.index(marker)
    start = src.rindex('"do\\nlocal _e = nil', 0, i)
    end = src.index('",\n\t);', i)
    literal = src[start + 1 : end]
    return literal.encode().decode("unicode_escape")


APIS = ["identifyexecutor", "checkcaller", "iscclosure", "islclosure", "newcclosure", "loadstring", "typeof"]

CASES = {
    "full executor env (APIs in getgenv, not in _G)": (
        "local E = {}\n"
        f"for _, k in ipairs({{}} ) do end\n"
        + "".join(f'E["{a}"] = function() return "x" end\n' for a in APIS)
        + 'E.debug = { getconstants = function() return {} end, getupvalues = function() return {} end, setupvalue = function() return true end }\n'
        "function getgenv() return E end\n",
        True,
        None,
    ),
    "executor missing deep debug (delta-like)": (
        "local E = {}\n"
        + "".join(f'E["{a}"] = function() return "x" end\n' for a in APIS)
        + "E.debug = { getconstants = function() return {} end }\n"
        "function getgenv() return E end\n",
        False,
        ["debug.getupvalues", "debug.setupvalue"],
    ),
    "empty env": (
        "function getgenv() return {} end\n",
        False,
        ["identifyexecutor", "checkcaller", "iscclosure", "islclosure", "newcclosure"],
    ),
}


def find_luau():
    candidate = os.environ.get("LUAU_BIN") or shutil.which("luau")
    return candidate if candidate and pathlib.Path(candidate).exists() else None


def main():
    luau = find_luau()
    if not luau:
        print("SKIP: no luau binary (set LUAU_BIN=)")
        return 0

    snippet = extract_snippet()
    work = pathlib.Path(tempfile.mkdtemp(prefix="obsucvim-precheck-"))
    failures = []
    try:
        for name, (setup, expect_pass, expect_missing) in CASES.items():
            program = (
                setup
                + "local ok, err = pcall(function()\n"
                + snippet
                + "end)\n"
                + 'if ok then print("RESULT:pass") else print("RESULT:fail|" .. tostring(err)) end\n'
            )
            path = work / "case.luau"
            path.write_text(program, encoding="utf-8")
            proc = subprocess.run([luau, str(path)], capture_output=True, text=True, timeout=120)
            out = (proc.stdout + proc.stderr).strip().splitlines()
            line = next((l for l in out if l.startswith("RESULT:")), "")
            got_pass = line == "RESULT:pass"
            ok = got_pass == expect_pass
            detail = ""
            if expect_missing is not None:
                for token in expect_missing:
                    if token not in line:
                        ok = False
                        detail = f"missing {token!r} in message"
            # 主机自证：干净环境下不应误报 _G 里存在的 API
            if not expect_pass and "typeof" in line and "typeof" not in (expect_missing or []):
                ok = False
                detail = "reported typeof as missing even though _G has it"
            print(("PASS " if ok else "FAIL ") + name + ((" | " + (detail or line[:110])) if (detail or not ok) else ""))
            if not ok:
                failures.append(name)
    finally:
        shutil.rmtree(work, ignore_errors=True)

    print()
    print(f"{len(CASES) - len(failures)}/{len(CASES)} passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
