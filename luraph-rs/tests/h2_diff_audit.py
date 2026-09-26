#!/usr/bin/env python3
"""
H2 构建差异化审计 —— 度量「同源码跨构建」与「异源码同构建」的输出差异度。

背景（HARDENING_DETAIL.md §二）：R028 vs R029 实测 54 段里 53/54 逐字节
相同，攻击者知识 100% 迁移。本脚本给出可复现的差异度指标，作为 H2 每项
落地的验收闸：**同源码不同 seed 的相同段比例必须低**。

三段口径：
  A. 可见层相同字节比例（同源码 × 不同 seed）—— 越低越好
  B. 密文段（长串/数组字面量）内容相同比例 —— 越低越好
  C. 结构形态相同 token 比例（去字面量后的骨架）—— 越低越好
"""
import re
import subprocess
import sys
import pathlib
import tempfile
import os

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOOL = ROOT / "target/release/luraph-rs"
TOOLS_BIN = pathlib.Path("/home/user/luraph/.tools/bin")
LUAU = TOOLS_BIN / "luau"
if not LUAU.exists():
    LUAU = pathlib.Path("/home/user/tools/bin/luau")

SRC = """local a = 12
local b = 34
local c = a + b * 2
local t = {c, b, a}
local function f(x) return x + t[1] end
local r = f(c)
print(r, c)
"""


def obf(seed, src_text, out):
    with open(out, "w") as fh:
        fh.write(src_text)
    res = subprocess.run(
        [str(TOOL), "--preset", "v15", "--seed", str(seed), out, out + ".lua"],
        capture_output=True, text=True,
    )
    if res.returncode != 0:
        raise SystemExit(f"obfuscate failed: {res.stderr[:300]}")
    return pathlib.Path(out + ".lua").read_text()


def skeleton(text):
    """去长串/短串/数字后的骨架 token 序列（结构形态）。"""
    t = re.sub(r"\[=*\[.*?\]=*\]", " LONG ", text, flags=re.S)
    t = re.sub(r'"(?:[^"\\]|\\.)*"', " STR ", t)
    t = re.sub(r"'(?:[^'\\]|\\.)*'", " STR ", t)
    t = re.sub(r"\d+", " N ", t)
    return re.findall(r"[A-Za-z_][A-Za-z_0-9]*|LONG|STR|N|[^\sA-Za-z_0-9]", t)


def ratio_same_tokens(a, b):
    """同位置同 token 的比例（长度取短）。"""
    n = min(len(a), len(b))
    if n == 0:
        return 0.0
    return sum(1 for i in range(n) if a[i] == b[i]) / n


def bytes_same_at_same_pos(a, b):
    n = min(len(a), len(b))
    if n == 0:
        return 0.0
    return sum(1 for i in range(n) if a[i] == b[i]) / n


def ngram_sets(text, n=24, step=7):
    return set(text[i:i + n] for i in range(0, max(0, len(text) - n), step))


def main():
    with tempfile.TemporaryDirectory() as td:
        out = os.path.join(td, "s.lua")
        a = obf(111111, SRC, out)
        b = obf(222222, SRC, out)
        c = obf(111111, SRC, out)  # 同 seed 复现

        print("== H2 构建差异化审计 ==")
        print(f"seed111111 size={len(a)}  seed222222 size={len(b)}")

        # 0. 确定性
        print(f"[0] 同 seed 逐字节确定: {'YES' if a == c else 'NO'}  (必须 YES)")

        # A. 可见层字节比例
        ba = bytes_same_at_same_pos(a, b)
        print(f"[A] 同位置相同字节比例      : {ba * 100:5.2f}%  (越低越好)")

        # B. 长串（密文段）内容集合
        la = re.findall(r"\[(=*)\[(.*?)\]\1\]", a, flags=re.S)
        lb = re.findall(r"\[(=*)\[(.*?)\]\1\]", b, flags=re.S)
        sa = set(x[1] for x in la)
        sb = set(x[1] for x in lb)
        shared = sa & sb
        print(f"[B] 长串段数 {len(sa)} vs {len(sb)}；完全相同段 {len(shared)}  "
              f"(相同率 {len(shared) / max(1, len(sa)) * 100:.1f}%)  (越低越好)")

        # C. 骨架 token 同位置比例
        ka, kb = skeleton(a), skeleton(b)
        print(f"[C] 骨架 token 数 {len(ka)} vs {len(kb)}；同位置相同 "
              f"{ratio_same_tokens(ka, kb) * 100:5.2f}%  (越低越好)")

        # D. ngram 集合 Jaccard
        ga, gb = ngram_sets(a), ngram_sets(b)
        j = len(ga & gb) / max(1, len(ga | gb))
        print(f"[D] 24-gram Jaccard 相似度   : {j * 100:5.2f}%  (越低越好)")

        # E. 功能等价（两 seed 都必须跑出同结果）
        oa = subprocess.run([str(LUAU), out + ".lua"], capture_output=True)
        # regenerate b to disk
        ob = obf(222222, SRC, out)
        ob = subprocess.run([str(LUAU), out + ".lua"], capture_output=True)
        print(f"[E] 两构建运行等价: "
              f"{'YES' if oa.stdout == ob.stdout else 'NO'}  "
              f"(out={oa.stdout!r})")

        ok = (a == c) and (oa.stdout == ob.stdout)
        print("\nH2 审计: " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
