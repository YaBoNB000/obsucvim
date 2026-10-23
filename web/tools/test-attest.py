#!/usr/bin/env python3
"""Attestation must always be present and must never break the script.

Rules this test enforces (they are requirements, not suggestions):

  * tools/attest.luau runs on *any* host without raising — bare Lua, a partial
    executor, a hostile environment where getgenv/typeof/indexing explode.
  * on a Delta-like host it reports ok = true and names the executor.
  * on a bare Lua host it reports ok = false, but still returns cleanly, so the
    script keeps running (attestation is evidence, not a gate).
  * it is served in front of the loader, so it is present on every run, and the
    loader forwards the verdict with the validate call;
  * the server records the verdict without ever using it to allow or deny access.

Run:
    python3 -m venv /tmp/luaenv && /tmp/luaenv/bin/pip install lupa
    /tmp/luaenv/bin/python web/tools/test-attest.py
"""
import pathlib
import sys

try:
    import lupa
except ImportError:
    sys.exit("lupa is required: python3 -m venv /tmp/luaenv && /tmp/luaenv/bin/pip install lupa")

TOOLS = pathlib.Path(__file__).resolve().parent
ATTEST = (TOOLS / "attest.luau").read_text(encoding="utf-8")
PRELUDE = (TOOLS / "luau-roblox-prelude.luau").read_text(encoding="utf-8")

results = []


def check(name, ok, detail=""):
    results.append((name, ok))
    print(("PASS " if ok else "FAIL ") + name + ((" | " + detail) if detail else ""))


def run_attest(prelude=None):
    """Load the attestation module into a fresh interpreter and return
    (ok, attested, score, total, executor, missing, runtime_error)."""
    lua = lupa.LuaRuntime(unpack_returned_tuples=True)
    g = lua.globals()
    if prelude:
        try:
            lua.execute(prelude)
        except Exception as error:  # noqa: BLE001 - the prelude itself failed
            return None, None, None, None, None, None, f"prelude: {error}"
    g.SRC = ATTEST
    outcome = lua.eval("""
    function()
      local f, err = load(SRC, "attest")
      if not f then return "compile: " .. tostring(err) end
      local ok, res = pcall(f)
      return tostring(ok) .. "|" .. tostring(res)
    end
    """)()
    ran, _, message = str(outcome).partition("|")
    report = lua.eval("OBS_Attest")
    if report is None:
        return ran, None, None, None, None, None, message
    missing = [str(k) for k in (report["missing"] or [])]
    return (
        ran,
        lua.eval("tostring(OBS_Attested)"),
        int(report["score"]),
        int(report["total"]),
        lua.eval("tostring(OBS_Attest.executor)"),
        missing,
        message,
    )


# 1. bare Lua: no executor, no Roblox, nothing to prove
ok, attested, score, total, executor, missing, message = run_attest()
check("bare Lua: module does not raise", ok == "true", message)
check("bare Lua: reports not attested", attested == "false")
check("bare Lua: enumerates what is missing", bool(missing) and total > 0, f"{score}/{total}")

# 2. Delta-like host
ok, attested, score, total, executor, missing, message = run_attest(PRELUDE)
check("executor host: module does not raise", ok == "true", message)
check("executor host: attested", attested == "true", f"{score}/{total}")
check("executor host: names the executor", executor == "Delta", str(executor))
check("executor host: score is meaningful", score >= total * 0.6, f"{score}/{total}")

# 3. hostile host: getgenv raises, typeof lies, indexing explodes
HOSTILE = """
function getgenv() error("nope") end
function typeof() error("nope") end
setmetatable(_G, { __index = function() error("blocked") end })
"""
ok, attested, score, total, executor, missing, message = run_attest(HOSTILE)
check("hostile host: module does not raise", ok == "true", message)
check("hostile host: reports not attested instead of dying", attested == "false")

# 4. partial executor: only some APIs
PARTIAL = """
function getgenv() return _G end
function gethwid() return "abcdefgh-123456" end
"""
ok, attested, score, total, executor, missing, message = run_attest(PARTIAL)
check("partial host: module does not raise", ok == "true", message)
check("partial host: attested stays false without identifyexecutor/GetService", attested == "false")
check("partial host: still scores the parts it could see", 0 < score < total, f"{score}/{total}")

# 5. the module must publish into getgenv, not just a local
lua = lupa.LuaRuntime(unpack_returned_tuples=True)
lua.execute(PRELUDE)
lua.globals().SRC = ATTEST
lua.eval("function() local f = assert(load(SRC, 'attest')); f() end")()
check("verdict is published on getgenv()", lua.eval("tostring(getgenv().OBS_Attested)") == "true")
check("detail table is published too", lua.eval("type(getgenv().OBS_Attest)") == "table")

# 6. the loader forwards the verdict, and nothing errors on it
LOADER = (TOOLS / "loader-template.luau").read_text(encoding="utf-8")
check("loader forwards the attestation verdict", "attested = attestedFlag" in LOADER
      and "OBS_Attest" in LOADER)
gate_lines = [line for line in LOADER.splitlines() if "attest" in line.lower() and "error(" in line]
check("loader never errors because of the verdict", gate_lines == [], str(gate_lines[:2]))

# 7. it is served in front of the loader (engine-independent, unremovable)
SERVER = (TOOLS / "engine-server.mjs").read_text(encoding="utf-8")
check("server always prepends attestation to the loader",
      "[attestTemplate(), loaderTemplate()]" in SERVER)
check("artifact injection stays guard-only (engine preamble cliff)",
      "Injected by Obsucvim licence guard" in SERVER and "attestTemplate(),\n" not in SERVER)

failed = [name for name, passed in results if not passed]
print()
print(f"{len(results) - len(failed)}/{len(results)} passed")
sys.exit(1 if failed else 0)
