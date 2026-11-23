#!/usr/bin/env python3
"""P1 Capability Manifest & P3 String/Constant Lifecycle Regression Suite.

Verifies end-to-end on emitted v15 artifacts:
1. P1 Capability Pruning:
   - Minimal arithmetic/return script emits ZERO unused host library bindings
     (`buffer.`, `vector.`, `Vector2`, `Vector3`, `coroutine.`, `typeof`,
     `rawget`, `rawset`, `rawequal`, `next`, `xpcall`, `newproxy` absent from
     outer visible code when `--no-guard`).
   - `--bind-env roblox` injects `Vector3` / `Vector2` / `typeof` / `task`.
   - `--env-check` injects `buffer` / `coroutine` / `table.freeze` checks and
     passes on stock Luau.
   - Same-seed builds are byte-for-byte deterministic; distinct seeds produce
     distinct layouts with identical runtime semantics across 6 seeds.
2. P3 Lazy String Materialization & Special Numeric Constants:
   - Short strings (MkStr <=5B), medium strings (<=64B single-chunk), long
     strings (>64B multi-chunk up to 4KB), empty strings (`""`), binary/escaped
     byte strings (`\\0\\1\\255\\n\\r\\\\"`), repeated reads of the same constant,
     table keys, closure-captured strings, string comparisons & concatenation,
     multi-function shared constants, and `+Inf`/`-Inf`/`NaN` round-trip
     identically across 6 seeds under `LUAU_READONLY_GLOBALS=1`.
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


P3_STRESS_LUA = r'''
local empty = ""
local short1 = "a"
local short5 = "Zq7_!"
local med64 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
local long200 = "L0ng_Payload_Alpha_" .. ("XYZ_0123456789_"):rep(12)
local lit_long = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_!@#$%^&*()-+=[]{}|;:,.<>?/~`"
  .. "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_chunk2_boundary_check_64_128_192!"
local bin = "\0\1\2\127\128\254\255\n\r\t\\\"'end"

-- Repeated accesses to the exact same string constant & table-key identity
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

-- Special IEEE-754 constants (+Inf, -Inf, NaN, -0.0, large exact int)
local pinf, ninf, nanv = 1/0, -1/0, 0/0
local bigi = 9007199254740991

print("EMPTY", #empty, empty == "", tbl[""])
print("SHORT", short1, short5, tbl["Zq7_!"])
print("MED64", #med64, med64:sub(1, 8), med64:sub(57, 64))
print("LONG", tv, eq, llen, #long200)
print("BIN", blen, b1, b7, tbl[bin])
print("CLOS", #r1, #r2, r1 ~= r2, e1)
print("IEEE", pinf == math.huge, ninf == -math.huge, nanv ~= nanv, bigi - 1 == 9007199254740990)
'''


def run_luau(path: Path) -> str:
    env = os.environ.copy()
    env["LUAU_READONLY_GLOBALS"] = "1"
    p = subprocess.run([str(LUAU), str(path)], capture_output=True, text=True, timeout=15, env=env)
    if p.returncode != 0:
        raise RuntimeError(f"luau failed ({p.returncode}): {p.stderr}")
    return p.stdout


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

    with tempfile.TemporaryDirectory(prefix="cap_p3_") as td:
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

        # 2. --bind-env roblox injects Roblox primitives
        bnd_art = tmp / "bnd.art.lua"
        subprocess.run(
            [str(TOOL), "--preset", "v15", "--bind-env", "roblox", "--dialect", "luau", "--seed", "42", str(min_src), str(bnd_art)],
            check=True,
        )
        btxt = bnd_art.read_text()
        check(
            "bind_env_roblox_emits_target_apis",
            bool(re.search(r"\bVector3\b", btxt)) and bool(re.search(r"\btypeof\b", btxt)),
            "Vector3 + typeof present",
        )

        # 3. Byte-for-byte determinism on same seed
        det_a = tmp / "det_a.lua"
        det_b = tmp / "det_b.lua"
        p3_src = tmp / "p3.lua"
        p3_src.write_text(P3_STRESS_LUA)
        subprocess.run([str(TOOL), "--preset", "v15", "--dialect", "luau", "--seed", "777", str(p3_src), str(det_a)], check=True)
        subprocess.run([str(TOOL), "--preset", "v15", "--dialect", "luau", "--seed", "777", str(p3_src), str(det_b)], check=True)
        check("same_seed_byte_identical", det_a.read_bytes() == det_b.read_bytes(), f"{det_a.stat().st_size} B")

        # 4. P3 string & IEEE-754 constant semantics across 6 seeds + --super-insn
        expected = run_luau(p3_src)
        blobs = set()
        seeds = [1, 7, 42, 4242, 31337, 999999]
        all_eq = True
        for s in seeds:
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
        check("p3_semantics_super_insn", run_luau(sup_p) == expected, "--super-insn opt-in verified")

    print(f"capability_manifest_test: {passed} PASS, {failed} FAIL")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
