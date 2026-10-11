#!/usr/bin/env python3
"""Generate stock-runtime dynamic-attack harnesses against the UNMODIFIED
R065 target. Methods replicated from public deobfuscator workflows:
load/loadstring hooking, environment recording (envlog/sandboxed simulation),
frozen-clock timing-guard patch, replaceable string-table hook.

No third-party code is executed; the target bytes are embedded verbatim.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
TARGET = ROOT / 'handoff/R065_target.luau.lua'
LUAU = ROOT / '.tools/bin/luau'


def long_string(text: str) -> str:
    for level in range(1, 64):
        eq = '=' * level
        if ']' + eq + ']' not in text:
            return f'[{eq}[\n{text}]{eq}]'
    raise ValueError('no collision-free long-string delimiter')


TEMPLATE = '''-- GENERATED research harness; R065 target embedded verbatim.
local REAL_PRINT = print
local TARGET = {target}
local variant = "{variant}"

local stats = {{
  loads_from_inside = 0, env_calls = 0, bysuda_seen = 0,
  string_hook_hits = {{byte = 0, sub = 0, char = 0}},
  global_reads = {{}}, completed = false, returned = nil,
}}

local function scan(value)
  if type(value) == "string" and string.find(value, "bysuda", 1, true) then
    stats.bysuda_seen += 1
  end
end

local BASE = getfenv(0)
local ENV = {{}}
for k, v in pairs(BASE) do ENV[k] = v end
ENV._G = ENV

local function wrap(name, fn)
  return function(...)
    stats.env_calls += 1
    stats.global_reads[name] = (stats.global_reads[name] or 0) + 1
    local n = select("#", ...)
    for i = 1, n do scan(select(i, ...)) end
    local results = table.pack(fn(...))
    for i = 1, results.n do scan(results[i]) end
    return table.unpack(results, 1, results.n)
  end
end

-- library recording (envlog method): wrap function values in table/string/
-- math/bit32/os/coroutine, except loadstring/load per variant policy
for _, libname in {{"table", "math", "bit32", "os", "coroutine", "utf8"}} do
  local lib = ENV[libname]
  if type(lib) == "table" then
    local copy = {{}}
    for k, v in pairs(lib) do copy[k] = v end
    for k, v in pairs(copy) do
      if type(v) == "function" then
        copy[k] = wrap(libname .. "." .. k, v)
      end
    end
    ENV[libname] = copy
  end
end

if variant == "plain_env" then
  -- control: cloned environment, NOTHING wrapped. If this hangs, the clone
  -- itself breaks the target and every other variant's result is void.
end

if variant == "load_hook" then
  -- classic unpacker step 1: capture plaintext handed to load/loadstring
  ENV.loadstring = wrap("loadstring", BASE.loadstring)
  ENV.load = wrap("load", BASE.load)
end

if variant == "string_hook" then
  -- replaceable string-table hook (copied table, wrapped byte/sub/char)
  local copy = {{}}
  for k, v in pairs(BASE.string) do copy[k] = v end
  copy.byte = function(...)
    stats.string_hook_hits.byte += 1
    scan((...))
    return BASE.string.byte(...)
  end
  copy.sub = function(...)
    stats.string_hook_hits.sub += 1
    scan((...))
    return BASE.string.sub(...)
  end
  copy.char = function(...)
    stats.string_hook_hits.char += 1
    local r = table.pack(BASE.string.char(...))
    for i = 1, r.n do scan(r[i]) end
    return table.unpack(r, 1, r.n)
  end
  ENV.string = copy
end

if variant == "frozen_clock" then
  -- attacker-controlled clock: constant time neutralizes the adaptive
  -- timing guard (window deltas are always 0)
  ENV.os = setmetatable({{clock = function() return 1000 end}}, {{
    __index = function(_, k) return wrap("os." .. k, BASE.os[k]) end,
  }})
end

local main, err = loadstring(TARGET, "R065")
assert(main, tostring(err))
setfenv(main, ENV)
local ok, ret = pcall(main)
stats.completed = ok
stats.returned = tostring(ret)

REAL_PRINT("RESULT\\t" .. table.concat({{
  variant, tostring(ok), tostring(ret), stats.loads_from_inside,
  stats.env_calls, stats.bysuda_seen, stats.string_hook_hits.byte,
  stats.string_hook_hits.sub, stats.string_hook_hits.char,
}}, "\\t"))
local names = {{}}
for k in pairs(stats.global_reads) do table.insert(names, k) end
table.sort(names)
for _, k in ipairs(names) do
  REAL_PRINT("ENVREAD\\t" .. k .. "\\t" .. stats.global_reads[k])
end
if not ok then REAL_PRINT("ERROR\\t" .. tostring(ret)) end
'''


def main():
    target_bytes = TARGET.read_bytes()
    expected = 'be049ee7abb482b1264ab945504332015a8188a3e6a134aeded2cfbd83220bec'
    assert hashlib.sha256(target_bytes).hexdigest() == expected, 'R065 target hash changed'
    results = {}
    for variant in ['plain_env', 'load_hook', 'envlog', 'string_hook', 'frozen_clock']:
        harness = HERE / 'work' / (variant + '.luau')
        harness.parent.mkdir(exist_ok=True)
        harness.write_text(
            TEMPLATE.format(target=long_string(target_bytes.decode()), variant=variant))
        try:
            run = subprocess.run([str(LUAU), str(harness)], capture_output=True, timeout=30)
            rc, out, err = run.returncode, run.stdout, run.stderr
        except subprocess.TimeoutExpired as exc:
            rc, out, err = 'TIMEOUT', exc.stdout or b'', exc.stderr or b''
        results[variant] = {
            'returncode': rc,
            'stdout': out.decode(errors='replace'),
            'stderr': err.decode(errors='replace')[:500],
        }
        print(f'== {variant} (rc={rc}) ==')
        print(out.decode(errors='replace'))
        if err:
            print('STDERR:', err.decode(errors='replace')[:500])
    (HERE / 'harness-results.json').write_text(json.dumps(results, indent=2) + '\n')
    # baseline: unmodified direct run for comparison
    run = subprocess.run([str(LUAU), str(TARGET)], capture_output=True, timeout=120)
    print('== direct (rc=%d) stdout=%r stderr=%r ==' % (
        run.returncode, run.stdout, run.stderr[:200]))


if __name__ == '__main__':
    sys.exit(main())
