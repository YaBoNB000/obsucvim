#!/usr/bin/env python3
"""H5-2A/B micro-op affine dialect and MkStr context audit."""
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

SRC = r'''local function make(prefix)
  return function(n, ...)
    local a,b,c,d,e = "a","bc","def","ghij","klmno"
    local s = prefix .. a .. b .. c .. d .. e
    for i=1,n do s = s .. (i % 2 == 0 and "x" or "y") end
    return s, select("#", ...)
  end
end
local f,g=make("L"),make("R")
local a,b=f(4,1,2)
local c,d=g(3,"z")
print(a,b,c,d)
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


def shard_values(m):
    ids = sorted(int(k.rsplit("_", 1)[1]) for k in m if k.startswith("MICRO_OP_MUL_"))
    return [tuple(m[f"MICRO_OP_{kind}_{i}"] for kind in ("MUL", "ADD", "CTX", "MOD")) for i in ids]


def main():
    if not TOOL.exists() or not LUAU.exists():
        print("missing release binary or Luau", file=sys.stderr)
        return 2
    with tempfile.TemporaryDirectory() as td0:
        td = pathlib.Path(td0)
        src = td / "ctx.lua"
        src.write_text(SRC)
        a, ma = build(525251, src, td / "a.lua", td / "a.keys")
        b, mb = build(626261, src, td / "b.lua", td / "b.keys")
        c, mc = build(525251, src, td / "c.lua", td / "c.keys")
        deterministic = a == c and ma == mc
        va, vb = shard_values(ma), shard_values(mb)
        affine = (len(va) == len(vb) == 4
                  and [x[3] for x in va] == [251, 241, 239, 229]
                  and [x[3] for x in vb] == [251, 241, 239, 229]
                  and len(set(va)) == 4 and len(set(vb)) == 4 and va != vb
                  and all(1 <= x[0] < x[3] and 1 <= x[2] < x[3] for x in va + vb))

        template = (ROOT / "src/vmgen/template.rs").read_text()
        micro_shape = all(x in template for x in [
            "op_mul: u8", "op_add: u8", "op_ctx: u8",
            "local ox=((E.vd%{opmod})*{opctx})%{opmod}",
            "op=(op*{opmul}+{opadd}+ox)%{opmod}",
            '"(({}*{}+{}+ox)%{})"',
        ])
        mkstr_shape = all(x in template for x in [
            "+E.sk+(E.pc-1)*E.si)%65536",
            "let mkstr_cond = if v15",
            "elseif {mw} then local sm=(qf*MM+i*MA+j*NI)%65536",
            "sk = (QF[fi] * MM + fi * MA) % 65536, si = NI",
        ])
        direct_first_shard = "local op,x,y=d{op},d{x},d{y} {dispatch}" in template

        expected = subprocess.run([str(LUAU), str(src)], capture_output=True)
        ra = subprocess.run([str(LUAU), str(td / "a.lua")], capture_output=True)
        rb = subprocess.run([str(LUAU), str(td / "b.lua")], capture_output=True)
        runtime = (expected.returncode == ra.returncode == rb.returncode == 0
                   and expected.stdout == ra.stdout == rb.stdout)

        print("== H5-2A/B Context Dialect 审计 ==")
        print(f"[0] 同 seed 确定: {'YES' if deterministic else 'NO'}")
        print(f"[1] 4 shard 互质模环/affine/context 独立: {'YES' if affine else 'NO'}")
        print(f"[2] 全 shard runtime affine compare: {'YES' if micro_shape and not direct_first_shard else 'NO'}")
        print(f"[3] MkStr 绑定 qf/proto/absolute PC: {'YES' if mkstr_shape else 'NO'}")
        print(f"[4] 双 seed 短字符串/闭包/变参等价: {'YES' if runtime else 'NO'} ({expected.stdout!r})")
        ok = deterministic and affine and micro_shape and not direct_first_shard and mkstr_shape and runtime
        print("\nH5-2A/B 审计: " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
