#!/usr/bin/env python3
"""Shared plumbing for the Phase 18 P3-1 black-box attack scripts.

Deliberately contains **no** knowledge of the obfuscator's internals: it only
knows how to (a) build an artifact with the public CLI, (b) run Luau, and
(c) wrap an artifact in the generic "replace the environment's string table"
hook that any public deobfuscation tutorial starts from.
"""
from __future__ import annotations

import os
import re
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ENGINE = HERE.parent.parent
REPO = ENGINE.parent
BIN = os.environ.get(
    "OBSUCVIM_BIN", str(ENGINE / "target" / "release" / "obsucvim")
)
TOOLS = Path(
    os.environ.get("OBSUCVIM_TOOLS", str(REPO / ".tools" / "bin"))
)
LUAU = str(TOOLS / "luau")

# Fixed by the task book: these three corpora, no picking.
CASES = ("basics", "control", "real_algo")


def case_path(name: str) -> Path:
    p = ENGINE / "tests" / "cases" / f"{name}.lua"
    if not p.is_file():
        raise SystemExit(f"missing corpus {p}")
    return p


def build(name: str, out: Path, seed: int = 42) -> Path:
    """Obfuscate `tests/cases/<name>.lua` with the default v15 preset."""
    subprocess.run(
        [
            BIN,
            "--preset",
            "v15",
            "--dialect",
            "luau",
            "--seed",
            str(seed),
            str(case_path(name)),
            str(out),
        ],
        check=True,
        capture_output=True,
    )
    return out


def build_source(src: Path, out: Path, seed: int = 42) -> Path:
    subprocess.run(
        [
            BIN,
            "--preset",
            "v15",
            "--dialect",
            "luau",
            "--seed",
            str(seed),
            str(src),
            str(out),
        ],
        check=True,
        capture_output=True,
    )
    return out


def run_luau(path: Path, timeout: float = 60.0) -> subprocess.CompletedProcess:
    return subprocess.run(
        [LUAU, str(path)], capture_output=True, text=True, timeout=timeout
    )


def long_string(src: str) -> str:
    """Wrap `src` in a Lua long string whose delimiter cannot collide."""
    level = 0
    while "]" + ("=" * level) + "]" in src:
        level += 1
    return f"[{'=' * level}[\n{src}\n]{'=' * level}]"


# The generic "hook the string table and dump every decoded byte" harness that
# public deobfuscation write-ups start from. It knows nothing about the VM.
HOOK_BODY = r'''
local hits = {byte = 0, sub = 0, char = 0}
local trace = {}
local copied = {}
for k, v in pairs(string) do copied[k] = v end
for _, name in ipairs({"byte", "sub", "char"}) do
    local fn = string[name]
    copied[name] = function(...)
        hits[name] = hits[name] + 1
        if name == "char" then
            local n = select("#", ...)
            trace[#trace + 1] = n
        end
        return fn(...)
    end
end
local env = {}
for k, v in pairs(_G) do env[k] = v end
env._G = env
env.string = copied
local chunk = assert(loadstring(ART, "blackbox"))
setfenv(chunk, env)
local ok, err = pcall(chunk)
print("BLACKBOX_RUN_OK", ok, err)
print("BLACKBOX_HOOKS", hits.byte, hits.sub, hits.char)
local parts = {}
for i = 1, #trace do parts[i] = tostring(trace[i]) end
print("BLACKBOX_TRACE_LEN", #trace)
print("BLACKBOX_TRACE", table.concat(parts, ","))
'''


def make_hook(artifact_src: str) -> str:
    return "local ART = " + long_string(artifact_src) + "\n" + HOOK_BODY


def strip_long_strings(text: str) -> str:
    """Remove Lua long strings (`[[...]]` / `[=[...]=]`) so their contents are
    not mistaken for code."""
    out = []
    i = 0
    n = len(text)
    while i < n:
        m = re.match(r"\[(=*)\[", text[i:])
        if m:
            lvl = m.group(1)
            close = "]" + lvl + "]"
            j = text.find(close, i + len(m.group(0)))
            i = n if j == -1 else j + len(close)
            out.append(' "" ')
            continue
        out.append(text[i])
        i += 1
    return "".join(out)


def strip_comments(text: str) -> str:
    text = re.sub(r"--\[(=*)\[.*?\]\1\]", " ", text, flags=re.S)
    return re.sub(r"--[^\n]*", " ", text)


def verdict(level: str, label: str) -> str:
    return f"BLACKBOX_VERDICT\t{label}\t{level}"
