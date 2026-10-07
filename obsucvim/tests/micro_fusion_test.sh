#!/usr/bin/env bash
# Exercise the safe micro-shard superoperators in both the v15 and legacy VM
# emitters. The source intentionally uses constants/moves, new tables, `not`,
# captured upvalues, branches, and explicit nil varargs.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$ROOT/target/release/obsucvim"
TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/luau" ] || [ ! -x "$TOOLS_BIN/luau-compile" ]; then
  TOOLS_BIN="$(cd "$(dirname "$0")/../../.tools/bin" && pwd)"
fi
LUAU="$TOOLS_BIN/luau"
LUAUC="$TOOLS_BIN/luau-compile"
if [ ! -x "$TOOL" ] || [ ! -x "$LUAU" ] || [ ! -x "$LUAUC" ]; then
  echo "missing release binary or Luau toolchain" >&2
  exit 2
fi
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
SRC="$TMP/micro_fusion.lua"
cat > "$SRC" <<'LUA'
local function factory(seed)
  local state = seed
  return function(delta)
    state = state + delta
    return state
  end
end

local function collect(...)
  local n = select("#", ...)
  local values = {...}
  if n > 2 then
    return n, values[1], values[2], values[3]
  end
  return n, nil, nil, nil
end

local bump = factory(4)
local first = bump(2)
local second = bump(3)
local invert = not (first == 6)
local values = {first, second}
local count, a, b, c = collect(nil, "tail", false, 7, nil)
local branch
if invert then
  branch = "true"
else
  branch = "false"
end
print(first, second, values[1], values[2], invert, branch, count, a, b, c)
LUA

expected="$(timeout 120 "$LUAU" "$SRC" 2>&1)"
for profile in v15 legacy; do
  OUT="$TMP/$profile.lua"
  if [ "$profile" = v15 ]; then
    "$TOOL" --preset v15 --dialect luau --seed 64067 "$SRC" "$OUT"
  else
    "$TOOL" --vm --dialect luau --seed 64067 "$SRC" "$OUT"
  fi
  timeout 60 "$LUAUC" "$OUT" >/dev/null
  actual="$(timeout 120 "$LUAU" "$OUT" 2>&1)"
  if [ "$actual" != "$expected" ]; then
    echo "FAIL [$profile] micro-fusion semantics diverged" >&2
    printf 'expected: %s\nactual:   %s\n' "$expected" "$actual" >&2
    exit 1
  fi
done

echo "micro-fusion v15/legacy semantics PASS"
