#!/usr/bin/env bash
# Regression: v15 must preserve native indexing/method lookup when the host
# protects string/table metatables behind a non-table __metatable sentinel.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOL="$ROOT/target/release/obsucvim"
TOOLS_BIN=/home/user/tools/bin
if [ ! -x "$TOOLS_BIN/luau" ]; then
    TOOLS_BIN="$ROOT/../.tools/bin"
fi
LUAU="$TOOLS_BIN/luau"
LUAC="$TOOLS_BIN/luau-compile"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cat > "$TMP/source.lua" <<'LUA'
local text = "ExecutorMetaCheck"
local meta
meta = {
    __index = function(_, key)
        if key == "answer" then return "table-meta-ok" end
    end,
    __newindex = function(t, key, value)
        rawset(t, key, "written:" .. tostring(value))
    end,
    __add = function(a, b)
        return setmetatable({value = a.value + b.value}, meta)
    end,
    __concat = function(a, b)
        return tostring(a.value) .. ":" .. tostring(b.value)
    end,
    __lt = function(a, b) return a.value < b.value end,
    __eq = function(a, b) return a.value == b.value end,
    __unm = function(a) return -a.value end,
    __len = function(a) return a.value end,
    __metatable = "locked-table",
}
local left = setmetatable({value = 7}, meta)
local right = setmetatable({value = 5}, meta)
local equal = left == setmetatable({value = 7}, meta)
local callable = setmetatable({value = 11}, {
    __call = function(self, n) return self.value + n end,
    __metatable = "locked-call",
})
left.extra = 3
local raw = "ExecutorMetaCheck"
return raw:sub(2, 5), raw:upper(), raw:rep(2), left.answer,
       left.extra, (left + right).value, left < right, equal,
       left .. right, -right, #left, callable(4)
LUA

"$TOOL" --preset v15 --dialect luau --no-bind-executor --seed 845064 \
    "$TMP/source.lua" "$TMP/target.lua"
"$LUAC" "$TMP/target.lua" >/dev/null

python3 - "$TMP/source.lua" "$TMP/target.lua" "$TMP/probe.luau" <<'PY'
from pathlib import Path
import sys

def long_literal(text: str) -> str:
    level = 0
    while "]" + "=" * level + "]" in text:
        level += 1
    return "[" + "=" * level + "[\n" + text + "\n]" + "=" * level + "]"

src = Path(sys.argv[1]).read_text()
obf = Path(sys.argv[2]).read_text()
harness = r'''
local native_getmetatable = getmetatable
local function run(code)
    local env = {}
    for k, v in pairs(_G) do env[k] = v end
    env._G = env
    env.getmetatable = function(v)
        if type(v) == "string" then return "locked-string" end
        local mt = native_getmetatable(v)
        if type(mt) == "table" and rawget(mt, "__metatable") ~= nil then
            return rawget(mt, "__metatable")
        end
        return mt
    end
    local f = assert(loadstring(code, "protected-meta regression"))
    setfenv(f, env)
    return pcall(f)
end
local source = table.pack(run(SRC))
local target = table.pack(run(OBF))
assert(source[1] and target[1], "source/target error: " .. tostring(target[2]))
assert(source.n == target.n, "return arity mismatch")
for i = 2, source.n do
    assert(source[i] == target[i], "return mismatch at " .. i .. ": " .. tostring(source[i]) .. " vs " .. tostring(target[i]))
end
print("PROTECTED_META_COMPAT_PASS", table.unpack(target, 2, target.n))
'''
wrapped = "local SRC = " + long_literal(src) + "\nlocal OBF = " + long_literal(obf) + "\n" + harness
Path(sys.argv[3]).write_text(wrapped)
PY

out="$("$LUAU" "$TMP/probe.luau")"
echo "$out"
grep -q '^PROTECTED_META_COMPAT_PASS' <<<"$out"
