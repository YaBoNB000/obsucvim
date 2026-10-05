#!/usr/bin/env python3
"""H4-5 per-function dialect audit (stage A: block-key family)."""
import os
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOOL = ROOT / "target/release/luraph-rs"
TOOLS = pathlib.Path("/home/user/luraph/.tools/bin")
LUAU = TOOLS / "luau"
if not LUAU.exists():
    LUAU = pathlib.Path("/home/user/tools/bin/luau")

SRC = """local function a(n) local x=1 for i=1,n do x=(x*3+i)%1009 end return x end
local function b(n) local x=2 for i=1,n do x=(x*5+i)%1013 end return x end
local function c(n) local x=3 for i=1,n do x=(x*7+i)%1019 end return x end
print(a(11), b(13), c(17), a(4)+b(5)+c(6))
"""


def manifest(path):
    out = {}
    for line in pathlib.Path(path).read_text().splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            out[k] = int(v)
    return out


def noise(path):
    vals = []
    for line in pathlib.Path(path).read_text().splitlines():
        row = dict(item.split("=", 1) for item in line.split())
        vals.append(int(row["chaff"]))
    return vals


def build(seed, src, out, keys, stats):
    env = os.environ.copy()
    env["LURAPH_KEY_MANIFEST"] = str(keys)
    env["LURAPH_NOISE_AUDIT"] = str(stats)
    p = subprocess.run(
        [str(TOOL), "--preset", "v15", "--dialect", "luau", "--seed", str(seed),
         str(src), str(out)], env=env, capture_output=True, text=True,
    )
    if p.returncode:
        raise RuntimeError(p.stderr[:500])
    return pathlib.Path(out).read_bytes(), manifest(keys), noise(stats)


def main():
    if not TOOL.exists():
        print("missing release binary", file=sys.stderr)
        return 2
    with tempfile.TemporaryDirectory() as td0:
        td = pathlib.Path(td0)
        src = td / "src.lua"
        src.write_text(SRC)
        a, ma, na = build(515151, src, td/"a.lua", td/"a.keys", td/"a.noise")
        b, mb, nb = build(616161, src, td/"b.lua", td/"b.keys", td/"b.noise")
        c, mc, nc = build(515151, src, td/"c.lua", td/"c.keys", td/"c.noise")
        keys = ["DIALECT_FM_Q", "DIALECT_FM_I", "DIALECT_FC_Q",
                "DIALECT_FC_I", "DIALECT_FB_Q", "DIALECT_FB_I"]
        deterministic = a == c and ma == mc and na == nc
        rerolled = all(k in ma and k in mb and ma[k] != mb[k] for k in keys)
        per_fn = len(na) >= 4 and len(set(na)) >= 3 and len(nb) >= 4 and len(set(nb)) >= 3
        expected = subprocess.run([str(LUAU), str(src)], capture_output=True)
        ra = subprocess.run([str(LUAU), str(td/"a.lua")], capture_output=True)
        rb = subprocess.run([str(LUAU), str(td/"b.lua")], capture_output=True)
        runtime = (expected.returncode == ra.returncode == rb.returncode == 0
                   and expected.stdout == ra.stdout == rb.stdout)
        template = (ROOT / "src/vmgen/template.rs").read_text()
        mirror = all(x in template for x in [
            "local fkm=2*((KM+qf*FQ+i*FI)%16000000)+1",
            "local fkm=2*((KM+qi*FQ+pf.sd*FI)%16000000)+1",
            "s0 * fpb", "fkm * st + fkc",
        ])
        print("== H4-5 每函数方言审计（A：block-key family） ==")
        print(f"[0] 同 seed 确定: {'YES' if deterministic else 'NO'}")
        print(f"[1] 六系数跨构建重掷: {'YES' if rerolled else 'NO'}")
        print(f"[2] 每函数 qf 方言输入分离: {'YES' if per_fn else 'NO'}")
        print(f"[3] BSS/RT 派生镜像接线: {'YES' if mirror else 'NO'}")
        print(f"[4] 双 seed 运行等价: {'YES' if runtime else 'NO'} ({expected.stdout!r})")
        ok = deterministic and rerolled and per_fn and mirror and runtime
        print("\nH4-5A 审计: " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
