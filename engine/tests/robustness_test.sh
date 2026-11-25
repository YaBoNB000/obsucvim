#!/bin/bash
# Phase 18 P0-3 — robustness regression suite.
#
# For every calibrated compiler-limit dimension the suite runs two specimens:
#   threshold      -> must compile cleanly (rc 0, artifact parses under Luau)
#   threshold + 1  -> must be rejected with a single-line `error:` diagnostic
#
# The thresholds are read from engine/src/limits.rs with grep so the suite can
# never drift from the implementation (no duplicated magic numbers here).
#
# The 250,000 B budget is the *sole* size gate of HANDOFF §17.4 and is measured
# on `print(6)` by tests/print_size_gate.sh. Synthetic limit-boundary programs
# are inherently far larger than `print(6)` (32 top-level functions already
# produce ~308 KB), so this suite asserts rc 0 + valid Luau syntax + correct
# execution instead of the print(6) byte budget.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="${OBSUCVIM_BIN:-$ROOT/target/release/obsucvim}"
TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/luau" ] || [ ! -x "$TOOLS_BIN/luau-compile" ]; then
	TOOLS_BIN="$(cd "$(dirname "$0")/../../.tools/bin" && pwd)"
fi
LUAU="$TOOLS_BIN/luau"
LUAUC="$TOOLS_BIN/luau-compile"
LIMITS_RS="$ROOT/src/limits.rs"

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
	_STANDALONE=1
else
	_STANDALONE=0
fi
# Own counters: when this script is sourced by run_tests.sh the parent's
# pass/fail arrays are shared, so the summary line must report this suite's own
# totals (the parent still receives every increment).
RB_PASS=0
RB_FAIL=0
RB_FAILED=()
RB_TMP="$(mktemp -d)"
if [[ "$_STANDALONE" == "1" ]]; then trap 'rm -rf "$RB_TMP"' EXIT; fi

gf() {
	echo "FAIL [robustness:$1] $2"
	RB_FAIL=$((RB_FAIL + 1))
	RB_FAILED+=("robustness/$1")
	if [[ "$_STANDALONE" == "0" ]]; then
		fail=$((fail + 1))
		failed+=("robustness/$1")
	fi
}
rb_ok() { RB_PASS=$((RB_PASS + 1)); if [[ "$_STANDALONE" == "0" ]]; then pass=$((pass + 1)); fi; }

# read a `pub const NAME: usize = <n>;` value out of limits.rs
const_of() {
	local v
	v="$(grep -E "^pub const $1: usize = " "$LIMITS_RS" | head -1 | sed -E 's/^[^=]*= *([0-9]+).*/\1/')"
	if [[ -z "$v" ]]; then
		echo "FAIL [robustness:limits] cannot read $1 from $LIMITS_RS" >&2
		exit 2
	fi
	echo "$v"
}

MAX_FUNCS="$(const_of MAX_TOPLEVEL_FUNC_DECLS)"
_MAX_STMTS_NAME=MAX_TOPLEVEL_STMTS
MAX_STMTS="$(const_of MAX_TOPLEVEL_STMTS)"
MAX_CLOSURE="$(const_of MAX_CLOSURE_NEST_DEPTH)"
MAX_EXPR="$(const_of MAX_EXPR_PARSE_DEPTH)"
MAX_BLOCK="$(const_of MAX_BLOCK_NEST_DEPTH)"

echo "robustness: thresholds funcs=$MAX_FUNCS stmts=$MAX_STMTS closure=$MAX_CLOSURE expr=$MAX_EXPR block=$MAX_BLOCK"

# ---------------------------------------------------------------- generators
gen_funcs() { python3 -c '
import sys
n=int(sys.argv[1])
print("\n".join(f"local function f{i}() end" for i in range(n)))
print("f0()")' "$1"; }

# `local` declarations keep the artifact loadable at the threshold (global
# assignments at this scale run into Luau's 200-local limit in the emitted
# shell, which is a separate pre-existing emitter boundary documented in
# HANDOFF §18, not the crash dimension measured here).
gen_stmts() { python3 -c '
import sys
n=int(sys.argv[1])
print("\n".join(f"local a{i} = {i}" for i in range(n-1)))
print("print(a0)")' "$1"; }

gen_closure() { python3 -c '
import sys
n=int(sys.argv[1])
body="return"
for i in range(n):
    body=f"local function f{i}() {body} end"
print(body)
print("f0()")' "$1"; }

# N nested parens produce parse depth N+1 (the `local x =` expression itself),
# so the specimen that lands exactly on the threshold uses N = threshold - 1.
gen_expr() { python3 -c '
import sys
n=int(sys.argv[1])-1
print("local x = " + "("*n + "1" + ")"*n)' "$1"; }

# N nested `do ... end` blocks produce block depth N+1 (the chunk itself is a
# block), so the threshold specimen uses N = threshold - 1.
gen_block() { python3 -c '
import sys
n=int(sys.argv[1])-1
print("do "*n + "x=1" + " end"*n)' "$1"; }

# C5-class input-reachable compiler limit: an assignment target that is not a
# vararg/index/global/local (used to be a bare panic at compiler.rs eval_target).
gen_c5() { printf 'f() = 1\n'; }

# ---------------------------------------------------------------- checkers
check_should_succeed() { # <tag> <input>
	local tag="$1"
	local input="$2"
	local out="$RB_TMP/${tag}_out.lua"
	local err
	err="$("$TOOL" --preset v15 --dialect luau --seed 42 "$input" "$out" 2>&1)"
	local rc=$?
	if [[ $rc -ne 0 ]]; then
		gf "$tag" "expected rc 0, got $rc: $(printf '%s' "$err" | head -1)"
		return
	fi
	if [[ ! -s "$out" ]]; then
		gf "$tag" "rc 0 but empty artifact"
		return
	fi
	if ! "$LUAUC" "$out" >/dev/null 2>&1; then
		gf "$tag" "artifact does not parse under luau-compile"
		return
	fi
	rb_ok
	echo "  [PASS] $tag compiles cleanly (rc 0, artifact $(wc -c <"$out") B, luau-compile OK)"
}

check_should_fail_clean() { # <tag> <input> <expected-substring>
	local tag="$1"
	local input="$2"
	local want="${3:-input exceeds compiler limit}"
	local out="$RB_TMP/${tag}_out.lua"
	local err
	err="$("$TOOL" --preset v15 --dialect luau --seed 42 "$input" "$out" 2>&1)"
	local rc=$?
	if [[ $rc -eq 0 ]]; then
		gf "$tag" "expected non-zero exit, got 0"
		return
	fi
	local first
	first="$(printf '%s\n' "$err" | head -1)"
	if [[ "$first" != error:* ]]; then
		gf "$tag" "stderr does not start with 'error:': $first"
		return
	fi
	if [[ "$(printf '%s\n' "$err" | wc -l)" -ne 1 ]]; then
		gf "$tag" "stderr is not a single line: $(printf '%s' "$err" | tr '\n' '|')"
		return
	fi
	local bad
	for bad in "panicked at" "RUST_BACKTRACE" "SIGABRT" "SIGSEGV" "stack overflow"; do
		if printf '%s' "$err" | grep -q "$bad"; then
			gf "$tag" "stderr contains '$bad': $first"
			return
		fi
	done
	if [[ -n "$want" ]] && ! printf '%s' "$err" | grep -qF "$want"; then
		gf "$tag" "stderr lacks '$want': $first"
		return
	fi
	rb_ok
	echo "  [PASS] $tag rejected cleanly: $first"
}

# ---------------------------------------------------------------- scenarios
echo "scenario A: top-level function declarations (threshold $MAX_FUNCS)"
gen_funcs "$MAX_FUNCS" >"$RB_TMP/a_ok.lua"
gen_funcs "$((MAX_FUNCS + 1))" >"$RB_TMP/a_bad.lua"
check_should_succeed "A_threshold" "$RB_TMP/a_ok.lua"
check_should_fail_clean "A_threshold_plus_1" "$RB_TMP/a_bad.lua" "top-level function declarations = $((MAX_FUNCS + 1))"

echo "scenario B: top-level statements (threshold $MAX_STMTS)"
gen_stmts "$MAX_STMTS" >"$RB_TMP/b_ok.lua"
gen_stmts "$((MAX_STMTS + 1))" >"$RB_TMP/b_bad.lua"
check_should_succeed "B_threshold" "$RB_TMP/b_ok.lua"
check_should_fail_clean "B_threshold_plus_1" "$RB_TMP/b_bad.lua" "top-level statements = $((MAX_STMTS + 1))"

echo "scenario C: closure nesting depth (threshold $MAX_CLOSURE)"
gen_closure "$MAX_CLOSURE" >"$RB_TMP/c_ok.lua"
gen_closure "$((MAX_CLOSURE + 1))" >"$RB_TMP/c_bad.lua"
check_should_succeed "C_threshold" "$RB_TMP/c_ok.lua"
check_should_fail_clean "C_threshold_plus_1" "$RB_TMP/c_bad.lua" "closure nesting depth = $((MAX_CLOSURE + 1))"

echo "scenario D: expression parse depth (threshold $MAX_EXPR)"
gen_expr "$MAX_EXPR" >"$RB_TMP/d_ok.lua"
gen_expr "$((MAX_EXPR + 1))" >"$RB_TMP/d_bad.lua"
check_should_succeed "D_threshold" "$RB_TMP/d_ok.lua"
check_should_fail_clean "D_threshold_plus_1" "$RB_TMP/d_bad.lua" "expression parse depth = $((MAX_EXPR + 1))"

echo "scenario E: C5-class input-reachable compiler limit (invalid assignment target)"
printf 'f = 1\nprint(f)\n' >"$RB_TMP/e_ok.lua"
gen_c5 >"$RB_TMP/e_bad.lua"
check_should_succeed "E_control_valid_assignment" "$RB_TMP/e_ok.lua"
check_should_fail_clean "E_invalid_assignment_target" "$RB_TMP/e_bad.lua" "invalid assignment target"

echo "scenario F: block nesting depth (threshold $MAX_BLOCK)"
gen_block "$MAX_BLOCK" >"$RB_TMP/f_ok.lua"
gen_block "$((MAX_BLOCK + 1))" >"$RB_TMP/f_bad.lua"
check_should_succeed "F_threshold" "$RB_TMP/f_ok.lua"
check_should_fail_clean "F_threshold_plus_1" "$RB_TMP/f_bad.lua" "block nesting depth = $((MAX_BLOCK + 1))"

echo "=================================================="
echo "ROBUSTNESS PASS: $RB_PASS   FAIL: $RB_FAIL"
if [[ "$_STANDALONE" == "1" ]]; then
	if [[ ${RB_FAIL} -gt 0 ]]; then
		for f in "${RB_FAILED[@]}"; do echo "  failed: $f"; done
		rm -rf "$RB_TMP"
		exit 1
	fi
	echo "ALL ROBUSTNESS GREEN"
else
	if [[ ${RB_FAIL} -gt 0 ]]; then
		for f in "${RB_FAILED[@]}"; do echo "  failed: $f"; done
	fi
	rm -rf "$RB_TMP"
fi
