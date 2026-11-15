#!/usr/bin/env python3
"""Regression for the authorized R035 mobile print-injection probe."""
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
REPO = ROOT.parent
GEN = ROOT / "tools/mobile_print_probe_gen.py"
TARGET = REPO / "engine/tests/fixtures/targets/R035_target.luau.lua"
TOOLS = REPO / ".tools/bin"
LUAU = TOOLS / "luau"
COMPILE = TOOLS / "luau-compile"
if not LUAU.exists():
    LUAU = pathlib.Path("/home/user/tools/bin/luau")
    COMPILE = pathlib.Path("/home/user/tools/bin/luau-compile")


def main():
    with tempfile.TemporaryDirectory() as td:
        td = pathlib.Path(td)
        probe = td / "probe.lua"
        subprocess.run([sys.executable, str(GEN), str(TARGET), str(probe)], check=True,
                       capture_output=True)
        syntax = subprocess.run([str(COMPILE), str(probe)], capture_output=True)
        tail = subprocess.run([str(LUAU), str(probe)], capture_output=True, text=True,
                              timeout=30)
        text = probe.read_text()
        print_probe = td / "print.lua"
        print_probe.write_text(text.replace('local MODE = "tail"',
                                            'local MODE = "print-hook"', 1))
        hooked = subprocess.run([str(LUAU), str(print_probe)], capture_output=True,
                                text=True, timeout=30)
        loader_probe = td / "loader.lua"
        loader_probe.write_text(text.replace('local MODE = "tail"',
                                             'local MODE = "loader-hook"', 1))
        loader_stalled = False
        try:
            subprocess.run([str(LUAU), str(loader_probe)], capture_output=True,
                           text=True, timeout=3)
        except subprocess.TimeoutExpired:
            loader_stalled = True

        checks = {
            "syntax": syntax.returncode == 0,
            "tail marker": tail.returncode == 0 and "MOBILE-TAIL>" in tail.stdout,
            "exact target": "bytes=150285 hash=1818006595" in tail.stdout,
            "top return nil": "top_return=\tnil" in tail.stdout,
            "print hook clean": hooked.returncode == 0
            and "captured_prints=0" in hooked.stdout
            and "finished ok=true" in hooked.stdout,
            "loader hook rejected": loader_stalled,
        }
        for name, ok in checks.items():
            print(("PASS" if ok else "FAIL") + " " + name)
        passed = sum(checks.values())
        print(f"MOBILE PRINT PROBE: {passed}/{len(checks)}")
        return 0 if passed == len(checks) else 1


if __name__ == "__main__":
    raise SystemExit(main())
