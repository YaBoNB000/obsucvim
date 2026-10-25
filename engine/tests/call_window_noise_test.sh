#!/bin/bash
# H3-6 regression: dead-register noise must never write inside an open
# dynamic call-result window (Call/CallE/CallM results land at logical
# base+1..base+nout at runtime; nout is unknown at build time).
#
# Discovered 2026-10-09 via the extended random-seed sweep: stress_multival
# silently corrupted vararg/multi-return forwarding on ~0.7% of seeds
# (e.g. `fwd1: 1 9 3` instead of `1 2 3`). Root cause and ablation proof:
# research/antidebug-20261009/root-cause/.
#
# Fixed 2026-10-11 in compiler.rs (call_window_mask + noise relocation +
# build-time invariant). This script pins the historical failure seeds and
# a fresh random sweep so the class cannot silently return.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$ROOT/target/release/obsucvim"
TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/luau" ]; then
	TOOLS_BIN="$(cd "$(dirname "$0")/../../.tools/bin" && pwd)"
fi
LUAU="$TOOLS_BIN/luau"
if [ ! -x "$TOOL" ]; then
	echo "error: missing $TOOL (cargo build --release first)" >&2
	exit 2
fi
if [ ! -x "$LUAU" ]; then
	echo "error: missing $LUAU (bash engine/tools/rebuild_tools.sh)" >&2
	exit 2
fi

SRC="$ROOT/tests/cases/stress_multival.lua"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# Historical failure seeds (9 known-bad pre-fix) plus adjacent controls.
SEEDS="
	10770986916448599986
	3525582012654347846
	12552293900259736015
	2087317583667228854
	16157647636138958350
	10407755351155896398
	6026130555315556988
	2791381894518475382
	12675743238487333885
	42
	64075
	65065
	66066
"

fail=0
expected="$("$LUAU" "$SRC")"
for seed in $SEEDS; do
	out="$TMP/out_$seed.lua"
	if ! "$TOOL" --no-bind-executor --seed "$seed" "$SRC" "$out" 2>"$TMP/err_$seed"; then
		echo "FAIL [build] seed=$seed: $(head -1 "$TMP/err_$seed")"
		fail=$((fail + 1))
		continue
	fi
	if ! "$LUAU" "$out" >"$TMP/run_$seed" 2>&1; then
		echo "FAIL [run] seed=$seed: $(head -1 "$TMP/run_$seed")"
		fail=$((fail + 1))
		continue
	fi
	if [[ "$expected" != "$(cat "$TMP/run_$seed")" ]]; then
		echo "FAIL [eq] seed=$seed: output differs from source"
		diff <(echo "$expected") "$TMP/run_$seed" | head -4
		fail=$((fail + 1))
	else
		echo "PASS seed=$seed"
	fi
done

echo "call-window noise regression: $((13 - fail))/13 PASS, FAIL=$fail"
[ "$fail" -eq 0 ]
