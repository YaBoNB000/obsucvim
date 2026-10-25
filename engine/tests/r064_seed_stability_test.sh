#!/usr/bin/env bash
# Regression for nested closure upvalues that alias the creating frame's
# permuted upvalue array. These seeds previously made the R064 canary call a
# numeric constant as a function (attempt to call a number value).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TOOL="$ROOT/engine/target/release/obsucvim"
SRC="$ROOT/artifacts/targets/R064_7kb_src.lua"
TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/luau" ] || [ ! -x "$TOOLS_BIN/luau-compile" ]; then
  TOOLS_BIN="$ROOT/.tools/bin"
fi
LUAU="$TOOLS_BIN/luau"
LUAUC="$TOOLS_BIN/luau-compile"
if [ ! -x "$TOOL" ] || [ ! -x "$LUAU" ] || [ ! -x "$LUAUC" ]; then
  echo "missing release binary or Luau toolchain" >&2
  exit 2
fi
# Historical failure seeds plus passing neighbors guard both regression and
# seed-to-seed mapping stability. Override with a whitespace-separated list.
SEEDS="${R064_SEEDS:-1 7 42 999 31337 4242 12345 65535 123456 1234567 999999 222222 555555 888888 10000000 12345678 64065 64067 64070 64073 64074 64075 64076 64077}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
EXPECTED="$TMP/expected.txt"
OUT="$TMP/target.lua"
ACTUAL="$TMP/actual.txt"
"$LUAU" "$SRC" > "$EXPECTED"
count=0
for seed in $SEEDS; do
  if ! "$TOOL" --preset v15 --dialect luau --no-bind-executor --seed "$seed" "$SRC" "$OUT" 2>"$TMP/build.err"; then
    echo "FAIL [build] R064 seed=$seed: $(head -1 "$TMP/build.err")" >&2
    exit 1
  fi
  if ! timeout 60 "$LUAUC" "$OUT" >/dev/null 2>"$TMP/compile.err"; then
    echo "FAIL [compile] R064 seed=$seed: $(head -1 "$TMP/compile.err")" >&2
    exit 1
  fi
  if ! timeout 120 "$LUAU" "$OUT" > "$ACTUAL" 2>"$TMP/run.err"; then
    echo "FAIL [run] R064 seed=$seed: $(head -1 "$TMP/run.err")" >&2
    exit 1
  fi
  if ! cmp -s "$EXPECTED" "$ACTUAL"; then
    echo "FAIL [stdout] R064 seed=$seed" >&2
    diff -u "$EXPECTED" "$ACTUAL" | head -20 >&2 || true
    exit 1
  fi
  count=$((count + 1))
done
echo "R064 nested-upvalue seed stability PASS ($count seeds)"
