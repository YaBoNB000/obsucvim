#!/usr/bin/env bash
# R063 regression: the complete v15 module table must not be passed to the
# loader-controlled global setmetatable. Required internal setmetatable calls
# remain allowed; the probe retains every table argument and inspects it after
# execution so lazy tables cannot evade the check by being empty at call time.
set -euo pipefail
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="$REPO/engine/target/release/obsucvim"
LUAU="${LUAU:-$REPO/.tools/bin/luau}"
LUAC="${LUAU_COMPILE:-$REPO/.tools/bin/luau-compile}"
SRC="$REPO/engine/tests/fixtures/targets/R062_src.lua"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

"$BIN" --preset v15 --seed 63063 "$SRC" "$TMP/target.lua"
"$LUAC" "$TMP/target.lua" >/dev/null
python3 "$REPO/engine/tests/v15_short_name_audit.py" "$TMP/target.lua"

# White-box static audit: KeyEmitter must have no public lookup tables, and
# every emitted key reconstruction must depend on actual carrier lengths.
OBSUCVIM_VM_TSRC=1 OBSUCVIM_KEY_MANIFEST="$TMP/keys.txt" \
    "$BIN" --preset v15 --seed 63063 "$SRC" "$TMP/static-audit.lua"
python3 "$REPO/engine/tests/key_literal_check.py" \
    "$TMP/static-audit.lua" "$TMP/keys.txt" --quiet
python3 - "$TMP/vm_tsrc.lua" <<'PY'
from pathlib import Path
import re
import sys
# The emitter writes a fixed diagnostic path; copy immediately for the check.
src = Path('/tmp/vm_tsrc.lua').read_text()
Path(sys.argv[1]).write_text(src)
for name in ('KA', 'KB', 'KC', 'KM'):
    # Match a complete Lua identifier. A raw substring check for `KM[` also
    # matched the suffix of the unrelated, live `CKM[` carrier-key array.
    assert not re.search(rf'(?<![A-Za-z0-9_]){name}\s*\[', src), \
        f'old key-table lookup remains: {name}'
    assert not re.search(rf'\blocal\s+{name}\s*=\s*\{{', src), \
        f'old key table remains: {name}'
assert 'for i=1,#d do s=(s*' in src and '#d[i]' in src, 'carrier-length digest loop missing'
assert 'local by=string.byte for i=1,#d do' not in src, 'key derivation added a byte-hook sink'
contexts = re.findall(r'end\)\(([^)]*)\)', src)
contexts = [ctx for ctx in contexts if ctx]
assert contexts.count('FN') >= 40, 'VM key sites are not tied to the carrier-length table'
assert any(ctx != 'FN' for ctx in contexts), 'sanitized HBoot context is missing'
print('R063_KEY_CONTEXT_AUDIT', src.count('for i=1,#d do s=(s*'), sorted(set(contexts)))
PY

# The former build-time debug environment variable must not emit raw keys or
# otherwise alter a fixed-seed artifact.
OBSUCVIM_KEY_DBG=1 "$BIN" --preset v15 --seed 63063 "$SRC" "$TMP/key-debug-env.lua"
cmp -s "$TMP/target.lua" "$TMP/key-debug-env.lua" || {
    echo 'FAIL OBSUCVIM_KEY_DBG changed the emitted artifact' >&2
    exit 1
}

python3 - "$TMP/target.lua" "$TMP/probe.luau" <<'PY'
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
local observed = {}
local env = {}
for k, v in pairs(_G) do env[k] = v end
env._G = env
env.setmetatable = function(t, mt)
    if type(t) == "table" then
        observed[#observed + 1] = t -- retain the live object, not a snapshot
    end
    return original(t, mt)
end
local chunk = assert(loadstring(ART, "R063 target"))
setfenv(chunk, env)
local ok, result = pcall(chunk)
assert(ok, result)
assert(result == 1713, "R063 source/target result mismatch")

local max_fields, max_functions, root_like = 0, 0, 0
for _, t in ipairs(observed) do
    local n, functions = 0, 0
    for _, v in pairs(t) do
        n = n + 1
        if type(v) == "function" then functions = functions + 1 end
    end
    if n > max_fields then max_fields = n end
    if functions > max_functions then max_functions = functions end
    if n >= 100 and functions >= 50 then root_like = root_like + 1 end
end
assert(root_like == 0,
       "a module-sized function table reached setmetatable: " .. root_like)
print("R063_ENTRY_TABLE_HOOK", #observed, max_fields, max_functions, result)
'''
wrapped = "local ART = " + open_delim + "\n" + src + "\n" + close_delim + "\n" + body
Path(sys.argv[2]).write_text(wrapped)
PY

out="$("$LUAU" "$TMP/probe.luau")"
echo "$out"
grep -q '^R063_ENTRY_TABLE_HOOK' <<<"$out"
