#!/usr/bin/env python3
"""H5-P0 static micro-descriptor sealing audit."""
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOOL = ROOT / "target/release/obsucvim"
TOOLS = pathlib.Path("/home/user/luraph/.tools/bin")
LUAU = TOOLS / "luau"
if not LUAU.exists():
    LUAU = pathlib.Path("/home/user/tools/bin/luau")

SRC = r'''local function factory(seed)
  local x = seed
  return function(n, ...)
    local t = {...}
    for i = 1, n do x = (x * 19 + i + #t) % 10009 end
    return x, t[#t]
  end
end
local f, g = factory(5), factory(11)
local function rec(n) if n == 0 then return f(4, "r", n) end return rec(n - 1) end
local a,b = f(7, "a", "b")
local c,d = g(9, "x")
local e,h = rec(3)
print(a,b,c,d,e,h,"micro-seal",6.5)
'''


def manifest(path):
    out = {}
    for line in pathlib.Path(path).read_text().splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            out[k] = int(v)
    return out


def build(seed, src, out, keys, tsrc=None):
    env = os.environ.copy()
    env["OBSUCVIM_KEY_MANIFEST"] = str(keys)
    if tsrc is not None:
        env["OBSUCVIM_VM_TSRC"] = "1"
    p = subprocess.run(
        [str(TOOL), "--preset", "v15", "--dialect", "luau", "--seed", str(seed),
         str(src), str(out)], env=env, capture_output=True, text=True,
    )
    if p.returncode:
        raise RuntimeError(p.stderr[:800])
    if tsrc is not None:
        shutil.copyfile("/tmp/vm_tsrc.lua", tsrc)
    return pathlib.Path(out).read_bytes(), manifest(keys)


def micro_key_tuple(m):
    names = sorted(k for k in m if k.startswith("MICRO_AT_") or k.startswith("MICRO_SESSION_"))
    return names, tuple(m[k] for k in names)


def main():
    if not TOOL.exists() or not LUAU.exists():
        print("missing release binary or Luau", file=sys.stderr)
        return 2
    with tempfile.TemporaryDirectory() as td0:
        td = pathlib.Path(td0)
        src = td / "p0.lua"
        src.write_text(SRC)
        a, ma = build(505051, src, td / "a.lua", td / "a.keys", td / "a.tsrc")
        b, mb = build(606061, src, td / "b.lua", td / "b.keys")
        c, mc = build(505051, src, td / "c.lua", td / "c.keys")
        deterministic = a == c and ma == mc

        nka, ka = micro_key_tuple(ma)
        nkb, kb = micro_key_tuple(mb)
        shard_count_a = len([k for k in nka if k.startswith("MICRO_AT_SEED_")])
        shard_count_b = len([k for k in nkb if k.startswith("MICRO_AT_SEED_")])
        keys_ok = (3 <= shard_count_a <= 5 and 3 <= shard_count_b <= 5
                   and len(nka) == shard_count_a * 5
                   and len(nkb) == shard_count_b * 5 and ka != kb)

        template = (ROOT / "src/vmgen/template.rs").read_text()
        shape = all(x in template for x in [
            "let mut micro_carriers: Vec<Vec<u8>>",
            "local _ms{shard}=(SALT%256+(CHQ%256)",
            "_mb{shard}=_out _src=nil _out=nil _g=nil _akm=nil _akc=nil",
            "local n=(BYTE({blob},o)-({sm}+o*{ss})%256)%256",
            "MZD=(_mc-({expected}))%268435456",
        ])
        retired = all(x not in template for x in [
            "fn micro_variant_assign", "local _mo={}", "local _md{}={}",
            "local _d=_md", "E,a,b,c,d,P,m,k,R,N",
        ])

        tsrc = (td / "a.tsrc").read_text()
        legacy_named = re.findall(r"_md\d+\[\d+\]", tsrc)
        legacy_shape = re.findall(r"=\{\{\d+(?:,\d+)*\},\d+,\d+\}", tsrc)
        no_static_descriptors = not legacy_named and not legacy_shape
        session_bound = ("SALT%256" in tsrc and "CHQ%256" in tsrc
                         and "_src=nil" in tsrc and "BYTE(" in tsrc)

        expected = subprocess.run([str(LUAU), str(src)], capture_output=True)
        ra = subprocess.run([str(LUAU), str(td / "a.lua")], capture_output=True)
        rb = subprocess.run([str(LUAU), str(td / "b.lua")], capture_output=True)
        runtime = (expected.returncode == ra.returncode == rb.returncode == 0
                   and expected.stdout == ra.stdout == rb.stdout)

        print("== H5-P0 Micro Descriptor 密封审计 ==")
        print(f"[0] 同 seed 确定: {'YES' if deterministic else 'NO'}")
        print(f"[1] 3..5 shard 独立 at-rest/session keys: {'YES' if keys_ok else 'NO'}")
        print(f"[2] packed carrier + SALT/CHQ reseal 接线: {'YES' if shape else 'NO'}")
        print(f"[3] 旧 47×{{program,mask,step}} 表退役: {'YES' if retired and no_static_descriptors else 'NO'}")
        print(f"[4] TSRC 无旧 descriptor extractor 命中: {'YES' if no_static_descriptors else 'NO'} "
              f"(named={len(legacy_named)}, shape={len(legacy_shape)})")
        print(f"[5] 运行期 session binding/临时钥匙清场: {'YES' if session_bound else 'NO'}")
        print(f"[6] 双 seed 闭包/递归/变参等价: {'YES' if runtime else 'NO'} ({expected.stdout!r})")
        ok = (deterministic and keys_ok and shape and retired and no_static_descriptors
              and session_bound and runtime)
        print("\nH5-P0 审计: " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
