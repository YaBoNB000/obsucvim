#!/usr/bin/env bash
# Semantic regressions for parenthesized calls, multiple results, long strings,
# protected metatables, numeric-string arithmetic, and Luau integer division.
# The v15 build check explicitly keeps executor binding enabled; it is compiled
# but not expected to execute in stock Luau.
set -euo pipefail
ENGINE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$ENGINE_ROOT/.." && pwd)"
TOOLS_BIN="$REPO_ROOT/.tools/bin"
LUAU="$TOOLS_BIN/luau"
LUAUC="$TOOLS_BIN/luau-compile"
SOURCE="$ENGINE_ROOT/tests/fixtures/targets/paren_vm_semantics.lua"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

if [[ -n "${OBSUCVIM_BIN:-}" ]]; then
    BIN="$OBSUCVIM_BIN"
else
    PATH="$TOOLS_BIN:$PATH" cargo build --manifest-path "$ENGINE_ROOT/Cargo.toml" --bin obsucvim --quiet
    BIN="$ENGINE_ROOT/target/debug/obsucvim"
fi
for tool in "$BIN" "$LUAU" "$LUAUC"; do
    [[ -x "$tool" ]] || { echo "missing executable: $tool" >&2; exit 2; }
done

"$LUAUC" "$SOURCE" >/dev/null
"$LUAU" "$SOURCE" >"$TMP/expected.out"

for preset in low medium high vm max; do
    out="$TMP/$preset.lua"
    "$BIN" --preset "$preset" --dialect luau --seed 84021 "$SOURCE" "$out"
    [[ "$(head -n 1 "$out")" == "-- This file was obfuscated by Obsucvim." ]]
    "$LUAUC" "$out" >/dev/null
    timeout 120 "$LUAU" "$out" >"$TMP/$preset.out"
    diff -u "$TMP/expected.out" "$TMP/$preset.out"
done

# Explicit --bind-executor is intentional: do not replace this with the old
# --no-bind-executor test path. Stock Luau must reject the bound v15 artifact.
"$BIN" --preset v15 --bind-executor --dialect luau --seed 84021 "$SOURCE" "$TMP/v15-bound.lua"
[[ "$(head -n 1 "$TMP/v15-bound.lua")" == "-- This file was obfuscated by Obsucvim." ]]
"$LUAUC" "$TMP/v15-bound.lua" >/dev/null
set +e
timeout 30 "$LUAU" "$TMP/v15-bound.lua" >"$TMP/v15.out" 2>&1
v15_status=$?
set -e
if grep -q 'VM-SEMANTICS-OK' "$TMP/v15.out"; then
    echo "FAIL: executor-bound v15 unexpectedly ran in stock Luau" >&2
    cat "$TMP/v15.out" >&2
    exit 1
fi
if [[ "$v15_status" -eq 124 ]]; then
    echo "FAIL: executor-bound v15 timed out in stock Luau" >&2
    exit 1
fi

echo "VM SEMANTICS REGRESSION PASS (v15 binding enabled; stock Luau rejected it)"
