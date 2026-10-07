#!/usr/bin/env python3
"""R061 easy-bypass regression: branchless dual operand integrity."""
import os
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOOL = ROOT / "target/release/obsucvim"
TOOLS = pathlib.Path("/home/user/luraph/.tools/bin")
LUAU = TOOLS / "luau"
if not LUAU.exists():
    LUAU = pathlib.Path("/home/user/tools/bin/luau")

SRC = '''local function f(n) local s=1 for i=1,n do s=(s*37+i)%10009 end return s end
local function g(n) if n==0 then return 2 end return f(n)+g(n-1) end
print(f(19),g(9),"r061")
'''


def build(seed, src, out):
    p = subprocess.run(
        [str(TOOL), "--preset", "v15", "--dialect", "luau", "--seed", str(seed),
         str(src), str(out)], capture_output=True, text=True,
    )
    if p.returncode:
        raise RuntimeError(p.stderr[:800])
    return pathlib.Path(out).read_bytes()


def main():
    if not TOOL.exists() or not LUAU.exists():
        print("missing release binary or Luau", file=sys.stderr)
        return 2
    template = (ROOT / "src/vmgen/template.rs").read_text()
    branchless = all(x in template for x in [
        "local d1=(pz.ck-OS[di])%4294967296",
        "local d2=(pz.ck2-OS2[di])%4294967296",
        "pz.iz1=((d1%65536)", "pz.iz2=((d2%65536)",
    ]) and "if PF_[di].ck~=OS[di] or PF_[di].ck2~=OS2[di]" not in template
    independent = all(x in template for x in [
        "local bz=(BD+(pf.iz1%65521)", "+(pf.iz2%65521)",
        "pf.iz1=nil pf.iz2=nil",
    ])
    fanout = all(x in template for x in [
        "(bz%720)*(1+MQ%719)", "(bz%65521)*CI2",
        "(bz%65521)*((i*FI+qf*FQ+s0*RP)%65521)",
        "(bz%65521)*((i*KI+j*NI+qf)%65521)",
        "(bz%65536)*((i+MQ)%65536)", "(bz%65536)*((i+NI)%65536)",
    ])

    with tempfile.TemporaryDirectory() as td0:
        td = pathlib.Path(td0)
        src = td / "r061.lua"
        src.write_text(SRC)
        a = build(616101, src, td / "a.lua")
        b = build(616102, src, td / "b.lua")
        c = build(616101, src, td / "c.lua")
        deterministic = a == c
        expected = subprocess.run([str(LUAU), str(src)], capture_output=True)
        ra = subprocess.run([str(LUAU), str(td / "a.lua")], capture_output=True)
        rb = subprocess.run([str(LUAU), str(td / "b.lua")], capture_output=True)
        runtime = (expected.returncode == ra.returncode == rb.returncode == 0
                   and expected.stdout == ra.stdout == rb.stdout)
        art = (td / "a.lua").read_text(errors="replace")
        foldable = len(re.findall(r'\[[0-9]+\+[0-9]+-[0-9-]+\]', art))

        print("== R061 易绕过点审计 ==")
        print(f"[0] 同 seed 确定: {'YES' if deterministic else 'NO'}")
        print(f"[1] OS/OS2 单分支 oracle 退役: {'YES' if branchless else 'NO'}")
        print(f"[2] 双差值独立 per-proto lane + 阅后即焚: {'YES' if independent else 'NO'}")
        print(f"[3] per-proto checksum 六面 fan-out: {'YES' if fanout else 'NO'}")
        print(f"[4] 双 seed 运行等价: {'YES' if runtime else 'NO'} ({expected.stdout!r})")
        print(f"[5] 待处理纯字面量下标（非本阶段门禁）: {foldable}")
        ok = deterministic and branchless and independent and fanout and runtime
        print("\nR061 bypass 门禁: " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
