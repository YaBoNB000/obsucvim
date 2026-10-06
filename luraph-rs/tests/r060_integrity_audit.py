#!/usr/bin/env python3
"""R060 dual operand-integrity and four-ring micro audit."""
import os
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOOL = ROOT / "target/release/luraph-rs"
TOOLS = pathlib.Path("/home/user/luraph/.tools/bin")
LUAU = TOOLS / "luau"
if not LUAU.exists():
    LUAU = pathlib.Path("/home/user/tools/bin/luau")

SRC = '''local function f(n) local s=0 for i=1,n do s=(s*31+i)%10007 end return s end
local function g(n) if n==0 then return 1 end return f(n)+g(n-1) end
print(f(17),g(8),"r060")
'''


def manifest(path):
    out = {}
    for line in pathlib.Path(path).read_text().splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            out[k] = int(v)
    return out


def build(seed, src, out, keys):
    env = os.environ.copy()
    env["LURAPH_KEY_MANIFEST"] = str(keys)
    p = subprocess.run(
        [str(TOOL), "--preset", "v15", "--dialect", "luau", "--seed", str(seed),
         str(src), str(out)], env=env, capture_output=True, text=True,
    )
    if p.returncode:
        raise RuntimeError(p.stderr[:800])
    return pathlib.Path(out).read_bytes(), manifest(keys)


def main():
    if not TOOL.exists() or not LUAU.exists():
        print("missing release binary or Luau", file=sys.stderr)
        return 2
    compiler = (ROOT / "src/vmgen/compiler.rs").read_text()
    template = (ROOT / "src/vmgen/template.rs").read_text()
    compile_dual = all(x in compiler for x in [
        "osum = (osum + ins.a as u64 + ins.b as u64 + ins.c as u64 + ins.d as u64)",
        "let pos = (st as u64) * ncode64 + (p as u64) + 1",
        "osum2 = (osum2 + (pos % 4294967296) * vals[k]) % 4294967296",
        "operand_sums2",
    ])
    decode_dual = all(x in template for x in [
        "ck = (ck + {}) % 4294967296",
        "ck2 = (ck2 + (({} * ncode + i) * ({})) % 4294967296)",
        "local OS={{{os}}} local OS2={{{os2}}}",
        "PF_[di].ck~=OS[di] or PF_[di].ck2~=OS2[di]",
        "BD=(BD+{ckpo})%268435456",
    ])
    fanout = all(x in template for x in [
        "(BD%720)*(1+MQ%719)", "(BD%65521)*CI2",
        "(BD%65521)*((i*FI+qf*FQ+s0*RP)%65521)",
        "(BD%65521)*((i*KI+j*NI+qf)%65521)",
        "(BD%65536)*((i+MQ)%65536)", "(BD%65536)*((i+NI)%65536)",
    ])

    with tempfile.TemporaryDirectory() as td0:
        td = pathlib.Path(td0)
        src = td / "r060.lua"
        src.write_text(SRC)
        a, ma = build(606001, src, td / "a.lua", td / "a.keys")
        b, mb = build(606002, src, td / "b.lua", td / "b.keys")
        c, mc = build(606001, src, td / "c.lua", td / "c.keys")
        deterministic = a == c and ma == mc
        mods_a = [ma.get(f"MICRO_OP_MOD_{i}") for i in range(4)]
        mods_b = [mb.get(f"MICRO_OP_MOD_{i}") for i in range(4)]
        four_rings = mods_a == mods_b == [251, 241, 239, 229] and ma.get("MICRO_ROUTE_N") == mb.get("MICRO_ROUTE_N") == 4
        affine_diff = tuple(ma.get(f"MICRO_OP_MUL_{i}") for i in range(4)) != tuple(
            mb.get(f"MICRO_OP_MUL_{i}") for i in range(4))

        exp = subprocess.run([str(LUAU), str(src)], capture_output=True)
        ra = subprocess.run([str(LUAU), str(td / "a.lua")], capture_output=True)
        rb = subprocess.run([str(LUAU), str(td / "b.lua")], capture_output=True)
        runtime = (exp.returncode == ra.returncode == rb.returncode == 0
                   and exp.stdout == ra.stdout == rb.stdout)

        print("== R060 双完整性/四环 Micro 审计 ==")
        print(f"[0] 同 seed 确定: {'YES' if deterministic else 'NO'}")
        print(f"[1] 编译期四列 Ou/De 双累计: {'YES' if compile_dual else 'NO'}")
        print(f"[2] 解析期 OS/OS2 双消费: {'YES' if decode_dual else 'NO'}")
        print(f"[3] 校验差值进入六面 BD fan-out: {'YES' if fanout else 'NO'}")
        print(f"[4] 精确四解释器/四素数环: {'YES' if four_rings else 'NO'} ({mods_a})")
        print(f"[5] affine 参数跨构建变化: {'YES' if affine_diff else 'NO'}")
        print(f"[6] 双 seed 运行等价: {'YES' if runtime else 'NO'} ({exp.stdout!r})")
        ok = deterministic and compile_dual and decode_dual and fanout and four_rings and affine_diff and runtime
        print("\nR060 完整性门禁: " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
