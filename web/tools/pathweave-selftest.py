#!/usr/bin/env python3
# 径织 PW-1 门禁（自验）：真车道 / 幻径车道 / 探针单点判别 / 篡改回退 /
# 确定性 / 体积口径。stdout 末行打印 PASS/FAIL 汇总，rc 随 FAIL 非零。
# 用法: LUAU_BIN=/path/luau python3 pathweave-selftest.py
import hashlib, os, pathlib, subprocess, sys, tempfile

TOOLS = pathlib.Path(__file__).resolve().parent
LUAU = os.environ.get("LUAU_BIN") or "luau"
FAILS = []

def check(name, ok, detail=""):
    print(("PASS " if ok else "FAIL ") + name + (" | " + detail[:100] if detail else ""))
    if not ok:
        FAILS.append(name)

def run_luau(path, timeout=120):
    p = subprocess.run([LUAU, str(path)], capture_output=True, text=True,
                       timeout=timeout)
    return p.returncode, (p.stdout or "") + (p.stderr or "")

STUB_TRUE = r'''
-- Delta 形制桩（仅供本地门禁：shadow type 让 game 呈 userdata）
local _t=type
game={}
type=function(v) if v==game then return "userdata" end return _t(v) end
identifyexecutor=function() return "Delta" end
checkcaller=function() return true end
Vector3={new=function() return 0 end}
task={spawn=function(f) return f() end}
'''

SRC = pathlib.Path("/tmp/repro2/src-kkksu-mini.lua")
BASE = pathlib.Path("/tmp/repro2/pw/nobind.lua")
FIB = "kkksu-mini:1,1,2,3,5,8,13,21,34,55"

def main():
    work = pathlib.Path(tempfile.mkdtemp(prefix="pw-gate-"))
    here = os.getcwd()
    os.chdir(work)
    try:
        # 0. 基件健康（纯净 luau 直跑真 fib）
        rc, out = run_luau(BASE)
        check("base no-bind fib prints", rc == 0 and FIB in out)

        # 1. 确定性：同种子双包逐字节恒等；异种子不同
        subprocess.run([sys.executable, str(TOOLS / "pathweave.py"), str(BASE),
                        "w1.lua", "--lanes", "6", "--seed", "20261025",
                        "--label", "kkksu-mini"], check=True, capture_output=True)
        subprocess.run([sys.executable, str(TOOLS / "pathweave.py"), str(BASE),
                        "w2.lua", "--lanes", "6", "--seed", "20261025",
                        "--label", "kkksu-mini"], check=True, capture_output=True)
        subprocess.run([sys.executable, str(TOOLS / "pathweave.py"), str(BASE),
                        "w3.lua", "--lanes", "6", "--seed", "424242",
                        "--label", "kkksu-mini"], check=True, capture_output=True)
        h1 = hashlib.sha256(pathlib.Path("w1.lua").read_bytes()).hexdigest()
        h2 = hashlib.sha256(pathlib.Path("w2.lua").read_bytes()).hexdigest()
        h3 = hashlib.sha256(pathlib.Path("w3.lua").read_bytes()).hexdigest()
        check("determinism: same seed byte-identical", h1 == h2)
        check("determinism: different seed differs", h1 != h3)

        # 2. 真车道：Delta 形制桩前置 → 真 fib
        pathlib.Path("true_lane.lua").write_text(
            STUB_TRUE + "\n" + pathlib.Path("w1.lua").read_text())
        rc, out = run_luau("true_lane.lua")
        check("PW-1 true lane (stubbed Delta shape): real payload prints fib",
              rc == 0 and FIB in out, out[:120])

        # 3. 幻径：裸 luau（无桩）→ 静默诱饵（exit 0、非 fib、无报错文案）
        rc, out = run_luau("w1.lua")
        check("PW-1 mirage on bare luau: decoy, silent, no fib",
              rc == 0 and FIB not in out and "error" not in out.lower(), out[:120])

        # 4. 探针单点判别（幻径 flight 本地版）：逐项翻桩探针 ⇒ 必须幻径
        flips = {
            "flip-identifyexecutor": STUB_TRUE.replace(
                'return "Delta"', 'return "Synapse X"'),
            "flip-Vector3": STUB_TRUE.replace(
                "Vector3={new=function() return 0 end}", "Vector3=nil"),
            "flip-task": STUB_TRUE.replace(
                "task={spawn=function(f) return f() end}", "task=nil"),
            "flip-checkcaller": STUB_TRUE.replace(
                "checkcaller=function() return true end", "checkcaller=nil"),
            "flip-type-shape": STUB_TRUE.replace(
                'return "userdata"', 'return "table"'),
        }
        for name, stub in flips.items():
            pathlib.Path(name + ".lua").write_text(
                stub + "\n" + pathlib.Path("w1.lua").read_text())
            rc, out = run_luau(name + ".lua")
            check("probe flight %s -> mirage" % name,
                  rc == 0 and FIB not in out, out[:90])

        # 5. 篡改回退（紊壳后语义化定位，与壳形无关）：找全文件最长 alphabet
        #    连跑（=某条车道密文段），中点 8 字符在 alphabet 集内映射改字
        #    （保证语法不烂、解码照常）→ 真帧校验失败 → 幻径。
        ALPH = ("0123456789"
                "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
                "abcdefghijklmnopqrstuvwxyz"
                "!#$%&()*+,-./:;<=>?@[]^")
        aset = set(ALPH)
        txt = pathlib.Path("w1.lua").read_text()
        best_s = best_e = 0
        s0 = None
        for idx, ch in enumerate(txt):
            if ch in aset:
                if s0 is None:
                    s0 = idx
            else:
                if s0 is not None and idx - s0 > best_e - best_s:
                    best_s, best_e = s0, idx
                s0 = None
        if s0 is not None and len(txt) - s0 > best_e - best_s:
            best_s, best_e = s0, len(txt)
        ok = best_e - best_s >= 64
        if ok:
            mid = best_s + (best_e - best_s) // 2
            buf = list(txt)
            for k in range(8):
                buf[mid + k] = ALPH[(ALPH.index(buf[mid + k]) + 17) % 85]
            pathlib.Path("true_lane_tamper.lua").write_text(
                STUB_TRUE + "\n" + "".join(buf))
            rc, out = run_luau("true_lane_tamper.lua")
            check("tamper: blob edit under true env -> mirage (no fib)",
                  rc == 0 and FIB not in out,
                  ("run=%dB " % (best_e - best_s)) + out[:80])
        else:
            check("tamper: blob edit under true env -> mirage (no fib)", False,
                  "ciphertext run not located")

        # 6. 体积口径打印（预算参考：引擎件 180–200KB 为旧约束）
        sz = pathlib.Path("w1.lua").stat().st_size
        print(f"INFO  size: base {BASE.stat().st_size}B -> wrapped {sz}B "
              f"(x{sz / BASE.stat().st_size:.3f})")
    finally:
        os.chdir(here)
    print()
    print(("FAILED" if FAILS else "OK") + f": {len(FAILS)} failing check(s)")
    return 1 if FAILS else 0

if __name__ == "__main__":
    sys.exit(main())
