#!/usr/bin/env python3
"""Phase 18 P0-3: Robustness regression gate (C1-C4 resource limits + edge inputs)."""
import os
import subprocess
import sys
import tempfile

BIN = os.environ.get("OBSUCVIM_BIN", "/home/user/obsucvim/engine/target/release/obsucvim")

CASES = {
    "C1_100_toplevel_funcs": "\n".join(f"local function f{i}() return {i} end" for i in range(100)) + "\nprint(f0())\n",
    "C2_3000_toplevel_stmts": "\n".join(f"local v{i} = {i}" for i in range(3000)) + "\n",
    "C3_20_nested_closures": (
        "local function f0()\n"
        + "".join("  " * i + f"local function f{i}()\n" for i in range(1, 20))
        + "  " * 20
        + "return 1\n"
        + "".join("  " * i + "end\n" + "  " * i + f"return f{i}()\n" for i in range(19, 0, -1))
        + "end\nprint(f0())\n"
    ),
    "C4_20000_nested_parens": "local x = " + ("(" * 20000) + "1" + (")" * 20000) + "\n",
}

def main():
    if not os.path.exists(BIN):
        print(f"FAIL: missing binary {BIN}", file=sys.stderr)
        return 2
    passed = 0
    for name, src in CASES.items():
        with tempfile.NamedTemporaryFile("w", suffix=".lua", delete=False) as f:
            f.write(src)
            inp = f.name
        out = inp + ".out.lua"
        try:
            p = subprocess.run([BIN, "--preset", "v15", "--dialect", "luau", "--seed", "42", inp, out], capture_output=True, text=True, timeout=30)
            err = p.stderr.strip()
            if p.returncode == 0 or "panicked at" in err or "SIGABRT" in err or "SIGSEGV" in err or "RUST_BACKTRACE" in err:
                print(f"FAIL [{name}]: rc={p.returncode} stderr={err!r}", file=sys.stderr)
                return 1
            print(f"  [PASS] {name}: rc={p.returncode} {err}")
            passed += 1
        finally:
            for path in (inp, out):
                if os.path.exists(path):
                    os.remove(path)
    print(f"ROBUSTNESS GATE PASS: {passed}/{len(CASES)}")
    return 0

if __name__ == "__main__":
    sys.exit(main())
