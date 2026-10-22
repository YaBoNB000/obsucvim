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
GOOD_KEY = "osk_" + "a" * 64
ALT_KEY = "osk_" + "b" * 64
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

    def __init__(self):
        self.http_gets = []

    def GetService(self, name):
        return HttpService()

    def HttpGet(self, url):
        self.http_gets.append(url)
        return "return 'LIB-LOADED-1234567890'"


LAST_GAME = None


def use_userdata_game():
    global LAST_GAME
    LAST_GAME = Game()
    g.game = LAST_GAME


def use_table_game():
    g.game = lua.table_from({"GetService": lua.eval("function(self, name) return game_http end")})
    g.game_http = HttpService()


lua.execute("""
SERVER_RESPONSES = {}
LAST_REQUEST = {}
REQ_COUNT = 0
function http_request(req)
  REQ_COUNT = REQ_COUNT + 1
  local body = req.Body or ""
  LAST_REQUEST[#LAST_REQUEST + 1] = body
  local path = string.match(req.Url, "/api/[%w/-]+")
  local names = { ["/api/licenses/validate"] = "validate", ["/api/releases/fetch"] = "fetch" }
  local key = names[path] or path
  local entry = SERVER_RESPONSES[key]
  if type(entry) == "function" then entry = entry() end
  if type(entry) ~= "string" then return { StatusCode = 404, Body = '{"code":"NOT_FOUND"}' } end
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

# A stable getgenv() table: executors hand out the same table every call, and the
# loader writes OBS_* into it. It must be a real Lua function (a Python callable
# shows up as userdata, and `type(getgenv) == "function"` would be false).
lua.execute("GENV = {}; function getgenv() return GENV end")
GENV = g.GENV


def set_genv(mapping):
    for key in list(GENV.keys()):
        GENV[key] = None
    for key, value in mapping.items():
        GENV[key] = value
    return GENV


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


def fill(template, script_id=SCRIPT_ID, endpoint=BASE, heartbeat=False, silent=False):
    """Fill both placeholder styles: the loader's __SCRIPT_ID__/__BASE_URL__ and
    the guard's __OBSUCVIM_SCRIPT_ID__/__OBSUCVIM_ENDPOINT__."""
    for token in ("__SCRIPT_ID__", "__OBSUCVIM_SCRIPT_ID__"):
        template = template.replace(token, script_id)
    for token in ("__BASE_URL__", "__OBSUCVIM_ENDPOINT__"):
        template = template.replace(token, endpoint)
    template = template.replace("__HEARTBEAT__", "true" if heartbeat else "false")
    template = template.replace("__SILENT__", "true" if silent else "false")
    return template


LOADER = (TOOLS / "loader-template.luau").read_text(encoding="utf-8")
LIBRARY = (TOOLS / "key-check-library.luau").read_text(encoding="utf-8")
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
set_genv({
    "key": GOOD_KEY,
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
set_genv({})
set_responses({"validate": json.dumps({
    "valid": False, "scriptId": "__SCRIPTID__", "nonce": "__NONCE__", "code": "KEY_MISSING",
})})
ok, out = run(fill(LOADER))
check("missing key tells the user what to set", (not ok) and "set getgenv().key" in out, out)

# 7. the on_missing_key hook can supply a key
set_genv({
    "obsucvim_on_missing_key": lua.eval(f"function() getgenv().key = '{ALT_KEY}' end"),
})
set_responses({
    "validate": json.dumps(VALID),
    "fetch": json.dumps({"version": "v7", "code": "return 'HOOKED-1234567890'"}),
})
ok, out = run(fill(LOADER))
check("obsucvim_on_missing_key can supply the key", ok and "HOOKED-1234567890" in out, out)

# 8. injected guard accepts a userdata game too
guard_endpoint = BASE + "/api/licenses/validate"
set_genv({
    "key": GOOD_KEY,
    "gethwid": lua.eval("function() return 'hwid-abc-123456' end"),
})
set_responses({"validate": json.dumps(VALID)})
ok, out = run(fill(GUARD, endpoint=guard_endpoint) + "\nreturn 'GUARD-PASSED'")
check("guard accepts a userdata game object", ok and "GUARD-PASSED" in out, out)

# 9. guard refuses to run without a key
set_genv({})
set_responses({"validate": json.dumps({
    "valid": False, "scriptId": "__SCRIPTID__", "nonce": "__NONCE__", "code": "KEY_MISSING",
})})
ok, out = run(fill(GUARD, endpoint=guard_endpoint) + "\nreturn 'GUARD-PASSED'")
check("guard refuses to run without a key", (not ok) and "set getgenv().key" in out, out)

# 10. FFA: no token, fetch still released
use_userdata_game()
set_genv({})
set_responses({
    "validate": json.dumps({
        "valid": True, "nonce": "__NONCE__", "scriptId": "__SCRIPTID__", "runtime": {"premium": False},
    }),
    "fetch": json.dumps({"version": "v9", "code": "return 'FFA-RAN-1234567890'"}),
})
ok, out = run(fill(LOADER))
check("FFA fetches and runs without a token", ok and "FFA-RAN-1234567890" in out, out)

# 11. heartbeat: second check happens, revocation marks the licence invalid
use_userdata_game()
set_genv({
    "key": GOOD_KEY,
    "gethwid": lua.eval("function() return 'hwid-abc-123456' end"),
    "obsucvim_on_heartbeat_fail": lua.eval("function(code) getgenv().HB_CODE = code end"),
})
g.wait = lua.eval("function() return true end")
lua.execute("""
HB_CALLS = 0
function hb_validate()
  HB_CALLS = HB_CALLS + 1
  if HB_CALLS == 1 then
    return '{"valid":true,"nonce":"__NONCE__","scriptId":"__SCRIPTID__","token":"lst_hb","runtime":{"premium":true,"secondsLeft":600}}'
  end
  return '{"valid":false,"code":"KEY_REVOKED"}'
end
""")
set_responses({"validate": lua.eval("hb_validate"), "fetch": json.dumps({"version": "v7", "code": "return 'HB-ARTIFACT-1234567890'" * 1})})
ok, out = run(fill(LOADER, heartbeat=True))
valid_flag = lua.eval("tostring(getgenv().__obsucvim_license and getgenv().__obsucvim_license.valid)")
reason = lua.eval("tostring(getgenv().__obsucvim_license and getgenv().__obsucvim_license.reason)")
hook_code = lua.eval("tostring(getgenv().HB_CODE)")
check("heartbeat re-checks the licence", ok and "HB-ARTIFACT-1234567890" in out and lua.eval("HB_CALLS") >= 2, out)
check("heartbeat marks the licence invalid on revoke", valid_flag == "false" and reason == "KEY_REVOKED", f"valid={valid_flag} reason={reason}")
check("heartbeat calls obsucvim_on_heartbeat_fail", hook_code == "KEY_REVOKED", f"hook={hook_code}")

# 12. heartbeat disabled -> exactly one validate call
use_userdata_game()
set_genv({
    "key": GOOD_KEY,
    "gethwid": lua.eval("function() return 'hwid-abc-123456' end"),
})
set_responses({"validate": json.dumps(VALID), "fetch": json.dumps(FETCH)})
lua.execute("REQ_COUNT = 0")
ok, out = run(fill(LOADER, heartbeat=False))
check("heartbeat off means a single validate call", ok and lua.eval("REQ_COUNT") == 2, f"requests={lua.eval('REQ_COUNT')}")  # validate + fetch

# 14-16. key-check library: check_key maps codes, load_script uses the stable URL
use_userdata_game()
set_genv({"gethwid": lua.eval("function() return 'hwid-abc-123456' end")})
set_responses({
    "validate": json.dumps(VALID),
    "fetch": json.dumps({"version": "v7", "code": "return 1"}),
})
lua.execute("""
LIB_CHECK_RESPONSES = {}
function lib_request(req)
  local path = string.match(req.Url, "/api/[%w/-]+")
  if path == "/api/key/check" then
    local entry = LIB_CHECK_RESPONSES.check
    if type(entry) == "function" then entry = entry() end
    return { StatusCode = 200, Body = entry or '{"ok":false,"code":"UNKNOWN_ERROR"}' }
  end
  return { StatusCode = 404, Body = "{}" }
end
""")
g.request = lua.eval("lib_request")
lib_api = lua.eval("(function(src) local f = assert(load(src, 'library')); return f() end)")(
    LIBRARY.replace("__BASE_URL__", BASE)
)
check("library returns a table", type(lib_api).__name__ in ("table", "_LuaTable"))

lua.execute("""
LIB_CHECK_RESPONSES.check = '{"ok":true,"code":"KEY_VALID","message":"ok","data":{"auth_expire":-1,"note":"n","total_executions":2}}'
""")
lib_api["script_id"] = SCRIPT_ID
status = lib_api["check_key"]("osk_" + "a" * 64)
check("library check_key returns KEY_VALID", status["code"] == "KEY_VALID" and status["data"]["total_executions"] == 2,
      str(lua.eval("tostring(1)")))

lua.execute("""
LIB_CHECK_RESPONSES.check = '{"ok":false,"code":"KEY_HWID_LOCKED","message":"locked"}'
""")
status2 = lib_api["check_key"]("osk_" + "b" * 64)
check("library passes through rejection codes", status2["code"] == "KEY_HWID_LOCKED")

status3 = lib_api["check_key"]("")
check("library rejects an empty key locally", status3["code"] == "KEY_INVALID")

ok, out = run("""
local ok, res = pcall(function() return LIB_API.load_script() end)
return tostring(ok) .. "|" .. tostring(res)
""".replace("LIB_API", "LIB_PLACEHOLDER")) if False else (None, None)
lua.execute("LIB_API = nil")
# 用 Lua 全局暴露库对象，走真实的 load_script 路径
lua.execute("function _set_lib(t) LIB_API = t end")
lua.eval("_set_lib")(lib_api)
LIBRUN = lua.eval("""
function()
  local ok, res = pcall(function() return LIB_API.load_script() end)
  return tostring(ok) .. "|" .. tostring(res)
end
""")
result = LIBRUN()
check("library load_script runs the stable loader", result.startswith("true|LIB-LOADED-1234567890"), str(result))
gets = LAST_GAME.http_gets if LAST_GAME else []
check("library load_script uses the stable loader URL",
      bool(gets) and gets[-1].endswith(f"/files/{SCRIPT_ID}.lua"), str(gets[-1] if gets else None))

# 20-23. key hygiene: local format check, whitespace trimming, uppercase
# (the library tests swapped `request` for their own mock; restore the server one)
g.request = g.http_request
use_userdata_game()
set_responses({"validate": json.dumps(VALID), "fetch": json.dumps(FETCH)})
set_genv({"key": "BAfjuLxndwTvMBNiCyqMsXMaTcOqXpcr"})
lua.execute("REQ_COUNT = 0")
ok, out = run(fill(LOADER))
check("wrong-shaped key is rejected locally", (not ok) and "key format looks wrong" in out and lua.eval("REQ_COUNT") == 0, out)

set_genv({"key": "lic_0123456789abcdef"})
ok, out = run(fill(LOADER))
check("licence id pasted as key is explained", (not ok) and "licence id" in out, out)

set_genv({"key": "  " + GOOD_KEY + "\n"})
ok, out = run(fill(LOADER))
check("whitespace around the key is trimmed", ok and "ARTIFACT-RAN-1234567890" in out, out)
validate_req = lua.eval("""
(function()
  for i = #LAST_REQUEST, 1, -1 do
    if string.find(LAST_REQUEST[i], '"key"', 1, true) then return LAST_REQUEST[i] end
  end
  return ""
end)()
""")
check("trimmed key is what reaches the server",
      GOOD_KEY in str(validate_req) and ("\\n" not in str(validate_req)),
      str(validate_req)[:100])

set_genv({"key": "osk_" + "A" * 64})
ok, out = run(fill(LOADER))
check("uppercase hex keys are accepted", ok and "ARTIFACT-RAN-1234567890" in out, out)

failed = [name for name, ok in results if not ok]
print()
print(f"{len(results) - len(failed)}/{len(results)} passed")
sys.exit(1 if failed else 0)
