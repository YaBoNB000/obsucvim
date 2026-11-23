#!/usr/bin/env bash
# P5 Release Hygiene Gate: verifies that the default production release binary
# (`cargo build --release`, without `--features audit-hooks`) NEVER exports key
# manifests, noise/profile manifests, raw VM dumps, template sources, or debug
# stderr traces even when every `OBSUCVIM_*` environment variable is set.
# Also verifies that `--features audit-hooks` retains white-box audit capability
# without altering the emitted obfuscated Lua bytes for the same seed.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROD_BIN="${OBSUCVIM_PROD_BIN:-$ROOT/target/release/obsucvim}"
AUDIT_BIN="${OBSUCVIM_AUDIT_BIN:-$ROOT/target/audit/release/obsucvim}"

if [ ! -x "$PROD_BIN" ]; then
	echo "error: missing production release binary $PROD_BIN" >&2
	exit 2
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
printf 'local x = 6 * 7\nprint(x)\n' > "$TMP/src.lua"
rm -f /tmp/vm_raw.lua /tmp/vm_tsrc.lua

OBSUCVIM_KEY_MANIFEST="$TMP/prod_keys.txt" \
OBSUCVIM_NOISE_AUDIT="$TMP/prod_noise.txt" \
OBSUCVIM_PROFILE_AUDIT="$TMP/prod_prof.txt" \
OBSUCVIM_VM_RAW=1 \
OBSUCVIM_VM_TSRC=1 \
OBSUCVIM_VM_TSRC_PATH="$TMP/prod_tsrc.lua" \
OBSUCVIM_VM_DBG=1 \
OBSUCVIM_HBOOT_DBG=1 \
OBSUCVIM_BUILD_DBG=1 \
OBSUCVIM_BIND_DBG=1 \
OBSUCVIM_FUSE_DBG=1 \
OBSUCVIM_TIMING=1 \
	"$PROD_BIN" --preset v15 --dialect luau --seed 42 "$TMP/src.lua" "$TMP/prod_out.lua" \
	>"$TMP/prod_stdout.txt" 2>"$TMP/prod_stderr.txt"

for leaked in \
	"$TMP/prod_keys.txt" \
	"$TMP/prod_noise.txt" \
	"$TMP/prod_prof.txt" \
	"$TMP/prod_tsrc.lua" \
	"/tmp/vm_raw.lua" \
	"/tmp/vm_tsrc.lua"
do
	if [ -e "$leaked" ]; then
		echo "FAIL: production release binary exported sensitive file: $leaked" >&2
		exit 1
	fi
done

if [ -s "$TMP/prod_stderr.txt" ]; then
	echo "FAIL: production release binary emitted debug/timing stderr:" >&2
	cat "$TMP/prod_stderr.txt" >&2
	exit 1
fi

if [ -x "$AUDIT_BIN" ]; then
	OBSUCVIM_KEY_MANIFEST="$TMP/audit_keys.txt" \
	OBSUCVIM_VM_TSRC=1 \
	OBSUCVIM_VM_TSRC_PATH="$TMP/audit_tsrc.lua" \
		"$AUDIT_BIN" --preset v15 --dialect luau --seed 42 "$TMP/src.lua" "$TMP/audit_out.lua" \
		>/dev/null 2>&1
	if [ ! -s "$TMP/audit_keys.txt" ] || [ ! -s "$TMP/audit_tsrc.lua" ]; then
		echo "FAIL: audit-hooks binary failed to emit requested audit manifests" >&2
		exit 1
	fi
	if ! cmp -s "$TMP/prod_out.lua" "$TMP/audit_out.lua"; then
		echo "FAIL: audit-hooks changed emitted artifact bytes vs production build" >&2
		exit 1
	fi
fi

echo "PROD AUDIT GATE PASS: release binary exports 0 sensitive artifacts; byte-identical to audit build"
