#!/usr/bin/env python3
"""Same-Input / Same-Seed A/B Benchmark & Structural Comparison Driver (P2).

Compares a baseline `obsucvim` binary (built from an isolated git worktree at
`f583c2c3c9c66770c38169dc6547d10c85025555`) against the candidate working-tree
binary across:
  - 10 representative Luau corpora (micro print, basics, strings, escapes,
    closures/upvalues, control flow, metatables, vararg/multival, coroutines,
    and real algorithm)
  - 6 fixed seeds (`1, 7, 42, 4242, 31337, 999999`)
  - 4 preset modes (`v15`, `v15_noguard`, `vm`, `high`)
  - >= 3 sequential timing rounds per (binary, corpus, mode, seed) capturing
    median & p90 wall-clock ms and child Peak RSS (`ru_maxrss` via `wait4`)
  - Full AST structural profiling (`artifact_profile.analyze_luau_ast`) on all
    `v15` and `v15_noguard` artifacts (tokens, AST nodes, functions, branches,
    loops, unbound global APIs, constant predicates, wrapper signature clusters)
  - `luau-compile` validation, stock Luau 0.735 stdout/exit-code equivalence,
    same-seed byte-for-byte determinism, and 6-seed output diversity.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import platform
import re
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
REPO = ROOT.parent
sys.path.insert(0, str(ROOT / "tests"))
import artifact_profile as ap  # noqa: E402

SEEDS = [1, 7, 42, 4242, 31337, 999999]

CORPORA: list[tuple[str, str | None, str | None]] = [
    ("01_micro_print6", None, "print(6)\n"),
    ("02_basics", "cases/basics.lua", None),
    ("03_strings", "cases/strings.lua", None),
    ("04_luau_escapes", "cases/luau_escapes.lua", None),
    ("05_functions", "cases/functions.lua", None),
    ("06_control", "cases/control.lua", None),
    ("07_metatables", "cases/metatables.lua", None),
    ("08_multival", "cases/multival.lua", None),
    ("09_coroutine", "cases/coroutine.lua", None),
    ("10_real_algo", "cases/real_algo.lua", None),
]

MODES: list[tuple[str, list[str], bool]] = [
    ("v15", ["--preset", "v15", "--dialect", "luau"], True),
    ("v15_noguard", ["--preset", "v15", "--no-guard", "--dialect", "luau"], True),
    ("vm", ["--preset", "vm", "--dialect", "luau"], False),
    ("high", ["--preset", "high", "--dialect", "luau"], False),
]

def run_timed(cmd: list[str], timeout: float = 90.0) -> tuple[int, float, int, bytes, bytes]:
    m = ap.run_measured(cmd, timeout_sec=timeout)
    return m["rc"], float(m["wall_ms"]), int(m["peak_rss_kb"]), m["stdout"], m["stderr"]


def p90(vals: list[float]) -> float:
    if not vals:
        return 0.0
    s = sorted(vals)
    if len(s) == 1:
        return s[0]
    pos = 0.9 * (len(s) - 1)
    lo = int(pos)
    hi = min(lo + 1, len(s) - 1)
    frac = pos - lo
    return s[lo] + (s[hi] - s[lo]) * frac


def pct_delta(base: float, cand: float) -> float:
    if base == 0:
        return 0.0 if cand == 0 else 100.0
    return ((cand - base) / base) * 100.0


def host_metadata(
    tools_bin: Path,
    base_bin: Path,
    cand_bin: Path,
    base_sha: str,
    cand_sha: str,
) -> dict[str, Any]:
    cpu_model = platform.processor() or "unknown"
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.lower().startswith("model name"):
                cpu_model = line.split(":", 1)[1].strip()
                break
    except OSError:
        pass
    mem_total_kb = 0
    try:
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemTotal:"):
                mem_total_kb = int(line.split()[1])
                break
    except OSError:
        pass
    env = os.environ.copy()
    env["PATH"] = f"{tools_bin}:{env.get('PATH', '')}"
    rustc_v = subprocess.run(["rustc", "--version"], env=env, capture_output=True, text=True).stdout.strip()
    cargo_v = subprocess.run(["cargo", "--version"], env=env, capture_output=True, text=True).stdout.strip()
    git_branch = subprocess.run(["git", "branch", "--show-current"], cwd=REPO, capture_output=True, text=True).stdout.strip()
    git_status = subprocess.run(["git", "status", "--short"], cwd=REPO, capture_output=True, text=True).stdout.strip()

    luau_ver_note = "0.735 (built from luau-lang/luau tag 0.735 via engine/tools/rebuild_tools.sh)"
    return {
        "timestamp_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "os_kernel": f"{platform.system()} {platform.release()} ({platform.machine()})",
        "cpu_model": cpu_model,
        "cpu_logical_cores": os.cpu_count() or 1,
        "ram_total_mb": round(mem_total_kb / 1024.0, 1),
        "rustc_version": rustc_v,
        "cargo_version": cargo_v,
        "luau_bin": str(tools_bin / "luau"),
        "luau_version": luau_ver_note,
        "branch": git_branch,
        "baseline_commit": base_sha,
        "candidate_commit": cand_sha,
        "candidate_git_status": git_status,
        "baseline_bin_sha256": hashlib.sha256(base_bin.read_bytes()).hexdigest(),
        "candidate_bin_sha256": hashlib.sha256(cand_bin.read_bytes()).hexdigest(),
    }


def ast_worker(path_str: str) -> tuple[str, dict[str, Any]]:
    text = Path(path_str).read_bytes().decode("utf-8", errors="surrogateescape")
    a = ap.analyze_luau_ast(text)
    preds = a["predicates"]
    stg = a["wrappers"]["staging_7arg"]
    hw = a["wrappers"]["vm_hw_5arg"]
    return path_str, {
        "tokens": a["token_count"],
        "ast_nodes": a["ast_nodes"],
        "functions": a["functions"],
        "branches": a["branches"],
        "loops": a["loops"],
        "global_api_count": a["global_api_count"],
        "global_api_refs": a["global_api_refs"],
        "const_pred_total": preds["provably_const_total"],
        "const_pred_int_parity": preds["provably_const_int_parity"],
        "domain_dependent_linear": preds["domain_dependent_linear"],
        "identical_branch_arms": preds["identical_branch_arms"],
        "staging_7arg_total": stg["total"],
        "staging_7arg_unique": stg["unique_signatures"],
        "staging_top_cluster_pct": round(stg["top_signature_ratio"] * 100.0, 2),
        "vm_hw_5arg_total": hw["total"],
        "vm_hw_5arg_unique": hw["unique_signatures"],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline-bin", type=Path, required=True)
    parser.add_argument("--candidate-bin", type=Path, required=True)
    parser.add_argument("--baseline-sha", type=str, required=True)
    parser.add_argument("--candidate-sha", type=str, required=True)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--json-out", type=Path, required=True)
    parser.add_argument("--md-out", type=Path, required=True)
    args = parser.parse_args()

    tools_bin = ap.find_tools_bin()
    luau_bin = tools_bin / "luau"
    luau_compile_bin = tools_bin / "luau-compile"

    meta = host_metadata(
        tools_bin,
        args.baseline_bin,
        args.candidate_bin,
        args.baseline_sha,
        args.candidate_sha,
    )
    meta["rounds_per_point"] = args.rounds
    meta["seeds"] = SEEDS

    with tempfile.TemporaryDirectory(prefix="obsucvim_ab_") as td:
        tmp = Path(td)
        src_paths: dict[str, Path] = {}
        src_sizes: dict[str, int] = {}
        ref_runs: dict[str, dict[str, Any]] = {}

        # 1. Materialize inputs & measure stock Luau 0.735 reference output + runtime
        for cid, rel_path, inline_code in CORPORA:
            if rel_path is not None:
                sp = ROOT / "tests" / rel_path
                raw = sp.read_bytes()
            else:
                assert inline_code is not None
                sp = tmp / f"{cid}.lua"
                raw = inline_code.encode("utf-8")
                sp.write_bytes(raw)
            src_paths[cid] = sp
            src_sizes[cid] = len(raw)
            r_dts = []
            r_rss = []
            ref_out = b""
            for _ in range(args.rounds):
                rc, dt, rss, out, err = run_timed([str(luau_bin), str(sp)])
                if rc != 0:
                    raise RuntimeError(f"reference Luau failed on {cid}: {err.decode('utf-8', 'replace')}")
                r_dts.append(dt)
                r_rss.append(rss)
                ref_out = out
            ref_runs[cid] = {
                "input_bytes": len(raw),
                "stdout_sha256": hashlib.sha256(ref_out).hexdigest(),
                "stdout_bytes": ref_out,
                "luau_ms_median": round(statistics.median(r_dts), 3),
                "luau_ms_p90": round(p90(r_dts), 3),
                "luau_rss_kb_median": int(statistics.median(r_rss)),
            }

        bins = [
            ("baseline", args.baseline_bin),
            ("candidate", args.candidate_bin),
        ]

        records: dict[tuple[str, str, str, int], dict[str, Any]] = {}
        ast_targets: list[str] = []

        total_points = len(CORPORA) * len(MODES) * len(SEEDS) * len(bins)
        done_points = 0
        t_start = time.time()

        # 2. Sequential timing & correctness loop (zero CPU contention across timed subprocesses)
        for cid, _, _ in CORPORA:
            sp = src_paths[cid]
            exp_out = ref_runs[cid]["stdout_bytes"]
            for mode_name, mode_flags, do_ast in MODES:
                for side, bin_path in bins:
                    sha_by_seed: set[str] = set()
                    det_ok = True
                    for seed in SEEDS:
                        out_p = tmp / f"{side}__{cid}__{mode_name}__s{seed}.lua"
                        b_dts: list[float] = []
                        b_rss: list[int] = []
                        for _ in range(args.rounds):
                            rc, dt, rss, _, err = run_timed(
                                [str(bin_path), *mode_flags, "--seed", str(seed), str(sp), str(out_p)]
                            )
                            if rc != 0:
                                raise RuntimeError(
                                    f"{side} build failed on {cid} {mode_name} seed={seed}: {err.decode('utf-8', 'replace')}"
                                )
                            b_dts.append(dt)
                            b_rss.append(rss)

                        art_bytes = out_p.read_bytes()
                        art_sha = hashlib.sha256(art_bytes).hexdigest()
                        sha_by_seed.add(art_sha)

                        # Same-seed determinism check across all seeds
                        dup_p = tmp / f"{side}__{cid}__{mode_name}__s{seed}_dup.lua"
                        rc_d, _, _, _, _ = run_timed(
                            [str(bin_path), *mode_flags, "--seed", str(seed), str(sp), str(dup_p)]
                        )
                        if rc_d != 0 or dup_p.read_bytes() != art_bytes:
                            raise RuntimeError(
                                f"determinism failure for {side} {cid} {mode_name} seed={seed}: repeated build differed"
                            )

                        # Compile check via luau-compile
                        lc_p = subprocess.run(
                            [str(luau_compile_bin), str(out_p)],
                            stdout=subprocess.DEVNULL,
                            stderr=subprocess.PIPE,
                            timeout=30,
                        )
                        compile_ok = lc_p.returncode == 0
                        if not compile_ok:
                            raise RuntimeError(
                                f"luau-compile failed for {side} {cid} {mode_name} seed={seed}: {lc_p.stderr.decode('utf-8', 'replace')}"
                            )

                        # 3-round execution timing & stdout equivalence
                        r_dts = []
                        r_rss = []
                        sem_ok = True
                        for _ in range(args.rounds):
                            rc, dt, rss, out, err = run_timed([str(luau_bin), str(out_p)])
                            if rc != 0 or out != exp_out:
                                sem_ok = False
                                raise RuntimeError(
                                    f"semantic mismatch for {side} {cid} {mode_name} seed={seed}: rc={rc} err={err[:200]!r}"
                                )
                            r_dts.append(dt)
                            r_rss.append(rss)

                        rec = {
                            "path": str(out_p),
                            "output_bytes": len(art_bytes),
                            "sha256": art_sha,
                            "luau_compile_ok": compile_ok,
                            "semantic_eq_ok": sem_ok,
                            "same_seed_det_s42": det_ok,
                            "build_ms_samples": [round(x, 3) for x in b_dts],
                            "build_ms_median": round(statistics.median(b_dts), 3),
                            "build_ms_p90": round(p90(b_dts), 3),
                            "build_rss_kb_median": int(statistics.median(b_rss)),
                            "run_ms_samples": [round(x, 3) for x in r_dts],
                            "run_ms_median": round(statistics.median(r_dts), 3),
                            "run_ms_p90": round(p90(r_dts), 3),
                            "run_rss_kb_median": int(statistics.median(r_rss)),
                        }
                        records[(side, cid, mode_name, seed)] = rec
                        if do_ast:
                            ast_targets.append(str(out_p))

                        done_points += 1
                    if len(sha_by_seed) != len(SEEDS):
                        raise RuntimeError(
                            f"seed diversity failure on {side} {cid} {mode_name}: only {len(sha_by_seed)}/{len(SEEDS)} unique outputs"
                        )
            elapsed = time.time() - t_start
            print(f"[ab] timing & equivalence: {cid} complete ({done_points}/{total_points}, {elapsed:.1f}s)", flush=True)

        # 3. Parallel deterministic AST structural profiling on all v15 & v15_noguard artifacts
        print(f"[ab] running AST structural analysis on {len(ast_targets)} artifacts...", flush=True)
        ast_by_path: dict[str, dict[str, Any]] = {}
        with concurrent.futures.ProcessPoolExecutor(max_workers=min(6, os.cpu_count() or 4)) as pool:
            for p_str, a_info in pool.map(ast_worker, ast_targets):
                ast_by_path[p_str] = a_info

        for key, rec in records.items():
            if rec["path"] in ast_by_path:
                rec["ast"] = ast_by_path[rec["path"]]
            del rec["path"]

        # 4. Aggregate per-(corpus, mode) summaries across 6 seeds
        summary: dict[str, Any] = {}
        for cid, _, _ in CORPORA:
            in_b = src_sizes[cid]
            summary[cid] = {
                "input_bytes": in_b,
                "stock_luau_ms_median": ref_runs[cid]["luau_ms_median"],
                "stock_luau_ms_p90": ref_runs[cid]["luau_ms_p90"],
                "modes": {},
            }
            for mode_name, _, do_ast in MODES:
                m_entry: dict[str, Any] = {}
                for side, _ in bins:
                    s_recs = [records[(side, cid, mode_name, s)] for s in SEEDS]
                    out_b_list = [r["output_bytes"] for r in s_recs]
                    b_med_list = [r["build_ms_median"] for r in s_recs]
                    b_p90_list = [r["build_ms_p90"] for r in s_recs]
                    r_med_list = [r["run_ms_median"] for r in s_recs]
                    r_p90_list = [r["run_ms_p90"] for r in s_recs]
                    b_rss_list = [r["build_rss_kb_median"] for r in s_recs]
                    r_rss_list = [r["run_rss_kb_median"] for r in s_recs]

                    side_sum: dict[str, Any] = {
                        "output_bytes_median": int(statistics.median(out_b_list)),
                        "output_bytes_p90": int(round(p90([float(x) for x in out_b_list]))),
                        "expansion_ratio_median": round(statistics.median(out_b_list) / max(1, in_b), 1),
                        "build_ms_median": round(statistics.median(b_med_list), 2),
                        "build_ms_p90": round(p90(b_p90_list), 2),
                        "build_rss_kb_median": int(statistics.median(b_rss_list)),
                        "run_ms_median": round(statistics.median(r_med_list), 2),
                        "run_ms_p90": round(p90(r_p90_list), 2),
                        "run_rss_kb_median": int(statistics.median(r_rss_list)),
                        "luau_compile_pass": f"{sum(1 for r in s_recs if r['luau_compile_ok'])}/{len(s_recs)}",
                        "semantic_eq_pass": f"{sum(1 for r in s_recs if r['semantic_eq_ok'])}/{len(s_recs)}",
                        "same_seed_det": all(r["same_seed_det_s42"] for r in s_recs),
                        "distinct_seed_builds": len({r["sha256"] for r in s_recs}),
                    }
                    if do_ast:
                        side_sum.update(
                            {
                                "tokens_median": int(statistics.median([r["ast"]["tokens"] for r in s_recs])),
                                "ast_nodes_median": int(statistics.median([r["ast"]["ast_nodes"] for r in s_recs])),
                                "functions_median": int(statistics.median([r["ast"]["functions"] for r in s_recs])),
                                "branches_median": int(statistics.median([r["ast"]["branches"] for r in s_recs])),
                                "loops_median": int(statistics.median([r["ast"]["loops"] for r in s_recs])),
                                "global_api_count_median": float(statistics.median([r["ast"]["global_api_count"] for r in s_recs])),
                                "const_pred_total_median": float(statistics.median([r["ast"]["const_pred_total"] for r in s_recs])),
                                "const_pred_int_parity_median": float(statistics.median([r["ast"]["const_pred_int_parity"] for r in s_recs])),
                                "domain_linear_median": float(statistics.median([r["ast"]["domain_dependent_linear"] for r in s_recs])),
                                "staging_top_cluster_pct_median": round(statistics.median([r["ast"]["staging_top_cluster_pct"] for r in s_recs]), 2),
                                "per_seed": {str(s): records[(side, cid, mode_name, s)] for s in SEEDS},
                            }
                        )
                    else:
                        side_sum["per_seed"] = {
                            str(s): {
                                "output_bytes": records[(side, cid, mode_name, s)]["output_bytes"],
                                "sha256": records[(side, cid, mode_name, s)]["sha256"],
                                "build_ms_median": records[(side, cid, mode_name, s)]["build_ms_median"],
                                "run_ms_median": records[(side, cid, mode_name, s)]["run_ms_median"],
                            }
                            for s in SEEDS
                        }
                    m_entry[side] = side_sum

                b_s = m_entry["baseline"]
                c_s = m_entry["candidate"]
                m_entry["delta"] = {
                    "output_bytes_pct": round(pct_delta(b_s["output_bytes_median"], c_s["output_bytes_median"]), 2),
                    "build_ms_pct": round(pct_delta(b_s["build_ms_median"], c_s["build_ms_median"]), 2),
                    "run_ms_pct": round(pct_delta(b_s["run_ms_median"], c_s["run_ms_median"]), 2),
                }
                if do_ast:
                    m_entry["delta"].update(
                        {
                            "tokens_pct": round(pct_delta(b_s["tokens_median"], c_s["tokens_median"]), 2),
                            "ast_nodes_pct": round(pct_delta(b_s["ast_nodes_median"], c_s["ast_nodes_median"]), 2),
                        }
                    )
                summary[cid]["modes"][mode_name] = m_entry

        payload = {
            "environment": meta,
            "corpora": summary,
        }
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(payload, indent=2) + "\n")

        # 5. Render verifiable Markdown report
        md: list[str] = []
        md.append("# Same-Input / Same-Seed A/B Benchmark Report (`f583c2c3` vs Working Branch)")
        md.append("")
        md.append("## 1. Environment & Binary Provenance")
        md.append("")
        md.append(f"- **Timestamp (UTC)**: `{meta['timestamp_utc']}`")
        md.append(f"- **OS / Kernel**: `{meta['os_kernel']}`")
        md.append(f"- **CPU / Cores / RAM**: `{meta['cpu_model']}` ({meta['cpu_logical_cores']} cores, `{meta['ram_total_mb']} MB` RAM)")
        md.append(f"- **Rust Toolchain**: `{meta['rustc_version']}` / `{meta['cargo_version']}`")
        md.append(f"- **Luau Runtime**: `{meta['luau_bin']}` — `{meta['luau_version']}`")
        md.append(f"- **Baseline Commit**: `{meta['baseline_commit']}` (`sha256:{meta['baseline_bin_sha256'][:16]}...`)")
        md.append(f"- **Candidate Commit (`{meta['branch']}`)**: `{meta['candidate_commit']}` (`sha256:{meta['candidate_bin_sha256'][:16]}...`)")
        md.append(f"- **Seeds (6)**: `{SEEDS}` | **Timing Rounds per Point**: `{args.rounds}` (sequential `wait4` measurement)")
        md.append("")
        md.append("## 2. Default `v15` (`--guard` ON) — 6-Seed Medians & p90s")
        md.append("")
        md.append("| Corpus | In (B) | Base Out (B) | Cand Out (B) | Δ Bytes | Base->Cand Tokens | Base->Cand AST | APIs (B->C) | ConstP / Par+Lin (B->C) | StgTop% (B->C) | Build ms Med/p90 (B->C) | Run ms Med/p90 (B->C) | Peak RSS KB (B->C) | Eq / Det / Rnd |")
        md.append("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
        for cid, _, _ in CORPORA:
            c = summary[cid]
            m = c["modes"]["v15"]
            b, k, d = m["baseline"], m["candidate"], m["delta"]
            b_pl = b["const_pred_int_parity_median"] + b["domain_linear_median"]
            k_pl = k["const_pred_int_parity_median"] + k["domain_linear_median"]
            md.append(
                f"| `{cid}` | {c['input_bytes']} | {b['output_bytes_median']:,} | {k['output_bytes_median']:,} | "
                f"{d['output_bytes_pct']:+.1f}% | {b['tokens_median']:,}->{k['tokens_median']:,} ({d['tokens_pct']:+.1f}%) | "
                f"{b['ast_nodes_median']:,}->{k['ast_nodes_median']:,} ({d['ast_nodes_pct']:+.1f}%) | "
                f"{b['global_api_count_median']:.0f}->{k['global_api_count_median']:.0f} | "
                f"{b['const_pred_total_median']:.1f}({b_pl:.1f})->{k['const_pred_total_median']:.1f}({k_pl:.1f}) | "
                f"{b['staging_top_cluster_pct_median']:.1f}%->{k['staging_top_cluster_pct_median']:.1f}% | "
                f"{b['build_ms_median']:.1f}/{b['build_ms_p90']:.1f}->{k['build_ms_median']:.1f}/{k['build_ms_p90']:.1f} | "
                f"{b['run_ms_median']:.1f}/{b['run_ms_p90']:.1f}->{k['run_ms_median']:.1f}/{k['run_ms_p90']:.1f} | "
                f"{b['run_rss_kb_median']}->{k['run_rss_kb_median']} | "
                f"{k['semantic_eq_pass']}/{'Y' if k['same_seed_det'] else 'N'}/{k['distinct_seed_builds']} |"
            )

        md.append("")
        md.append("## 3. `v15 --no-guard` — 6-Seed Medians & p90s")
        md.append("")
        md.append("| Corpus | Base Out (B) | Cand Out (B) | Δ Bytes | Base->Cand Tokens | APIs (B->C) | Build ms Med/p90 (B->C) | Run ms Med/p90 (B->C) | Eq / Det / Rnd |")
        md.append("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
        for cid, _, _ in CORPORA:
            m = summary[cid]["modes"]["v15_noguard"]
            b, k, d = m["baseline"], m["candidate"], m["delta"]
            md.append(
                f"| `{cid}` | {b['output_bytes_median']:,} | {k['output_bytes_median']:,} | {d['output_bytes_pct']:+.1f}% | "
                f"{b['tokens_median']:,}->{k['tokens_median']:,} ({d['tokens_pct']:+.1f}%) | "
                f"{b['global_api_count_median']:.0f}->{k['global_api_count_median']:.0f} | "
                f"{b['build_ms_median']:.1f}/{b['build_ms_p90']:.1f}->{k['build_ms_median']:.1f}/{k['build_ms_p90']:.1f} | "
                f"{b['run_ms_median']:.1f}/{b['run_ms_p90']:.1f}->{k['run_ms_median']:.1f}/{k['run_ms_p90']:.1f} | "
                f"{k['semantic_eq_pass']}/{'Y' if k['same_seed_det'] else 'N'}/{k['distinct_seed_builds']} |"
            )

        md.append("")
        md.append("## 4. Legacy Preset Controls (`vm` & `high`) — 6-Seed Medians")
        md.append("")
        md.append("| Corpus | Stock Luau ms | `vm` Out B (B->C, Δ%) | `vm` Run ms Med/p90 (B->C) | `high` Out B (B->C, Δ%) | `high` Run ms Med/p90 (B->C) | Eq |")
        md.append("|---|---:|---:|---:|---:|---:|---:|")
        for cid, _, _ in CORPORA:
            c = summary[cid]
            mv = c["modes"]["vm"]
            mh = c["modes"]["high"]
            bv, kv, dv = mv["baseline"], mv["candidate"], mv["delta"]
            bh, kh, dh = mh["baseline"], mh["candidate"], mh["delta"]
            md.append(
                f"| `{cid}` | {c['stock_luau_ms_median']:.2f} | "
                f"{bv['output_bytes_median']:,}->{kv['output_bytes_median']:,} ({dv['output_bytes_pct']:+.1f}%) | "
                f"{bv['run_ms_median']:.1f}/{bv['run_ms_p90']:.1f}->{kv['run_ms_median']:.1f}/{kv['run_ms_p90']:.1f} | "
                f"{bh['output_bytes_median']:,}->{kh['output_bytes_median']:,} ({dh['output_bytes_pct']:+.1f}%) | "
                f"{bh['run_ms_median']:.1f}/{bh['run_ms_p90']:.1f}->{kh['run_ms_median']:.1f}/{kh['run_ms_p90']:.1f} | "
                f"12/12 |"
            )

        args.md_out.parent.mkdir(parents=True, exist_ok=True)
        args.md_out.write_text("\n".join(md) + "\n")
        print(f"[ab] wrote JSON -> {args.json_out} ({args.json_out.stat().st_size} B)")
        print(f"[ab] wrote Markdown -> {args.md_out} ({args.md_out.stat().st_size} B)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
