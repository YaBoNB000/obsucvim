#!/usr/bin/env python3
"""H4-6A decoded block-window/cache audit."""
import os
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOOL = pathlib.Path(os.environ["OBSUCVIM_BIN"]) if "OBSUCVIM_BIN" in os.environ else (
    ROOT / "target/audit/release/obsucvim" if (ROOT / "target/audit/release/obsucvim").exists() else (
    ROOT / "target/debug/obsucvim" if (ROOT / "target/debug/obsucvim").exists() else ROOT / "target/release/obsucvim"))
TOOLS = (ROOT.parent / ".tools/bin")
LUAU = TOOLS / "luau"
if not LUAU.exists():
    LUAU = pathlib.Path("/home/user/tools/bin/luau")
SRC = ROOT / "tests/cases/stress_control.lua"


def build(seed, out, mani):
    env = os.environ.copy()
    env["OBSUCVIM_KEY_MANIFEST"] = str(mani)
    p = subprocess.run([str(TOOL), "--preset", "v15", "--dialect", "luau",
                        "--seed", str(seed), str(SRC), str(out)],
                       env=env, capture_output=True, text=True)
    if p.returncode:
        raise RuntimeError(p.stderr[:500])
    vals = {}
    for line in pathlib.Path(mani).read_text().splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            vals[k] = int(v)
    return pathlib.Path(out).read_bytes(), vals


def main():
    with tempfile.TemporaryDirectory() as td0:
        td = pathlib.Path(td0)
        a, ma = build(606061, td/"a.lua", td/"a.keys")
        b, mb = build(707071, td/"b.lua", td/"b.keys")
        c, mc = build(606061, td/"c.lua", td/"c.keys")
        deterministic = a == c and ma == mc
        caps = [ma.get("BLOCK_CACHE_CAP", -1), mb.get("BLOCK_CACHE_CAP", -1)]
        bounded = all(0 <= x <= 2 for x in caps)
        reseed_keys = ["WINDOW_RESEED_M", "WINDOW_RESEED_B"]
        reseed_present = all(k in ma and k in mb for k in reseed_keys)
        reseed_diff = reseed_present and tuple(ma[k] for k in reseed_keys) != tuple(mb[k] for k in reseed_keys)
        exp = subprocess.run([str(LUAU), str(SRC)], capture_output=True)
        ra = subprocess.run([str(LUAU), str(td/"a.lua")], capture_output=True)
        rb = subprocess.run([str(LUAU), str(td/"b.lua")], capture_output=True)
        runtime = (exp.returncode == ra.returncode == rb.returncode == 0
                   and exp.stdout == ra.stdout == rb.stdout)
        template = (ROOT / "src/vmgen/template.rs").read_text()
        shape = all(x in template for x in [
            "if BC==0 and E.ww", "if #E.bq>BC", "E.bch[oid]=nil",
            "wt[wx]=nil", "E.bq = {{}}", "E.ww,E.w0,E.w1,E.w2,E.w3=nil",
            "E.er=E.er+1", "let cache_remask = block_word_perm",
            "c[{pos}][wx]=(c[{pos}][wx]-om+wm)%{modulus}",
            "(E.ww[{bi}]-E.br)%256", "{wa}[{bi}]-E.br",
            "for _ti=1,3 do T[_ti]=nil", "for _oi=1,4 do O[_oi]=nil",
        ])
        print("== H4-6A/B 单块窗口/重播种审计 ==")
        print(f"[0] 同 seed 确定: {'YES' if deterministic else 'NO'}")
        print(f"[1] cache cap 位于 0..2: {'YES' if bounded else 'NO'} (caps={caps})")
        print(f"[2] window reseed 跨构建重掷: {'YES' if reseed_diff else 'NO'}")
        print(f"[3] FIFO/覆盖/重掩码/Micro 清场: {'YES' if shape else 'NO'}")
        print(f"[4] 双 seed 长控制流等价: {'YES' if runtime else 'NO'} ({exp.stdout[:80]!r})")
        ok = deterministic and bounded and reseed_diff and shape and runtime
        print("\nH4-6A/B 审计: " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
