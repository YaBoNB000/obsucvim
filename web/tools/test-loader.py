#!/usr/bin/env python3
"""Loader / guard regression tests for the hosted loader.

These run the real templates from tools/ inside a small Lua interpreter (via lupa)
with a mocked Roblox host: `game` is exposed as a userdata with GetService, the
same shape a real executor gives, plus a `request` function that answers like the
licence server. That distinction matters — an earlier version of the loader only
accepted `game` as a table, which passes a naive mock but fails in Roblox.

Run:
    python3 -m venv /tmp/luaenv && /tmp/luaenv/bin/pip install lupa
    /tmp/luaenv/bin/python web/tools/test-loader.py

Exit code 0 means every check passed. This is a protocol/behaviour test: it does
not prove anything about a specific executor, and it cannot run the obfuscated
artifact because that output uses Luau syntax (compound assignment) which stock
Lua rejects.
"""
import json
import pathlib
import sys

try:
    import lupa
except ImportError:
    sys.exit("lupa is required: python3 -m venv /tmp/luaenv && /tmp/luaenv/bin/pip install lupa")

TOOLS = pathlib.Path(__file__).resolve().parent
SCRIPT_ID = "scr_cacdb4e8aa05e2867fa40560"
BASE = "https://obsucvim.l2.ink"

lua = lupa.LuaRuntime(unpack_returned_tuples=True)
g = lua.globals()


def to_lua(value):
    if isinstance(value, dict):
        return lua.table_from({k: to_lua(v) for k, v in value.items()})
    if isinstance(value, list):
        return lua.table_from([to_lua(v) for v in value])
    return value


def to_py(value):
    if type(value).__name__ in ("table", "_LuaTable"):
        return {k: to_py(v) for k, v in value.items()}
    return value


class HttpService:
    def JSONEncode(self, table):
        return json.dumps(to_py(table))

    def JSONDecode(self, text):
        return to_lua(json.loads(text))


class Game:
    """Stands in for the Roblox DataModel: lupa hands this to Lua as userdata."""

    def GetService(self, name):
        return HttpService()


def use_userdata_game():
    g.game = Game()


def use_table_game():
    g.game = lua.table_from({"GetService": lua.eval("function(self, name) return game_http end")})
    g.game_http = HttpService()


lua.execute("""
SERVER_RESPONSES = {}
LAST_REQUEST = {}
function http_request(req)
  local body = req.Body or ""
  LAST_REQUEST[#LAST_REQUEST + 1] = body
  local path = string.match(req.Url, "/api/[%w/-]+")
  local names = { ["/api/licenses/validate"] = "validate", ["/api/releases/fetch"] = "fetch" }
  local key = names[path] or path
  local entry = SERVER_RESPONSES[key]
  if not entry then return { StatusCode = 404, Body = '{"code":"NOT_FOUND"}' } end
  -- the real server echoes the nonce and the script id it validated
  local nonce = string.match(body, '"nonce"%s*:%s*"([^"]*)"') or ""
  local scriptId = string.match(body, '"scriptId"%s*:%s*"([^"]*)"') or ""
  entry = string.gsub(entry, '"__NONCE__"', '"' .. nonce .. '"')
  entry = string.gsub(entry, '"__SCRIPTID__"', '"' .. scriptId .. '"')
  return { StatusCode = 200, Body = entry }
end
""")
g.request = g.http_request
g.loadstring = lua.eval("load")

RUN = lua.eval("""
function(src)
  local f, err = load(src, "chunk")
  if not f then return false, "compile: " .. tostring(err) end
  local ok, res = pcall(f)
  return ok, tostring(res)
end
""")


def set_responses(mapping):
    g.SERVER_RESPONSES = to_lua(mapping)


def run(src):
    return RUN(src)


def fill(template, script_id=SCRIPT_ID, endpoint=BASE):
    """Fill both placeholder styles: the loader's __SCRIPT_ID__/__BASE_URL__ and
    the guard's __OBSUCVIM_SCRIPT_ID__/__OBSUCVIM_ENDPOINT__."""
    for token in ("__SCRIPT_ID__", "__OBSUCVIM_SCRIPT_ID__"):
        template = template.replace(token, script_id)
    for token in ("__BASE_URL__", "__OBSUCVIM_ENDPOINT__"):
        template = template.replace(token, endpoint)
    return template


LOADER = (TOOLS / "loader-template.luau").read_text(encoding="utf-8")
GUARD = (TOOLS / "license-guard.luau").read_text(encoding="utf-8")

results = []


def check(name, condition, detail=""):
    results.append((name, condition))
    print(("PASS " if condition else "FAIL ") + name + ((" | " + detail) if detail else ""))


VALID = {
    "valid": True,
    "nonce": "__NONCE__",
    "scriptId": "__SCRIPTID__",
    "token": "lst_abc123",
    "runtime": {"premium": True, "note": "vip", "secondsLeft": 3600, "totalExecutions": 7},
}
FETCH = {"version": "v7", "code": "return 'ARTIFACT-RAN-1234567890'"}

# 1-2. executor-style userdata `game`: the case that used to fail outright
use_userdata_game()
g.getgenv = lambda: lua.table_from({
    "key": "OBS-TEST-KEY",
    "gethwid": lua.eval("function() return 'hwid-abc-123456' end"),
})
set_responses({"validate": json.dumps(VALID), "fetch": json.dumps(FETCH)})
ok, out = run(fill(LOADER))
check("loader accepts a userdata game object", ok and "ARTIFACT-RAN-1234567890" in out, out)

# 3. table-shaped `game` keeps working
use_table_game()
ok, out = run(fill(LOADER))
check("loader still accepts a table game object", ok and "ARTIFACT-RAN-1234567890" in out, out)

# 4. no `game` at all -> clean, readable failure
del g.game
ok, out = run(fill(LOADER))
check("missing game object fails cleanly", (not ok) and "game object unavailable" in out, out)

# 5. banned licence shows the reason
use_userdata_game()
set_responses({"validate": json.dumps({
    "valid": False, "scriptId": "__SCRIPTID__", "nonce": "__NONCE__",
    "code": "KEY_BANNED", "banReason": "chargeback",
})})
ok, out = run(fill(LOADER))
check("banned licence reports its reason", (not ok) and "licence banned" in out and "chargeback" in out, out)

# 6. missing key message
g.getgenv = lambda: lua.table_from({})
set_responses({"validate": json.dumps({
    "valid": False, "scriptId": "__SCRIPTID__", "nonce": "__NONCE__", "code": "KEY_MISSING",
})})
ok, out = run(fill(LOADER))
check("missing key tells the user what to set", (not ok) and "set getgenv().key" in out, out)

# 7. the on_missing_key hook can supply a key
g.getgenv = lambda: lua.table_from({
    "obsucvim_on_missing_key": lua.eval("function() getgenv().key = 'HOOK-KEY' end"),
})
set_responses({
    "validate": json.dumps(VALID),
    "fetch": json.dumps({"version": "v7", "code": "return 'HOOKED-1234567890'"}),
})
ok, out = run(fill(LOADER))
check("obsucvim_on_missing_key can supply the key", ok and "HOOKED-1234567890" in out, out)

# 8. injected guard accepts a userdata game too
guard_endpoint = BASE + "/api/licenses/validate"
g.getgenv = lambda: lua.table_from({
    "key": "OBS-TEST-KEY",
    "gethwid": lua.eval("function() return 'hwid-abc-123456' end"),
})
set_responses({"validate": json.dumps(VALID)})
ok, out = run(fill(GUARD, endpoint=guard_endpoint) + "\nreturn 'GUARD-PASSED'")
check("guard accepts a userdata game object", ok and "GUARD-PASSED" in out, out)

# 9. guard refuses to run without a key
g.getgenv = lambda: lua.table_from({})
set_responses({"validate": json.dumps({
    "valid": False, "scriptId": "__SCRIPTID__", "nonce": "__NONCE__", "code": "KEY_MISSING",
})})
ok, out = run(fill(GUARD, endpoint=guard_endpoint) + "\nreturn 'GUARD-PASSED'")
check("guard refuses to run without a key", (not ok) and "set getgenv().key" in out, out)

# 10. FFA: no token, fetch still released
use_userdata_game()
g.getgenv = lambda: lua.table_from({})
set_responses({
    "validate": json.dumps({
        "valid": True, "nonce": "__NONCE__", "scriptId": "__SCRIPTID__", "runtime": {"premium": False},
    }),
    "fetch": json.dumps({"version": "v9", "code": "return 'FFA-RAN-1234567890'"}),
})
ok, out = run(fill(LOADER))
check("FFA fetches and runs without a token", ok and "FFA-RAN-1234567890" in out, out)

failed = [name for name, ok in results if not ok]
print()
print(f"{len(results) - len(failed)}/{len(results)} passed")
sys.exit(1 if failed else 0)
