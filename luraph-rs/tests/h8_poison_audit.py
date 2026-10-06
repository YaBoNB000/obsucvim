#!/usr/bin/env python3
"""H4-8 distributed semantic-poisoning audit."""
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

SRC = r'''local function factory(seed)
  local x = seed
  return function(n, ...)
    local t = {...}
    for i = 1, n do x = (x * 17 + i + #t) % 10007 end
    return x, t[#t]
  end
end
local f, g = factory(3), factory(9)
local function rec(n) if n == 0 then return f(5, "r", n) end return rec(n - 1) end
local a,b = f(8, "a", "b")
local c,d = g(11, "x")
local e,h = rec(4)
print(a,b,c,d,e,h,"poison-audit",3.25)
'''


def read_manifest(path):
    out = {}
    for line in pathlib.Path(path).read_text().splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            out[k] = int(v)
    return out


def read_qf(path):
    vals = []
    for line in pathlib.Path(path).read_text().splitlines():
        row = dict(x.split("=", 1) for x in line.split())
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
        raise RuntimeError(p.stderr[:800])
    return pathlib.Path(out).read_bytes(), read_manifest(keys), read_qf(stats)


def signatures(m, qfs):
    mod = 268435456
    modq = 65521
    pz1, pz2, pz3, pz4, mzd = 11, 13, 17, 19, 23
    fq, kq, ki = m["DIALECT_FM_Q"], m["DIALECT_FC_Q"], m["DIALECT_FC_I"]
    rq = m["DIALECT_REKEY_PERIOD_Q"]
    ri = m["DIALECT_REKEY_PERIOD_I"]
    rp = m["DIALECT_REKEY_STATE_P"]
    nq, ni = m["METADATA_NP_Q"], m["METADATA_NP_I"]
    mi, mm = m["METADATA_UP_I"], m["METADATA_UP_M"]
    out = []
    for off, qf in enumerate(qfs[:8], 1):
        bi, s0 = 1 + off % 3, 1 + off * 7
        vd = (qf * nq + off * ni + mm) % modq
        pzb = ((pz1 % modq) * ((off + fq) % modq)
               + (pz3 % modq) * ((qf + ki) % modq)
               + (pz4 % modq) * ((bi + ri) % modq)
               + (mzd % modq) * ((s0 + rq) % modq)) % mod
        pzc = ((pz2 % modq) * ((off + kq) % modq)
               + (pz3 % modq) * ((qf + ki) % modq)
               + (pz4 % modq) * ((off + nq) % modq)
               + (mzd % modq) * ((qf + ni) % modq)) % mod
        pg = ((pz1 % modq) * ((off + fq) % modq)
              + (pz2 % modq) * ((qf + kq) % modq)
              + (pz3 % modq) * ((off + ni) % modq)
              + (pz4 % modq) * ((qf + ri) % modq)
              + (mzd % modq) * ((off + mm) % modq)) % mod
        delay = 2 + ((qf * rq + off * ri + vd) % 15)
        tag = ((pz2 % 720) * (1 + nq % 719)
               + (pz3 % 720) * (1 + ni % 719)
               + (pz4 % 720) * (1 + mi % 719)
               + (mzd % 720) * (1 + mm % 719)) % 720
        out.append((pzb, pzc, pg, delay, tag))
    return out


def main():
    if not TOOL.exists() or not LUAU.exists():
        print("missing release binary or Luau", file=sys.stderr)
        return 2
    with tempfile.TemporaryDirectory() as td0:
        td = pathlib.Path(td0)
        src = td / "h8.lua"
        src.write_text(SRC)
        a, ma, qa = build(828281, src, td / "a.lua", td / "a.keys", td / "a.noise")
        b, mb, qb = build(838381, src, td / "b.lua", td / "b.keys", td / "b.noise")
        c, mc, qc = build(828281, src, td / "c.lua", td / "c.keys", td / "c.noise")
        deterministic = a == c and ma == mc and qa == qc

        integrity_keys = [
            "BSS_PHASE_CK", "BSS_CKPOISON", "RT_PHASE_CK", "RT_PHASE_POISON",
            "HBOOT_PHASE_CK", "HBOOT_PHASE_POISON", "MICRO_CHAFF_CK",
        ]
        keys_present = all(k in ma and k in mb for k in integrity_keys)
        keys_diff = keys_present and tuple(ma[k] for k in integrity_keys) != tuple(
            mb[k] for k in integrity_keys)

        template = (ROOT / "src/vmgen/template.rs").read_text()
        bss_fanout = all(x in template for x in [
            "local BD=(_bc-{expected})%268435456", "BD=(BD+BSSKPX)%268435456",
            "local d1=(pz.ck-OS[di])%4294967296",
            "local d2=(pz.ck2-OS2[di])%4294967296",
            "local bz=(BD+(pf.iz1%65521)",
            "(bz%720)*(1+MQ%719)", "(bz%65521)*CI2",
            "(bz%65536)*((i+MQ)%65536)",
            "(bz%65521)*((i*KI+j*NI+qf)%65521)",
        ]) and all(x not in template for x in [
            "PB=(PB+(_bc-{expected}))",
            "if PF_[di].ck~=OS[di] or PF_[di].ck2~=OS2[di]",
        ])
        rt_fanout = all(x in template for x in [
            "local PZ1=(H1*CB1+H2*CB2+CBC)%268435456",
            "local PZ2=(H1*CC1+H2*CC2+CCC)%268435456",
            "local PZ3=(_qk-RTQCKX)%268435456",
            "local PZ4=(_rc-RTPCX)%268435456",
            "MZD=(_mc-({expected}))%268435456", "local pzb=", "local pzc=",
            "local pg=((PZ1%65521)",
        ])
        delayed = all(x in template for x in [
            "pn = 0, pd = 2 +", "E.pn = E.pn + 1", "if E.pn == E.pd then E.md",
            "E.pg=(E.pg+((E.vd%65521)*WM+WB+E.pn*RQ)",
            "E.pd=E.pn+2+(E.vd%7)",
        ])
        hboot_split = all(x in template for x in [
            "local HD1=HBD_ local HD2=HBD_*257", "(PB_+HD1)%268435456",
            "local h1=({iv1}+HD1)%{m1}", "local h2=({iv2}+HD2)%{m2}",
        ])
        no_single_oracle = all(x not in template for x in [
            "SALT=(SALT+(_mc-", "SLT=(SLT+_qd)", "SLTC=(SLTC+_qd)",
            "7777777", "local tampered", "error(\"tamper",
        ])

        sa, sb = signatures(ma, qa), signatures(mb, qb)
        per_fn = len(sa) >= 3 and len(set(sa)) >= 3 and len(sb) >= 3 and len(set(sb)) >= 3
        per_build = sa != sb

        expected = subprocess.run([str(LUAU), str(src)], capture_output=True)
        ra = subprocess.run([str(LUAU), str(td / "a.lua")], capture_output=True)
        rb = subprocess.run([str(LUAU), str(td / "b.lua")], capture_output=True)
        runtime = (expected.returncode == ra.returncode == rb.returncode == 0
                   and expected.stdout == ra.stdout == rb.stdout)

        print("== H4-8 分散语义投毒审计（A/B/C） ==")
        print(f"[0] 同 seed 确定: {'YES' if deterministic else 'NO'}")
        print(f"[1] 完整性系数跨构建重掷: {'YES' if keys_diff else 'NO'}")
        print(f"[2] BSS delta 六面分散: {'YES' if bss_fanout else 'NO'}")
        print(f"[3] RT 独立 PZ/MZD 多面消费: {'YES' if rt_fanout else 'NO'}")
        print(f"[4] Micro/计时帧局部延迟: {'YES' if delayed else 'NO'}")
        print(f"[5] HBOOT 双 lane: {'YES' if hboot_split else 'NO'}")
        print(f"[6] 无单点 SALT/SLT/fixed oracle: {'YES' if no_single_oracle else 'NO'}")
        print(f"[7] 函数/构建 poison signature 分离: {'YES' if per_fn and per_build else 'NO'}")
        print(f"[8] 双 seed 运行等价: {'YES' if runtime else 'NO'} ({expected.stdout!r})")
        ok = (deterministic and keys_diff and bss_fanout and rt_fanout and delayed
              and hboot_split and no_single_oracle and per_fn and per_build and runtime)
        print("\nH4-8 审计: " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
