#!/usr/bin/env python3
"""P1/P4 Guard Stage Manifest Bidirectional Consistency & Tamper Matrix Suite.

Verifies:
1. Bidirectional manifest consistency:
   - Parses `STAGE_MANIFEST` in `engine/src/anti/mod.rs` and enforces exact 1:1
     set equality with the executable `cases` registry in this test suite (zero
     missing IDs in either direction, zero duplicates).
2. Optional host/sandbox capabilities (`xpcall = nil`, `getfenv = nil`,
   `print = nil` + `warn = nil`, `unpack = nil` with `table.unpack`,
   `table.unpack = nil` with `unpack`, `next = nil`, `debug = nil`,
   `debug.info = nil`, `os = nil`, `os.clock = nil`, `newproxy = nil`,
   `LUAU_READONLY_GLOBALS=1`, `setfenv` `__index` proxy tables, legitimate host
   `print` redirection into an environment buffer) DO NOT false-positive (`ALLOW`).
3. Active tamper & broken capability contracts (`present && wrong-type`, broken
   `xpcall`/`unpack`/`next`/metatable/`rawget`/`pcall(error)` semantics, `false`
   boolean shadowing, `debug.info` `[C]` spoofing, Lua `loadstring` hooks, and
   simultaneous absence of both `unpack` and `table.unpack`) are rejected with
   explicit outcome classification (`REJECTED_TIMEOUT_FAIL_CLOSED` vs
   `REJECTED_ERROR`, never clean exit `rc == 0` or payload leak).
4. Differential controls (`--no-guard`) pass on guard-only tamper vectors,
   proving default `--guard` is non-vacuous and `--no-guard` prunes guard checks.
"""
import argparse
import concurrent.futures
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO = ROOT.parent
ANTI_MOD_RS = ROOT / "src" / "anti" / "mod.rs"


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
SILENT_SRC_CODE = "silent_out = 6 * 7\n"


def long_str(s: str) -> str:
    for lvl in range(1, 32):
        eq = "=" * lvl
        if f"]{eq}]" not in s:
            return f"[{eq}[\n{s}]{eq}]"
    raise RuntimeError("cannot wrap long string")


def build_harness(artifact_src: str, setup_lua: str, epilogue_lua: str = "") -> str:
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
{epilogue_lua}
"""


def parse_rust_stage_manifest(rs_path: Path) -> dict[str, list[str]]:
    text = rs_path.read_text()
    m = re.search(r"pub const STAGE_MANIFEST:\s*\[GuardStageMeta;\s*\d+\]\s*=\s*\[(.*?)\n\];", text, re.S)
    if not m:
        raise RuntimeError(f"cannot locate STAGE_MANIFEST in {rs_path}")
    body = m.group(1)
    stages: dict[str, list[str]] = {}
    for sm in re.finditer(r'id:\s*"([^"]+)".*?test_ids:\s*&\[(.*?)\]', body, re.S):
        stage_id = sm.group(1)
        ids = re.findall(r'"([^"]+)"', sm.group(2))
        stages[stage_id] = ids
    if not stages:
        raise RuntimeError("parsed 0 stages from STAGE_MANIFEST")
    return stages


def run_luau(code_path: Path, timeout_s: float = 4.5, readonly: bool = False):
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
    ap = argparse.ArgumentParser()
    ap.add_argument("--keep-harness-dir", type=Path, default=None, help="Persist generated harness scripts to this directory")
    args = ap.parse_args()

    tmp_ctx = tempfile.TemporaryDirectory(prefix="guard_matrix_")
    tmp = Path(tmp_ctx.name)
    try:
        src_file = tmp / "src.lua"
        silent_src_file = tmp / "silent_src.lua"
        art_guard = tmp / "art_guard.lua"
        art_noguard = tmp / "art_noguard.lua"
        art_silent_guard = tmp / "art_silent_guard.lua"
        src_file.write_text(SRC_CODE)
        silent_src_file.write_text(SILENT_SRC_CODE)

        subprocess.run(
            [str(OBSUCVIM), "--preset", "v15", "--dialect", "luau", "--seed", "4242", str(src_file), str(art_guard)],
            check=True,
        )
        subprocess.run(
            [str(OBSUCVIM), "--preset", "v15", "--no-guard", "--dialect", "luau", "--seed", "4242", str(src_file), str(art_noguard)],
            check=True,
        )
        subprocess.run(
            [str(OBSUCVIM), "--preset", "v15", "--dialect", "luau", "--seed", "4242", str(silent_src_file), str(art_silent_guard)],
            check=True,
        )
        guard_txt = art_guard.read_text()
        noguard_txt = art_noguard.read_text()
        silent_guard_txt = art_silent_guard.read_text()

        # (name, mode, setup_lua_or_None, readonly, expect_kind, expected_out)
        # expect_kind:
        #   "ALLOW"  -> rc == 0, not timed_out, stdout == expected_out
        #   "REJECT" -> stdout != expected_out AND (timed_out or rc != 0); never clean exit rc==0
        cases = [
            # --- 1. Direct stock & readonly-globals runs ---
            ("clean_stock_luau", "direct_guard", None, False, "ALLOW", EXPECTED),
            ("clean_stock_luau_guard_on", "direct_guard", None, False, "ALLOW", EXPECTED),
            ("clean_stock_luau_guard_off", "direct_noguard", None, False, "ALLOW", EXPECTED),
            ("readonly_globals", "direct_guard", None, True, "ALLOW", EXPECTED),
            ("readonly_globals_guard_on", "direct_guard", None, True, "ALLOW", EXPECTED),
            ("readonly_globals_guard_off", "direct_noguard", None, True, "ALLOW", EXPECTED),

            # --- 2. Optional APIs absent or redirected (must NOT false-positive) ---
            ("xpcall_nil_allowed", "guard", "ENV.xpcall = nil", False, "ALLOW", EXPECTED),
            ("getfenv_nil_allowed", "guard", "ENV.getfenv = nil", False, "ALLOW", EXPECTED),
            (
                "print_warn_nil_allowed",
                "custom",
                build_harness(
                    silent_guard_txt,
                    "ENV.getfenv = nil\nENV.print = nil\nENV.warn = nil",
                    'REAL_G.print("SILENT_OK", ENV.silent_out)',
                ),
                False,
                "ALLOW",
                "SILENT_OK\t42",
            ),
            ("warn_nil_allowed", "guard", "ENV.warn = nil", False, "ALLOW", EXPECTED),
            (
                "print_redirect_allowed",
                "custom",
                build_harness(
                    guard_txt,
                    """local cap = {}
ENV.getfenv = nil
ENV.print = function(...)
    local t = { ... }
    for i = 1, #t do t[i] = tostring(t[i]) end
    cap[#cap + 1] = table.concat(t, "\\t")
end""",
                    "REAL_G.print(cap[1])",
                ),
                False,
                "ALLOW",
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
                "ALLOW",
                EXPECTED,
            ),
            ("debug_nil_allowed", "guard", "ENV.debug = nil", False, "ALLOW", EXPECTED),
            ("debug_info_nil_allowed", "guard", "ENV.debug = { traceback = REAL_G.debug.traceback }", False, "ALLOW", EXPECTED),
            ("newproxy_nil_allowed", "guard", "ENV.newproxy = nil", False, "ALLOW", EXPECTED),
            ("os_nil_allowed", "guard", "ENV.os = nil", False, "ALLOW", EXPECTED),
            ("os_clock_nil_allowed", "guard", "ENV.os = {}", False, "ALLOW", EXPECTED),
            ("unpack_nil_allowed", "guard", "ENV.unpack = nil", False, "ALLOW", EXPECTED),
            (
                "table_unpack_nil_allowed",
                "guard",
                "ENV.table = setmetatable({}, { __index = function(_, k) if k == 'unpack' then return nil end return REAL_G.table[k] end })",
                False,
                "ALLOW",
                EXPECTED,
            ),
            ("next_nil_allowed", "guard", "ENV.next = nil", False, "ALLOW", EXPECTED),

            # --- 3. Active tamper & missing required base contracts (must REJECT / fail-closed) ---
            ("pcall_tampered_nonfn", "guard", "ENV.pcall = 123", False, "REJECT", EXPECTED),
            ("shadow_pcall_false_tamper", "guard", "ENV.pcall = false", False, "REJECT", EXPECTED),
            ("pcall_error_swallowed", "guard", "ENV.pcall = function(f, ...) if f == REAL_G.error then return true end return REAL_G.pcall(f, ...) end", False, "REJECT", EXPECTED),
            ("rawget_tampered", "guard", "ENV.rawget = function(t, k) if k == -271823 then return nil end return REAL_G.rawget(t, k) end", False, "REJECT", EXPECTED),
            (
                "shadow_rawget_hook_with_xpcall_nil",
                "guard",
                "ENV.xpcall = nil\nlocal rg = REAL_G.rawget\nENV.rawget = function(t, k) return rg(t, k) end",
                False,
                "REJECT",
                EXPECTED,
            ),
            ("xpcall_tampered_nonfn", "guard", "ENV.xpcall = 123", False, "REJECT", EXPECTED),
            ("xpcall_stub_broken", "guard", "ENV.xpcall = function() return true, 0 end", False, "REJECT", EXPECTED),
            ("getfenv_tampered_nonfn", "guard", "ENV.getfenv = 123", False, "REJECT", EXPECTED),
            ("warn_tampered_nonfn", "guard", 'ENV.warn = "bad_warn"', False, "REJECT", EXPECTED),
            ("debug_non_table_tamper", "guard", "ENV.debug = 123", False, "REJECT", EXPECTED),
            ("debug_info_nonfn_tamper", "guard", "ENV.debug = { info = 123 }", False, "REJECT", EXPECTED),
            (
                "debug_info_spoof_c",
                "guard",
                'local di = REAL_G.debug.info\nENV.debug = { info = function(f, w, ...) if w == "s" then return "[C]" end return di(f, w, ...) end }',
                False,
                "REJECT",
                EXPECTED,
            ),
            (
                "loadstring_lua_hook",
                "guard",
                "local ls = REAL_G.loadstring\nENV.loadstring = function(s, ...) return ls(s, ...) end",
                False,
                "REJECT",
                EXPECTED,
            ),
            (
                "loadstring_hook_with_env_debug_nil",
                "guard",
                "ENV.debug = nil\nlocal ls = REAL_G.loadstring\nENV.loadstring = function(s, ...) return ls(s, ...) end",
                False,
                "REJECT",
                EXPECTED,
            ),
            ("newproxy_tampered_nonfn", "guard", "ENV.newproxy = 99", False, "REJECT", EXPECTED),
            ("metatable_lock_tampered", "guard", "ENV.getmetatable = function() return nil end", False, "REJECT", EXPECTED),
            ("os_non_table_tamper", "guard", 'ENV.os = "bad_os"', False, "REJECT", EXPECTED),
            ("os_clock_nonfn_tamper", "guard", "ENV.os = { clock = 42 }", False, "REJECT", EXPECTED),
            (
                "table_unpack_tampered_nonfn",
                "guard",
                "ENV.unpack = nil\nENV.table = { concat = REAL_G.table.concat, pack = REAL_G.table.pack, unpack = 123 }",
                False,
                "REJECT",
                EXPECTED,
            ),
            (
                "both_unpack_nil_rejected",
                "guard",
                "ENV.unpack = nil\nENV.table = setmetatable({}, { __index = function(_, k) if k == 'unpack' then return nil end return REAL_G.table[k] end })",
                False,
                "REJECT",
                EXPECTED,
            ),
            ("unpack_broken_tamper", "guard", 'ENV.unpack = function() error("boom") end', False, "REJECT", EXPECTED),
            ("next_tampered_nonfn", "guard", "ENV.next = 123", False, "REJECT", EXPECTED),
            ("next_broken_tamper", "guard", "ENV.next = function() return 99, 99 end", False, "REJECT", EXPECTED),

            # --- 4. Differential control: guard-only vector passes under --no-guard ---
            ("differential_noguard_warn_nonfn", "noguard", 'ENV.warn = "bad_warn"', False, "ALLOW", EXPECTED),
            ("differential_noguard_newproxy_nonfn", "noguard", "ENV.newproxy = 99", False, "ALLOW", EXPECTED),
            ("differential_noguard_next_nonfn", "noguard", "ENV.next = 123", False, "ALLOW", EXPECTED),
        ]

        # Bidirectional STAGE_MANIFEST <-> cases check
        stage_map = parse_rust_stage_manifest(ANTI_MOD_RS)
        manifest_list = [tid for ids in stage_map.values() for tid in ids]
        case_list = [c[0] for c in cases]
        manifest_set = set(manifest_list)
        case_set = set(case_list)
        if len(manifest_list) != len(manifest_set):
            print("  [FAIL] stage_manifest_unique_ids: duplicate test_ids in STAGE_MANIFEST")
            return 1
        if len(case_list) != len(case_set):
            print("  [FAIL] guard_cases_unique_ids: duplicate case names in guard_matrix_test.py")
            return 1
        missing_in_py = sorted(manifest_set - case_set)
        missing_in_rs = sorted(case_set - manifest_set)
        if missing_in_py or missing_in_rs:
            print(f"  [FAIL] stage_manifest_bidirectional_sync: missing_in_py={missing_in_py}, missing_in_rs={missing_in_rs}")
            return 1
        print(f"  [PASS] stage_manifest_bidirectional_sync ({len(manifest_set)} test_ids across {len(stage_map)} stages)")

        def exec_case(idx_case):
            idx, (name, mode, setup, readonly, expect_kind, exp_out) = idx_case
            cpath = tmp / f"case_{idx:02d}_{name}.lua"
            if mode == "direct_guard":
                run_path = art_guard
            elif mode == "direct_noguard":
                run_path = art_noguard
            elif mode == "custom":
                cpath.write_text(setup)
                run_path = cpath
            elif mode == "guard":
                cpath.write_text(build_harness(guard_txt, setup))
                run_path = cpath
            elif mode == "noguard":
                cpath.write_text(build_harness(noguard_txt, setup))
                run_path = cpath
            else:
                raise ValueError(mode)

            rc, out, err, timed_out = run_luau(run_path, timeout_s=4.5, readonly=readonly)
            if timed_out:
                obs_class = "REJECTED_TIMEOUT_FAIL_CLOSED"
            elif rc != 0:
                obs_class = "REJECTED_ERROR"
            elif out == exp_out:
                obs_class = "OK"
            else:
                obs_class = "UNEXPECTED_CLEAN_EXIT"

            if expect_kind == "ALLOW":
                ok = obs_class == "OK"
                detail = f"class={obs_class} rc={rc} out={out!r}" + (f" err={err[:80]!r}" if err else "")
            elif expect_kind == "REJECT":
                ok = obs_class in ("REJECTED_TIMEOUT_FAIL_CLOSED", "REJECTED_ERROR") and exp_out not in out
                detail = f"class={obs_class} rc={rc} timeout={timed_out} script={run_path.name}"
                if not ok:
                    detail += f" SURVIVED out={out!r} err={err[:80]!r}"
            else:
                raise ValueError(expect_kind)
            return name, ok, obs_class, detail

        passed = 0
        failed = []
        class_counts: dict[str, int] = {}
        with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
            for name, ok, obs_class, detail in pool.map(exec_case, enumerate(cases)):
                class_counts[obs_class] = class_counts.get(obs_class, 0) + 1
                status = "PASS" if ok else "FAIL"
                print(f"  [{status}] {name}: {detail}")
                if ok:
                    passed += 1
                else:
                    failed.append((name, detail))

        if args.keep_harness_dir is not None:
            if args.keep_harness_dir.exists():
                shutil.rmtree(args.keep_harness_dir)
            shutil.copytree(tmp, args.keep_harness_dir)
            print(f"  [INFO] persisted harness scripts to {args.keep_harness_dir}")
        elif failed:
            persist = Path(tempfile.mkdtemp(prefix="guard_matrix_failed_"))
            shutil.copytree(tmp, persist, dirs_exist_ok=True)
            print(f"  [INFO] failure reproducer harnesses saved to {persist}")

        print(
            f"guard_matrix_test: {passed}/{len(cases)} PASS, {len(failed)} FAIL "
            f"(outcomes: {class_counts})"
        )
        return 1 if failed else 0
    finally:
        tmp_ctx.cleanup()


if __name__ == "__main__":
    sys.exit(main())
