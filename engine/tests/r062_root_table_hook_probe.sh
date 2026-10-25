#!/usr/bin/env bash
# Reproduce a transparent entry-table capture against the committed R062 sample.
# This is a red-team probe, not a passing security control: it demonstrates
# that a loader-controlled environment can observe the outer module table.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ART="$ROOT/artifacts/targets/R062_target.luau.lua"
LUAU="${LUAU:-$ROOT/.tools/bin/luau}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

python3 - "$ART" "$TMP/probe.luau" <<'PY'
from pathlib import Path
import sys
src = Path(sys.argv[1]).read_text()
level = 0
while "]" + "=" * level + "]" in src:
    level += 1
open_delim = "[" + "=" * level + "["
close_delim = "]" + "=" * level + "]"
body = r'''
local original = setmetatable
local captured
local env = {}
for k, v in pairs(_G) do env[k] = v end
env._G = env
env.setmetatable = function(t, mt)
    if captured == nil and type(t) == "table" then
        local n, numeric, functions, strings = 0, 0, 0, 0
        for k, v in pairs(t) do
            n = n + 1
            if type(k) == "number" then numeric = numeric + 1 end
            if type(v) == "function" then functions = functions + 1 end
            if type(v) == "string" then strings = strings + 1 end
        end
        captured = { n = n, numeric = numeric, functions = functions, strings = strings }
    end
    return original(t, mt)
end
local chunk = assert(loadstring(ART, "R062 target"))
setfenv(chunk, env)
local ok, result = pcall(chunk)
assert(ok, result)
assert(captured ~= nil, "entry table was not observed")
assert(captured.n >= 100 and captured.numeric >= 50 and captured.functions >= 50,
       "captured table shape unexpectedly small")
print("ROOT_TABLE_CAPTURE", captured.n, captured.numeric, captured.functions,
      captured.strings, result)
'''
wrapped = "local ART = " + open_delim + "\n" + src + "\n" + close_delim + "\n" + body
Path(sys.argv[2]).write_text(wrapped)
PY

out="$("$LUAU" "$TMP/probe.luau")"
echo "$out"
grep -q '^ROOT_TABLE_CAPTURE' <<<"$out"
