#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="$ROOT/luraph-rs/target/release/luraph-rs"
LUAU="${LUAU:-$ROOT/.tools/bin/luau}"
LUAC="${LUAU_COMPILE:-$ROOT/.tools/bin/luau-compile}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cat > "$TMP/input.lua" <<'LUA'
local short = "suda"
local marker = "r062-seal-canary-7f31"
local unicode = "脚本"
local binary = "A\0B"
local values = { [short] = marker, ["script-key"] = unicode }
local function read_twice()
    local x, y = values[short], values["script-key"]
    return x .. x, y, binary
end
local repeated, text, bytes = read_twice()
print(values[short], text, #repeated, #bytes, bytes:byte(2))
LUA

expected=$'r062-seal-canary-7f31\t脚本\t42\t3\t0'
for profile in legacy v15; do
    out="$TMP/$profile.lua"
    if [[ "$profile" == legacy ]]; then
        "$BIN" --vm --vm-seal-strings --seed 620062 "$TMP/input.lua" "$out"
    else
        "$BIN" --preset v15 --vm-seal-strings --seed 620062 "$TMP/input.lua" "$out"
    fi
    "$LUAC" "$out" >/dev/null
    actual="$("$LUAU" "$out")"
    [[ "$actual" == "$expected" ]] || {
        printf 'FAIL %s semantic mismatch\nexpected: %q\nactual:   %q\n' "$profile" "$expected" "$actual" >&2
        exit 1
    }
    if grep -Fq 'r062-seal-canary-7f31' "$out" || grep -Fq 'suda' "$out"; then
        echo "FAIL $profile plaintext source literal found in artifact" >&2
        exit 1
    fi
done

# Verify the experimental flag cannot silently fall back to a non-VM pipeline.
if "$BIN" --preset high --vm-seal-strings "$TMP/input.lua" "$TMP/no-vm.lua" >/dev/null 2>&1; then
    echo 'FAIL --vm-seal-strings accepted without VM' >&2
    exit 1
fi

echo 'R062 string-seal experiment: PASS (legacy/v15 semantic parity, canary absent)'
