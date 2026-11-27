#!/usr/bin/env python3
"""Phase 17 §4.2 Controlled Single-Point VM Artifact Tamper Matrix.

Builds a harmless, multi-feature repository fixture under production `v15`
(`target/release/obsucvim --preset v15 --dialect luau`) across multiple seeds
and performs deterministic, syntactically valid single-point mutations across
all five architectural protection surfaces:

  1. `vm_carrier_span`:
     - Live `[bw_slot] = [[...]]` base-94 outer carrier words (header & mid).
  2. `const_and_sealed_string_payload`:
     - `CDEC` lazy constant-proxy string/number stream-key selector (`CSJ`/`CNJ`),
     - `M_LOAD_STR` (`strseal`) string stream-cipher constant (`65521`),
     - Carrier word inside a 600-byte sealed string payload span.
  3. `opcode_and_operand_stream`:
     - `BSS` basic-block leader table `BL` (`{{1, K, ...}}`),
     - `BSS` per-proto opcode-dialect selector `S_IDX` (`{0..3, ...}`),
     - Final `[bw_slot]` carrier word carrying SoA opcode/operand streams.
  4. `dispatch_and_micro_wrapper`:
     - Micro-VM `MPF` sparse rank dispatch table `zC` (`{[wire]=rank, ...}`),
     - `HW2[(wire + AV) % 256] = function(...)` micro-wrapper registration,
     - Outer `v15` staging state-machine transition state return.
  5. `integrity_checksum_and_phase_machine`:
     - `HBOOT` 3-byte phase program (`_hp` polynomial checksum -> `HD1`/`HD2`),
     - `BSS` 3-byte phase program (`_bp` polynomial checksum -> 6-way `bz` fanout),
     - `BSS` dual branchless operand-sum table (`OS` -> `iz1`/`iz2`),
     - Outer `st_slots` key/checksum fragment table (`KSUM` / stream schedule).

Also executes explicit differential controls so ordinary syntax breakage and
unreferenced decoy/length-only junk mutations are NEVER miscounted as tamper
rejections:
  - `CleanExecution`      (unmodified baseline)
  - `DeadDataIgnored`     (outer camo long string, decoy fn, same-length `#str` junk)
  - `SyntaxCompileError`  (broken long-string delimiter caught by `luau-compile`)
  - `TamperRejectedFault` / `TamperRejectedPoisonedOutput` / `TamperRejectedTimeout`
"""
from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import re
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
LUAUC = find_bin(
    "LUAUC_BIN",
    [REPO / ".tools" / "bin" / "luau-compile", Path("/home/user/tools/bin/luau-compile")],
)

# Harmless, deterministic repo-internal workload exercising:
# - long >64B sealed strings (`strseal` + `CDEC`), short `MkStr` strings,
# - closures + upvalues, multi-return + tail calls, branching loops (multi-block `BL`),
# - tables, arithmetic, and exact output checksum.
FIXTURE_LUA = r'''
local sealed_blob = "SealedPayload_Alpha_0123456789_abcdefghijklmnopqrstuvwxyz_ABCDEFGHIJKLMNOPQRSTUVWXYZ_"
    .. "ChunkBoundary64_0123456789_abcdefghijklmnopqrstuvwxyz_ABCDEFGHIJKLMNOPQRSTUVWXYZ_"
    .. "ChunkBoundary128_0123456789_abcdefghijklmnopqrstuvwxyz_ABCDEFGHIJKLMNOPQRSTUVWXYZ_"
    .. "ChunkBoundary192_0123456789_abcdefghijklmnopqrstuvwxyz_ABCDEFGHIJKLMNOPQRSTUVWXYZ_"
    .. "ChunkBoundary256_0123456789_abcdefghijklmnopqrstuvwxyz_ABCDEFGHIJKLMNOPQRSTUVWXYZ_Omega!"

local function make_acc(seed_bias)
    local acc = seed_bias
    return function(step, limit)
        for i = 1, limit do
            if i % 3 == 0 then
                acc = (acc * 33 + i + step) % 1000003
            elseif i % 3 == 1 then
                acc = (acc + i * 17) % 1000003
            else
                acc = (acc - i + 1000003) % 1000003
            end
        end
        return acc, "ok", #sealed_blob
    end
end

local function tail_run(fn, a, b)
    return fn(a, b)
end

local runner = make_acc(4242)
local v1, tag, slen = tail_run(runner, 7, 25)
local s_head = sealed_blob:sub(1, 20)
local s_tail = sealed_blob:sub(-12)
print("VMTAMPER_OK", v1, tag, slen, s_head, s_tail)
'''


def swap_two_words_in_longstr(full_txt: str, body_start: int, body_end: int, word_idx_a: int, word_idx_b: int) -> tuple[str, int, str, str]:
    body = full_txt[body_start:body_end]
    lead_nl = 1 if body.startswith("\n") else 0
    raw = body[lead_nl:]
    n_words = len(raw) // 5
    if n_words < 3:
        raise RuntimeError(f"carrier body too short ({len(raw)} B)")
    wa = word_idx_a % n_words
    wb = word_idx_b % n_words
    if wa == wb:
        wb = (wa + 1) % n_words
    w1 = raw[wa * 5 : (wa + 1) * 5]
    # Pick a distinct word that contains no `]` or `=` hazard if delimiter is `[[...]]`
    for step in range(n_words):
        cand = (wb + step) % n_words
        w2 = raw[cand * 5 : (cand + 1) * 5]
        if cand != wa and w2 != w1 and "]" not in w1 and "]" not in w2:
            wb = cand
            break
    else:
        # Mutate single safe base-94 char inside wa
        ch = raw[wa * 5]
        new_ch = "A" if ch != "A" else "B"
        pos = body_start + lead_nl + wa * 5
        mutated = full_txt[:pos] + new_ch + full_txt[pos + 1 :]
        return mutated, pos, ch, new_ch

    if wa > wb:
        wa, wb = wb, wa
        w1, w2 = w2, w1
    p1 = body_start + lead_nl + wa * 5
    p2 = body_start + lead_nl + wb * 5
    mutated = (
        full_txt[:p1]
        + w2
        + full_txt[p1 + 5 : p2]
        + w1
        + full_txt[p2 + 5 :]
    )
    return mutated, p1, w1, w2


def build_mutations_for_artifact(art_txt: str, seed: int, expected_out: str = "") -> list[dict]:
    cases: list[dict] = []

    def add(case_id: str, surface: str, expected_group: str, pos: int, orig: str, repl: str, mutated: str, note: str = ""):
        assert mutated != art_txt, f"mutation {case_id} was a no-op!"
        cases.append(
            {
                "case_id": f"s{seed}_{case_id}",
                "seed": seed,
                "surface": surface,
                "expected_group": expected_group,
                "offset": pos,
                "orig_excerpt": orig[:48],
                "mutated_excerpt": repl[:48],
                "note": note,
                "code": mutated,
            }
        )

    # Control 0: Clean baseline
    cases.append(
        {
            "case_id": f"s{seed}_control_clean_baseline",
            "seed": seed,
            "surface": "control_clean",
            "expected_group": "CleanExecution",
            "offset": 0,
            "orig_excerpt": "<unmodified>",
            "mutated_excerpt": "<unmodified>",
            "note": "Unmodified production v15 artifact",
            "code": art_txt,
        }
    )

    # Locate numeric-keyed carrier slots `[N]=[=*[...]=*]` vs named camo decoy `name=[=*[...]=*]`
    ls_re = re.compile(r"(\[\d+\]|[A-Za-z_]\w*)=\[(=*)\[(.*?)\]\2\]", re.S)
    num_carriers = []
    named_decoys = []
    for m in ls_re.finditer(art_txt):
        key = m.group(1)
        b_start, b_end = m.start(3), m.end(3)
        if key.startswith("["):
            num_carriers.append((key, b_start, b_end, m.start(), m.end()))
        elif b_end - b_start >= 5000:
            named_decoys.append((key, b_start, b_end, m.start(), m.end()))

    if len(num_carriers) < 2 or not named_decoys:
        raise RuntimeError(
            f"seed={seed}: unexpected long-string layout: num_carriers={len(num_carriers)}, named_decoys={len(named_decoys)}"
        )

    # Sort numeric carriers by length descending so slot 0/1 have full 450B chunks
    num_by_size = sorted(num_carriers, key=lambda x: x[2] - x[1], reverse=True)

    # --- Surface 1: `vm_carrier_span` ---
    k0, bs0, be0, _, _ = num_by_size[0]
    m_s1a, pos_s1a, o_s1a, r_s1a = swap_two_words_in_longstr(art_txt, bs0, be0, 0, 5)
    add("carrier_slot0_head_swap", "vm_carrier_span", "TamperRejected", pos_s1a, o_s1a, r_s1a, m_s1a, f"outer carrier {k0} head words")

    k1, bs1, be1, _, _ = num_by_size[1]
    m_s1b, pos_s1b, o_s1b, r_s1b = swap_two_words_in_longstr(art_txt, bs1, be1, 7, 19)
    add("carrier_slot1_mid_swap", "vm_carrier_span", "TamperRejected", pos_s1b, o_s1b, r_s1b, m_s1b, f"outer carrier {k1} mid words")

    # --- Surface 2: `const_and_sealed_string_payload` ---
    # 2a. CDEC string/number constant stream-key branch `==<tm>[3]and <CSJ> or <CNJ>`
    m_cdec = re.search(r"==(\w+)\[3\]and (\w+) or (\w+)", art_txt)
    if not m_cdec:
        raise RuntimeError(f"seed={seed}: cannot locate CDEC CSJ/CNJ selector")
    orig_cdec = m_cdec.group(0)
    repl_cdec = f"=={m_cdec.group(1)}[3]and {m_cdec.group(3)} or {m_cdec.group(2)}"
    add(
        "cdec_const_stream_key_swap",
        "const_and_sealed_string_payload",
        "TamperRejected",
        m_cdec.start(),
        orig_cdec,
        repl_cdec,
        art_txt[: m_cdec.start()] + repl_cdec + art_txt[m_cdec.end() :],
        "swap CSJ/CNJ constant-proxy string stream step",
    )

    # 2b. M_LOAD_STR (`strseal`) stream-cipher modulus inside `257+`..`%65521` string unsealer
    m_strseal = re.search(r"\*257\+\w+\+\w+\+\w+\)%65521", art_txt)
    if not m_strseal:
        raise RuntimeError(f"seed={seed}: cannot locate M_LOAD_STR strseal stream cipher")
    orig_ss = m_strseal.group(0)
    repl_ss = orig_ss.replace("%65521", "%65519")
    add(
        "strseal_cipher_modulus_perturb",
        "const_and_sealed_string_payload",
        "TamperRejected",
        m_strseal.start(),
        orig_ss,
        repl_ss,
        art_txt[: m_strseal.start()] + repl_ss + art_txt[m_strseal.end() :],
        "perturb strseal LoadStr stream-cipher prime modulus",
    )

    # 2c. Carrier word inside middle carrier slot carrying the 340B+ sealed string constant
    k_mid, bs_mid, be_mid, _, _ = num_by_size[len(num_by_size) // 2]
    m_s2c, pos_s2c, o_s2c, r_s2c = swap_two_words_in_longstr(art_txt, bs_mid, be_mid, 12, 24)
    add("sealed_const_carrier_word_swap", "const_and_sealed_string_payload", "TamperRejected", pos_s2c, o_s2c, r_s2c, m_s2c, f"sealed-const carrier {k_mid}")

    # --- Surface 3: `opcode_and_operand_stream` ---
    # 3a. Basic-block start table `BL = {{1, ...}, ...}` + `S_IDX = {d0, d1, ...}`
    m_bl = re.search(
        r"local (\w+)=(\{\{1(?:,\d+)*\}(?:,\{1(?:,\d+)*\})*\});local (\w+)=\{(\d+)((?:,\d+)*)\}",
        art_txt,
    )
    if not m_bl:
        raise RuntimeError(f"seed={seed}: cannot locate BSS BL / S_IDX tables")
    bl_lit = m_bl.group(2)
    m_inner_blk = re.search(r"\{1,(\d+)", bl_lit)
    if not m_inner_blk:
        raise RuntimeError(f"seed={seed}: expected multi-block proto in BL={bl_lit}")
    old_bstart = int(m_inner_blk.group(1))
    new_bl_lit = (
        bl_lit[: m_inner_blk.start(1)]
        + str(old_bstart + 1)
        + bl_lit[m_inner_blk.end(1) :]
    )
    add(
        "bss_basic_block_leader_shift",
        "opcode_and_operand_stream",
        "TamperRejected",
        m_bl.start(2),
        bl_lit,
        new_bl_lit,
        art_txt[: m_bl.start(2)] + new_bl_lit + art_txt[m_bl.end(2) :],
        "shift basic-block leader offset in BSS BL",
    )

    # 3b. BSS `enc_lanes` 5-lane block stream packing (`bx[xo] = (pf.<LANE>[p] + ...)`):
    # swapping two lane field names swaps the opcode/operand streams inside every basic block `bx`.
    lane_matches = list(re.finditer(r"(\w+)\.(\w+)\[(\w+)\]\+[^;]{1,80}?%\s*65536\)", art_txt))
    if len(lane_matches) < 2:
        raise RuntimeError(f"seed={seed}: cannot locate >=2 BSS enc_lanes stream packers")
    lm0, lm1 = lane_matches[0], lane_matches[1]
    lane0, lane1 = lm0.group(2), lm1.group(2)
    mut_3b = (
        art_txt[: lm0.start(2)]
        + lane1
        + art_txt[lm0.end(2) : lm1.start(2)]
        + lane0
        + art_txt[lm1.end(2) :]
    )
    add(
        "bss_block_stream_lane_swap",
        "opcode_and_operand_stream",
        "TamperRejected",
        lm0.start(2),
        f"{lm0.group(1)}.{lane0} <-> {lm1.group(1)}.{lane1}",
        f"{lm0.group(1)}.{lane1} <-> {lm1.group(1)}.{lane0}",
        mut_3b,
        "swap two SoA/opcode lanes in BSS 5-word block stream packer (bx)",
    )

    # 3c. Tail carrier slot carrying SoA opcode/operand lanes
    k_last, bs_last, be_last, _, _ = num_carriers[-1]
    m_s3c, pos_s3c, o_s3c, r_s3c = swap_two_words_in_longstr(art_txt, bs_last, be_last, 1, 4)
    add("soa_tail_carrier_word_swap", "opcode_and_operand_stream", "TamperRejected", pos_s3c, o_s3c, r_s3c, m_s3c, f"tail carrier {k_last}")

    # --- Surface 4: `dispatch_and_micro_wrapper` ---
    # 4a. Micro-VM `zC` sparse rank dispatch tables `{[k1]=1,[k2]=2,...}` across all 5 MPF
    # wire dialects: cyclically shift every table's rank values by +1 mod M (zero fixed points).
    zc_matches = list(re.finditer(r"local \w+=\{((?:\[\d+\]=\d+,?){10,}\})", art_txt))
    if not zc_matches:
        raise RuntimeError(f"seed={seed}: cannot locate Micro-VM zC rank tables")
    mut_zc_txt = art_txt
    first_orig_zc = ""
    first_repl_zc = ""
    for m_zc in reversed(zc_matches):
        zc_body = m_zc.group(1)[:-1]
        zc_pairs = [p.split("=") for p in zc_body.split(",") if p]
        zc_keys = [kv[0] for kv in zc_pairs]
        zc_vals = [kv[1] for kv in zc_pairs]
        zc_vals_rot = zc_vals[1:] + zc_vals[:1]
        repl_zc_body = ",".join(f"{k}={v}" for k, v in zip(zc_keys, zc_vals_rot)) + "}"
        first_orig_zc = m_zc.group(1)[:48]
        first_repl_zc = repl_zc_body[:48]
        mut_zc_txt = mut_zc_txt[: m_zc.start(1)] + repl_zc_body + mut_zc_txt[m_zc.end(1) :]
    add(
        "micro_vm_zc_rank_derange",
        "dispatch_and_micro_wrapper",
        "TamperRejected",
        zc_matches[0].start(1),
        first_orig_zc,
        first_repl_zc,
        mut_zc_txt,
        f"cyclically derange all {len(zc_matches)} MPF dialect rank tables (zC)",
    )

    # 4b. `HW2[(wire + AV) % 256] = function(...)` micro-wrapper wire association
    m_hw2 = re.search(r"(\w+\[\()(\d+)(\+\w+\)%256\]=function\()", art_txt)
    if not m_hw2:
        raise RuntimeError(f"seed={seed}: cannot locate HW2 micro-wrapper registration")
    old_wire = int(m_hw2.group(2))
    new_wire = (old_wire + 1) % 256
    add(
        "hw2_micro_wrapper_wire_shift",
        "dispatch_and_micro_wrapper",
        "TamperRejected",
        m_hw2.start(),
        m_hw2.group(0),
        f"{m_hw2.group(1)}{new_wire}{m_hw2.group(3)}",
        art_txt[: m_hw2.start(2)] + str(new_wire) + art_txt[m_hw2.end(2) :],
        "shift HW2 wire-to-wrapper association by +1",
    )

    # 4c. Outer v15 staging chunk-loader state transition `C[idx]=...;return <state>,C,...`
    # The scaffold also emits inert decoy staging fields with the same textual
    # shape; field order is shuffled per build, so "first regex match" is not
    # necessarily on the execution path. Select the first match whose
    # perturbation is actually rejected (trial-run), keeping the assertion
    # itself unchanged: a *live* staging state perturbation must not execute
    # cleanly. If every match were inert, raise instead of weakening the case.
    staging_matches = list(
        re.finditer(r"((\w+)\[\d+\]=[^;]+;return )(\d+)(,\2(?:,\w+){4,8} end)", art_txt)
    )
    if not staging_matches:
        raise RuntimeError(f"seed={seed}: cannot locate outer v15 chunk-staging return")
    chosen = None
    for m_stg in staging_matches:
        old_st = int(m_stg.group(3))
        new_st = old_st + 1 if old_st < 230 else old_st - 1
        mutated = art_txt[: m_stg.start(3)] + str(new_st) + art_txt[m_stg.end(3) :]
        if not expected_out:
            chosen = (m_stg, new_st, mutated)
            break
        with tempfile.TemporaryDirectory(prefix="vm_tamper_probe_") as ptd:
            probe = Path(ptd) / "probe.lua"
            probe.write_text(mutated)
            try:
                run = subprocess.run(
                    [str(LUAU), str(probe)],
                    capture_output=True, text=True, encoding="utf-8", errors="replace",
                    timeout=3.5,
                )
                inert = run.returncode == 0 and run.stdout.strip() == expected_out
            except subprocess.TimeoutExpired:
                inert = False
        if not inert:
            chosen = (m_stg, new_st, mutated)
            break
    if chosen is None:
        raise RuntimeError(f"seed={seed}: all staging-state matches are inert decoys")
    m_stg, new_st, mutated_stg = chosen
    add(
        "outer_staging_dispatch_state_perturb",
        "dispatch_and_micro_wrapper",
        "TamperRejected",
        m_stg.start(3),
        m_stg.group(0)[-48:],
        f"{new_st}{m_stg.group(4)}",
        mutated_stg,
        "perturb live outer v15 staging state transition ID",
    )

    # --- Surface 5: `integrity_checksum_and_phase_machine` ---
    phase_matches = list(
        re.finditer(
            r"local (\w+)=\{(\d+),(\d+),(\d+)\};local (\w+)=0;\5=\(\5\*257\+",
            art_txt,
        )
    )
    if len(phase_matches) < 2:
        raise RuntimeError(f"seed={seed}: expected >=2 phase-machine arrays (BSS + HBOOT), got {len(phase_matches)}")

    # 5a & 5b: BSS phase array & HBOOT phase array
    for idx_pm, label in [(0, "bss_phase_program_tamper"), (1, "hboot_phase_program_tamper")]:
        pm = phase_matches[idx_pm]
        b0 = int(pm.group(2))
        nb0 = (b0 + 1) % 256
        add(
            label,
            "integrity_checksum_and_phase_machine",
            "TamperRejected",
            pm.start(2),
            pm.group(0),
            pm.group(0).replace(f"{{{b0},", f"{{{nb0},", 1),
            art_txt[: pm.start(2)] + str(nb0) + art_txt[pm.end(2) :],
            f"1-byte mutation in {label.split('_')[0].upper()} 3-phase program",
        )

    # 5c. BSS dual operand-sum table `OS`
    m_os = re.search(
        r"local (\w+)=\{(\([^}]+\))\};local (\w+)=\{(\([^}]+\))\};local function \w+\(",
        art_txt,
    )
    if not m_os:
        raise RuntimeError(f"seed={seed}: cannot locate BSS OS/OS2 integrity tables")
    first_os_comma = art_txt.find(",", m_os.start(2), m_os.end(2))
    ins_at = first_os_comma if first_os_comma != -1 else m_os.end(2)
    add(
        "bss_operand_checksum_os_perturb",
        "integrity_checksum_and_phase_machine",
        "TamperRejected",
        m_os.start(2),
        art_txt[m_os.start(2) : ins_at],
        art_txt[m_os.start(2) : ins_at] + "+1",
        art_txt[:ins_at] + "+1" + art_txt[ins_at:],
        "perturb OS[1] dual operand-sum integrity constant (+1)",
    )

    # 5d. Outer v15 staging key-checksum (`KSUM`) rolling Horner step `b:MOD(b:MUL(s,31)+...)`
    # Target step index 1 *of a Horner chain* (`k = 1`, where `s = ksum_0 > 0`), not the
    # second `*31` occurrence in the file: at `k = 0` the accumulator is still 0, so
    # `0*31 == 0*37` and the mutation is a no-op. Chains are identified by their
    # (object, accumulator) pair, which keeps the targeting stable no matter how many
    # independent chains the emitter happens to place before the live one.
    ks_matches = list(re.finditer(r"((\w+):\w+\(\2:\w+\((\w+),)(31)(\)\+)", art_txt))
    ks_chains: dict[tuple[str, str], list[int]] = {}
    for ks_i, ks_m in enumerate(ks_matches):
        ks_chains.setdefault((ks_m.group(2), ks_m.group(3)), []).append(ks_i)
    ks_chain = next((idxs for idxs in ks_chains.values() if len(idxs) >= 2), None)
    if ks_chain is None:
        raise RuntimeError(f"seed={seed}: cannot locate an outer staging KSUM Horner chain (>=2 *31 steps)")
    m_ksum = ks_matches[ks_chain[1]]
    add(
        "outer_key_ksum_horner_perturb",
        "integrity_checksum_and_phase_machine",
        "TamperRejected",
        m_ksum.start(4),
        m_ksum.group(0),
        f"{m_ksum.group(1)}37{m_ksum.group(5)}",
        art_txt[: m_ksum.start(4)] + "37" + art_txt[m_ksum.end(4) :],
        "perturb outer staging KSUM rolling accumulator multiplier (*31 -> *37 at k=1)",
    )

    # --- Differential Controls (Dead Data vs Syntax Damage) ---
    # Control 1: Outer named camo decoy long string (`hb_name=[=[...10500B...]=]`)
    dk, dbs, dbe, _, _ = named_decoys[0]
    m_dec1, pos_dec1, o_dec1, r_dec1 = swap_two_words_in_longstr(art_txt, dbs, dbe, 3, 25)
    add(
        "control_dead_camo_longstr",
        "control_dead_data",
        "DeadDataIgnored",
        pos_dec1,
        o_dec1,
        r_dec1,
        m_dec1,
        f"unreferenced outer camo decoy string `{dk}`",
    )

    # Control 2: Unreferenced numeric decoy function `[decoy]=function(x)local y=7;...`
    m_df = re.search(r"(\[\d+\]=function\(\w+\)local \w+=)(7)(;)", art_txt)
    if not m_df:
        raise RuntimeError(f"seed={seed}: cannot locate outer decoy function")
    add(
        "control_dead_decoy_function",
        "control_dead_data",
        "DeadDataIgnored",
        m_df.start(2),
        m_df.group(0),
        f"{m_df.group(1)}8{m_df.group(3)}",
        art_txt[: m_df.start(2)] + "8" + art_txt[m_df.end(2) :],
        "unreferenced outer decoy method slot",
    )

    # Control 3: Same-length `#"...junk..."` literal character flip
    m_jk = re.search(r'#\"([A-Za-z0-9]{10,24})\"', art_txt)
    if m_jk:
        j_old = m_jk.group(1)
        j_new = ("Z" if j_old[0] != "Z" else "Y") + j_old[1:]
        add(
            "control_dead_junk_strlen_char",
            "control_dead_data",
            "DeadDataIgnored",
            m_jk.start(1),
            f'#"{j_old}"',
            f'#"{j_new}"',
            art_txt[: m_jk.start(1)] + j_new + art_txt[m_jk.end(1) :],
            "same-length char swap inside length-only #str junk expression",
        )

    # Control 4: Syntax damage (inject unmatched `end end` right after carrier slot 0 delimiter)
    _, _, _, ls0_start, ls0_end = num_by_size[0]
    broken_syntax = art_txt[:ls0_end] + " end end " + art_txt[ls0_end:]
    add(
        "control_syntax_damage_delimiter",
        "control_syntax_damage",
        "SyntaxCompileError",
        ls0_end,
        art_txt[ls0_end - 6 : ls0_end],
        art_txt[ls0_end - 6 : ls0_end] + " end end ",
        broken_syntax,
        "inject unmatched `end end` after delimiter (must classify as SyntaxCompileError, NOT TamperRejected)",
    )

    return cases


def evaluate_case(tmp: Path, expected_out: str, item: dict) -> dict:
    p = tmp / f"{item['case_id']}.lua"
    p.write_text(item["code"])

    # Step 1: Explicit syntax check with `luau-compile` so syntax errors are
    # never conflated with runtime tamper rejection.
    comp = subprocess.run([str(LUAUC), str(p)], capture_output=True, timeout=10)
    if comp.returncode != 0:
        observed_class = "SyntaxCompileError"
        observed_group = "SyntaxCompileError"
        rc = comp.returncode
        out = ""
        err = comp.stderr.decode("utf-8", errors="replace").strip()
    else:
        try:
            run = subprocess.run(
                [str(LUAU), str(p)],
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=3.5,
            )
            rc = run.returncode
            out = run.stdout.strip()
            err = run.stderr.strip()
            if rc == 0 and out == expected_out:
                if item["expected_group"] == "CleanExecution":
                    observed_class = "CleanExecution"
                    observed_group = "CleanExecution"
                else:
                    observed_class = "DeadDataIgnored"
                    observed_group = "DeadDataIgnored"
            elif rc != 0 and expected_out not in out:
                observed_class = "TamperRejectedFault"
                observed_group = "TamperRejected"
            elif expected_out not in out:
                observed_class = "TamperRejectedPoisonedOutput"
                observed_group = "TamperRejected"
            else:
                observed_class = "TamperBypassedValidOutputLeaked"
                observed_group = "TamperBypassed"
        except subprocess.TimeoutExpired:
            rc = 124
            out = ""
            err = "TIMEOUT"
            observed_class = "TamperRejectedTimeout"
            observed_group = "TamperRejected"

    valid_output_leaked = out == expected_out or (expected_out in out and item["expected_group"] == "TamperRejected")
    ok = observed_group == item["expected_group"] and (
        not valid_output_leaked if item["expected_group"] == "TamperRejected" else True
    )
    return {
        "case_id": item["case_id"],
        "seed": item["seed"],
        "surface": item["surface"],
        "offset": item["offset"],
        "orig_excerpt": item["orig_excerpt"],
        "mutated_excerpt": item["mutated_excerpt"],
        "note": item["note"],
        "expected_group": item["expected_group"],
        "observed_class": observed_class,
        "observed_group": observed_group,
        "syntax_valid": comp.returncode == 0,
        "rc": rc,
        "valid_output_leaked": valid_output_leaked,
        "stdout_excerpt": out[:100],
        "stderr_excerpt": err[:120],
        "ok": ok,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seeds", default="42,7,31337", help="Comma-separated seeds")
    ap.add_argument("--json-out", type=Path, default=None)
    args = ap.parse_args()
    seeds = [int(x) for x in args.seeds.split(",") if x.strip()]

    with tempfile.TemporaryDirectory(prefix="vm_tamper_matrix_") as td:
        tmp = Path(td)
        src_path = tmp / "fixture.lua"
        src_path.write_text(FIXTURE_LUA)

        ref = subprocess.run([str(LUAU), str(src_path)], capture_output=True, text=True, check=True)
        expected_out = ref.stdout.strip()
        assert expected_out.startswith("VMTAMPER_OK\t"), f"unexpected fixture output: {expected_out!r}"

        all_items = []
        for s in seeds:
            art_path = tmp / f"art_s{s}.lua"
            subprocess.run(
                [
                    str(OBSUCVIM),
                    "--preset",
                    "v15",
                    "--dialect",
                    "luau",
                    "--seed",
                    str(s),
                    str(src_path),
                    str(art_path),
                ],
                check=True,
            )
            all_items.extend(build_mutations_for_artifact(art_path.read_text(), s, expected_out))

        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(lambda it: evaluate_case(tmp, expected_out, it), all_items))

        passed = 0
        failed = 0
        hist: dict[str, int] = {}
        surface_counts: dict[str, int] = {}
        for r in results:
            hist[r["observed_class"]] = hist.get(r["observed_class"], 0) + 1
            surface_counts[r["surface"]] = surface_counts.get(r["surface"], 0) + 1
            status = "PASS" if r["ok"] else "FAIL"
            print(
                f"  [{status}] {r['case_id']} ({r['surface']} @ +{r['offset']}): "
                f"expected={r['expected_group']} observed={r['observed_class']} "
                f"syntax_valid={r['syntax_valid']} rc={r['rc']}"
                + (f" stderr={r['stderr_excerpt']!r}" if not r["ok"] else "")
            )
            if r["ok"]:
                passed += 1
            else:
                failed += 1

        required_surfaces = {
            "vm_carrier_span",
            "const_and_sealed_string_payload",
            "opcode_and_operand_stream",
            "dispatch_and_micro_wrapper",
            "integrity_checksum_and_phase_machine",
            "control_clean",
            "control_dead_data",
            "control_syntax_damage",
        }
        missing_surfaces = sorted(required_surfaces - set(surface_counts))
        if missing_surfaces:
            print(f"  [FAIL] missing required tamper surfaces: {missing_surfaces}")
            failed += 1

        if args.json_out is not None:
            args.json_out.parent.mkdir(parents=True, exist_ok=True)
            args.json_out.write_text(
                json.dumps(
                    {
                        "seeds": seeds,
                        "expected_out": expected_out,
                        "passed": passed,
                        "failed": failed,
                        "observed_class_histogram": hist,
                        "surface_counts": surface_counts,
                        "results": results,
                    },
                    indent=2,
                )
            )

        print(f"vm_tamper_matrix surfaces: {surface_counts}")
        print(f"vm_tamper_matrix histogram: {hist}")
        print(f"vm_tamper_matrix_test: {passed} PASS, {failed} FAIL")
        return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
