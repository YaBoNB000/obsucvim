#!/usr/bin/env python3
"""End-to-end test of the *real* loader against a *real* server over HTTP.

This is the test that would have caught the "BAD_REQUEST" report: every other
test mocks HTTP, so a mismatch between what the loader sends and what the
server accepts could only show up in a live run.

What it does:
  1. starts tools/engine-server.mjs on a random port (fresh state dir)
  2. registers an account, creates a folder/script, publishes a release
  3. fetches /files/<id>.lua exactly like a user would
  4. runs that loader inside a Lua interpreter (lupa) whose `request` performs
     real HTTP, with `getgenv()` / `gethwid()` / `game` faked like an executor
  5. asserts the artifact really comes back and executes

Run:
    python3 -m venv /tmp/luaenv && /tmp/luaenv/bin/pip install lupa
    /tmp/luaenv/bin/python web/tools/test-live-loader.py
"""
import json
import os
import pathlib
import random
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid

try:
    import lupa
except ImportError:
    sys.exit("lupa is required: python3 -m venv /tmp/luaenv && /tmp/luaenv/bin/pip install lupa")

SERVER = pathlib.Path(__file__).resolve().parent / "engine-server.mjs"

results = []


def check(name, ok, detail=""):
    results.append((name, ok))
    print(("PASS " if ok else "FAIL ") + name + ((" | " + detail) if detail else ""))


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


class Server:
    def __init__(self):
        self.port = free_port()
        self.base = f"http://127.0.0.1:{self.port}"
        self.work = tempfile.mkdtemp(prefix="obsucvim-live-")
        self.state_file = os.path.join(self.work, "state.json")
        self.log = open(os.path.join(self.work, "server.log"), "w+")
        self.proc = subprocess.Popen(
            ["node", str(SERVER)],
            cwd=str(SERVER.parent.parent),
            env={
                **os.environ,
                "HOST": "127.0.0.1",
                "PORT": str(self.port),
                "OBSUCVIM_STATE_FILE": self.state_file,
                "OBSUCVIM_DATA_DIR": self.work,
                "OBSUCVIM_PUBLIC_ORIGIN": self.base,
                "OBSUCVIM_TRUSTED_PROXY_IPS": "127.0.0.1",
                "CLOUDFLARE_TURNSTILE_SITE_KEY": "",
                "CLOUDFLARE_TURNSTILE_SECRET_KEY": "",
            },
            stdout=self.log,
            stderr=subprocess.STDOUT,
        )
        for _ in range(120):
            try:
                urllib.request.urlopen(self.base + "/", timeout=1)
                return
            except urllib.error.HTTPError:
                return  # 503 without dist/ still means the listener is up
            except URLError:
                time.sleep(0.1)
        raise RuntimeError("server did not start:\n" + self.read_log())

    def read_log(self):
        self.log.flush()
        self.log.seek(0)
        return self.log.read()

    def stop(self):
        self.proc.kill()
        self.proc.wait(timeout=10)
        self.log.close()
        shutil.rmtree(self.work, ignore_errors=True)


def call(server, method, path, body=None, cookie=None):
    data = json.dumps(body or {}).encode() if method != "GET" else None
    headers = {"content-type": "application/json"}
    if cookie:
        headers["cookie"] = cookie
    req = urllib.request.Request(server.base + path, data=data, method=method, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=20) as res:
            return res.status, json.loads(res.read().decode() or "{}"), res.headers.get("set-cookie")
    except urllib.error.HTTPError as err:
        return err.code, json.loads(err.read().decode() or "{}"), err.headers.get("set-cookie")


def register(server):
    _, cap, _ = call(server, "POST", "/api/auth/captcha")
    captcha_id = cap["captchaId"]
    time.sleep(0.9)  # the server rejects captcha answers younger than 700 ms
    snapshot = json.load(open(server.state_file))
    answer = snapshot["captchas"][captcha_id]["targetX"]
    email = f"live{uuid.uuid4().hex[:8]}@example.com"
    _, reg, _ = call(server, "POST", "/api/auth/register", {
        "email": email, "password": "correct-horse-battery",
        "captchaId": captcha_id, "captchaAnswer": str(answer),
    })
    if not reg.get("demoCode"):
        raise RuntimeError(f"register failed: {reg}")
    _, conf, cookie = call(server, "POST", "/api/auth/register/confirm", {
        "email": email, "code": reg["demoCode"],
    })
    if not conf.get("ok"):
        raise RuntimeError(f"confirm failed: {conf}")
    return cookie.split(";")[0]


SOURCE = 'print("live artifact")\nreturn "LIVE-ARTIFACT-OK"\n'


def publish(server, cookie, script_id):
    _, started, _ = call(server, "POST", "/api/publish", {
        "scriptId": script_id, "version": "v1", "source": SOURCE,
    }, cookie)
    if started.get("jobId") is None:
        raise RuntimeError(f"publish rejected: {started}")
    for _ in range(200):
        time.sleep(0.15)
        _, job, _ = call(server, "GET", f"/api/publish/{started['jobId']}", None, cookie)
        if job.get("status") != "processing":
            if job.get("status") != "completed":
                raise RuntimeError(f"publish failed: {job}")
            return job["result"]
    raise RuntimeError("publish timed out")


def build_lua(loader_source, server):
    lua = lupa.LuaRuntime(unpack_returned_tuples=True)
    g = lua.globals()

    def to_py(value):
        if type(value).__name__ in ("table", "_LuaTable"):
            return {k: to_py(v) for k, v in value.items()}
        return value

    def to_lua(value):
        if isinstance(value, dict):
            return lua.table_from({k: to_lua(v) for k, v in value.items()})
        if isinstance(value, list):
            return lua.table_from([to_lua(v) for v in value])
        return value

    class HttpService:
        def JSONEncode(self, table):
            return json.dumps(to_py(table))

        def JSONDecode(self, text):
            return to_lua(json.loads(text))

    class Game:
        def GetService(self, name):
            return HttpService()

    g.game = Game()

    def http_post(request):
        try:
            req = urllib.request.Request(
                request.Url, data=request.Body.encode(), method="POST",
                headers={"content-type": "application/json"},
            )
            with urllib.request.urlopen(req, timeout=20) as res:
                return lua.table_from({"StatusCode": res.status, "Body": res.read().decode()})
        except urllib.error.HTTPError as err:
            return lua.table_from({"StatusCode": err.code, "Body": err.read().decode()})
        except Exception as err:  # noqa: BLE001 - surfaced to Lua as a failed request
            return lua.table_from({"StatusCode": 0, "Body": json.dumps({"error": str(err)})})

    g.http_post = http_post
    lua.execute("request = function(r) return http_post(r) end")
    lua.execute("GENV = {}; function getgenv() return GENV end")
    lua.execute("function gethwid() return 'delta_hwid_0123456789abcdef' end")
    # Roblox/executors expose loadstring; stock Lua 5.2+ only has load.
    lua.execute("loadstring = load")
    g.SRC = loader_source
    run = lua.eval("""
    function()
      local f, err = load(SRC, "loader")
      if not f then return false, "compile: " .. tostring(err) end
      local ok, res = pcall(f)
      return ok, tostring(res)
    end
    """)
    return lua, run


def main():
    server = Server()
    try:
        cookie = register(server)
        _, folder, _ = call(server, "POST", "/api/auth/folders", {"name": "Live hub"}, cookie)
        folder_id = folder["folder"]["folderId"]
        _, script, _ = call(server, "POST", "/api/auth/scripts", {"name": "Live script", "folderId": folder_id}, cookie)
        script_id = script["script"]["scriptId"]
        result = publish(server, cookie, script_id)
        check("release published through the async job", bool(result.get("sha256")))

        _, lic, _ = call(server, "POST", "/api/auth/licenses", {"folderId": folder_id, "label": "live"}, cookie)
        key = lic["licenseKey"]

        with urllib.request.urlopen(server.base + f"/files/{script_id}.lua", timeout=20) as res:
            loader_source = res.read().decode()
        check("loader served over HTTP", script_id in loader_source and "https://" not in loader_source.split("BASE =")[1][:20])

        lua, run = build_lua(loader_source, server)

        # (a) the pipeline artifact is delivered byte-for-byte
        import hashlib

        _, verdict, _ = call(server, "POST", "/api/licenses/validate", {
            "scriptId": script_id, "key": key, "hwid": "delta_hwid_0123456789abcdef",
            "nonce": uuid.uuid4().hex,
        })
        _, fetched, _ = call(server, "POST", "/api/releases/fetch", {
            "scriptId": script_id, "token": verdict["token"],
        })
        digest = hashlib.sha256(fetched["code"].encode()).hexdigest()
        check("pipeline artifact matches the recorded sha256", digest == result["sha256"],
              f"{digest[:12]} vs {result['sha256'][:12]}")

        lua.execute(f"getgenv().key = '{key}'")
        ok, out = run()
        check("Luau artifact reaches the executor (execution needs Luau)",
              (not ok) and "failed to compile" in out, str(out)[:110])

        # (b) a hand-uploaded plain-Lua release runs for real through the same loader
        _, manual, _ = call(server, "POST", "/api/auth/releases", {
            "scriptId": script_id, "version": "v2",
            "code": 'return "LIVE-ARTIFACT-OK"',
        }, cookie)
        check("manual plain-Lua release accepted", manual.get("ok") is True)
        ok, out = run()
        check("real loader runs the published artifact", ok and "LIVE-ARTIFACT-OK" in out, out)

        lua.execute("getgenv().key = '  " + key + "\\n'")
        ok, out = run()
        check("padded key still works over real HTTP", ok and "LIVE-ARTIFACT-OK" in out, out)

        lua.execute("getgenv().key = '" + key.upper() + "'")
        ok, out = run()
        check("uppercase key still works over real HTTP", ok and "LIVE-ARTIFACT-OK" in out, out)

        lua.execute("getgenv().key = 'BAfjuLxndwTvMBNiCyqMsXMaTcOqXpcr'")
        ok, out = run()
        check("32-character key is explained, not just BAD_REQUEST",
              (not ok) and "key format looks wrong" in out, out)

        lua.execute("getgenv().key = 'lic_0123456789abcdef'")
        ok, out = run()
        check("licence id is explained", (not ok) and "licence id" in out, out)

        lua.execute("getgenv().key = ''")
        ok, out = run()
        check("missing key is explained", (not ok) and "set getgenv().key" in out, out)

        lua.execute("getgenv().key = 'osk_" + "0" * 64 + "'")
        ok, out = run()
        check("unknown but well-formed key reports invalid", (not ok) and "invalid or revoked" in out, out)

        # A freshly issued key on another device must match the device-lock path.
        _, lic2, _ = call(server, "POST", "/api/auth/licenses", {"folderId": folder_id, "label": "device"}, cookie)
        key2 = lic2["licenseKey"]
        lua.execute(f"getgenv().key = '{key2}'")
        ok, _ = run()
        lua.execute("function gethwid() return 'another_hwid_zzzzzzzz' end")
        ok2, out2 = run()
        check("second device is refused with a clear message",
              (not ok2) and "bound to another device" in out2, out2)
    finally:
        if any(not ok for _, ok in results):
            print("\n--- server log ---")
            print(server.read_log()[-2000:])
        server.stop()

    failed = [name for name, ok in results if not ok]
    print()
    print(f"{len(results) - len(failed)}/{len(results)} passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    from urllib.error import URLError  # noqa: E402  (used inside Server startup)
    main()
