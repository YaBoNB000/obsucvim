#!/bin/bash
# WTF-PORT Phase 2 regression: tier-2 Roblox/executor check battery
# (--env-check + --bind-executor).
#
# Artifact level: tier-2 builds compile, are deterministic, and every
# pre-existing flag combination stays byte-identical to Phase 1 (tier-2
# emission fires ONLY when both flags are on).
# Fragment level: the ENVFRAG factory is extracted from a tier-2 build and
# run against the faithful Roblox mock (wtf_port_roblox_mock.luau):
#   - positive (no break): the rolling difference must be EXACTLY 0
#   - one BREAK hook per tier-2 check group: difference must be non-zero
# Tier-1 checks also run inside the same factory against the mock env
# (real CLI libraries), so the positive path exercises both tiers.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$ROOT/target/release/obsucvim"
TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/luau" ]; then
	TOOLS_BIN="$(cd "$(dirname "$0")/../../.tools/bin" && pwd)"
fi
LUAU="$TOOLS_BIN/luau"
if [ ! -x "$TOOL" ]; then
	echo "error: missing $TOOL" >&2; exit 2
fi
if [ ! -x "$LUAU" ]; then
	echo "error: missing $LUAU" >&2; exit 2
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
pass=0; fail=0
ok() { pass=$((pass + 1)); }
bad() { fail=$((fail + 1)); echo "FAIL $1"; }

CANARY="$TMP/canary.lua"
printf 'print("T2", 6*7)\n' > "$CANARY"

# --- 1. artifact level ------------------------------------------------------
if ! "$TOOL" --bind-executor --env-check --seed 42 "$CANARY" "$TMP/t2a.lua" 2>"$TMP/err"; then
	bad "tier2 build: $(head -1 "$TMP/err")"
else
	ok
fi
"$TOOL" --bind-executor --env-check --seed 42 "$CANARY" "$TMP/t2b.lua"
cmp -s "$TMP/t2a.lua" "$TMP/t2b.lua" && ok || bad "tier2 determinism"
"$LUAU" "$TOOLS_BIN/../../../engine/tools/../../" >/dev/null 2>&1 # no-op
if "$TOOLS_BIN/luau-compile" "$TMP/t2a.lua" > /dev/null 2>&1 || luau-compile "$TMP/t2a.lua" >/dev/null 2>&1; then
	ok
else
	bad "tier2 syntax (luau-compile)"
fi
out="$(timeout 30 "$LUAU" "$TMP/t2a.lua" 2>&1)"
if [[ "$out" == *"T2"* ]]; then bad "tier2 CLI must die silently"; else ok; fi

# pre-existing flag combos must stay byte-identical to the pinned snapshots
# (canary = print("T2", 6*7)). These are shape locks; behavioral checks below
# remain the semantic authority.
# 2026-11-21 重钉：ChaCha20 RNG、按需 VM reader 和守卫字面量拆分改变了固定 seed 输出，
# 旧 PRNG/reader 时代快照已过时，更新为当前构建的确定性形态锚。
EC_SHA=74a5dc23e81f51b63b6bb1beb5b74e6764c2a432d08a5d66aaa59a7923844634
BIND_SHA=3ea684955f3dd6add30881a15bc5b4743b97b464eae5e00c79abc5575a20227a
# 2026-10-11 起绑定默认开启：tier-1-only / 关闭态的对照锚均需显式退绑；
# tier-2 双开(现在的 --env-check 默认形态)与显式绑定保持 Phase-1/2 快照。
"$TOOL" --env-check --no-bind-executor --seed 42 "$CANARY" "$TMP/ec_only.lua"
[ "$(sha256sum "$TMP/ec_only.lua" | cut -d' ' -f1)" == "$EC_SHA" ] && ok || bad "env-only(tier-1) parity vs Phase-1 snapshot"
"$TOOL" --bind-executor --seed 42 "$CANARY" "$TMP/bind_only.lua"
[ "$(sha256sum "$TMP/bind_only.lua" | cut -d' ' -f1)" == "$BIND_SHA" ] && ok || bad "bind-only parity vs Phase-1 snapshot"
"$TOOL" --no-bind-executor --seed 42 "$ROOT/tests/cases/basics.lua" "$TMP/off.lua" 2>/dev/null
if cmp -s "$TMP/off.lua" "$ROOT/examples/basics.v15.luau.lua"; then ok; else bad "flag-off parity"; fi
# 默认构建现在即绑定态：== 显式 --bind-executor，且 --env-check 默认形态
# (tier-1+tier-2+守卫) == 双开显式形态
"$TOOL" --seed 42 "$CANARY" "$TMP/def_only.lua"
cmp -s "$TMP/def_only.lua" "$TMP/bind_only.lua" && ok || bad "default build == explicit bind (new base layer)"
"$TOOL" --env-check --seed 42 "$CANARY" "$TMP/ec_default.lua"
cmp -s "$TMP/ec_default.lua" "$TMP/t2a.lua" && ok || bad "env-check default form == both-flags form"

# --- 2. fragment level (mock matrix) ----------------------------------------
# Extract the ENVFRAG factory from the tier-2 build's template dump.
rm -f /tmp/vm_tsrc.lua
OBSUCVIM_VM_TSRC=1 "$TOOL" --bind-executor --env-check --seed 42 "$CANARY" "$TMP/t2c.lua" 2>/dev/null
if [ ! -f /tmp/vm_tsrc.lua ]; then
	bad "tsrc dump missing"
else
	ok
fi

run_frag() { # $1 = BREAK mode, echoes EVD output
	local mode="$1"
	python3 - "$mode" "$ROOT/tests/wtf_port_roblox_mock.luau" /tmp/vm_tsrc.lua "$TMP/frag_$1.luau" <<'PYGEN'
import sys
from pathlib import Path
mode, mock_path, tsrc_path, out_path = sys.argv[1:5]
mock = Path(mock_path).read_text().replace("@BREAK@", mode)
tsrc = Path(tsrc_path).read_text()
line = [l for l in tsrc.splitlines() if "ENVFRAG=function" in l][0]
idx = line.index("ENVFRAG=function")
end = line.rindex(" do") if line.rstrip().endswith(" do") else len(line)
seg = line[idx + len("ENVFRAG="):end]
def long_string(text):
    for level in range(1, 64):
        eq = "=" * level
        if "]" + eq + "]" not in text:
            return f"[{eq}[\n{text}]{eq}]"
    raise ValueError("delimiter")
tail = f"""
local FRAGSRC = {long_string(seg)}
local loader = loadstring("return " .. FRAGSRC, "envfrag")
setfenv(loader, ENV)
local ENVFRAG = loader()
print("EVD", ENVFRAG())
"""
Path(out_path).write_text(mock + tail)
PYGEN
	timeout 30 "$LUAU" "$TMP/frag_$1.luau" 2>&1
}

# positive: unbroken mock must yield exactly 0
pos="$(run_frag none)"
if [[ "$pos" == *"EVD	0"* ]] || [[ "$pos" == *"EVD 0"* ]]; then
	ok
else
	bad "positive: EVD != 0 under faithful mock: $pos"
fi

# negatives: each break hook must push the difference away from zero
for mode in no-game enum-interning random-value random-clone task-spawn dotcall signal destroy-lock identity-swap; do
	neg="$(run_frag "$mode")"
	if [[ "$neg" == *"EVD	0"* ]] || [[ "$neg" == *"EVD 0"* ]]; then
		bad "mock $mode: difference stayed 0 (group not wired)"
	else
		ok
	fi
done

echo "wtf-port tier-2 regression: $pass PASS, $fail FAIL"
[ "$fail" -eq 0 ]
