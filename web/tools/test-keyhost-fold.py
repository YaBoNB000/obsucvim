#!/usr/bin/env python3
"""v16-a --key-host 转录折叠公式端到端数值验证。

验证三层一致（同一套算术）：
  a) 运行期片段公式（Lua，逐字复刻 exguard.rs keyhost 片段）在 prelude
     高保真 mock 上算出 a/b/c 三项；
  b) 编译期模型常量（rust 侧 keyhost_profile("delta")）；
  c) TRW 转录折叠链（(TR*31+v)%2^32，Lua 与 Python 互验）。

矩阵：
  1. prelude 默认（IsStudio=false，真 Delta 客户端形态）：
     a=1143546169  b=1317214663  c=0+2+5*4=22，转录值 == Python 侧同公式值。
  2. OBSUCVIM_MOCK_STUDIO=true（观测不符）：
     c=1+2+5*4=23，转录值 != 客户端期望（= 静默投毒的数学根源），
     且 == Python 侧把 c 换成 23 的值（失步是确定性的，可复算）。

注意：本测试不替代真 Delta 矩阵（完整 hard 还需 4 个 debug 内省 API，沙箱
无法 mock）；它证明的是「公式 + 折叠算术 + 编译期/运行期两侧口径」一致。
"""
from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PRELUDE = ROOT / "web/tools/luau-roblox-prelude.luau"
LUAU_CANDIDATES = [
    Path("/tmp/luau-master/build/luau"),
    ROOT / "web/tools/luau",
]
LUAU = next((p for p in LUAU_CANDIDATES if p.exists()), None)

F31_WANT_A = 1_143_546_169   # fold31("1, 2, 3")
F31_WANT_B = 1_317_214_663   # fold31("Vector3")
SEED = 67067                 # 任意固定；转录初值载体
SIX = [111111111, 222222222, 333333333, 444444444, 555555555, 666666666]

# 与 exguard.rs keyhost 片段逐字同源的运行期片段（mock 直取代替 VK/ERD 捕获）
HARNESS = r"""
local function _f31(s) local h = 7 for i = 1, #s do h = (h * 31 + string.byte(s, i)) % 2147483647 end return h end
local _v3 = Vector3.new(1, 2, 3)
local _kha = _f31(tostring(_v3))
local _khb = _f31(typeof(_v3))
local _rs2 = game:GetService("RunService")
local _bs = (_rs2:IsStudio() == true) and 1 or 0
local _cok3, _cres3 = pcall(checkcaller)
local _bc = (_cok3 and _cres3 == true) and 1 or 0
local _okn, _nm = pcall(identifyexecutor)
local _khc = _bs + _bc * 2 + #_nm * 4
local TR = SEEDV
local function TRW(v) TR = (TR * 31 + v) % 4294967296 end
TRW(111111111) TRW(222222222) TRW(333333333) TRW(444444444) TRW(555555555) TRW(666666666)
TRW(_kha) TRW(_khb) TRW(_khc)
print(string.format("KHRES a=%d b=%d c=%d tr=%d", _kha, _khb, _khc, TR))
"""


def fold_chain(seed: int, values: list[int]) -> int:
    tr = seed
    for v in values:
        tr = (tr * 31 + v) % 4294967296
    return tr


def run_luau(script_path: Path) -> list[str]:
    assert LUAU is not None, "no luau binary found (need /tmp/luau-master/build/luau or web/tools/luau)"
    proc = subprocess.run(
        [str(LUAU), str(script_path)], capture_output=True, text=True, timeout=60
    )
    out = (proc.stdout + proc.stderr).replace("\r", "").splitlines()
    return [l.strip() for l in out if l.strip()]


def main() -> int:
    failures = []

    harness = HARNESS.replace("SEEDV", str(SEED))
    prelude = PRELUDE.read_text()

    cases = {
        "client": (prelude + "\n" + harness, 0, 22),
        "studio": ("OBSUCVIM_MOCK_STUDIO=true\n" + prelude + "\n" + harness, 1, 23),
    }
    results = {}
    for name, (body, want_bs, want_c) in cases.items():
        with tempfile.NamedTemporaryFile("w", suffix=".luau", delete=False) as f:
            f.write(body)
            tmp = Path(f.name)
        lines = run_luau(tmp)
        tmp.unlink()
        res = next((l for l in lines if l.startswith("KHRES ")), None)
        if res is None:
            failures.append(f"{name}: harness produced no KHRES line (got: {lines[-3:]})")
            continue
        parts = dict(kv.split("=") for kv in res.split()[1:])
        got = {k: int(v) for k, v in parts.items()}
        results[name] = got
        print(f"[{name}] a={got['a']} b={got['b']} c={got['c']} tr={got['tr']}")

        if got["a"] != F31_WANT_A:
            failures.append(f"{name}: a={got['a']} != {F31_WANT_A} (fold31(tostring(Vector3.new(1,2,3))))")
        if got["b"] != F31_WANT_B:
            failures.append(f"{name}: b={got['b']} != {F31_WANT_B} (fold31(typeof))")
        if got["c"] != want_c:
            failures.append(f"{name}: c={got['c']} != {want_c} (bs={want_bs} bc=1 namelen=5)")
        expect_tr = fold_chain(SEED, SIX + [F31_WANT_A, F31_WANT_B, want_c])
        if got["tr"] != expect_tr:
            failures.append(f"{name}: tr={got['tr']} != python-recomputed {expect_tr} (TRW 折叠算术双侧不一致)")

    if "client" in results and "studio" in results:
        client_tr = fold_chain(SEED, SIX + [F31_WANT_A, F31_WANT_B, 22])
        if results["studio"]["tr"] == client_tr:
            failures.append("studio: tr 与 client 期望相同——观测不附导致失步失败，投毒链不成立")
        else:
            print(f"[studio] tr mismatch confirmed (poison path): {results['studio']['tr']} != {client_tr}")

    if failures:
        print("\nFAIL:")
        for f in failures:
            print(" -", f)
        return 1
    print("\nPASS: keyhost 折叠公式双层互验通过（client 匹配 / studio 失步可复算）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
