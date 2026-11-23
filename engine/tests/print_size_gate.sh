#!/usr/bin/env bash
# Phase 17 §7 Sole Size Gate: `print(6)` default-v15 artifact <= 250,000 bytes.
#
# Spec §0.1/§7: input is exactly `print(6)\n`; command is production release
# binary with `--preset v15 --dialect luau --seed <seed>` (default Guard ON,
# no --no-guard); seeds are exactly {1, 7, 42, 4242, 31337, 999999}; EVERY
# seed's full output file must be <= 250000 bytes (max is the gate result).
# Each output records size + SHA-256; the script also records the binary
# SHA-256 and source commit SHA. Watermark presence, `luau-compile` parse,
# and stock Luau execution (`6`, exit 0) are verified per seed.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_TOOLS="$(cd "$ROOT/.." && pwd)/.tools/bin"
export PATH="$REPO_TOOLS:$HOME/.cargo/bin:$PATH"

BIN="${OBSUCVIM_BIN:-$ROOT/target/release/obsucvim}"
LUAU="${LUAU_BIN:-$(command -v luau || echo "$REPO_TOOLS/luau")}"
LUAU_COMPILE="${LUAU_COMPILE_BIN:-$(command -v luau-compile || echo "$REPO_TOOLS/luau-compile")}"
MAX_BYTES="${PRINT_SIZE_CAP_BYTES:-250000}"

if [ ! -x "$BIN" ]; then
	echo "FAIL: missing release binary at $BIN" >&2
	exit 2
fi

BIN_SHA="$(sha256sum "$BIN" | awk '{print $1}')"
SRC_SHA="$(git -C "$ROOT/.." rev-parse HEAD 2>/dev/null || echo unknown)"
DIRTY="$(git -C "$ROOT/.." status --porcelain 2>/dev/null | head -n 1)"
if [ -n "$DIRTY" ]; then SRC_SHA="${SRC_SHA}+dirty"; fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
printf 'print(6)\n' > "$TMP/p6.lua"

SEEDS=(1 7 42 4242 31337 999999)
max_seen=0
min_seen=999999999

echo "print_size_gate: bin_sha256=$BIN_SHA src=$SRC_SHA cap=${MAX_BYTES}B"
for seed in "${SEEDS[@]}"; do
	out="$TMP/p6_s${seed}.lua"
	"$BIN" --preset v15 --dialect luau --seed "$seed" "$TMP/p6.lua" "$out"
	sz=$(wc -c < "$out" | tr -d ' ')
	sha=$(sha256sum "$out" | awk '{print $1}')
	if [ "$sz" -gt "$max_seen" ]; then max_seen="$sz"; fi
	if [ "$sz" -lt "$min_seen" ]; then min_seen="$sz"; fi
	if [ "$sz" -gt "$MAX_BYTES" ]; then
		echo "FAIL [print_size_gate]: seed=$seed size=${sz}B exceeds cap ${MAX_BYTES}B sha256=$sha" >&2
		exit 1
	fi
	if ! grep -q "obfuscated by Obsucvim" "$out"; then
		echo "FAIL [print_size_gate]: seed=$seed missing watermark" >&2
		exit 1
	fi
	if ! "$LUAU_COMPILE" "$out" > /dev/null 2>&1; then
		echo "FAIL [print_size_gate]: seed=$seed luau-compile parse failed" >&2
		exit 1
	fi
	got="$(LUAU_READONLY_GLOBALS=1 "$LUAU" "$out")"
	if [ "$got" != "6" ]; then
		echo "FAIL [print_size_gate]: seed=$seed expected '6', got '$got'" >&2
		exit 1
	fi
	echo "  seed=$seed size=${sz}B sha256=$sha OK"
done

echo "PRINT SIZE GATE PASS: 6/6 seeds verified (min=${min_seen}B, max=${max_seen}B <= ${MAX_BYTES}B cap; headroom=$((MAX_BYTES - max_seen))B)"
