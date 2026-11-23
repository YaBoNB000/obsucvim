#!/usr/bin/env python3
"""P4 Guard portability & tamper matrix test.

Verifies:
1. Optional host/sandbox capabilities (`xpcall = nil`, `debug = nil`,
   `debug.info = nil`, `os = nil`, `os.clock = nil`, `newproxy = nil`,
   `warn = nil`, `LUAU_READONLY_GLOBALS=1`, `setfenv` `__index` proxy tables,
   legitimate host `print` redirection) DO NOT false-positive under default
   `--guard`.
2. Active tamper (`present && wrong-type`, broken `xpcall`/`unpack`/metatable
   semantics, `false` boolean shadowing, `debug.info` `[C]` spoofing, Lua
   `loadstring` hook with or without `env.debug = nil`, and core-primitive
   shadowing even when `xpcall = nil`) FAILS CLOSED within bounded subprocess
   timeout and never produces clean program output.
3. `--guard` (default) and `--no-guard` exhibit verified differential behavior
   on guard-only tamper vectors (proving default guard is never a no-op).
"""
import concurrent.futures
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO = ROOT.parent


def find_bin(env_key: str, candidates: list[Path]) -> Path:
    if env_key in os.environ and Path(os.environ[env_key]).is_file():
        return Path(os.environ[env_key])
    for c in candidates:
        if c.is_file() and os.access(c, os.X_OK):
            return c
    raise SystemExit(f"missing binary for {env_key}: tried {candidates}")


OBSUCVIM = find_bin(
    "OBSUCVIM_BIN",
    [ROOT / "target" / "release" / "obsucvim", ROOT / "target" / "debug" / "obsucvim"],
)
LUAU = find_bin(
    "LUAU_BIN",
    [REPO / ".tools" / "bin" / "luau", Path("/home/user/tools/bin/luau")],
)

EXPECTED = "GMATRIX_OK\t42\tabc"
SRC_CODE = 'local a = 6 * 7\nprint("GMATRIX_OK", a, "a" .. "bc")\n'


def long_str(s: str) -> str:
    for lvl in range(1, 32):
        eq = "=" * lvl
        if f"]{eq}]" not in s:
            return f"[{eq}[\n{s}]{eq}]"
    raise RuntimeError("cannot wrap long string")


def build_harness(artifact_src: str, setup_lua: str) -> str:
    return f"""local ART = {long_str(artifact_src)}
local REAL_G = getfenv(0)
local ENV = {{}}
for k, v in pairs(REAL_G) do
    ENV[k] = v
end
ENV._G = ENV
{setup_lua}
local chunk = assert(REAL_G.loadstring(ART, "gmatrix"))
setfenv(chunk, ENV)
chunk()
"""


def run_luau(code_path: Path, timeout_s: float = 5.0, readonly: bool = False):
    env = os.environ.copy()
    if readonly:
        env["LUAU_READONLY_GLOBALS"] = "1"
    try:
        p = subprocess.run(
            [str(LUAU), str(code_path)],
            capture_output=True,
            text=True,
            timeout=timeout_s,
            env=env,
        )
        return p.returncode, p.stdout.strip(), p.stderr.strip(), False
    except subprocess.TimeoutExpired:
        return 124, "", "TIMEOUT", True


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="guard_matrix_") as td:
        tmp = Path(td)
        src_file = tmp / "src.lua"
        art_guard = tmp / "art_guard.lua"
        art_noguard = tmp / "art_noguard.lua"
        src_file.write_text(SRC_CODE)

        subprocess.run(
            [str(OBSUCVIM), "--preset", "v15", "--dialect", "luau", "--seed", "4242", str(src_file), str(art_guard)],
            check=True,
        )
        subprocess.run(
            [str(OBSUCVIM), "--preset", "v15", "--no-guard", "--dialect", "luau", "--seed", "4242", str(src_file), str(art_noguard)],
            check=True,
        )
        guard_txt = art_guard.read_text()
        noguard_txt = art_noguard.read_text()

        # (name, mode, setup_lua_or_None, readonly, expect_pass, expected_out)
        cases = [
            # --- 1. Direct stock & readonly-globals runs ---
            ("clean_stock_luau_guard_on", "direct_guard", None, False, True, EXPECTED),
            ("clean_stock_luau_guard_off", "direct_noguard", None, False, True, EXPECTED),
            ("readonly_globals_guard_on", "direct_guard", None, True, True, EXPECTED),
            ("readonly_globals_guard_off", "direct_noguard", None, True, True, EXPECTED),

            # --- 2. Optional APIs absent (must NOT false-positive) ---
            ("xpcall_nil_allowed", "guard", "ENV.xpcall = nil", False, True, EXPECTED),
            ("debug_nil_allowed", "guard", "ENV.debug = nil", False, True, EXPECTED),
            ("debug_info_nil_allowed", "guard", "ENV.debug = { traceback = REAL_G.debug.traceback }", False, True, EXPECTED),
            ("os_nil_allowed", "guard", "ENV.os = nil", False, True, EXPECTED),
            ("os_clock_nil_allowed", "guard", "ENV.os = {}", False, True, EXPECTED),
            ("newproxy_nil_allowed", "guard", "ENV.newproxy = nil", False, True, EXPECTED),
            ("warn_nil_allowed", "guard", "ENV.warn = nil", False, True, EXPECTED),
            (
                "print_redirect_allowed",
                "guard",
                """local cap = {}
ENV.print = function(...)
    local t = { ... }
    for i = 1, #t do t[i] = tostring(t[i]) end
    REAL_G.print(table.concat(t, "\\t"))
end""",
                False,
                True,
                EXPECTED,
            ),
            (
                "setfenv_index_proxy_allowed",
                "custom",
                f"""local ART = {long_str(guard_txt)}
local proxy = setmetatable({{}}, {{ __index = getfenv(0) }})
local chunk = assert(loadstring(ART, "proxy"))
setfenv(chunk, proxy)
chunk()
""",
                False,
                True,
                EXPECTED,
            ),

            # --- 3. Active tamper (must fail-closed under --guard) ---
            ("pcall_tampered_nonfn", "guard", "ENV.pcall = 123", False, False, EXPECTED),
            ("shadow_pcall_false_tamper", "guard", "ENV.pcall = false", False, False, EXPECTED),
            (
                "shadow_rawget_hook_with_xpcall_nil",
                "guard",
                "ENV.xpcall = nil\nlocal rg = REAL_G.rawget\nENV.rawget = function(t, k) return rg(t, k) end",
                False,
                False,
                EXPECTED,
            ),
            ("xpcall_tampered_nonfn", "guard", "ENV.xpcall = 123", False, False, EXPECTED),
            ("xpcall_stub_broken", "guard", "ENV.xpcall = function() return true, 0 end", False, False, EXPECTED),
            ("warn_tampered_nonfn", "guard", 'ENV.warn = "bad_warn"', False, False, EXPECTED),
            ("debug_non_table_tamper", "guard", "ENV.debug = 123", False, False, EXPECTED),
            ("debug_info_nonfn_tamper", "guard", "ENV.debug = { info = 123 }", False, False, EXPECTED),
            (
                "debug_info_spoof_c",
                "guard",
                'local di = REAL_G.debug.info\nENV.debug = { info = function(f, w, ...) if w == "s" then return "[C]" end return di(f, w, ...) end }',
                False,
                False,
                EXPECTED,
            ),
            (
                "loadstring_lua_hook",
                "guard",
                "local ls = REAL_G.loadstring\nENV.loadstring = function(s, ...) return ls(s, ...) end",
                False,
                False,
                EXPECTED,
            ),
            (
                "loadstring_hook_with_env_debug_nil",
                "guard",
                "ENV.debug = nil\nlocal ls = REAL_G.loadstring\nENV.loadstring = function(s, ...) return ls(s, ...) end",
                False,
                False,
                EXPECTED,
            ),
            ("newproxy_tampered_nonfn", "guard", "ENV.newproxy = 99", False, False, EXPECTED),
            ("os_non_table_tamper", "guard", 'ENV.os = "bad_os"', False, False, EXPECTED),
            ("os_clock_nonfn_tamper", "guard", "ENV.os = { clock = 42 }", False, False, EXPECTED),
            ("unpack_broken_tamper", "guard", 'ENV.unpack = function() error("boom") end', False, False, EXPECTED),
            ("metatable_lock_tampered", "guard", "ENV.getmetatable = function() return nil end", False, False, EXPECTED),

            # --- 4. Differential control: guard-only vector passes under --no-guard ---
            ("differential_noguard_warn_nonfn", "noguard", 'ENV.warn = "bad_warn"', False, True, EXPECTED),
            ("differential_noguard_newproxy_nonfn", "noguard", "ENV.newproxy = 99", False, True, EXPECTED),
        ]

        def exec_case(idx_case):
            idx, (name, mode, setup, readonly, expect_pass, exp_out) = idx_case
            cpath = tmp / f"case_{idx}_{name}.lua"
            if mode == "direct_guard":
                cpath = art_guard
            elif mode == "direct_noguard":
                cpath = art_noguard
            elif mode == "custom":
                cpath.write_text(setup)
            elif mode == "guard":
                cpath.write_text(build_harness(guard_txt, setup))
            elif mode == "noguard":
                cpath.write_text(build_harness(noguard_txt, setup))
            else:
                raise ValueError(mode)
            rc, out, err, timed_out = run_luau(cpath, timeout_s=4.5, readonly=readonly)
            if expect_pass:
                ok = rc == 0 and out == exp_out and not timed_out
                detail = f"rc={rc} out={out!r} err={err[:80]!r}"
            else:
                ok = out != exp_out
                detail = f"rejected (rc={rc}, timeout={timed_out})" if ok else f"SURVIVED out={out!r}"
            return name, ok, detail

        passed = 0
        failed = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
            for name, ok, detail in pool.map(exec_case, enumerate(cases)):
                status = "PASS" if ok else "FAIL"
                print(f"  [{status}] {name}: {detail}")
                if ok:
                    passed += 1
                else:
                    failed.append((name, detail))

        print(f"guard_matrix_test: {passed}/{len(cases)} PASS, {len(failed)} FAIL")
        return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
