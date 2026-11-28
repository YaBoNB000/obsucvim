#!/usr/bin/env python3
"""Black-box attack #4: control-flow-graph simplification (控制流图简化).

Public technique: the obfuscator flattens control flow into a dispatcher
(`while true do if <var> == <k> then ... end end`). A deobfuscator tries to
merge the resulting redundant basic blocks back into structured code, i.e. to
rebuild the original CFG.

This script measures whether that is feasible, on the artifact text only:

  * `arms`       -- dispatch arms (`if`/`elseif` conditions) found inside
                    `while true do ... end` dispatcher loops;
  * `single`     -- arms whose body is exactly one statement, i.e. the blocks
                    a simplifier could merge into a flat jump table;
  * `mergeable`  -- single / arms, the fraction of the CFG that collapses;
  * stability    -- the same counts on a second seed: a normalizer needs the
                    dispatcher shape to be stable, otherwise every seed needs
                    its own hand-written pass.

Verdicts:
  成功     mergeable >= 0.80 and the arm count is stable across seeds (+-5%)
  部分成功  mergeable >= 0.50, or stable but not mergeable
  失败     mergeable < 0.50
"""
from __future__ import annotations

import re
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _common import CASES, build, case_path, strip_comments, strip_long_strings, verdict  # noqa: E402
from dead_code_elimination import _code_text, _matching_end  # noqa: E402

TOKEN = re.compile(r"\b(?:function|if|then|do|repeat|until|end|elseif|else|while|for)\b")
ARM = re.compile(r"\b(elseif|if)\b")


def _blocks(code: str) -> tuple[int, int]:
    """(arms, single-statement arms) inside `while true do` dispatchers."""
    arms = 0
    single = 0
    for m in re.finditer(r"\bwhile\s+true\s+do\b", code):
        end = _matching_end(code, m.end())
        if end == -1:
            continue
        body = code[m.end() : end]
        for a in ARM.finditer(body):
            then = re.search(r"\bthen\b", body[a.end() :])
            if not then:
                continue
            start = a.end() + then.end()
            # arm body runs to the next elseif/else/end at depth 0
            depth = 0
            stop = len(body)
            for tok in TOKEN.finditer(body, start):
                t = tok.group(0)
                if t in ("then", "do", "function", "repeat"):
                    depth += 1
                elif t in ("end", "until"):
                    if depth == 0:
                        stop = tok.start()
                        break
                    depth -= 1
                elif t in ("elseif", "else") and depth == 0:
                    stop = tok.start()
                    break
            arms += 1
            chunk = body[start:stop]
            stmts = [c for c in re.split(r"[;\n]", chunk) if c.strip()]
            if len(stmts) <= 1:
                single += 1
    return arms, single


def measure(text: str) -> tuple[int, int, float]:
    code = _code_text(text)
    arms, single = _blocks(code)
    return arms, single, (single / arms if arms else 0.0)


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        print("--- source corpora (no dispatcher, arms must be ~0) ---")
        for name in CASES:
            a, s, r = measure(case_path(name).read_text())
            print(f"[{name}] arms={a} single_stmt_arms={s} mergeable={r*100:.1f}%")

        print("--- artifacts, seed 42 vs seed 4242 ---")
        rows = []
        for name in CASES:
            per = {}
            for seed in (42, 4242):
                art = build(name, tmp / f"{name}_{seed}.lua", seed=seed)
                per[seed] = measure(art.read_text())
            a42, s42, r42 = per[42]
            a7, s7, r7 = per[4242]
            stable = abs(a42 - a7) <= max(1, 0.05 * max(a42, a7))
            rows.append((name, a42, s42, r42, a7, s7, r7, stable))
            print(
                f"[{name}] seed42 arms={a42} single={s42} mergeable={r42*100:.1f}%  |  "
                f"seed4242 arms={a7} single={s7} mergeable={r7*100:.1f}%  "
                f"stable={stable}"
            )

    mean_merge = sum(r[3] for r in rows) / len(rows)
    all_stable = all(r[7] for r in rows)
    print(f"TOTAL mean mergeable={mean_merge*100:.1f}%  arm_count_stable_across_seeds={all_stable}")
    print(
        "NOTE the arm counts are large and seed-dependent: the dispatcher is "
        "regenerated per build, so a simplifier cannot be written once against "
        "a fixed shape."
    )
    if mean_merge >= 0.80 and all_stable:
        level = "成功"
    elif mean_merge >= 0.50:
        level = "部分成功"
    else:
        level = "失败"
    print(verdict(level, "cfg_simplification"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
