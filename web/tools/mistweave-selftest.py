#!/usr/bin/env python3
# MW-1 雾织门禁（自验）：语义逐位等价（语料×变换前后输出恒等）、
# 谓词族数学审计（x 域 [-2000,2000] 逐点）、确定性、引擎 R079 E2E 串接。
# 用法: LUAU_BIN=/path/luau python3 mistweave-selftest.py  （在仓库根跑）
import hashlib, os, pathlib, subprocess, sys, tempfile

TOOLS = pathlib.Path(__file__).resolve().parent
RUNNER = TOOLS / "wasi-runner.mjs"
WASM = TOOLS / "obsucvim.wasm"
# R081：发布 wasm 拒绝 --no-bind-executor（绑定层恒开）。第 4 段要的是
# 「未绑定产物在裸 Luau 里跑通」的 E2E 对照，改走原生开发构建。
NATIVE = pathlib.Path(os.environ.get("ENGINE_BIN")
                      or (TOOLS.parent.parent / "engine" / "target" / "release" / "obsucvim"))
LUAU = os.environ.get("LUAU_BIN") or "luau"
FAILS = []

def check(name, ok, detail=""):
    print(("PASS " if ok else "FAIL ") + name + (" | " + detail[:100] if detail else ""))
    if not ok:
        FAILS.append(name)

def run_luau(path, timeout=120):
    p = subprocess.run([LUAU, str(path)], capture_output=True, text=True, timeout=timeout)
    return p.returncode, (p.stdout or "") + (p.stderr or "")

CORPUS = {
    "fib": ('local function f(n) if n<2 then return n end return f(n-1)+f(n-2) end\n'
            'local o={} for i=1,12 do o[#o+1]=tostring(f(i)) end\n'
            'print("c-fib:"..table.concat(o,","))\n'),
    "closure": ('local function mk() local s=0 return function(d) s=s+d return s end end\n'
                'local a,b=mk(),mk() a(1) a(2) b(10)\n'
                'print("c-clo:"..a(3)..","..b(20))\n'),
    "strtab": ('local t={} t.x="ab" t[2]="cd" local s=t.x..t[2]\n'
               'local u=s:upper() print("c-str:"..u..":"..#s..":"..("q"..s):find("q",1,true) and "hit" or "miss")\n'),
    "loops": ('local s=0 for i=1,20,3 do s=s+i end repeat s=s-3 until s<=10\n'
              'while s<100 do s=s*2 end print("c-loop:"..s)\n'),
    "methods": ('local o={v=7} function o:add(x) self.v=self.v+x return self.v end\n'
                'print("c-mth:"..o:add(3)..":"..o:add(4))\n'),
    "candies": ('-- 注释块 [[notcode123]] -- 假数字 999\n'
                'local longstr=[[num 123 in long string]] local esc="a\\n5"\n'
                'local hex=0xFF local frac=2.5 local big=2e6\n'
                'print("c-mix:"..#longstr..":"..hex..":"..frac..":"..big)\n'),
}
ENGV_FIB = "kkksu-mini:1,1,2,3,5,8,13,21,34,55"

def main():
    work = pathlib.Path(tempfile.mkdtemp(prefix="mw-gate-"))
    here = os.getcwd()
    os.chdir(work)
    try:
        # 1) 语料语义逐位等价 × 确定性
        for name, src in CORPUS.items():
            pathlib.Path(f"p_{name}.lua").write_text(src)
            rc0, out0 = run_luau(f"p_{name}.lua")
            subprocess.run([sys.executable, str(TOOLS / "mistweave.py"),
                            f"p_{name}.lua", f"m_{name}.lua", "--seed", "777",
                            "--num-fog", "0.7", "--blocks", "5"],
                           check=True, capture_output=True)
            rc1, out1 = run_luau(f"m_{name}.lua")
            check(f"corpus {name}: semantic identity", rc0 == 0 and rc1 == 0 and out0 == out1,
                  (out0 if out0 != out1 else "")[:80])

        # 2) 确定性：同种子恒等 / 异种子相异
        subprocess.run([sys.executable, str(TOOLS / "mistweave.py"),
                        "p_fib.lua", "d1.lua", "--seed", "777",
                        "--num-fog", "0.7", "--blocks", "5"],
                       check=True, capture_output=True)
        subprocess.run([sys.executable, str(TOOLS / "mistweave.py"),
                        "p_fib.lua", "d2.lua", "--seed", "888",
                        "--num-fog", "0.7", "--blocks", "5"],
                       check=True, capture_output=True)
        h = lambda p: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
        check("determinism same-seed", h("m_fib.lua") == h("d1.lua"))
        check("determinism diff-seed", h("d1.lua") != h("d2.lua"))

        # 3) 谓词族数学审计：x ∈ [-2000,2000] 逐点
        audit = ('local bad=0 local function f(x) '
                 'if ((x*x+x)%2)==1 then bad=bad+1 end '
                 'if (x*x)%4==2 then bad=bad+1 end '
                 'if ((x%3)*(x%3))%3==2 then bad=bad+1 end '
                 'if ((x*(5*5))%5)==3 then bad=bad+1 end '
                 'if (x*(x+1))%2==1 then bad=bad+1 end '
                 'if ((x+x+1)%2)==0 then bad=bad+1 end '
                 'if not (((x*x+x)%2)==0) then bad=bad+1 end '
                 'if not ((x*x)%4~=3) then bad=bad+1 end '
                 'if not (((x*(7*7))%7)==0) then bad=bad+1 end '
                 'if not (((x+x)%2)==0) then bad=bad+1 end '
                 'end for x=-2000,2000 do f(x) end print("audit:"..bad)\n')
        pathlib.Path("audit.lua").write_text(audit)
        rc, out = run_luau("audit.lua")
        check("opaque gate families exact on [-2000,2000]", rc == 0 and "audit:0" in out, out[:60])

        # 4) 引擎 E2E：kkksu-mini → MW-1 → R079(no-bind) → fib
        kk = pathlib.Path("/tmp/repro2/src-kkksu-mini.lua").read_text()
        pathlib.Path("k_src.lua").write_text(kk)
        subprocess.run([sys.executable, str(TOOLS / "mistweave.py"),
                        "k_src.lua", "k_mw.lua", "--seed", "20261025",
                        "--num-fog", "0.55", "--blocks", "6"],
                       check=True, capture_output=True)
        if NATIVE.exists():
            subprocess.run([str(NATIVE), "--preset", "v15", "--env-check",
                            "--no-bind-executor", "--seed", "20261025",
                            "k_mw.lua", "k_eng.lua"],
                           check=True, capture_output=True)
        else:
            eng = {"wasmPath": str(WASM), "workDir": str(work),
                   "args": ["--preset", "v15", "--env-check", "--bind-executor",
                            "--bind-env", "roblox", "--no-executor-guard",
                            "--seed", "20261025", "/work/k_mw.lua", "/work/k_eng.lua"]}
            subprocess.run(["node", str(RUNNER), __import__("json").dumps(eng)],
                           check=True, capture_output=True)
        rc, out = run_luau("k_eng.lua")
        check("E2E kkksu-mini -> MW-1 -> engine -> bare fib",
              rc == 0 and ENGV_FIB in out, out[:80])

        # 5) 体积打印
        b0 = len(kk.encode()); b1 = pathlib.Path("k_mw.lua").stat().st_size
        b2 = pathlib.Path("k_eng.lua").stat().st_size
        print(f"INFO  kkksu-mini: src {b0}B -> mw {b1}B (x{b1/b0:.1f}) -> engine {b2}B")
    finally:
        os.chdir(here)
    print()
    print(("FAILED" if FAILS else "OK") + f": {len(FAILS)} failing check(s)")
    return 1 if FAILS else 0

if __name__ == "__main__":
    sys.exit(main())
