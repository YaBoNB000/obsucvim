#!/bin/bash
# Luau-only benchmark suite covering default `v15`, `v15 --no-guard`, and
# legacy presets (`low`, `medium`, `high`, `vm`, `max`) across 8 representative
# corpus workloads (simple, strings, functions/closures, control flow, tables,
# coroutine, real algorithm, and big-table stress).
#
# Reports 3-round median wall-clock build/run times, input/output sizes,
# slowdown ratio, child Peak RSS (Linux wait4 ru_maxrss in KB), exit code,
# and exact stdout equivalence against stock Luau 0.735.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="${OBSUCVIM_BIN:-$ROOT/target/release/obsucvim}"
TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/luau" ]; then
	TOOLS_BIN="$(cd "$(dirname "$0")/../../.tools/bin" && pwd)"
fi
LUAU="$TOOLS_BIN/luau"
if [ ! -x "$TOOL" ]; then
	echo "error: missing $TOOL (run cargo build --release first)" >&2
	exit 2
fi
if [ ! -x "$LUAU" ]; then
	echo "error: missing $LUAU (run bash engine/tools/rebuild_tools.sh first)" >&2
	exit 2
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

CASES="${BENCH_CASES:-basics strings functions control tables coroutine real_algo stress_bigtable}"
MODES="${BENCH_MODES:-v15 v15_noguard low medium high vm max}"
ROUNDS="${BENCH_ROUNDS:-3}"

python3 - "$TOOL" "$LUAU" "$ROOT" "$TMP" "$ROUNDS" "$MODES" $CASES <<'PY'
import os
import platform
import statistics
import subprocess
import sys
import time

tool, luau, root, tmp, rounds_s, modes_s = sys.argv[1:7]
rounds = int(rounds_s)
modes = modes_s.split()
cases = sys.argv[7:]

MODE_FLAGS = {
    "v15": ["--preset", "v15", "--dialect", "luau"],
    "v15_noguard": ["--preset", "v15", "--no-guard", "--dialect", "luau"],
    "low": ["--preset", "low", "--dialect", "luau"],
    "medium": ["--preset", "medium", "--dialect", "luau"],
    "high": ["--preset", "high", "--dialect", "luau"],
    "vm": ["--preset", "vm", "--dialect", "luau"],
    "max": ["--preset", "max", "--dialect", "luau"],
}


def run_measured(cmd, timeout=120):
    """Run cmd in an isolated Python helper that calls os.wait4 to capture exact per-child ru_maxrss (KB)."""
    helper = (
        "import os, resource, subprocess, sys, time\n"
        "t0 = time.perf_counter()\n"
        "p = subprocess.Popen(sys.argv[1:], stdout=subprocess.PIPE, stderr=subprocess.PIPE)\n"
        "out, err = p.communicate()\n"
        "dt = (time.perf_counter() - t0) * 1000.0\n"
        "rss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss\n"
        "sys.stdout.buffer.write(f'{p.returncode}|{dt:.3f}|{rss}\\n'.encode() + out)\n"
    )
    p = subprocess.run([sys.executable, "-c", helper, *cmd], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
    nl = p.stdout.index(b"\n")
    hdr = p.stdout[:nl].decode()
    out = p.stdout[nl + 1:]
    rc_s, dt_s, rss_s = hdr.split("|")
    return int(rc_s), float(dt_s), int(rss_s), out


def median_run(cmd, n, timeout=120):
    dts = []
    rsss = []
    last_rc = 0
    last_out = b""
    for _ in range(n):
        rc, dt, rss, out = run_measured(cmd, timeout=timeout)
        dts.append(dt)
        rsss.append(rss)
        last_rc = rc
        last_out = out
    return last_rc, statistics.median(dts), int(statistics.median(rsss)), last_out


rustc_ver = "N/A"
for cand in [os.path.join(os.path.dirname(luau), "rustc"), "rustc"]:
    try:
        rustc_ver = subprocess.check_output([cand, "--version"], text=True).strip()
        break
    except Exception:
        pass

print(f"# Benchmark Environment: {platform.system()} {platform.release()} ({platform.machine()}), {rustc_ver}, Luau 0.735, rounds={rounds} (median), RSS via Linux wait4 ru_maxrss (KB)")
print("| 语料 | 模式 | 输入 B | 输出 B | 构建 ms | 原脚本 ms | 混淆后 ms | 减速 | Peak RSS KB | 退出码 | 输出等价 |")
print("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|:---:|")

failures = 0
for case in cases:
    src = os.path.join(root, "tests", "cases", f"{case}.lua")
    in_b = os.path.getsize(src)
    orig_rc, orig_ms, _, orig_out = median_run([luau, src], rounds, timeout=30)
    for mode in modes:
        flags = MODE_FLAGS[mode]
        out = os.path.join(tmp, f"{case}.{mode}.lua")
        obf_dts = []
        obf_rc = 0
        for _ in range(rounds):
            t0 = time.perf_counter()
            p = subprocess.run([tool, *flags, "--seed", "42", src, out], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90)
            obf_dts.append((time.perf_counter() - t0) * 1000.0)
            obf_rc = p.returncode
        obf_ms = statistics.median(obf_dts)
        out_b = os.path.getsize(out) if os.path.isfile(out) else 0
        if obf_rc != 0:
            print(f"| `{case}` | `{mode}` | {in_b} | {out_b} | {obf_ms:.1f} | {orig_ms:.2f} | FAIL | N/A | N/A | {obf_rc} | FAIL |")
            failures += 1
            continue
        run_rc, run_ms, rss_kb, run_out = median_run([luau, out], rounds, timeout=120)
        eq = (run_rc == orig_rc and run_out == orig_out)
        if not eq:
            failures += 1
        slow = (run_ms / orig_ms) if orig_ms > 0.001 else float("nan")
        print(
            f"| `{case}` | `{mode}` | {in_b} | {out_b} | {obf_ms:.1f} | "
            f"{orig_ms:.2f} | {run_ms:.2f} | {slow:.1f}× | {rss_kb} | {run_rc} | {'PASS' if eq else 'FAIL'} |"
        )
        sys.stdout.flush()

if failures:
    sys.exit(1)
PY
