#!/usr/bin/env python3
"""H4-5 per-function dialect audit (stage A: block-key family)."""
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
    env["OBSUCVIM_KEY_MANIFEST"] = str(keys)
    env["OBSUCVIM_NOISE_AUDIT"] = str(stats)
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
        lane_keys = ["DIALECT_LANE_BASE", "DIALECT_LANE_STEP", "DIALECT_LANE_CK"]
        lane_present = all(k in ma and k in mb for k in lane_keys)
        lane_rerolled = lane_present and tuple(ma[k] for k in lane_keys) != tuple(mb[k] for k in lane_keys)
        ca = [(ma["DIALECT_LANE_BASE"] + (i + 1) * ma["DIALECT_LANE_STEP"]) % 24
              for i in range(len(na))] if lane_present else []
        cb = [(mb["DIALECT_LANE_BASE"] + (i + 1) * mb["DIALECT_LANE_STEP"]) % 24
              for i in range(len(nb))] if lane_present else []
        lane_per_fn = len(set(ca)) == len(ca) and len(set(cb)) == len(cb)
        micro_keys = ["MICRO_ROUTE_BASE", "MICRO_ROUTE_STEP", "MICRO_ROUTE_N"]
        micro_present = all(k in ma and k in mb for k in micro_keys)
        micro_rerolled = micro_present and tuple(ma[k] for k in micro_keys) != tuple(mb[k] for k in micro_keys)
        def routes(m, count):
            n = m["MICRO_ROUTE_N"]
            return [(m["MICRO_ROUTE_BASE"] + (i + 1) * m["MICRO_ROUTE_STEP"]) % n
                    for i in range(count)]
        mra = routes(ma, len(na)) if micro_present else []
        mrb = routes(mb, len(nb)) if micro_present else []
        micro_per_fn = (micro_present and 3 <= ma["MICRO_ROUTE_N"] <= 5
                        and 3 <= mb["MICRO_ROUTE_N"] <= 5
                        and len(set(mra[:ma["MICRO_ROUTE_N"]])) == min(len(mra), ma["MICRO_ROUTE_N"])
                        and len(set(mrb[:mb["MICRO_ROUTE_N"]])) == min(len(mrb), mb["MICRO_ROUTE_N"]))
        tag_keys = ["DIALECT_TAG_BASE", "DIALECT_TAG_STEP"]
        tag_present = all(k in ma and k in mb for k in tag_keys)
        tag_rerolled = tag_present and tuple(ma[k] for k in tag_keys) != tuple(mb[k] for k in tag_keys)
        ta = [(ma["DIALECT_TAG_BASE"] + (i + 1) * ma["DIALECT_TAG_STEP"]) % 720
              for i in range(len(na))] if tag_present else []
        tb = [(mb["DIALECT_TAG_BASE"] + (i + 1) * mb["DIALECT_TAG_STEP"]) % 720
              for i in range(len(nb))] if tag_present else []
        tag_per_fn = len(set(ta)) == len(ta) and len(set(tb)) == len(tb)
        rekey_keys = ["DIALECT_REKEY_PERIOD_Q", "DIALECT_REKEY_PERIOD_I",
                      "DIALECT_REKEY_STATE_Q", "DIALECT_REKEY_STATE_P"]
        rekey_present = all(k in ma and k in mb for k in rekey_keys)
        rekey_rerolled = rekey_present and tuple(ma[k] for k in rekey_keys) != tuple(mb[k] for k in rekey_keys)
        rpa = [3 + ((q * ma["DIALECT_REKEY_PERIOD_Q"] + (i + 1) * ma["DIALECT_REKEY_PERIOD_I"]) % 7)
               for i, q in enumerate(na)] if rekey_present else []
        rpb = [3 + ((q * mb["DIALECT_REKEY_PERIOD_Q"] + (i + 1) * mb["DIALECT_REKEY_PERIOD_I"]) % 7)
               for i, q in enumerate(nb)] if rekey_present else []
        rekey_per_fn = len(set(rpa)) >= 2 and len(set(rpb)) >= 2
        per_fn = len(na) >= 4 and len(set(na)) >= 3 and len(nb) >= 4 and len(set(nb)) >= 3
        expected = subprocess.run([str(LUAU), str(src)], capture_output=True)
        ra = subprocess.run([str(LUAU), str(td/"a.lua")], capture_output=True)
        rb = subprocess.run([str(LUAU), str(td/"b.lua")], capture_output=True)
        runtime = (expected.returncode == ra.returncode == rb.returncode == 0
                   and expected.stdout == ra.stdout == rb.stdout)
        template = (ROOT / "src/vmgen/template.rs").read_text()
        mirror = all(x in template for x in [
            "local fkm=2*((KM+qf*FQ+i*FI)%16000000)+1",
            "local fkm=2*((KM+qi*FQ+fi*FI)%16000000)+1",
            "s0 * fpb", "fkm * st + fkc",
            "local dp=(LB+i*LS)%24", "local dp=(LB+fi*LS)%24",
            "u[pm[ps]+1]=pv[ps]",
            "md = (fi * {} + {}) % {}", "E.md", "register_micro_variant",
            "local td=(TB+i*TS+", "local td=({}+fi*{}+", "tm[6]",
            "local frp=3+((qf*RQ+i*RI)%7)",
            "local frp=3+((qi*RQ+fi*RI)%7)",
            "(p-s0+1)%frp", "i%frp",
        ])
        print("== H4-5 每函数方言审计（A/B/C/D/E） ==")
        print(f"[0] 同 seed 确定: {'YES' if deterministic else 'NO'}")
        print(f"[1] 六系数跨构建重掷: {'YES' if rerolled else 'NO'}")
        print(f"[2] 每函数 qf 方言输入分离: {'YES' if per_fn else 'NO'}")
        print(f"[3] lane 方言跨构建/函数分离: {'YES' if lane_rerolled and lane_per_fn else 'NO'}")
        print(f"[4] micro route 跨构建/函数分离: {'YES' if micro_rerolled and micro_per_fn else 'NO'}")
        print(f"[5] constant tag 跨构建/函数分离: {'YES' if tag_rerolled and tag_per_fn else 'NO'}")
        print(f"[6] rekey 周期跨构建/函数分离: {'YES' if rekey_rerolled and rekey_per_fn else 'NO'}")
        print(f"[7] BSS/RT 全方言镜像接线: {'YES' if mirror else 'NO'}")
        print(f"[8] 双 seed 运行等价: {'YES' if runtime else 'NO'} ({expected.stdout!r})")
        ok = (deterministic and rerolled and per_fn and lane_rerolled
              and lane_per_fn and micro_rerolled and micro_per_fn
              and tag_rerolled and tag_per_fn and rekey_rerolled
              and rekey_per_fn and mirror and runtime)
        print("\nH4-5A/B/C/D/E 审计: " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
