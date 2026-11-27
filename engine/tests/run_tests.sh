#!/bin/bash
# M0+ test matrix (mandatory workflow):
# every corpus case x every applicable dialect:
#   1. tool must succeed
#   2. obfuscated output must run with IDENTICAL stdout+exit as the original
#      (under the dialect's interpreter)
#   3. 5.1-target output must also run identically under Luau
#   4. luau-target output must pass luau-compile syntax check
# A timeout (exit 124) is always a FAIL (the corpus must terminate).
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$ROOT/target/release/obsucvim"
# toolchain: prefer /home/user/tools/bin; fall back to the in-repo
# .tools/bin (survives sandbox resets that wipe /home/user/tools)
TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/lua51" ] || [ ! -x "$TOOLS_BIN/luau-compile" ]; then
	# repo-root .tools (survives sandbox resets that wipe /home/user/tools)
	TOOLS_BIN="$(cd "$(dirname "$0")/../../.tools/bin" && pwd)"
fi
LUA51="$TOOLS_BIN/lua51"
LUAU="$TOOLS_BIN/luau"
LUAUC="$TOOLS_BIN/luau-compile"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

pass=0
fail=0
failed=()

run() { # run <interp> <file>  -> stdout+stderr; returns exit code
	timeout 30 "$1" "$2" 2>&1
}

for case in "$ROOT"/tests/cases/*.lua; do
	base="$(basename "$case" .lua)"
	# ㉖: 产品线 Luau-only（5.1 相位移除）
	dialects="luau"
	for d in $dialects; do
		out="$TMP/${base}__${d}.lua"
		if ! "$TOOL" --dialect "$d" --seed 42 "$case" "$out" 2>"$TMP/tool_err.txt"; then
			echo "FAIL [tool]  $base ($d): $(head -1 "$TMP/tool_err.txt")"
			fail=$((fail + 1)); failed+=("$base/$d/tool")
			continue
		fi
		if [[ "$d" == "5.1" ]]; then
			interp=$LUA51
		else
			interp=$LUAU
		fi
		o1="$(run "$interp" "$case")"; c1=$?
		o2="$(run "$interp" "$out")"; c2=$?
		if [[ "$c1" != "$c2" || "$o1" != "$o2" ]]; then
			echo "FAIL [run]   $base ($d): exit $c1 vs $c2"
			diff <(echo "$o1") <(echo "$o2") | head -6
			fail=$((fail + 1)); failed+=("$base/$d/run")
		else
			pass=$((pass + 1))
		fi
		# cross: 5.1-target output must also run identically under luau
		if [[ "$d" == "5.1" ]]; then
			c3="$(run "$LUAU" "$out")"; rc3=$?
			c1l="$(run "$LUAU" "$case")"; rc1l=$?
			if [[ "$rc3" != "$rc1l" || "$c3" != "$c1l" ]]; then
				echo "FAIL [cross] $base (5.1out on luau): exit $rc1l vs $rc3"
				diff <(echo "$c1l") <(echo "$c3") | head -6
				fail=$((fail + 1)); failed+=("$base/cross")
			else
				pass=$((pass + 1))
			fi
		fi
		# luau-compile syntax check for luau targets
		if [[ "$d" == "luau" ]]; then
			if ! timeout 30 "$LUAUC" "$out" > /dev/null 2>"$TMP/luac_err.txt"; then
				echo "FAIL [luac] $base: $(head -1 "$TMP/luac_err.txt")"
				fail=$((fail + 1)); failed+=("$base/luac")
			else
				pass=$((pass + 1))
			fi
		fi
	done
done

# ---------------------------------------------------------------
# VM phase (M4/L6): same matrix with --vm (private bytecode +
# generated obfuscated interpreter). VM containers are large and
# run a Lua-on-Lua interpreter, so timeouts are 120s.
# ---------------------------------------------------------------
runvm() { # runvm <interp> <file>
	timeout 120 "$1" "$2" 2>&1
}

for case in "$ROOT"/tests/cases/*.lua; do
	base="$(basename "$case" .lua)"
	# ㉖: 产品线 Luau-only（5.1 相位移除）
	dialects="luau"
	for d in $dialects; do
		out="$TMP/${base}__${d}.vm.lua"
		if ! "$TOOL" --vm --dialect "$d" --seed 42 "$case" "$out" 2>"$TMP/tool_err.txt"; then
			echo "FAIL [vm:tool]  $base ($d): $(head -1 "$TMP/tool_err.txt")"
			fail=$((fail + 1)); failed+=("$base/$d/vm/tool")
			continue
		fi
		if [[ "$d" == "5.1" ]]; then
			interpret=$LUA51
		else
			interpret=$LUAU
		fi
		o1v="$(runvm "$interpret" "$case")"; c1v=$?
		o2v="$(runvm "$interpret" "$out")"; c2v=$?
		if [[ "$c1v" != "$c2v" || "$o1v" != "$o2v" ]]; then
			echo "FAIL [vm:run]   $base ($d): exit $c1v vs $c2v"
			diff <(echo "$o1v") <(echo "$o2v") | head -6
			fail=$((fail + 1)); failed+=("$base/$d/vm/run")
		else
			pass=$((pass + 1))
		fi
		# cross: 5.1-target VM output must also run identically under luau
		if [[ "$d" == "5.1" ]]; then
			c3v="$(runvm "$LUAU" "$out")"; rc3v=$?
			c1lv="$(runvm "$LUAU" "$case")"; rc1lv=$?
			if [[ "$rc3v" != "$rc1lv" || "$c3v" != "$c1lv" ]]; then
				echo "FAIL [vm:cross] $base (5.1out on luau): exit $rc1lv vs $rc3v"
				diff <(echo "$c1lv") <(echo "$c3v") | head -6
				fail=$((fail + 1)); failed+=("$base/$d/vm/cross")
			else
				pass=$((pass + 1))
			fi
		fi
		# luau-compile syntax check for luau-target VM output
		if [[ "$d" == "luau" ]]; then
			if ! timeout 60 "$LUAUC" "$out" > /dev/null 2>"$TMP/luac_err.txt"; then
				echo "FAIL [vm:luac] $base: $(head -1 "$TMP/luac_err.txt")"
				fail=$((fail + 1)); failed+=("$base/$d/vm/luac")
			else
				pass=$((pass + 1))
			fi
		fi
	done
done

# ---------------------------------------------------------------
# ㊴: 激活门 (--bind-key) 已退役——bind_gate_test.sh 随之移除。
# ---------------------------------------------------------------

# ---------------------------------------------------------------
# 增量㉑ — environment binding (--bind-env): bound output only loads
# in the target runtime; generic sandbox dies at load. Verifies syntax
# passes + CLI load fails (binding works) + unbound stays runnable.
# ---------------------------------------------------------------
source "$ROOT/tests/env_bind_test.sh"

# ---------------------------------------------------------------
# 增量㊺ — 绑定折叠前移 + ef1 读前写/镜像错位根治: 忠实仿真正向等价
# （绑定态产物在忠实桩环境必须跑通且输出逐字一致）+ ef1 赋值早于消费
# 的顺序回归守卫 + hqi 解掩种子前移消费存在性。
# ---------------------------------------------------------------
source "$ROOT/tests/faithful_stub_test.sh"

# ---------------------------------------------------------------
# 增量㊾ (R021 回合 — 物化去钩化): 钩住 string.char 的钩子环境下程序
# 仍正确运行、且载荷既不被整串钩中也不被逐字符序列钩中（短/长载荷，
# 覆盖 MkStr 与常量池路径）。
# ---------------------------------------------------------------
source "$ROOT/tests/charhook_test.sh"

# ---------------------------------------------------------------
# 增量㉒ (选项B — B-2 碎片即用即毁): prototypes live ENCODED at rest;
# boot destroys the flat decoded table, per-call DDEC materializes one
# frame's bytecode into fresh tables. Verifies template shape (ENC/
# DDEC wiring, PF=nil, MPF entry, no flat-table refs), fragment count,
# nested-closure equivalence, constant round-trip, deep recursion.
# ---------------------------------------------------------------
source "$ROOT/tests/proto_enc_test.sh"

# R063: replace the module-wide setmetatable capture point without removing
# required internal metatables; retain hook arguments and inspect live tables.
bash "$ROOT/tests/r063_entry_table_hook_regression.sh"

# Protected metatables may make getmetatable(string/table) return a sentinel
# string. String methods and hidden __index/__newindex must still follow native
# Luau indexing semantics instead of indexing the protection sentinel.
bash "$ROOT/tests/protected_meta_compat_test.sh"

# R064 dynamic micro-trace mitigation: safe internal sequences are fused into
# one per-shard record, while both v15 and legacy VM semantics remain equal.
bash "$ROOT/tests/micro_fusion_test.sh"

# Phase 18 P0-3: pre-compilation resource-budget limits. Every calibrated
# dimension is exercised at its threshold (must compile cleanly) and at
# threshold+1 (must be rejected with a single-line `error:` diagnostic).
source "$ROOT/tests/robustness_test.sh"

# Core VM byte/sub/char primitives must not be fetched through a loader's
# replaceable global string table. Verify copied-library hooks miss the VM.
bash "$ROOT/tests/string_primitive_hook_test.sh"

# Phase 18 P1-1/P1-2: string-cache hardening harvest simulation. Proves that a
# constant with a single static reference never enters the persistent cache,
# that multi-use constants do (negative control), and that clear-after-last-use
# respects the loop / cross-frame scope boundaries. Needs the audit build.
if ! python3 "$ROOT/tests/string_cache_dump_test.py"; then
	echo "FAIL [string-cache-dump] string cache hardening harvest simulation"
	fail=$((fail + 1)); failed+=("p1/string-cache-dump")
fi

# Phase 18 P1-3: distributed runtime canaries. A guard build must trip the
# bounded fail-closed path when string.char is swapped mid-execution, the same
# payload must still run to completion under --no-guard, and canary placement
# must be seed-dependent (no fixed offset to fingerprint).
if ! python3 "$ROOT/tests/canary_runtime_test.py"; then
	echo "FAIL [canary-runtime] distributed string-cache canary runtime"
	fail=$((fail + 1)); failed+=("p1/canary-runtime")
fi

# Phase G (spec section 6/G4): API capability slots + staged closures.
# Entry closures survive being stored and called after their parent
# returned, coroutine resume continues execution, slot remapping across
# builds changes bytes but not behavior, and a host missing a required
# API fails deterministically.
if ! python3 "$ROOT/tests/phase_g_api_test.py"; then
	echo "FAIL [phase-g-api] API slots / staged closure runtime"
	fail=$((fail + 1)); failed+=("phase-g/api")
fi

# R064 nested upvalues crossed parent/child scatter permutations used to map
# a child capture to the wrong parent upvalue. Recheck the full handoff canary
# on historical failure seeds (plus passing neighbors).
if ! bash "$ROOT/tests/r064_seed_stability_test.sh"; then
	echo "FAIL [r064-seed-stability] nested upvalue scatter regression"
	fail=$((fail + 1)); failed+=("r064/seed-stability")
fi

# H3-6: dead-register noise must not write into open dynamic call-result
# windows (stress_multival seed corruption class, fixed 2026-10-11).
if ! bash "$ROOT/tests/call_window_noise_test.sh"; then
	echo "FAIL [call-window-noise] dead-register noise hit a call window"
	fail=$((fail + 1)); failed+=("h3-6/call-window-noise")
fi

# WTF-PORT Phase 1: --env-check battery (mock-environment kill matrix).
if ! bash "$ROOT/tests/wtf_port_test.sh"; then
	echo "FAIL [wtf-port] environment check battery regression"
	fail=$((fail + 1)); failed+=("wtf-port/env-check")
fi

# Phase 18 P3-1: engine/tests/attack_blackbox/*.py 是黑盒分析性测试，结果见
# HANDOFF §18，不计入本脚本的 PASS/FAIL 统计（按任务书要求保持独立）。

echo "=================================================="
echo "PASS checks: $pass   FAIL: $fail"
if [[ $fail -gt 0 ]]; then
	for f in "${failed[@]}"; do
		echo "  failed: $f"
	done
	exit 1
fi
echo "ALL GREEN"
