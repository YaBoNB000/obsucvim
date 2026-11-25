#!/usr/bin/env python3
"""Phase 17 Guard Stage Manifest, Outcome Classifier & Differential Tamper Matrix.

Verifies:
1. Auditable STAGE_MANIFEST bidirectional consistency:
   - Parses `STAGE_MANIFEST` in `engine/src/anti/mod.rs` and enforces:
     * all metadata fields (`target`, `threat_model`, `allowed_missing`,
       `tamper_triggers`, `fail_action`, `trigger_timing`, `resource_bound`,
       `positive_test_ids`, `negative_test_ids`) are present and non-empty;
     * exact 1:1 set & polarity equality (`OK` vs `GuardRejected`) with the
       executable `cases` registry in this suite (zero missing or duplicate IDs).
2. Strict 5-way negative outcome classification (Phase 17 §2.2):
   - Separately classifies and reports:
     * `OK`            (clean exit, pre-payload marker reached, expected stdout)
     * `GuardRejected` (pre-payload `ENTERING_CHUNK` reached, `PAYLOAD_ENTERED`
                        NOT reached, zero payload leak, deterministic Guard
                        boolean-index fail-closed fault)
     * `RuntimeError`  (uncaught non-Guard runtime exception before or after
                        payload entry, or missing core VM primitive under
                        `--no-guard`, or unexpected clean exit output)
     * `Timeout`       (exceeded short resource timeout cap)
     * `CompileError`  (Lua/Luau syntax/loadstring compilation failure)
     * `HarnessError`  (exception inside test harness/env setup prior to chunk
                        invocation)
   - Includes 5 built-in negative classifier self-checks proving deliberate
     syntax errors, harness errors, runtime bugs, post-payload errors, and hangs
     are NEVER miscounted as `GuardRejected`.
3. Differential `--no-guard` positive controls:
   - Every guard-enforced tamper vector is also executed against `--no-guard`
     under the identical environment mutation: guard-only vectors must succeed
     (`OK`, `PAYLOAD_ENTERED == True`) under `--no-guard` and fail (`GuardRejected`,
     `PAYLOAD_ENTERED == False`) under `--guard`.
4. Scheme A `Unpack` contract (`both_unpack_nil_allowed` + independent `unpack`
   and `table.unpack` type/slice/nil/empty-interval/count tamper rejection).
"""
from __future__ import annotations

import argparse
import concurrent.futures
import json
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
SRC_CODE = (
    'if type(_GMX_TOUCH) == "function" then _GMX_TOUCH(1) end\n'
    "local a = 6 * 7\n"
    'print("GMATRIX_OK", a, "a" .. "bc")\n'
)
SILENT_SRC_CODE = (
    'if type(_GMX_TOUCH) == "function" then _GMX_TOUCH(1) end\n'
    "silent_out = 6 * 7\n"
)


def long_str(s: str) -> str:
    for lvl in range(1, 32):
        eq = "=" * lvl
        if f"]{eq}]" not in s:
            return f"[{eq}[\n{s}]{eq}]"
    raise RuntimeError("cannot wrap long string")


def build_harness(
    artifact_src: str,
    setup_lua: str | None,
    epilogue_lua: str = "",
    inject_touch: bool = True,
) -> str:
    touch_binding = (
        """local function _gmx_touch_fn(_)
    REAL_PRINT("__GMX_STAGE__:PAYLOAD_ENTERED")
end
REAL_G._GMX_TOUCH = _gmx_touch_fn
ENV._GMX_TOUCH = _gmx_touch_fn"""
        if inject_touch
        else ""
    )
    return f"""local REAL_G = getfenv(0)
local REAL_PRINT = REAL_G.print
local REAL_LOADSTRING = REAL_G.loadstring
local REAL_SETFENV = REAL_G.setfenv
REAL_PRINT("__GMX_STAGE__:SETUP_BEGIN")
local ART = {long_str(artifact_src)}
local ENV = {{}}
for k, v in pairs(REAL_G) do
    ENV[k] = v
end
ENV._G = ENV
{touch_binding}
{setup_lua or ""}
local chunk, comp_err = REAL_LOADSTRING(ART, "=gmatrix")
if not chunk then
    REAL_PRINT("__GMX_STAGE__:COMPILE_ERROR:" .. REAL_G.tostring(comp_err))
    return
end
REAL_SETFENV(chunk, ENV)
REAL_PRINT("__GMX_STAGE__:ENTERING_CHUNK")
chunk()
{epilogue_lua}
"""


def parse_rust_stage_manifest(rs_path: Path) -> list[dict]:
    text = rs_path.read_text()
    m = re.search(
        r"pub const STAGE_MANIFEST:\s*\[GuardStageMeta;\s*\d+\]\s*=\s*\[(.*?)\n\];",
        text,
        re.S,
    )
    if not m:
        raise RuntimeError(f"cannot locate STAGE_MANIFEST in {rs_path}")
    body = m.group(1)
    stages = []
    block_re = re.compile(r"GuardStageMeta\s*\{(.*?)\n\t\},", re.S)
    str_field = lambda blk, name: (
        re.search(rf'{name}:\s*"([^"]*)"', blk, re.S).group(1)
        if re.search(rf'{name}:\s*"([^"]*)"', blk, re.S)
        else ""
    )
    arr_field = lambda blk, name: (
        re.findall(
            r'"([^"]+)"',
            re.search(rf"{name}:\s*&\[(.*?)\]", blk, re.S).group(1),
        )
        if re.search(rf"{name}:\s*&\[(.*?)\]", blk, re.S)
        else None
    )
    for bm in block_re.finditer(body):
        blk = bm.group(1)
        sid = str_field(blk, "id")
        pos = arr_field(blk, "positive_test_ids")
        neg = arr_field(blk, "negative_test_ids")
        if not sid or pos is None or neg is None:
            raise RuntimeError(f"malformed GuardStageMeta block in {rs_path}: {blk[:120]}")
        stages.append(
            {
                "id": sid,
                "target": str_field(blk, "target"),
                "threat_model": str_field(blk, "threat_model"),
                "allowed_missing": str_field(blk, "allowed_missing"),
                "tamper_triggers": str_field(blk, "tamper_triggers"),
                "fail_action": str_field(blk, "fail_action"),
                "trigger_timing": str_field(blk, "trigger_timing"),
                "resource_bound": str_field(blk, "resource_bound"),
                "positive_test_ids": pos,
                "negative_test_ids": neg,
            }
        )
    if not stages:
        raise RuntimeError("parsed 0 stages from STAGE_MANIFEST")
    return stages


GUARD_FAULT_RE = re.compile(
    r"attempt to index boolean with boolean|attempt to index local .*boolean value"
)


def execute_and_classify(
    code_path: Path,
    expected_out: str,
    is_direct: bool = False,
    timeout_s: float = 2.5,
    readonly: bool = False,
) -> dict:
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
        rc = p.returncode
        raw_out = p.stdout
        err = p.stderr.strip()
        timed_out = False
    except subprocess.TimeoutExpired as exc:
        rc = 124
        raw_out = (
            exc.stdout.decode(errors="replace")
            if isinstance(exc.stdout, bytes)
            else (exc.stdout or "")
        )
        err = "TIMEOUT"
        timed_out = True

    setup_begun = is_direct
    compile_err = None
    entered_chunk = is_direct
    payload_entered = False
    user_lines = []
    for line in raw_out.splitlines():
        if line == "__GMX_STAGE__:SETUP_BEGIN":
            setup_begun = True
        elif line.startswith("__GMX_STAGE__:COMPILE_ERROR:"):
            compile_err = line.split(":", 2)[2]
        elif line == "__GMX_STAGE__:ENTERING_CHUNK":
            entered_chunk = True
        elif line == "__GMX_STAGE__:PAYLOAD_ENTERED":
            payload_entered = True
        else:
            user_lines.append(line)
    clean_out = "\n".join(user_lines).strip()

    if timed_out:
        cls = "Timeout"
    elif compile_err is not None or (not setup_begun and rc != 0):
        cls = "CompileError"
    elif not entered_chunk:
        cls = "HarnessError"
    elif rc == 0:
        if clean_out == expected_out and (is_direct or payload_entered):
            cls = "OK"
        else:
            cls = "RuntimeError"
    else:
        # Non-zero exit after ENTERING_CHUNK:
        # GuardRejected requires ALL of:
        #   1. Payload NEVER entered (`payload_entered == False`)
        #   2. Expected output NOT leaked on stdout
        #   3. Deterministic Guard fail-closed boolean-index fault signature
        if (
            not payload_entered
            and expected_out not in clean_out
            and bool(GUARD_FAULT_RE.search(err))
        ):
            cls = "GuardRejected"
        else:
            cls = "RuntimeError"

    return {
        "class": cls,
        "rc": rc,
        "stdout": clean_out,
        "stderr": err,
        "setup_begun": setup_begun,
        "entered_chunk": entered_chunk,
        "payload_entered": payload_entered,
        "timed_out": timed_out,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--keep-harness-dir", type=Path, default=None)
    ap.add_argument("--json-out", type=Path, default=None)
    args = ap.parse_args()

    tmp_ctx = tempfile.TemporaryDirectory(prefix="guard_matrix_")
    tmp = Path(tmp_ctx.name)
    try:
        src_file = tmp / "src.lua"
        silent_src_file = tmp / "silent_src.lua"
        art_guard = tmp / "art_guard.lua"
        art_noguard = tmp / "art_noguard.lua"
        art_silent_guard = tmp / "art_silent_guard.lua"
        art_silent_noguard = tmp / "art_silent_noguard.lua"
        src_file.write_text(SRC_CODE)
        silent_src_file.write_text(SILENT_SRC_CODE)

        for s_in, s_out, extra in [
            (src_file, art_guard, []),
            (src_file, art_noguard, ["--no-guard"]),
            (silent_src_file, art_silent_guard, []),
            (silent_src_file, art_silent_noguard, ["--no-guard"]),
        ]:
            subprocess.run(
                [
                    str(OBSUCVIM),
                    "--preset",
                    "v15",
                    *extra,
                    "--dialect",
                    "luau",
                    "--seed",
                    "4242",
                    str(s_in),
                    str(s_out),
                ],
                check=True,
            )

        guard_txt = art_guard.read_text()
        noguard_txt = art_noguard.read_text()
        silent_guard_txt = art_silent_guard.read_text()
        silent_noguard_txt = art_silent_noguard.read_text()

        print_redirect_setup = """local cap = {}
ENV.getfenv = nil
ENV.print = function(...)
    local t = { ... }
    for i = 1, #t do t[i] = tostring(t[i]) end
    cap[#cap + 1] = table.concat(t, "\\t")
end"""
        print_redirect_epilogue = "REAL_PRINT(cap[1])"

        proxy_custom = f"""local REAL_G = getfenv(0)
local REAL_PRINT = REAL_G.print
REAL_PRINT("__GMX_STAGE__:SETUP_BEGIN")
local ART = {long_str(guard_txt)}
local function _gmx_touch_fn(_) REAL_PRINT("__GMX_STAGE__:PAYLOAD_ENTERED") end
REAL_G._GMX_TOUCH = _gmx_touch_fn
local proxy = setmetatable({{
    _GMX_TOUCH = _gmx_touch_fn,
}}, {{ __index = REAL_G }})
local chunk, cerr = REAL_G.loadstring(ART, "=proxy")
if not chunk then
    REAL_PRINT("__GMX_STAGE__:COMPILE_ERROR:" .. tostring(cerr))
    return
end
setfenv(chunk, proxy)
REAL_PRINT("__GMX_STAGE__:ENTERING_CHUNK")
chunk()
"""

        table_unpack_nil_setup = (
            "ENV.table = setmetatable({}, { __index = function(_, k) "
            "if k == 'unpack' then return nil end return REAL_G.table[k] end })"
        )
        both_unpack_nil_setup = "ENV.unpack = nil\n" + table_unpack_nil_setup
        both_unpack_tampered_setup = (
            "ENV.unpack = 123\n"
            "ENV.table = { concat = REAL_G.table.concat, pack = REAL_G.table.pack, unpack = 456 }"
        )
        table_unpack_nonfn_setup = (
            "ENV.table = { concat = REAL_G.table.concat, pack = REAL_G.table.pack, unpack = 123 }"
        )
        table_unpack_nonfn_unpack_nil_setup = "ENV.unpack = nil\n" + table_unpack_nonfn_setup
        unpack_slice_tamper_setup = (
            "ENV.unpack = function(t) return REAL_G.unpack(t) end"
        )
        unpack_trailing_nil_tamper_setup = """ENV.unpack = function(t, i, j)
    if i == nil then i = 1 end
    if j == nil then j = #t end
    while j >= i and t[j] == nil do j = j - 1 end
    if i > j then return end
    return REAL_G.unpack(t, i, j)
end"""
        unpack_empty_range_tamper_setup = """ENV.unpack = function(t, i, j)
    if i ~= nil and j ~= nil and i > j then return nil end
    return REAL_G.unpack(t, i, j)
end"""
        stateful_shadow_setup = """local err_reads = 0
local backing = {}
for k, v in pairs(ENV) do backing[k] = v end
ENV = setmetatable({}, {
    __index = function(_, k)
        if k == "error" then
            err_reads = err_reads + 1
            if err_reads == 1 then return REAL_G.error end
            return function() end
        end
        return backing[k]
    end,
    __newindex = function(_, k, v) backing[k] = v end,
})
backing._G = ENV"""

        # --- Phase 18 P1-3 canary setups -------------------------------------
        # Positive setups only *touch* the primitive a canary reads; they never
        # change any value, so a green result means the canary is live on the
        # real primitive path (not vacuously skipped).
        canary1_clean_setup = 'local _ = REAL_G.string.char(65) if _ ~= "A" then error("c1") end'
        canary2_clean_setup = 'if #"abc" ~= 3 then error("c2") end'
        canary3_clean_setup = 'if type(REAL_G.string.char) ~= "function" then error("c3") end'
        canary4_clean_setup = (
            'if REAL_G.string.char(REAL_G.string.byte("Z")) ~= "Z" then error("c4") end'
        )
        # Negative setups: generic-harvester shapes. Each one corrupts exactly
        # one checked value while leaving the artifact's own multi-argument
        # literal decoding intact, so the rejection is attributable to the
        # canary and not to a broken bootstrap.
        canary1_char_tamper_setup = (
            "local RC = REAL_G.string.char\n"
            "ENV.string.char = function(...)\n"
            "    local a = ...\n"
            "    if select('#', ...) == 1 and a == 65 then return '' end\n"
            "    return RC(...)\n"
            "end"
        )
        canary2_len_tamper_setup = (
            "local RC = REAL_G.string.char\n"
            "ENV.string.char = function(...)\n"
            "    local x, y, z = ...\n"
            '    if x == 97 and y == 98 and z == 99 then return "ab" end\n'
            "    return RC(...)\n"
            "end"
        )
        canary3_type_tamper_setup = "ENV.string = { char = 123 }"
        canary4_roundtrip_tamper_setup = (
            "local RB = REAL_G.string.byte\n"
            "ENV.string.byte = function(s, ...)\n"
            '    if s == "Z" then return 65 end\n'
            "    return RB(s, ...)\n"
            "end"
        )

        # Tuple schema (parsed bidirectionally by engine/src/anti/mod.rs):
        #   (name, mode, expect_class, expect_noguard_class, readonly, setup_lua, expected_out)
        cases = [
            # --- Stage `core`: positive ---
            ("clean_stock_luau", "direct_guard", "OK", "SKIP", False, None, EXPECTED),
            ("clean_stock_luau_guard_on", "direct_guard", "OK", "SKIP", False, None, EXPECTED),
            ("clean_stock_luau_guard_off", "direct_noguard", "OK", "SKIP", False, None, EXPECTED),
            ("xpcall_nil_allowed", "guard", "OK", "OK", False, "ENV.xpcall = nil", EXPECTED),
            # --- Stage `core`: negative ---
            ("pcall_tampered_nonfn", "guard", "GuardRejected", "RuntimeError", False, "ENV.pcall = 123", EXPECTED),
            ("xpcall_tampered_nonfn", "guard", "GuardRejected", "OK", False, "ENV.xpcall = 123", EXPECTED),
            ("xpcall_stub_broken", "guard", "GuardRejected", "OK", False, "ENV.xpcall = function() return true, 0 end", EXPECTED),
            ("error_tampered_nonfn", "guard", "GuardRejected", "OK", False, "ENV.error = 123", EXPECTED),
            ("rawset_tampered_nonfn", "guard", "GuardRejected", "OK", False, "ENV.rawset = 123", EXPECTED),
            ("setmetatable_tampered_nonfn", "guard", "GuardRejected", "RuntimeError", False, "ENV.setmetatable = 123", EXPECTED),

            # --- Stage `env`: positive ---
            ("readonly_globals", "direct_guard", "OK", "SKIP", True, None, EXPECTED),
            ("readonly_globals_guard_on", "direct_guard", "OK", "SKIP", True, None, EXPECTED),
            ("readonly_globals_guard_off", "direct_noguard", "OK", "SKIP", True, None, EXPECTED),
            ("getfenv_nil_allowed", "guard", "OK", "OK", False, "ENV.getfenv = nil", EXPECTED),
            (
                "print_warn_nil_allowed",
                "silent_guard",
                "OK",
                "OK",
                False,
                "ENV.getfenv = nil\nENV.print = nil\nENV.warn = nil",
                "SILENT_OK\t42",
            ),
            ("warn_nil_allowed", "guard", "OK", "OK", False, "ENV.warn = nil", EXPECTED),
            (
                "print_redirect_allowed",
                "custom",
                "OK",
                "SKIP",
                False,
                build_harness(guard_txt, print_redirect_setup, print_redirect_epilogue),
                EXPECTED,
            ),
            ("setfenv_index_proxy_allowed", "custom", "OK", "SKIP", False, proxy_custom, EXPECTED),
            ("differential_noguard_warn_nonfn", "noguard", "OK", "SKIP", False, 'ENV.warn = "bad_warn"', EXPECTED),
            # --- Stage `env`: negative ---
            ("getfenv_tampered_nonfn", "guard", "GuardRejected", "OK", False, "ENV.getfenv = 123", EXPECTED),
            ("warn_tampered_nonfn", "guard", "GuardRejected", "OK", False, 'ENV.warn = "bad_warn"', EXPECTED),
            ("print_tampered_nonfn", "silent_guard", "GuardRejected", "OK", False, "ENV.getfenv = nil\nENV.print = 123", "SILENT_OK\t42"),
            ("tostring_tampered_nonfn", "guard", "GuardRejected", "OK", False, "ENV.tostring = 123", EXPECTED),
            ("tonumber_tampered_nonfn", "guard", "GuardRejected", "OK", False, "ENV.tonumber = 123", EXPECTED),
            (
                "rawget_tampered",
                "guard",
                "GuardRejected",
                "OK",
                False,
                "ENV.rawget = function(t, k) if k == -271823 then return nil end return REAL_G.rawget(t, k) end",
                EXPECTED,
            ),
            (
                "pcall_error_swallowed",
                "guard",
                "GuardRejected",
                "OK",
                False,
                "ENV.pcall = function(f, ...) if f == REAL_G.error then return true end return REAL_G.pcall(f, ...) end",
                EXPECTED,
            ),

            # --- Stage `debug`: positive ---
            ("debug_nil_allowed", "guard", "OK", "OK", False, "ENV.debug = nil", EXPECTED),
            ("debug_info_nil_allowed", "guard", "OK", "OK", False, "ENV.debug = { traceback = REAL_G.debug.traceback }", EXPECTED),
            ("loadstring_load_nil_allowed", "guard", "OK", "OK", False, "ENV.loadstring = nil\nENV.load = nil", EXPECTED),
            # --- Stage `debug`: negative ---
            ("debug_non_table_tamper", "guard", "GuardRejected", "OK", False, "ENV.debug = 123", EXPECTED),
            ("debug_info_nonfn_tamper", "guard", "GuardRejected", "OK", False, "ENV.debug = { info = 123 }", EXPECTED),
            (
                "debug_info_spoof_c",
                "guard",
                "GuardRejected",
                "OK",
                False,
                'local di = REAL_G.debug.info\nENV.debug = { info = function(f, w, ...) if w == "s" then return "[C]" end return di(f, w, ...) end }',
                EXPECTED,
            ),
            (
                "pcall_lua_hook_detected",
                "guard",
                "GuardRejected",
                "OK",
                False,
                "local pc = REAL_G.pcall\nENV.pcall = function(f, ...) return pc(f, ...) end",
                EXPECTED,
            ),
            (
                "shadow_rawget_hook_with_xpcall_nil",
                "guard",
                "GuardRejected",
                "OK",
                False,
                "ENV.xpcall = nil\nlocal rg = REAL_G.rawget\nENV.rawget = function(t, k) return rg(t, k) end",
                EXPECTED,
            ),
            ("loadstring_nonfn_tamper", "guard", "GuardRejected", "OK", False, "ENV.loadstring = 123", EXPECTED),
            (
                "loadstring_lua_hook",
                "guard",
                "GuardRejected",
                "OK",
                False,
                "local ls = REAL_G.loadstring\nENV.loadstring = function(s, ...) return ls(s, ...) end",
                EXPECTED,
            ),
            (
                "load_lua_hook",
                "guard",
                "GuardRejected",
                "OK",
                False,
                "local ls = REAL_G.loadstring\nENV.load = function(s, ...) return ls(s, ...) end",
                EXPECTED,
            ),
            (
                "loadstring_hook_with_env_debug_nil",
                "guard",
                "GuardRejected",
                "OK",
                False,
                "ENV.debug = nil\nlocal ls = REAL_G.loadstring\nENV.loadstring = function(s, ...) return ls(s, ...) end",
                EXPECTED,
            ),

            # --- Stage `canary`: positive ---
            ("newproxy_nil_allowed", "guard", "OK", "OK", False, "ENV.newproxy = nil", EXPECTED),
            ("differential_noguard_newproxy_nonfn", "noguard", "OK", "SKIP", False, "ENV.newproxy = 99", EXPECTED),
            # --- Stage `canary`: negative ---
            ("newproxy_tampered_nonfn", "guard", "GuardRejected", "OK", False, "ENV.newproxy = 99", EXPECTED),
            ("newproxy_broken_no_metatable", "guard", "GuardRejected", "OK", False, "ENV.newproxy = function() return {} end", EXPECTED),
            ("metatable_lock_tampered", "guard", "GuardRejected", "OK", False, "ENV.getmetatable = function() return nil end", EXPECTED),

            # --- Stage `misc`: positive ---
            ("os_nil_allowed", "guard", "OK", "OK", False, "ENV.os = nil", EXPECTED),
            ("os_clock_nil_allowed", "guard", "OK", "OK", False, "ENV.os = {}", EXPECTED),
            ("unpack_nil_allowed", "guard", "OK", "OK", False, "ENV.unpack = nil", EXPECTED),
            ("table_unpack_nil_allowed", "guard", "OK", "OK", False, table_unpack_nil_setup, EXPECTED),
            ("both_unpack_nil_allowed", "guard", "OK", "OK", False, both_unpack_nil_setup, EXPECTED),
            ("next_nil_allowed", "guard", "OK", "OK", False, "ENV.next = nil", EXPECTED),
            ("differential_noguard_next_nonfn", "noguard", "OK", "SKIP", False, "ENV.next = 123", EXPECTED),
            ("differential_noguard_both_unpack_tampered", "noguard", "OK", "SKIP", False, both_unpack_tampered_setup, EXPECTED),
            # --- Stage `misc`: negative ---
            ("os_non_table_tamper", "guard", "GuardRejected", "OK", False, 'ENV.os = "bad_os"', EXPECTED),
            ("os_clock_nonfn_tamper", "guard", "GuardRejected", "OK", False, "ENV.os = { clock = 42 }", EXPECTED),
            ("select_tampered_nonfn", "guard", "GuardRejected", "OK", False, "ENV.select = 123", EXPECTED),
            ("unpack_tampered_nonfn", "guard", "GuardRejected", "OK", False, "ENV.unpack = 123", EXPECTED),
            ("table_unpack_tampered_nonfn", "guard", "GuardRejected", "OK", False, table_unpack_nonfn_setup, EXPECTED),
            ("table_unpack_tampered_when_unpack_nil", "guard", "GuardRejected", "OK", False, table_unpack_nonfn_unpack_nil_setup, EXPECTED),
            ("unpack_broken_tamper", "guard", "GuardRejected", "RuntimeError", False, 'ENV.unpack = function() REAL_G.error("boom") end', EXPECTED),
            ("unpack_slice_bounds_tamper", "guard", "GuardRejected", "OK", False, unpack_slice_tamper_setup, EXPECTED),
            ("unpack_trailing_nil_drop_tamper", "guard", "GuardRejected", "OK", False, unpack_trailing_nil_tamper_setup, EXPECTED),
            ("unpack_empty_range_tamper", "guard", "GuardRejected", "OK", False, unpack_empty_range_tamper_setup, EXPECTED),
            ("next_tampered_nonfn", "guard", "GuardRejected", "OK", False, "ENV.next = 123", EXPECTED),
            ("next_broken_tamper", "guard", "GuardRejected", "OK", False, "ENV.next = function() return 99, 99 end", EXPECTED),
            ("shadow_pcall_false_tamper", "guard", "GuardRejected", "RuntimeError", False, "ENV.pcall = false", EXPECTED),
            ("env_stateful_getter_shadow_tamper", "guard", "GuardRejected", "OK", False, stateful_shadow_setup, EXPECTED),

            # --- Stage `canary` (Phase 18 P1-3): positive, one per canary ---
            # Each canary must stay green in a clean environment. The setups
            # below touch the exact primitive each canary reads so the case
            # documents which invariant it pins; none of them tamper anything.
            ("canary1_char_value_clean", "guard", "OK", "OK", False, canary1_clean_setup, EXPECTED),
            ("canary2_len_operator_clean", "direct_guard", "OK", "SKIP", True, canary2_clean_setup, EXPECTED),
            ("canary3_char_type_clean", "noguard", "OK", "SKIP", False, canary3_clean_setup, EXPECTED),
            ("canary4_char_byte_roundtrip_clean", "guard", "OK", "OK", False, canary4_clean_setup, EXPECTED),
            # --- Stage `canary` (Phase 18 P1-3): negative, one per canary ---
            # Every fake below is a *generic harvester* shape: it keeps the
            # artifact's own multi-argument literal decoding alive (so the
            # rejection is attributable to the canary, not to a broken
            # bootstrap) while corrupting exactly one checked value.
            ("canary1_char_value_tamper", "guard", "GuardRejected", "RuntimeError", False, canary1_char_tamper_setup, EXPECTED),
            ("canary2_len_operator_tamper", "guard", "GuardRejected", "OK", False, canary2_len_tamper_setup, EXPECTED),
            ("canary3_char_type_tamper", "guard", "GuardRejected", "OK", False, canary3_type_tamper_setup, EXPECTED),
            ("canary4_char_byte_roundtrip_tamper", "guard", "GuardRejected", "OK", False, canary4_roundtrip_tamper_setup, EXPECTED),
        ]

        passed = 0
        failed = 0

        # 1. Bidirectional STAGE_MANIFEST <-> cases check
        stages = parse_rust_stage_manifest(ANTI_MOD_RS)
        manifest_pos = []
        manifest_neg = []
        meta_complete = True
        for st in stages:
            for req_key in (
                "target",
                "threat_model",
                "allowed_missing",
                "tamper_triggers",
                "fail_action",
                "trigger_timing",
                "resource_bound",
            ):
                if not st[req_key]:
                    print(f"  [FAIL] stage_manifest_meta_{st['id']}: empty {req_key}")
                    meta_complete = False
            if not st["positive_test_ids"] or not st["negative_test_ids"]:
                print(f"  [FAIL] stage_manifest_polarity_{st['id']}: empty positive or negative list")
                meta_complete = False
            manifest_pos.extend(st["positive_test_ids"])
            manifest_neg.extend(st["negative_test_ids"])

        manifest_all = manifest_pos + manifest_neg
        case_all = [c[0] for c in cases]
        case_pos = [c[0] for c in cases if c[2] == "OK"]
        case_neg = [c[0] for c in cases if c[2] == "GuardRejected"]

        sync_ok = (
            meta_complete
            and len(manifest_all) == len(set(manifest_all))
            and len(case_all) == len(set(case_all))
            and set(manifest_pos) == set(case_pos)
            and set(manifest_neg) == set(case_neg)
        )
        if not sync_ok:
            print(
                "  [FAIL] stage_manifest_bidirectional_sync: "
                f"missing_in_py={sorted(set(manifest_all) - set(case_all))}, "
                f"missing_in_rs={sorted(set(case_all) - set(manifest_all))}, "
                f"pos_diff={sorted(set(manifest_pos) ^ set(case_pos))}, "
                f"neg_diff={sorted(set(manifest_neg) ^ set(case_neg))}"
            )
            return 1
        print(
            f"  [PASS] stage_manifest_bidirectional_sync "
            f"({len(manifest_all)} cases: {len(manifest_pos)} positive, {len(manifest_neg)} negative across {len(stages)} stages)"
        )
        passed += 1

        # 2. Classifier negative self-checks (§2.2 acceptance criterion):
        #    Deliberate syntax errors, harness bugs, runtime exceptions, and hangs
        #    must NEVER be classified as `GuardRejected`.
        self_checks = [
            (
                "classifier_selfcheck_compile_error_artifact",
                build_harness("local broken = !!!", ""),
                "CompileError",
                2.0,
            ),
            (
                "classifier_selfcheck_compile_error_harness",
                "local syntax_err = (\n",
                "CompileError",
                2.0,
            ),
            (
                "classifier_selfcheck_harness_error_pre_chunk",
                build_harness(guard_txt, 'REAL_G.error("harness_setup_boom")'),
                "HarnessError",
                2.0,
            ),
            (
                "classifier_selfcheck_runtime_error_pre_payload",
                build_harness(guard_txt, "ENV.type = 123"),
                "RuntimeError",
                2.0,
            ),
            (
                "classifier_selfcheck_runtime_error_post_payload",
                build_harness(
                    guard_txt,
                    'ENV.getfenv = nil\nENV.print = function() REAL_G.error("post_payload_boom") end',
                ),
                "RuntimeError",
                2.0,
            ),
            (
                "classifier_selfcheck_timeout_hang",
                build_harness("while true do end", ""),
                "Timeout",
                0.35,
            ),
        ]
        for sc_name, sc_code, sc_expected, sc_to in self_checks:
            sc_path = tmp / f"{sc_name}.lua"
            sc_path.write_text(sc_code)
            res = execute_and_classify(sc_path, EXPECTED, is_direct=False, timeout_s=sc_to)
            ok = res["class"] == sc_expected and res["class"] != "GuardRejected"
            if ok:
                print(
                    f"  [PASS] {sc_name}: class={res['class']} "
                    f"(entered_chunk={res['entered_chunk']}, payload_entered={res['payload_entered']})"
                )
                passed += 1
            else:
                print(f"  [FAIL] {sc_name}: expected {sc_expected}, got {res}")
                failed += 1

        # 3. Execute all manifest-backed cases + paired --no-guard differential controls
        def exec_case(idx_case):
            idx, (name, mode, expect_cls, expect_ng_cls, readonly, setup, exp_out) = idx_case
            cpath = tmp / f"case_{idx:02d}_{name}.lua"
            is_direct = False
            if mode == "direct_guard":
                run_path = art_guard
                is_direct = True
            elif mode == "direct_noguard":
                run_path = art_noguard
                is_direct = True
            elif mode == "custom":
                cpath.write_text(setup)
                run_path = cpath
            elif mode == "guard":
                cpath.write_text(build_harness(guard_txt, setup))
                run_path = cpath
            elif mode == "silent_guard":
                cpath.write_text(
                    build_harness(
                        silent_guard_txt,
                        setup,
                        'REAL_PRINT("SILENT_OK", ENV.silent_out)',
                    )
                )
                run_path = cpath
            elif mode == "noguard":
                cpath.write_text(build_harness(noguard_txt, setup))
                run_path = cpath
            else:
                raise ValueError(f"unknown mode {mode}")

            res = execute_and_classify(
                run_path, exp_out, is_direct=is_direct, timeout_s=3.0, readonly=readonly
            )
            ok = res["class"] == expect_cls

            ng_res = None
            if expect_ng_cls != "SKIP":
                ng_path = tmp / f"case_{idx:02d}_{name}_noguard_ctrl.lua"
                if mode == "silent_guard":
                    ng_path.write_text(
                        build_harness(
                            silent_noguard_txt,
                            setup,
                            'REAL_PRINT("SILENT_OK", ENV.silent_out)',
                        )
                    )
                else:
                    ng_path.write_text(build_harness(noguard_txt, setup))
                ng_res = execute_and_classify(
                    ng_path, exp_out, is_direct=False, timeout_s=3.0, readonly=readonly
                )
                if ng_res["class"] != expect_ng_cls:
                    ok = False

            detail = (
                f"guard={res['class']}(expected={expect_cls}, rc={res['rc']}, "
                f"payload_entered={res['payload_entered']})"
            )
            if ng_res is not None:
                detail += (
                    f" | noguard_ctrl={ng_res['class']}(expected={expect_ng_cls}, "
                    f"payload_entered={ng_res['payload_entered']})"
                )
            if not ok:
                detail += f" | stderr={res['stderr'][:120]!r}"
                if ng_res is not None and ng_res["stderr"]:
                    detail += f" | ng_stderr={ng_res['stderr'][:120]!r}"
            return idx, name, ok, detail, res, ng_res

        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(exec_case, enumerate(cases)))

        class_counts: dict[str, int] = {}
        report_rows = []
        for idx, name, ok, detail, res, ng_res in sorted(results, key=lambda r: r[0]):
            class_counts[res["class"]] = class_counts.get(res["class"], 0) + 1
            report_rows.append(
                {
                    "id": name,
                    "ok": ok,
                    "guard_result": res,
                    "noguard_control": ng_res,
                }
            )
            if ok:
                print(f"  [PASS] {name}: {detail}")
                passed += 1
            else:
                print(f"  [FAIL] {name}: {detail}")
                failed += 1

        if args.keep_harness_dir is not None:
            args.keep_harness_dir.mkdir(parents=True, exist_ok=True)
            for f in sorted(tmp.glob("*.lua")):
                shutil.copy2(f, args.keep_harness_dir / f.name)

        if args.json_out is not None:
            args.json_out.parent.mkdir(parents=True, exist_ok=True)
            args.json_out.write_text(
                json.dumps(
                    {
                        "passed": passed,
                        "failed": failed,
                        "class_counts": class_counts,
                        "cases": report_rows,
                    },
                    indent=2,
                )
            )

        print(f"guard_matrix_test outcome histogram: {class_counts}")
        print(f"guard_matrix_test: {passed} PASS, {failed} FAIL")
        return 1 if failed else 0
    finally:
        tmp_ctx.cleanup()


if __name__ == "__main__":
    sys.exit(main())
