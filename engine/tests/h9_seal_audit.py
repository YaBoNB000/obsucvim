#!/usr/bin/env python3
"""H4-9 密封有效性审计（取代恒假无后果的 +7919 版本）。

判定密封“真”而不是“假”的四条标准，全部用产物本体取证：

  [1] 密封被初始化且非 nil —— 否则比对因 nil~=nil 被永久跳过（恒假陷阱）。
  [2] 正确构建：两类比对槽相等，产物语义与源码一致（密封不误报）。
  [3] 篡改解码载荷字节 → 必须触发可见失败（不能只“加常数后自愈”）。
  [4] 篡改存放的校验值 → 必须触发可见失败。

[3] 是关键：恒等式型密封（(a+b-a-b)、自比较自身）在被篡改时仍通过，
本审计会把它判红。审计只读产物文本，不改引擎状态。
"""
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOOL = ROOT / "target/release/obsucvim"
for cand in (pathlib.Path("/home/user/luraph/.tools/bin"),
             pathlib.Path("/home/user/tools/bin"),
             ROOT.parent / ".tools/bin"):
    if (cand / "luau").exists():
        LUAU = cand / "luau"
        break
else:
    LUAU = pathlib.Path("/home/user/tools/bin/luau")

SRC = r'''local t = {}
local function f(x) for i = 1, x do t[#t + 1] = i * i end return #t end
print(f(6), t[3], "sealprobe")
'''

# 保真构建：关掉所有会重写源码形态的 pass，便于按结构定位密封片段。
RAW_FLAGS = ["--preset", "v15", "--seed", "42", "--no-guard", "--no-mangle",
             "--no-minify", "--no-strings", "--no-numbers", "--no-junk"]


def build(src_path, out_path, flags):
    p = subprocess.run([str(TOOL), *flags, str(src_path), str(out_path)],
                       capture_output=True, text=True)
    if p.returncode:
        raise RuntimeError(p.stderr[:600])
    return pathlib.Path(out_path).read_text()


def run(path):
    p = subprocess.run([str(LUAU), str(path)], capture_output=True, timeout=120)
    return p.returncode, p.stdout


def find_seal(text):
    """定位 V2 密封：`if C[a] ~= C[b] then ... C[f]=1 end`。"""
    m = re.search(r"if C\[(\d+)\] ~= C\[(\d+)\] then([\s\S]{0,240}?)end", text)
    if not m:
        return None
    return {"seal": int(m.group(1)), "sum": int(m.group(2)), "span": m.group(0)}


def main():
    if not TOOL.exists() or not LUAU.exists():
        print("missing release binary or Luau", file=sys.stderr)
        return 2
    with tempfile.TemporaryDirectory() as td:
        td = pathlib.Path(td)
        src = td / "src.lua"
        src.write_text(SRC)
        honest = build(src, td / "honest.lua", RAW_FLAGS)
        seal = find_seal(honest)
        print("== H4-9 密封有效性审计 ==")
        if not seal:
            print("[1] 产物中存在两槽不等比对      : NO")
            print("\nH4-9 审计: FAIL")
            return 1

        # [1] 初始化：C[seal] = C[sum] 必须出现在入口装配处（非 nil）。
        init_pat = (r"C\[{s}\] = 0;\s*\n?\s*C\[\w+\] = 0;\s*\n?\s*"
                    r"C\[{v}\] = C\[{s}\]").format(v=seal["seal"], s=seal["sum"])
        has_init = re.search(init_pat, honest) is not None
        print(f"[1] 密封槽初始化且非 nil        : "
              f"{'YES' if has_init else 'NO'} "
              f"(C[{seal['seal']}] 对 C[{seal['sum']}])")

        # [2] 正确构建不误报。
        rc_src, out_src = run(src)
        rc_ok, out_ok = run(td / "honest.lua")
        faithful = rc_src == rc_ok == 0 and out_src == out_ok
        print(f"[2] 正确构建语义等价(不误报)    : "
              f"{'YES' if faithful else 'NO'} (out={out_ok!r})")

        # [3] 篡改解码载荷 → 必须失败。载荷折叠里第一处 `+= b[n](C[1], i)`。
        acc = re.search(r"(\w+) \+= b\[\d+\]\(C\[\d+\], i\)", honest)
        tampered_payload = None
        if acc:
            bad = honest.replace(acc.group(0), acc.group(0) + " + 1", 1)
            p = td / "payload.lua"
            p.write_text(bad)
            rc3, out3 = run(p)
            tampered_payload = (rc3 != 0 or out3 != out_src)
        print(f"[3] 篡改解码载荷触发失败        : "
              f"{'YES' if tampered_payload else ('NO' if acc else 'SKIP')}")

        # [4] 篡改存放的校验值 → 必须失败。
        store = re.search(r"C\[{}\] = (\w+);".format(seal["seal"]), honest)
        tampered_seal = None
        if store:
            bad = honest.replace(store.group(0),
                                 f"C[{seal['seal']}] = {store.group(1)} + 1;", 1)
            p = td / "seal.lua"
            p.write_text(bad)
            rc4, out4 = run(p)
            tampered_seal = (rc4 != 0 or out4 != out_src)
        print(f"[4] 篡改校验值触发失败          : "
              f"{'YES' if tampered_seal else ('NO' if store else 'SKIP')}")

        ok = all([has_init, faithful, tampered_payload, tampered_seal])
        print("\nH4-9 审计: " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
