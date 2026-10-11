#!/usr/bin/env bash
# v15 core primitives must not be fetched through a loader-replaceable
# global `string` library table. Check both a minimal canary and the 7KB R064
# program under a setfenv environment that transparently wraps byte/sub/char.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="$(cd "$ROOT/.." && pwd)"
TOOL="$ROOT/target/release/obsucvim"
TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/luau" ] || [ ! -x "$TOOLS_BIN/luau-compile" ]; then
  TOOLS_BIN="$REPO/.tools/bin"
fi
LUAU="$TOOLS_BIN/luau"
LUAUC="$TOOLS_BIN/luau-compile"
if [ ! -x "$TOOL" ] || [ ! -x "$LUAU" ] || [ ! -x "$LUAUC" ]; then
  echo "missing release binary or Luau toolchain" >&2
  exit 2
fi
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cat > "$TMP/basic.lua" <<'LUA'
local canary = "native primitive hook check"
print(canary)
LUA

cat > "$TMP/make_hook.py" <<'PY'
from pathlib import Path
import sys
src = Path(sys.argv[1]).read_text()
level = 0
while "]" + ("=" * level) + "]" in src:
    level += 1
open_delim = "[" + "=" * level + "["
close_delim = "]" + "=" * level + "]"
body = r'''
local original = {byte=string.byte, sub=string.sub, char=string.char}
local hits = {byte=0, sub=0, char=0}
local copied = {}
for k, v in pairs(string) do copied[k] = v end
for k, fn in pairs(original) do
    copied[k] = function(...)
        hits[k] = hits[k] + 1
        return fn(...)
    end
end
local env = {}
for k, v in pairs(_G) do env[k] = v end
env._G = env
env.string = copied
local chunk = assert(loadstring(ART, "R064 string primitive hook"))
setfenv(chunk, env)
local ok, err = pcall(chunk)
assert(ok, err)
print("STRING_TABLE_HOOKS", hits.byte, hits.sub, hits.char)
'''
wrapped = "local ART = " + open_delim + "\n" + src + "\n" + close_delim + "\n" + body
Path(sys.argv[2]).write_text(wrapped)
PY

check_case() {
  local label="$1" seed="$2" src="$3"
  local out="$TMP/$label.lua" wrapper="$TMP/$label-hook.lua"
  local expected="$TMP/$label.expected" actual="$TMP/$label.actual"
  timeout 120 "$LUAU" "$src" > "$expected"
  "$TOOL" --preset v15 --dialect luau --no-bind-executor --seed "$seed" "$src" "$out"
  timeout 60 "$LUAUC" "$out" >/dev/null
  python3 "$TMP/make_hook.py" "$out" "$wrapper"
  timeout 120 "$LUAU" "$wrapper" > "$actual"
  python3 - "$expected" "$actual" "$label" <<'PY'
from pathlib import Path
import sys
expected = Path(sys.argv[1]).read_text()
lines = Path(sys.argv[2]).read_text().splitlines()
marker = [line for line in lines if line.startswith("STRING_TABLE_HOOKS\t")]
actual = "\n".join(line for line in lines if not line.startswith("STRING_TABLE_HOOKS\t")) + "\n"
if actual != expected:
    raise SystemExit(f"FAIL [{sys.argv[3]}] output changed under string-table hooks")
if len(marker) != 1:
    raise SystemExit(f"FAIL [{sys.argv[3]}] hook marker missing/duplicated: {marker}")
parts = marker[0].split("\t")
if parts[1:] != ["0", "0", "0"]:
    raise SystemExit(f"FAIL [{sys.argv[3]}] copied string table observed VM calls: {marker[0]}")
print(f"string-table hook bypass PASS [{sys.argv[3]}]: byte/sub/char=0/0/0")
PY
}

check_case minimal 64080 "$TMP/basic.lua"
check_case R064 64075 "$REPO/handoff/R064_7kb_src.lua"
