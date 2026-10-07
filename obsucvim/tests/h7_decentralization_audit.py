#!/usr/bin/env python3
"""H4-7 closure metadata / call protocol / constant materialization audit."""
import os
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOOL = ROOT / "target/release/obsucvim"
TOOLS = pathlib.Path("/home/user/luraph/.tools/bin")
LUAU = TOOLS / "luau"
if not LUAU.exists():
    LUAU = pathlib.Path("/home/user/tools/bin/luau")

SRC = r'''local function factory(start)
  local state = start
  return function(a, ...)
    local n = select("#", ...)
    local t = {...}
    state = state + a + n
    return state, t[n], n
  end
end
local function rec(n, f)
  if n == 0 then return f(1, "z", n) end
  return rec(n - 1, f)
end
local f, g = factory(10), factory(100)
local a,b,c = f(3, "x", "y")
local d,e,h = f(4, "p")
local i,j,k = g(7, "q", "r", "s")
local x,y,z = rec(4, g)
print(a,b,c,d,e,h,i,j,k,x,y,z,"alpha","alpha",3.25,17)
'''


def manifest(path):
    vals = {}
    for line in pathlib.Path(path).read_text().splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            vals[k] = int(v)
    return vals


def build(seed, src, out, keys):
    env = os.environ.copy()
    env["OBSUCVIM_KEY_MANIFEST"] = str(keys)
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
    with tempfile.TemporaryDirectory() as td0:
        td = pathlib.Path(td0)
        src = td / "h7.lua"
        src.write_text(SRC)
        a, ma = build(717171, src, td / "a.lua", td / "a.keys")
        b, mb = build(818181, src, td / "b.lua", td / "b.keys")
        c, mc = build(717171, src, td / "c.lua", td / "c.keys")

        deterministic = a == c and ma == mc
        meta_keys = [
            "METADATA_UP_Q", "METADATA_UP_I", "METADATA_UP_M", "METADATA_UP_A",
            "METADATA_NP_Q", "METADATA_NP_I", "METADATA_PID_A",
            "METADATA_CHILD_M", "METADATA_CHILD_Q", "METADATA_CHILD_I",
        ]
        meta_present = all(k in ma and k in mb for k in meta_keys)
        meta_diff = meta_present and tuple(ma[k] for k in meta_keys) != tuple(mb[k] for k in meta_keys)
        protocol_keys = ["CALL_SLOT_STEP", "CONST_NUM_STEP", "CONST_STR_STEP"]
        protocol_present = all(k in ma and k in mb for k in protocol_keys)
        protocol_diff = protocol_present and tuple(ma[k] for k in protocol_keys) != tuple(mb[k] for k in protocol_keys)
        streams_split = protocol_present and all(
            m["CONST_NUM_STEP"] != m["CONST_STR_STEP"] for m in (ma, mb)
        )

        template = (ROOT / "src/vmgen/template.rs").read_text()
        metadata_shape = all(x in template for x in [
            "us = mu, np = mnp", "local src = (mu[i] - mst) % 65536",
            "local npa = (pf.np -", "sx = (i+PM)%65521",
            "local fi = ((pf.sx - PM - 1) % 65521) + 1",
        ]) and "upsrc = pf.upsrc" not in template and "nparams = pf.nparams" not in template
        graph_shape = all(x in template for x in [
            "local ct=(ci*CM2+qf*CQ2+i*CI2",
            "ks[j]=ct", "EP[i][km[j]] = EP[km[j+1]]",
            "makefn(E.bt[O[y]],V,ups,S)",
        ]) and "EP[i].k[" not in template and "E.k[O[y]+1]" not in template
        call_shape = all(x in template for x in [
            "fn call_slot(logical: usize, family: usize)",
            "fn frame_slot(logical: usize)", "E.callx=(E.vd+1)%17",
            "local ck = (E.callx - E.vd) % 17", "for _ci=0,8 do E[E.cb+{page}+_ci*E.cs]=nil",
            "vargs[1+((j+E.vd-1)%vargc)]", "vargc=(vargc-vd)%65536",
        ]) and all(x not in template for x in ["E.cf=fn", "E.ca=av", "E.out=out", "E.total=total"])
        const_shape = all(x in template for x in [
            "tv==\\\"string\\\" and CSJ or CNJ",
            "ty==tm[3] and CSJ or CNJ", "if ty == tm[3] then return s",
            "local C = {} local done = {}",
        ]) and "{[0]=raw}" not in template

        expected = subprocess.run([str(LUAU), str(src)], capture_output=True)
        ra = subprocess.run([str(LUAU), str(td / "a.lua")], capture_output=True)
        rb = subprocess.run([str(LUAU), str(td / "b.lua")], capture_output=True)
        runtime = (expected.returncode == ra.returncode == rb.returncode == 0
                   and expected.stdout == ra.stdout == rb.stdout)

        print("== H4-7 去中心化审计（A1/A2/B/C） ==")
        print(f"[0] 同 seed 确定: {'YES' if deterministic else 'NO'}")
        print(f"[1] metadata 钥匙跨构建重掷: {'YES' if meta_diff else 'NO'}")
        print(f"[2] nparams/upvalue 独立掩码: {'YES' if metadata_shape else 'NO'}")
        print(f"[3] child token/原型图去固定形态: {'YES' if graph_shape else 'NO'}")
        print(f"[4] Call/Return/Vararg 函数方言: {'YES' if call_shape else 'NO'}")
        print(f"[5] 数值/字符串流分离且字符串不共享缓存: {'YES' if streams_split and const_shape else 'NO'}")
        print(f"[6] 协议系数跨构建重掷: {'YES' if protocol_diff else 'NO'}")
        print(f"[7] 双 seed 闭包/递归/变参等价: {'YES' if runtime else 'NO'} ({expected.stdout!r})")
        ok = (deterministic and meta_diff and metadata_shape and graph_shape
              and call_shape and streams_split and const_shape and protocol_diff and runtime)
        print("\nH4-7 审计: " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
