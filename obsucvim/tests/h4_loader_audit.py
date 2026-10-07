#!/usr/bin/env python3
"""H4-0 internal-loader leakage audit.

Builds a fresh v15 artifact, runs it in a cloned environment whose load/loadstring
wrappers record every plaintext argument, and spoofs debug.info wrappers as [C].
This models the low-cost observation boundary demonstrated by high-fidelity
Luraph tracers without vendoring or executing a third-party deobfuscator.

Modes:
  --expect-open   H4-0 baseline: PASS only when the current loader leak is seen.
  --enforce-zero  H4 completion gate: PASS only with zero internal loader calls.
  (no mode)       report only; infrastructure failures still return non-zero.
"""
from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
REPO = ROOT.parent
TOOL = ROOT / "target/release/obsucvim"
TOOLS = REPO / ".tools/bin"
LUAU = TOOLS / "luau"
if not LUAU.exists():
    LUAU = pathlib.Path("/home/user/tools/bin/luau")

SOURCE = """local x = 17
local y = 26
local z = x * y + 9
local t = {y, x, z}
local function h(v) return v - t[2] + t[1] end
local r = h(z)
print("H4_RESULT", z, r)
"""
EXPECTED = "H4_RESULT\t451\t460"
LOAD_RE = re.compile(
    r"H4LOAD\|(\d+)\|(\d+)\|(\d+)\|([01])\|([01])\|([01])"
)
DONE_RE = re.compile(r"H4DONE\|([01])\|(\d+)\|(.*)")


def long_string(text: str) -> str:
    for level in range(1, 64):
        eq = "=" * level
        if "]" + eq + "]" not in text:
            # Lua ignores the first newline after a long-string opener. Our
            # inserted newline keeps the target bytes exact.
            return f"[{eq}[\n{text}]{eq}]"
    raise ValueError("no collision-free Lua long-string delimiter")


def harness(target: str) -> str:
    target_lit = long_string(target)
    return f'''-- H4-0 test harness; never emitted into protected artifacts.
local TARGET = {target_lit}
local REAL_PRINT = print
local REAL_LOADSTRING = loadstring
local REAL_LOAD = load
local REAL_GETFENV = getfenv
local REAL_SETFENV = setfenv
local REAL_DEBUG = debug
local BASE = REAL_GETFENV(0)
local ENV = {{}}
for k, v in pairs(BASE) do ENV[k] = v end
ENV._G = ENV

local calls = 0
local wrapped = setmetatable({{}}, {{ __mode = "k" }})

local function hash(s)
    local h = 0
    for i = 1, #s do h = (h * 131 + string.byte(s, i)) % 2147483647 end
    return h
end

local function emit(src)
    calls = calls + 1
    local rf = string.find(src, "return function", 1, true) and 1 or 0
    local vm = (string.find(src, "HW2", 1, true)
        or string.find(src, "bdec", 1, true)
        or string.find(src, "CDEC", 1, true)) and 1 or 0
    local grammar = (string.find(src, "local ", 1, true)
        and string.find(src, "function", 1, true)) and 1 or 0
    REAL_PRINT(string.format(
        "H4LOAD|%d|%d|%d|%d|%d|%d",
        calls, #src, hash(src), rf, vm, grammar
    ))
    -- Force pipe-buffered CLIs to flush before a deliberate tamper stall.
    REAL_PRINT(string.rep(" ", 8192))
end

local function hookedLoadstring(src, chunkname)
    if type(src) == "string" then emit(src) end
    local fn, err = REAL_LOADSTRING(src, chunkname)
    if fn and REAL_SETFENV then REAL_SETFENV(fn, ENV) end
    return fn, err
end
wrapped[hookedLoadstring] = true
ENV.loadstring = hookedLoadstring

if type(REAL_LOAD) == "function" then
    local function hookedLoad(src, chunkname)
        if type(src) == "string" then emit(src) end
        local fn, err = REAL_LOAD(src, chunkname)
        if fn and REAL_SETFENV then REAL_SETFENV(fn, ENV) end
        return fn, err
    end
    wrapped[hookedLoad] = true
    ENV.load = hookedLoad
end

local fakeDebug = {{}}
for k, v in pairs(REAL_DEBUG) do fakeDebug[k] = v end
local realInfo = REAL_DEBUG.info

local function fakeFields(fn, opts)
    local out = {{}}
    for ch in string.gmatch(opts or "", ".") do
        if ch == "s" then out[#out + 1] = "[C]"
        elseif ch == "l" then out[#out + 1] = -1
        elseif ch == "n" then out[#out + 1] = ""
        elseif ch == "a" then out[#out + 1] = 0; out[#out + 1] = true
        elseif ch == "f" then out[#out + 1] = fn end
    end
    return table.unpack(out)
end

local fakeInfo
fakeInfo = function(a, b, c)
    if type(a) == "function" then
        if wrapped[a] or a == fakeInfo then return fakeFields(a, b) end
        return realInfo(a, b)
    end
    if type(a) == "thread" then
        if type(b) == "number" then return realInfo(a, b + 1, c) end
        return realInfo(a, b, c)
    end
    if type(a) == "number" then
        if a == 0 then return fakeFields(fakeInfo, b) end
        return realInfo(a + 1, b)
    end
    return realInfo(a, b, c)
end
wrapped[fakeInfo] = true
fakeDebug.info = fakeInfo
fakeDebug.traceback = function(...)
    local text = REAL_DEBUG.traceback(...)
    if type(text) ~= "string" then return text end
    local lines = {{}}
    for line in string.gmatch(text, "[^\\n]+") do
        if not string.find(line, "h4_loader_spy", 1, true) then
            lines[#lines + 1] = line
        end
    end
    return table.concat(lines, "\\n")
end
ENV.debug = fakeDebug

local outer, compileErr = REAL_LOADSTRING(TARGET, "=h4_target")
if not outer then
    REAL_PRINT("H4DONE|0|" .. calls .. "|compile:" .. tostring(compileErr))
    return
end
REAL_SETFENV(outer, ENV)
-- The protected boot reads getfenv(0), i.e. the current thread environment,
-- not merely the compiled function's environment. Run it in a fresh coroutine
-- and set that thread to ENV, matching high-fidelity tracer behavior.
local co = coroutine.create(function()
    REAL_SETFENV(0, ENV)
    return outer()
end)
local resumed = table.pack(coroutine.resume(co))
local ok = resumed[1]
local err = resumed[2]
REAL_PRINT("H4DONE|" .. (ok and "1" or "0") .. "|" .. calls .. "|" .. tostring(err))
REAL_PRINT(string.rep(" ", 8192))
'''


def run(cmd: list[str], timeout: int = 60) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    group = ap.add_mutually_exclusive_group()
    group.add_argument("--expect-open", action="store_true")
    group.add_argument("--enforce-zero", action="store_true")
    ap.add_argument("--max-calls", type=int, default=None,
                    help="optional incremental ceiling before the final zero-loader gate")
    ap.add_argument("--timeout", type=int, default=25)
    args = ap.parse_args()

    if not TOOL.exists() or not LUAU.exists():
        print("H4 audit infrastructure missing; build release and toolchain first", file=sys.stderr)
        return 2

    with tempfile.TemporaryDirectory() as td_raw:
        td = pathlib.Path(td_raw)
        src = td / "case.lua"
        out = td / "case.v15.lua"
        spy = td / "h4_loader_spy.lua"
        src.write_text(SOURCE)

        built = run([
            str(TOOL), "--preset", "v15", "--seed", "440041",
            str(src), str(out),
        ])
        if built.returncode != 0:
            print("obfuscation failed:\n" + built.stderr[-1000:], file=sys.stderr)
            return 2

        clean = run([str(LUAU), str(out)], timeout=args.timeout)
        if clean.returncode != 0 or EXPECTED not in clean.stdout:
            print("clean artifact failed:\n" + clean.stdout + clean.stderr, file=sys.stderr)
            return 2

        target = out.read_text()
        spy.write_text(harness(target))

        timed_out = False
        try:
            observed = run([str(LUAU), str(spy)], timeout=args.timeout)
            stream = observed.stdout + observed.stderr
            spy_rc = observed.returncode
        except subprocess.TimeoutExpired as exc:
            timed_out = True
            parts = []
            for chunk in (exc.stdout, exc.stderr):
                if isinstance(chunk, bytes):
                    parts.append(chunk.decode(errors="replace"))
                elif chunk:
                    parts.append(chunk)
            stream = "".join(parts)
            spy_rc = 124

        rows = [
            {
                "index": int(m.group(1)),
                "length": int(m.group(2)),
                "hash": int(m.group(3)),
                "return_function": m.group(4) == "1",
                "vm_marker": m.group(5) == "1",
                "grammar": m.group(6) == "1",
            }
            for m in LOAD_RE.finditer(stream)
        ]
        done_matches = list(DONE_RE.finditer(stream))
        done = done_matches[-1] if done_matches else None
        completed = done is not None
        done_ok = completed and done.group(1) == "1"
        source_like = sum(r["return_function"] or r["grammar"] for r in rows)
        vm_like = sum(r["vm_marker"] for r in rows)
        largest = max((r["length"] for r in rows), default=0)
        result_seen = EXPECTED in stream

        print("== H4-0 Loader 泄漏审计 ==")
        print(f"clean run                 : YES ({EXPECTED})")
        print(f"spy run completed         : {'YES' if completed else 'NO'}"
              f" (rc={spy_rc}, timeout={timed_out})")
        print(f"internal loader calls     : {len(rows)}")
        print(f"source-like loader args   : {source_like}")
        print(f"VM/runtime marker args    : {vm_like}")
        print(f"largest observed argument : {largest} bytes")
        print(f"payload result survived   : {'YES' if result_seen and done_ok else 'NO'}")
        call_budget_ok = args.max_calls is None or len(rows) <= args.max_calls
        if args.max_calls is not None:
            print(f"incremental call ceiling  : {len(rows)}/{args.max_calls} "
                  f"({'PASS' if call_budget_ok else 'FAIL'})")
        if rows:
            preview = ", ".join(str(r["length"]) for r in rows[:12])
            print(f"first argument sizes      : {preview}")

        zero = (
            len(rows) == 0
            and source_like == 0
            and completed
            and done_ok
            and result_seen
        )
        leak_open = len(rows) > 0 and source_like > 0

        if args.enforce_zero:
            passed = zero and call_budget_ok
            print("\nH4 ZERO-LOADER GATE: " + ("PASS" if passed else "FAIL"))
            return 0 if passed else 1
        if args.expect_open:
            passed = leak_open and call_budget_ok
            print("\nH4-0 BASELINE CAPTURE: " + ("PASS" if passed else "FAIL"))
            print("status: " + ("OPEN (incremental migration in progress)" if leak_open else "unexpectedly closed"))
            return 0 if passed else 1

        print("\nH4 status: " + ("CLOSED" if zero else "OPEN"))
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
