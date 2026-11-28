#!/usr/bin/env python3
"""Phase 17 Capability Binding Unification, Scheme A Unpack Fallback & Constant Lifecycle Suite.

Verifies end-to-end on emitted artifacts:
1. Single-source-of-truth capability pruning & template binding:
   - Minimal `--no-guard` arithmetic return script emits ZERO unused host APIs
     (`buffer.`, `vector.`, `Vector2`, `Vector3`, `typeof`, `coroutine.`,
     `rawget`, `rawset`, `rawequal`, `xpcall`, `newproxy`, `next`, `table.pack`)
     and ZERO algebraic tautology predicates.
   - Emitted `v15` and `vm` artifacts carry the sealed 3-tier Scheme A
     fallback (`unpack -> table.unpack -> pure-Lua _uf`), the `getfenv`
     bootstrap fallback, and (when calls exist on Luau `v15`) the
     `table.pack` fallback, all resolved through numeric char-code
     names (`VmPrim::sealed_bind_expr`); no readable `table.unpack` /
     `table.pack` member path and no raw `unpack or table.unpack`
     bypass appears in `LoadStr` or anywhere else.
   - `--bind-env roblox` injects `Vector3` / `typeof`; `--env-check` runs cleanly
     on stock Luau; same-seed builds are byte-for-byte deterministic.
2. Runtime fallback equivalence across 6 seeds (`1, 7, 42, 4242, 31337, 999999`)
   and `guard=true` / `guard=false` (`--no-guard`) + `--preset vm`:
   - `getfenv == nil`
   - `unpack == nil` (`table.unpack` present)
   - `table.unpack == nil` (`unpack` present)
   - `both_unpack_nil` (`unpack == nil` AND `table.unpack == nil` -> exercises
     Scheme A pure-Lua `_uf` slice unpacker across 0-arg, 4-arg, 5-arg, 13-arg
     calls, intermediate & trailing `nil` multi-returns, `select("#", ...)`,
     tail calls, varargs, and >64B chunked string decoding)
   - `table.pack == nil`
   - `all_fallbacks_combined` (`getfenv == nil`, `unpack == nil`,
     `table.unpack == nil`, AND `table.pack == nil` simultaneously)
3. Lazy String Materialization & IEEE-754 Constants (`""`, `1..5B`, `64B`,
   `>64B`, binary `\0..\255`, shared closure constants, `0.0`, `-0.0` sign bit,
   `+Inf`, `-Inf`, `NaN`, `2^53 - 1`) across 6 seeds under `LUAU_READONLY_GLOBALS=1`
   + `--super-insn`.
"""
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO = ROOT.parent
TOOL = Path(os.environ.get("OBSUCVIM_BIN", str(ROOT / "target/release/obsucvim")))
LUAU = REPO / ".tools/bin/luau"
if not LUAU.exists():
    LUAU = Path("/home/user/tools/bin/luau")

sys.path.insert(0, str(ROOT / "tests"))
import artifact_profile as ap  # noqa: E402

SEEDS = [1, 7, 42, 4242, 31337, 999999]


P0_FALLBACK_WORKLOAD = r'''
local function zero_ret()
    return
end

local function zero_arg_call()
    return 19
end

local function sum13(a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13)
    return a1 + (a2 or 0) + a3 + a4 + a5 + (a6 or 0) + a7 + a8 + a9 + a10 + a11 + a12 + a13,
           a2 == nil and a6 == nil
end

local function multi_with_nils(tag)
    return tag, nil, 30, nil, nil
end

local function tail_caller(x)
    return multi_with_nils("T:" .. tostring(x))
end

local function tail_zero()
    return zero_ret()
end

local function vararg_echo(...)
    local n = select("#", ...)
    local parts = { tostring(n) }
    for i = 1, n do
        parts[#parts + 1] = tostring(select(i, ...))
    end
    return table.concat(parts, ",")
end

local id_tbl = { k = "val" }
local function id_fn() return 77 end
local function ret_identities()
    return id_tbl, id_fn
end

local a1, a2, a3, a4, a5 = multi_with_nils("M")
local c_multi = select("#", multi_with_nils("C"))
local c_tail = select("#", tail_caller(9))
local c_zero = select("#", zero_ret())
local c_tzero = select("#", tail_zero())
local z_val = zero_arg_call()
local s13, nils13 = sum13(1, nil, 3, 4, 5, nil, 7, 8, 9, 10, 11, 12, 13)
local t1, f1 = ret_identities()
local long_s = "Fallback_Long_String_0123456789_abcdefghijklmnopqrstuvwxyz_ABCDEFGHIJKLMNOPQRSTUVWXYZ_ChunkTail_End!"

print("NILS", a1, tostring(a2), a3, tostring(a4), tostring(a5), c_multi, c_tail, c_zero, c_tzero)
print("ARITY", z_val, s13, nils13)
print("VARG", vararg_echo(multi_with_nils("V")))
print("IDENT", t1 == id_tbl, f1 == id_fn, f1())
print("LONG", #long_s, long_s:sub(1, 20), long_s:sub(-14))
'''


P3_STRESS_LUA = r'''
local empty = ""
local short1 = "a"
local short5 = "Zq7_!"
local med64 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
local long200 = "L0ng_Payload_Alpha_" .. ("XYZ_0123456789_"):rep(12)
local lit_long = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_!@#$%^&*()-+=[]{}|;:,.<>?/~`"
  .. "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_chunk2_boundary_check_64_128_192!"
local bin = "\0\1\2\127\128\254\255\n\r\t\\\"'end"

local tbl = {}
tbl[lit_long] = 9001
tbl[bin] = 9002
tbl[empty] = 9003
tbl[short5] = 9004

local function reader_a(k)
    local s1 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_!@#$%^&*()-+=[]{}|;:,.<>?/~`"
      .. "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_chunk2_boundary_check_64_128_192!"
    return tbl[s1], s1 == k, #s1
end

local function make_closure(prefix)
    local captured = "shared_const_across_protos_0123456789_abcdefghijklmnopqrstuvwxyz_ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    return function(suffix)
        return prefix .. ":" .. captured .. ":" .. suffix, tbl[empty], #bin, string.byte(bin, 1), string.byte(bin, 7)
    end
end

local c1 = make_closure("P1")
local c2 = make_closure("P2")
local r1, e1, blen, b1, b7 = c1("S1")
local r2 = c2("S2")
local tv, eq, llen = reader_a(lit_long)

local pzero, nzero = 0.0, -0.0
local pinf, ninf, nanv = 1/0, -1/0, 0/0
local min_norm, nmin_norm = 2.2250738585072014e-308, -2.2250738585072014e-308
local bigi = 9007199254740991

print("EMPTY", #empty, empty == "", tbl[""])
print("SHORT", short1, short5, tbl["Zq7_!"])
print("MED64", #med64, med64:sub(1, 8), med64:sub(57, 64))
print("LONG", tv, eq, llen, #long200)
print("BIN", blen, b1, b7, tbl[bin])
print("CLOS", #r1, #r2, r1 ~= r2, e1)
print("IEEE", 1 / pzero == math.huge, 1 / nzero == -math.huge, pinf == math.huge, ninf == -math.huge, nanv ~= nanv, min_norm > 0, nmin_norm < 0, bigi - 1 == 9007199254740990)
'''


def long_str(s: str) -> str:
    for lvl in range(1, 32):
        eq = "=" * lvl
        if f"]{eq}]" not in s:
            return f"[{eq}[\n{s}]{eq}]"
    raise RuntimeError("cannot wrap long string")


def run_luau(path: Path, readonly: bool = True) -> str:
    env = os.environ.copy()
    if readonly:
        env["LUAU_READONLY_GLOBALS"] = "1"
    p = subprocess.run([str(LUAU), str(path)], capture_output=True, text=True, timeout=15, env=env)
    if p.returncode != 0:
        raise RuntimeError(f"luau failed ({p.returncode}) on {path}: {p.stderr}")
    return p.stdout


def run_in_sandbox(tmp: Path, tag: str, art_txt: str, env_setup: str) -> str:
    harness = tmp / f"sb_{tag}.lua"
    harness.write_text(
        f"""local ART = {long_str(art_txt)}
local REAL_G = getfenv(0)
local ENV = {{}}
for k, v in pairs(REAL_G) do
    ENV[k] = v
end
ENV._G = ENV
{env_setup}
local chunk = assert(REAL_G.loadstring(ART, {tag!r}))
setfenv(chunk, ENV)
chunk()
"""
    )
    return run_luau(harness, readonly=False).strip()


def main() -> int:
    passed = 0
    failed = 0

    def check(name: str, cond: bool, detail: str = ""):
        nonlocal passed, failed
        if cond:
            print(f"  [PASS] {name}" + (f" ({detail})" if detail else ""))
            passed += 1
        else:
            print(f"  [FAIL] {name}: {detail}")
            failed += 1

    with tempfile.TemporaryDirectory(prefix="cap_p0_p3_") as td:
        tmp = Path(td)

        # 1. Minimal capability pruning (--no-guard arithmetic return)
        min_src = tmp / "min.lua"
        min_src.write_text("local x = 10 + 32\nreturn x\n")
        min_art = tmp / "min.art.lua"
        subprocess.run(
            [str(TOOL), "--preset", "v15", "--no-guard", "--dialect", "luau", "--seed", "42", str(min_src), str(min_art)],
            check=True,
        )
        prof = ap.profile_existing_artifact(min_art, tools_bin=ap.find_tools_bin())
        ana = prof["ast"]
        preds = ana["predicates"]
        txt = min_art.read_text()
        banned_minimal = [
            r"\bbuffer\.",
            r"\bvector\.",
            r"\bVector[23]\b",
            r"\btypeof\b",
            r"\bcoroutine\.",
            r"\brawget\b",
            r"\brawset\b",
            r"\brawequal\b",
            r"\bxpcall\b",
            r"\bnewproxy\b",
            r"\bnext\b",
            r"\btable\.pack\b",
        ]
        leaked = [pat for pat in banned_minimal if re.search(pat, txt)]
        check(
            "minimal_script_prunes_unused_apis",
            not leaked and ana["global_api_count"] <= 26,
            f"global_apis={ana['global_api_count']}, leaked={leaked}",
        )
        check(
            "zero_algebraic_constant_if_predicates",
            preds["provably_const_int_parity"] == 0 and preds["domain_dependent_linear"] == 0,
            f"predicates={preds}",
        )

        # 2. Structural check on --preset vm (--no-body --no-strings --no-guard) & --preset v15
        fb_src = tmp / "fb_src.lua"
        fb_src.write_text(P0_FALLBACK_WORKLOAD)
        vm_art = tmp / "fb_vm.lua"
        vm_guard_art = tmp / "fb_vm_guard.lua"
        v15_guard_art = tmp / "fb_v15_guard.lua"
        v15_noguard_art = tmp / "fb_v15_noguard.lua"
        subprocess.run(
            [str(TOOL), "--preset", "vm", "--no-body", "--no-strings", "--no-guard", "--dialect", "luau", "--seed", "42", str(fb_src), str(vm_art)],
            check=True,
        )
        subprocess.run(
            [str(TOOL), "--preset", "vm", "--no-body", "--no-strings", "--dialect", "luau", "--seed", "42", str(fb_src), str(vm_guard_art)],
            check=True,
        )
        subprocess.run([str(TOOL), "--preset", "v15", "--dialect", "luau", "--seed", "42", str(fb_src), str(v15_guard_art)], check=True)
        subprocess.run([str(TOOL), "--preset", "v15", "--no-guard", "--dialect", "luau", "--seed", "42", str(fb_src), str(v15_noguard_art)], check=True)

        vm_txt = vm_art.read_text()
        vmg_txt = vm_guard_art.read_text()
        g_txt = v15_guard_art.read_text()
        ng_txt = v15_noguard_art.read_text()
        check(
            "vm_emits_scheme_a_unpack_and_getfenv_fallbacks_without_bypass",
            bool(re.search(r"\bgetfenv\b", vm_txt))
            and not bool(re.search(r"\btable\.unpack\b", vm_txt))
            and not bool(re.search(r"\bunpack\s+or\s+table\.unpack\b", vm_txt))
            and not bool(re.search(r"\btable\.pack\b", vm_txt))
            and not bool(re.search(r"\bnext\b", vm_txt)),
            "vm binds getfenv + sealed Scheme A unpack fallback, zero readable member paths",
        )
        check(
            "v15_emits_scheme_a_fallbacks_and_gates_next_on_guard",
            bool(re.search(r"\bgetfenv\b", g_txt))
            and "117,110,112,97,99,107" in g_txt.replace(" ", "")
            and "112,97,99,107" in g_txt.replace(" ", "")
            and not bool(re.search(r"\btable\.(unpack|pack)\b", g_txt))
            and "110,101,120,116" in g_txt.replace(" ", "")
            and "110,101,120,116" not in ng_txt.replace(" ", "")
            and not bool(re.search(r"\bnext\b", ng_txt)),
            "v15 binds sealed Scheme A fallbacks; next sealed-name present iff guard=true",
        )

        # 3. Runtime fallback equivalence across ALL 6 seeds (guard=true, guard=false, and --preset vm)
        expected_fb = run_luau(fb_src, readonly=False).strip()

        tbl_no_unpack = "setmetatable({}, { __index = function(_, k) if k == 'unpack' then return nil end return REAL_G.table[k] end })"
        tbl_no_pack = "setmetatable({}, { __index = function(_, k) if k == 'pack' then return nil end return REAL_G.table[k] end })"
        tbl_no_unpack_or_pack = (
            "setmetatable({}, { __index = function(_, k) "
            "if k == 'unpack' or k == 'pack' then return nil end return REAL_G.table[k] end })"
        )

        sandboxes = [
            ("getfenv_nil", "ENV.getfenv = nil"),
            ("unpack_nil", "ENV.unpack = nil"),
            ("table_unpack_nil", f"ENV.table = {tbl_no_unpack}"),
            ("both_unpack_nil_scheme_a", f"ENV.unpack = nil\nENV.table = {tbl_no_unpack}"),
            ("table_pack_nil", f"ENV.table = {tbl_no_pack}"),
            (
                "all_fallbacks_combined",
                f"ENV.getfenv = nil\nENV.unpack = nil\nENV.table = {tbl_no_unpack_or_pack}",
            ),
        ]

        fb_ok = True
        fb_fail_detail = ""
        for s in SEEDS:
            ag = tmp / f"fb_g_{s}.lua"
            ang = tmp / f"fb_ng_{s}.lua"
            subprocess.run([str(TOOL), "--preset", "v15", "--dialect", "luau", "--seed", str(s), str(fb_src), str(ag)], check=True)
            subprocess.run([str(TOOL), "--preset", "v15", "--no-guard", "--dialect", "luau", "--seed", str(s), str(fb_src), str(ang)], check=True)
            ag_t = ag.read_text()
            ang_t = ang.read_text()
            for sb_name, sb_setup in sandboxes:
                for mode_name, art_t in [("guard", ag_t), ("noguard", ang_t)]:
                    got_out = run_in_sandbox(tmp, f"{sb_name}_{mode_name}_{s}", art_t, sb_setup)
                    if got_out != expected_fb:
                        fb_ok = False
                        fb_fail_detail = f"seed={s} mode={mode_name} sb={sb_name}: got={got_out!r}, expected={expected_fb!r}"
                        break
                if not fb_ok:
                    break
            if not fb_ok:
                break
        check(
            "p0_runtime_fallbacks_6_seeds_guard_and_noguard",
            fb_ok,
            fb_fail_detail or f"{len(SEEDS)} seeds x 2 guard modes x {len(sandboxes)} sandboxes = {len(SEEDS)*2*len(sandboxes)} runs",
        )

        # Verify --preset vm (both --guard and --no-guard) in all_fallbacks_combined sandbox
        vm_ng_out = run_in_sandbox(tmp, "vm_ng_all", vm_txt, sandboxes[-1][1])
        vm_g_out = run_in_sandbox(tmp, "vm_g_all", vmg_txt, sandboxes[-1][1])
        check(
            "p0_runtime_fallbacks_preset_vm_guard_and_noguard",
            vm_ng_out == expected_fb and vm_g_out == expected_fb,
            "preset vm (--guard & --no-guard) with getfenv=nil + unpack=nil + table.unpack=nil + table.pack=nil",
        )

        # 4. --bind-env roblox & --env-check
        bnd_art = tmp / "bnd.art.lua"
        subprocess.run(
            [str(TOOL), "--preset", "v15", "--bind-env", "roblox", "--dialect", "luau", "--seed", "42", str(min_src), str(bnd_art)],
            check=True,
        )
        btxt = bnd_art.read_text()
        check(
            "bind_env_roblox_emits_target_apis",
            "86,101,99,116,111,114,51" in btxt.replace(" ", "")
            and "116,121,112,101,111,102" in btxt.replace(" ", "")
            and not bool(re.search(r"\bVector3\b", btxt))
            and not bool(re.search(r"\btypeof\b", btxt)),
            "Vector3 + typeof bound via sealed char-code names, no readable target API names",
        )
        envchk_art = tmp / "envchk.art.lua"
        p3_env_src = tmp / "p3_envchk.lua"
        p3_env_src.write_text(P3_STRESS_LUA)
        subprocess.run(
            [str(TOOL), "--preset", "v15", "--env-check", "--dialect", "luau", "--seed", "42", str(p3_env_src), str(envchk_art)],
            check=True,
        )
        check(
            "env_check_passes_on_stock_luau",
            run_luau(envchk_art, readonly=True) == run_luau(p3_env_src, readonly=True),
            "--env-check verified under LUAU_READONLY_GLOBALS=1",
        )

        # 5. Byte-for-byte determinism on same seed
        det_a = tmp / "det_a.lua"
        det_b = tmp / "det_b.lua"
        p3_src = tmp / "p3.lua"
        p3_src.write_text(P3_STRESS_LUA)
        subprocess.run([str(TOOL), "--preset", "v15", "--dialect", "luau", "--seed", "777", str(p3_src), str(det_a)], check=True)
        subprocess.run([str(TOOL), "--preset", "v15", "--dialect", "luau", "--seed", "777", str(p3_src), str(det_b)], check=True)
        check("same_seed_byte_identical", det_a.read_bytes() == det_b.read_bytes(), f"{det_a.stat().st_size} B")

        # 6. String, binary, closure-shared & IEEE-754 (including -0.0) semantics across 6 seeds + --super-insn + --preset vm
        expected = run_luau(p3_src)
        blobs = set()
        all_eq = True
        for s in SEEDS:
            out_p = tmp / f"p3_{s}.lua"
            subprocess.run([str(TOOL), "--preset", "v15", "--dialect", "luau", "--seed", str(s), str(p3_src), str(out_p)], check=True)
            blobs.add(out_p.read_bytes())
            got = run_luau(out_p)
            if got != expected:
                all_eq = False
                check(f"p3_semantics_seed_{s}", False, f"expected {expected!r}, got {got!r}")
        check("p3_semantics_6_seeds_readonly_globals", all_eq and len(blobs) == 6, f"6/6 seeds eq, {len(blobs)} distinct builds")

        sup_p = tmp / "p3_super.lua"
        subprocess.run([str(TOOL), "--preset", "v15", "--super-insn", "--dialect", "luau", "--seed", "42", str(p3_src), str(sup_p)], check=True)
        vm_p3 = tmp / "p3_vm.lua"
        subprocess.run([str(TOOL), "--preset", "vm", "--dialect", "luau", "--seed", "42", str(p3_src), str(vm_p3)], check=True)
        check(
            "p3_semantics_super_insn_and_preset_vm",
            run_luau(sup_p) == expected and run_luau(vm_p3) == expected,
            "--super-insn + --preset vm IEEE-754/-0.0/string semantics verified",
        )

    print(f"capability_manifest_test: {passed} PASS, {failed} FAIL")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
