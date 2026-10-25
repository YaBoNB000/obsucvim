#!/usr/bin/env python3
"""Generate a single-file Luau print-injection probe for an owned target.

The output embeds the target and supports three edit-time modes:
  tail         append a print marker after the protected chunk
  print-hook   run under a cloned environment with print intercepted
  loader-hook  also wrap nested loadstring calls (expected to trip guards)

Usage:
  python3 tools/mobile_print_probe_gen.py input.lua output.luau.lua
"""
from __future__ import annotations

import argparse
from pathlib import Path


def long_bracket(text: str) -> str:
    """Return a collision-free Lua long string literal."""
    for level in range(1, 64):
        eq = "=" * level
        closer = "]" + eq + "]"
        if closer not in text:
            # Lua discards exactly one newline immediately after a long-string
            # opener. Insert our own so the embedded target bytes stay exact,
            # including a target that itself begins with a newline.
            return f"[{eq}[\n{text}]{eq}]"
    raise ValueError("could not find collision-free long-string delimiter")


def make_probe(target: str, target_name: str) -> str:
    source = long_bracket(target)
    return f'''-- Authorized red-team probe generated for: {target_name}
-- Edit ONLY this value: "tail" / "print-hook" / "loader-hook"
local MODE = "tail"

local TARGET = {source}
local REAL_PRINT = print
local REAL_WARN = warn
local REAL_LOADSTRING = loadstring
local REAL_GETFENV = getfenv
local REAL_SETFENV = setfenv
local BASE_ENV = REAL_GETFENV and REAL_GETFENV(0) or _G
local REPORT = {{}}
local CAPTURED = 0
local LOAD_CALLS = 0

local function text(...)
    local n = select("#", ...)
    local out = {{}}
    for i = 1, n do out[i] = tostring(select(i, ...)) end
    return table.concat(out, "\t")
end

local function log(...)
    local line = text(...)
    REPORT[#REPORT + 1] = line
    REAL_PRINT(line)
end

local function alert(...)
    local line = text(...)
    REPORT[#REPORT + 1] = line
    if type(REAL_WARN) == "function" then REAL_WARN(line) else REAL_PRINT(line) end
end

local function fingerprint(s)
    local h = 0
    for i = 1, #s do h = (h * 131 + string.byte(s, i)) % 2147483647 end
    return h
end

local function clone_env()
    local env = {{}}
    for k, v in pairs(BASE_ENV) do env[k] = v end
    env._G = env
    return env
end

local function copy_report()
    local body = table.concat(REPORT, "\\n")
    local names = {{"setclipboard", "toclipboard", "copyclipboard", "writeclipboard"}}
    for i = 1, #names do
        local fn = BASE_ENV[names[i]]
        if type(fn) == "function" then
            local ok = pcall(fn, body)
            if ok then REAL_PRINT("MOBILE-PROBE> report copied by " .. names[i]); return end
        end
    end
    local clip = BASE_ENV.clipboard
    if type(clip) == "table" and type(clip.set) == "function" then
        local ok = pcall(clip.set, body)
        if ok then REAL_PRINT("MOBILE-PROBE> report copied by clipboard.set"); return end
    end
    REAL_PRINT("MOBILE-PROBE> no clipboard API; copy console lines manually")
end

local function compile(s)
    local fn, err = REAL_LOADSTRING(s)
    if type(fn) ~= "function" then return nil, err end
    return fn
end

log("MOBILE-PROBE> target={target_name}")
log("MOBILE-PROBE> mode=" .. MODE .. " bytes=" .. #TARGET .. " hash=" .. fingerprint(TARGET))

local ok, err
if MODE == "tail" then
    -- This proves that text can be edited and execution reaches the end. It does
    -- NOT expose locals inside the protected chunk.
    local patched, replaced = string.gsub(
        TARGET, "^%s*return%s+", "local __mobile_return = ", 1)
    if replaced == 0 then patched = TARGET end
    patched = patched .. "\\nprint('MOBILE-TAIL> reached protected chunk end; top_return=', __mobile_return)"
    local fn, ce = compile(patched)
    if not fn then
        ok, err = false, "compile failed: " .. tostring(ce)
    else
        log("MOBILE-PROBE> tail marker armed")
        ok, err = pcall(fn)
    end
elseif MODE == "print-hook" or MODE == "loader-hook" then
    local env = clone_env()
    env.print = function(...)
        CAPTURED = CAPTURED + 1
        log("CAPTURED-PRINT#" .. CAPTURED .. ">", ...)
    end

    if MODE == "loader-hook" then
        local function hooked_loader(s)
            LOAD_CALLS = LOAD_CALLS + 1
            if LOAD_CALLS <= 16 then
                alert("LOADSTRING#" .. LOAD_CALLS .. "> len=" .. #s .. " hash=" .. fingerprint(s))
            end
            local fn, le = REAL_LOADSTRING(s)
            if type(fn) == "function" and REAL_SETFENV then REAL_SETFENV(fn, env) end
            return fn, le
        end
        env.loadstring = hooked_loader
        env.load = hooked_loader
        alert("MOBILE-PROBE> loader hook armed; guard may intentionally stall")
    else
        log("MOBILE-PROBE> print hook armed")
    end

    local fn, ce = compile(TARGET)
    if not fn then
        ok, err = false, "compile failed: " .. tostring(ce)
    else
        if REAL_SETFENV then REAL_SETFENV(fn, env) end
        ok, err = pcall(fn)
    end
else
    ok, err = false, "unknown MODE (use tail / print-hook / loader-hook)"
end

log("MOBILE-PROBE> finished ok=" .. tostring(ok) .. " err=" .. tostring(err))
log("MOBILE-PROBE> captured_prints=" .. CAPTURED .. " load_calls=" .. LOAD_CALLS)
copy_report()
'''


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("input", type=Path)
    ap.add_argument("output", type=Path)
    ns = ap.parse_args()
    target = ns.input.read_text(encoding="utf-8")
    ns.output.parent.mkdir(parents=True, exist_ok=True)
    ns.output.write_text(make_probe(target, ns.input.name), encoding="utf-8")
    print(f"wrote {ns.output} ({ns.output.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
