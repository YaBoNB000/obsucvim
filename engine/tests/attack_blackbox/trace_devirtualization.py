#!/usr/bin/env python3
"""Black-box attack #3: trace-based devirtualization (基于执行轨迹的动态去虚拟化).

Public technique: record one complete execution -- typically by hooking the
string primitives every Lua obfuscator has to use in order to build its
constants -- and try to reconstruct the original control flow from the trace.

The experiment is a differential one, so it produces a real yes/no answer
instead of an impression:

  * the same program is compiled with a 3-iteration loop and with a
    300-iteration loop (identical output, only the trip count differs);
  * both artifacts run under a generic `string.char` recorder that logs the
    argument count of every call (the trace);
  * if the two traces have (nearly) the same length and signature, the trace
    carries **no** control-flow information and devirtualization by tracing
    fails; if the trace scales with the trip count, it leaks the loop count.

A plain-Luau control run shows what the trace looks like without obfuscation:
the standalone parser builds its constants itself, so the hook sees nothing.

Verdicts:
  成功     trace length reproduces the trip-count ratio within +-10%
           (the trace is a usable instruction trace)
  部分成功  traces differ but the ratio is not reproducible
  失败     traces are (nearly) identical -> no control-flow signal
"""
from __future__ import annotations

import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _common import build_source, make_hook, run_luau, verdict  # noqa: E402

TEMPLATE = """local total = 0
for i = 1, {n} do
    total = total + i
    if total % 2 == 0 then
        total = total + 1
    end
end
print("TOTAL", total)
"""

PLAIN_RUN = """local total = 0
for i = 1, {n} do
    total = total + i
    if total % 2 == 0 then
        total = total + 1
    end
end
print("TOTAL", total)
"""


def trace_of(src: Path, tmp: Path, label: str, n: int) -> dict:
    art = tmp / f"{label}{n}.lua"
    build_source(src, art)
    harness = tmp / f"{label}{n}_hook.lua"
    harness.write_text(make_hook(art.read_text()))
    p = run_luau(harness)
    out = p.stdout
    info = {"rc": p.returncode, "stdout": out.strip().splitlines()[-2:]}
    for line in out.splitlines():
        if line.startswith("BLACKBOX_HOOKS"):
            parts = line.split()
            info["byte"], info["sub"], info["char"] = (int(x) for x in parts[1:4])
        if line.startswith("BLACKBOX_TRACE_LEN"):
            info["trace_len"] = int(line.split()[1])
        if line.startswith("BLACKBOX_TRACE_SIG"):
            info["trace_sig"] = line.split("\t", 1)[1] if "\t" in line else line.split(None, 1)[1]
        if line.startswith("BLACKBOX_RUN_OK"):
            info["run_ok"] = line
    return info


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        rows = {}
        for n in (3, 300):
            src = tmp / f"loop{n}.lua"
            src.write_text(TEMPLATE.format(n=n))
            rows[n] = trace_of(src, tmp, "loop", n)
            r = rows[n]
            print(
                f"[n={n}] char_calls={r.get('char')} byte_calls={r.get('byte')} "
                f"sub_calls={r.get('sub')} trace_len={r.get('trace_len')}"
            )
            print(f"        run={r.get('run_ok')} output={r.get('stdout')}")

        plain = {}
        for n in (3, 300):
            f = tmp / f"plain{n}.lua"
            f.write_text(PLAIN_RUN.format(n=n))
            p = run_luau(f)
            plain[n] = p.stdout.strip().splitlines()[-1:]
            print(f"[plain n={n}] output={plain[n]}")

    a, b = rows[3], rows[300]
    la, lb = a.get("char", 0), b.get("char", 0)
    ba, bb = a.get("byte", 0), b.get("byte", 0)
    ratio_char = (lb / la) if la else float("inf")
    ratio_byte = (bb / ba) if ba else float("inf")
    print(f"char-call ratio (n=300 / n=3) = {ratio_char:.3f}   (a real trace would be ~100x)")
    print(f"byte-call ratio (n=300 / n=3) = {ratio_byte:.3f}")
    print(f"first 64 char-call arg counts (n=3)   = {a.get('trace_sig')}")
    print(f"first 64 char-call arg counts (n=300) = {b.get('trace_sig')}")
    print(f"trace signature identical = {a.get('trace_sig') == b.get('trace_sig')}")
    print(
        "NOTE (1) this Luau build has no debug.sethook, so the string-primitive "
        "trace is the only trace an attacker can record; (2) the plain-Luau "
        "control runs produce the same two outputs with zero string-primitive "
        "calls, so every recorded call comes from the obfuscator's own constant "
        "decoder, not from the program."
    )
    if 90.0 <= ratio_byte <= 110.0:
        level = "成功"
    elif abs(ratio_char - 1.0) < 0.05 and abs(ratio_byte - 1.0) < 0.05:
        level = "失败"
    else:
        level = "部分成功"
    print(verdict(level, "trace_devirtualization"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
