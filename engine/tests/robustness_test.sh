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
# Scenario letters follow the task book: E = C5 (`child_index >= fns.len()`),
# whose trigger condition is "a closure in an AST position the slot counter
# does not enumerate" -- so E puts a closure in every enumerated position and
# additionally checks that the artifact *runs* with the reference output. G/H
# are the P4-audit findings (break/continue inside a function nested in a loop;
# the u16 constants-section length bound).
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
	v="$(grep -E "^pub const $1: usize = " "$LIMITS_RS" | head -1 | sed -E 's/^[^=]*= *([0-9_]+).*/\1/' | tr -d '_')"
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
MAX_CSEC="$(const_of MAX_CONSTS_SECTION_BYTES)"

echo "robustness: thresholds funcs=$MAX_FUNCS stmts=$MAX_STMTS closure=$MAX_CLOSURE expr=$MAX_EXPR block=$MAX_BLOCK consts_section=$MAX_CSEC"

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

# Scenario E = C5: a closure in every syntactic position count_stmt /
# count_expr enumerates (local value, table array/key/index value, call and
# method args, call callee, if/elseif/else condition, while/repeat condition,
# for-num start/limit/step, for-in iterator, every loop body, do body, unary
# and binary operands, if-expression arms, index key, concat, nested closures
# capturing an upvalue). Kept in sync with the Rust unit test
# `c5_count_fns_matches_compiled_closure_slots` (40 `function` keywords; the
# v15 S table caps a program at main + 41 closures, so the specimen stays under it).
gen_c5_positions() { cat <<'C5EOF'
local function lf() return 1 end
local a = function() return 2 end
local obj = {}
function obj:m(x) return function() return x end end
function obj.f() return 3 end
local t = { function() return 4 end, k = function() return 5 end, [1] = function() return 6 end }
local key = (function() return 9 end)()
t[key] = 9
print(function() return 7 end)
local r = (function() return 8 end)()
local x = true and function() return 10 end or 1
local y = not function() return false end
local i = t[function() return 1 end]
local z = a and (function() return 1 end) or function() return 2 end
local acc = 0
if (function() return true end)() then acc = acc + (function() return 1 end)()
elseif (function() return false end)() then acc = acc + (function() return 2 end)()
else acc = acc + (function() return 3 end)() end
while (function() return false end)() do local w = function() end end
local u
repeat u = function() acc = acc + 4 end until (function() return true end)()
u()
for i = (function() return 1 end)(), (function() return 2 end)(), (function() return 1 end)() do local b = function() return i end end
for k, v in (function() return pairs({}) end)() do local c = function() end end
do local d = function() acc = acc + 5 end d() end
local e = ("x"):rep(1, (function() return "" end)())
local m = ("abc"):sub((function() return 1 end)(), 2)
local n = t.a
local q = if x then function() return 11 end else function() return 12 end
local up = 6
local function o() return function() return function() return up end end end
local s = "x" .. (function() return "y" end)()
print(lf(), a(), obj.f(), obj:m(7)(), r, i, z, acc, e, m, q(), o()()(), s)
C5EOF
}

# P4 audit finding: invalid assignment target (clean rejection; it was never
# C5 and is no longer labelled that way).
gen_invalid_target() { printf 'f() = 1\n'; }

# P4 audit finding: `break`/`continue` inside a function nested in a loop. The
# parser's loop counter used to leak into function bodies and the statement hit
# the bare `expect("break outside loop")` panic in vmgen (rc 101).
gen_break_in_fn() { printf 'while true do local function f() break end end\n'; }
gen_continue_in_fn() { printf 'for i=1,2 do local f = function() continue end end\n'; }

# P4 audit finding: the per-function constants section stores its byte length
# as a u16 varint; `n` bytes of sealed literal in one function.
gen_consts_section() { python3 -c '
import sys
n=int(sys.argv[1])
print("local s = \"" + "a"*n + "\"")
print("print(#s)")' "$1"; }

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

echo "scenario E: C5 child_index >= fns.len() -- a closure in every counted position must compile AND run"
gen_c5_positions >"$RB_TMP/e_ok.lua"
check_should_succeed "E_c5_closure_positions" "$RB_TMP/e_ok.lua"
if [[ $RB_FAIL -eq 0 ]]; then
	# 生效证据：产物在参考解释器下的输出必须与源码一致（打印顺序/值全部相同）。
	# `print(function() ...)` / `local z = a and (function) or function` print
	# function addresses: normalise them so only values are compared.
	norm() { sed -E 's/function: 0x[0-9a-f]+/function: ADDR/g'; }
	e_ref="$("$LUAU" "$RB_TMP/e_ok.lua" 2>&1 | norm)"
	e_got="$("$LUAU" "$RB_TMP/E_c5_closure_positions_out.lua" 2>&1 | norm)"
	if [[ "$e_ref" != "$e_got" ]]; then
		gf "E_c5_closure_positions_run" "artifact output differs from the source under luau"
	else
		rb_ok
		echo "  [PASS] E_c5_closure_positions_run artifact output == source output under luau"
	fi
fi

echo "scenario G: P4 audit -- input-reachable rejections that used to be mislabelled or were bare panics"
printf 'f() = 1\n' >"$RB_TMP/g_target.lua"
check_should_fail_clean "G_invalid_assignment_target" "$RB_TMP/g_target.lua" "invalid assignment target"
gen_break_in_fn >"$RB_TMP/g_break.lua"
gen_continue_in_fn >"$RB_TMP/g_continue.lua"
check_should_fail_clean "G_break_inside_function_in_loop" "$RB_TMP/g_break.lua" "'break' outside of a loop"
check_should_fail_clean "G_continue_inside_function_in_loop" "$RB_TMP/g_continue.lua" "'continue' outside of a loop"
printf 'for i=1,2 do if i==2 then break end end\nprint("ok")\n' >"$RB_TMP/g_ok.lua"
check_should_succeed "G_control_break_inside_loop" "$RB_TMP/g_ok.lua"

echo "scenario H: P4 audit -- per-function constants section u16 length bound ($MAX_CSEC B, structural)"
gen_consts_section $((MAX_CSEC - 64)) >"$RB_TMP/h_ok.lua"
gen_consts_section $((MAX_CSEC + 1)) >"$RB_TMP/h_bad.lua"
check_should_succeed "H_consts_section_under_limit" "$RB_TMP/h_ok.lua"
if [[ $RB_FAIL -eq 0 ]]; then
	h_got="$(timeout 60 "$LUAU" "$RB_TMP/H_consts_section_under_limit_out.lua" 2>&1 | head -1)"
	if [[ "$h_got" != "$((MAX_CSEC - 64))" ]]; then
		gf "H_consts_section_under_limit_run" "artifact must print the literal length $((MAX_CSEC - 64)), got: $h_got"
	else
		rb_ok
		echo "  [PASS] H_consts_section_under_limit_run artifact prints $h_got under luau"
	fi
fi
check_should_fail_clean "H_consts_section_over_limit" "$RB_TMP/h_bad.lua" "constant section = "

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
