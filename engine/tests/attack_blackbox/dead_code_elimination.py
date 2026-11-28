#!/usr/bin/env python3
"""Black-box attack #2: dead code elimination (死代码消除).

Public technique: find branches that can never execute (constant-false
conditions) and statements that can never be reached (code after an
unconditional `return`/`break` inside the same block), then delete them and
check that the program still behaves identically.

This script measures exactly that, on the artifact text only:

  dead_bytes = bytes inside `if <const-false> then ... end` /
               `while <const-false> do ... end` bodies
               + bytes after an unconditional `return`/`break` before the
                 enclosing block's `end`

Two controls make the number meaningful:
  * source control  -- the same measurement on the original corpus, which must
    be ~0% (the corpus has no dead code by construction);
  * positive control -- a synthetic file that is 50% dead code by
    construction, which must be detected as ~50%. If the detector cannot find
    the synthetic dead code, the artifact number is worthless.

Verdicts:
  成功     artifact dead fraction >= 10%
  部分成功  0 < artifact dead fraction < 10%
  失败     artifact dead fraction == 0
"""
from __future__ import annotations

import re
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _common import CASES, build, case_path, strip_comments, strip_long_strings, verdict  # noqa: E402

TOKEN = re.compile(r"\b(?:function|if|then|do|repeat|until|end|elseif|return|break)\b")
FALSE_COND = re.compile(r"\b(?:false|nil|0)\b")


def _code_text(text: str) -> str:
    """Comments and long strings removed; quoted strings blanked but kept the
    same length so byte offsets still line up with the original text."""
    text = strip_comments(text)
    out = []
    i = 0
    n = len(text)
    while i < n:
        m = re.match(r"\[(=*)\[", text[i:])
        if m:
            lvl = m.group(1)
            close = "]" + lvl + "]"
            j = text.find(close, i + len(m.group(0)))
            j = n if j == -1 else j + len(close)
            out.append(" " * (j - i))
            i = j
            continue
        if text[i] in "\"'":
            q = text[i]
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == q:
                    j += 1
                    break
                j += 1
            out.append(" " * (j - i))
            i = j
            continue
        out.append(text[i])
        i += 1
    return "".join(out)


def _matching_end(code: str, open_end: int) -> int:
    """Index just past the `end` that closes the block opened at `open_end`
    (which must point just past a `then`/`do`). Returns -1 if unbalanced."""
    depth = 1
    for m in TOKEN.finditer(code, open_end):
        t = m.group(0)
        if t in ("then", "do", "function", "repeat"):
            if t == "then":
                prev = code[max(0, m.start() - 6) : m.start()]
                if prev.rstrip().endswith("elseif"):
                    continue
            depth += 1
        elif t in ("end", "until"):
            depth -= 1
            if depth == 0:
                return m.end()
    return -1


def dead_regions(code: str) -> list[tuple[int, int]]:
    regions: list[tuple[int, int]] = []
    # (a) constant-false branch bodies
    for m in re.finditer(r"\b(if|while)\s+([^\n]{0,40}?)\s*(then|do)\b", code):
        cond = m.group(2).strip()
        if cond in ("false", "nil", "0"):
            end = _matching_end(code, m.end())
            if end != -1:
                regions.append((m.start(), end))
    # (b) unreachable tail after an unconditional return/break.
    # Only a *bare* jump followed by another statement is flagged: `return x`,
    # `return function() ... end` and `break` directly before `end` are normal
    # control flow, and Lua allows a newline between `return` and its value, so
    # anything that could still be an expression is left alone (conservative).
    for m in re.finditer(r"\b(return|break)\b", code):
        after = code[m.end() :]
        depth = 0
        for tok in TOKEN.finditer(after):
            t = tok.group(0)
            if t in ("then", "do", "function", "repeat"):
                if t == "then" and after[max(0, tok.start() - 6) : tok.start()].rstrip().endswith("elseif"):
                    continue
                depth += 1
            elif t in ("end", "until", "else", "elseif"):
                if depth == 0:
                    break
                depth -= 1
            elif t in ("local", "if", "while", "for", "repeat", "do", "break", "return") and depth == 0:
                # a statement follows the jump inside the same block
                depth2 = 0
                for tok2 in TOKEN.finditer(after, tok.start()):
                    t2 = tok2.group(0)
                    if t2 in ("then", "do", "function", "repeat"):
                        if t2 == "then" and after[max(0, tok2.start() - 6) : tok2.start()].rstrip().endswith("elseif"):
                            continue
                        depth2 += 1
                    elif t2 in ("end", "until"):
                        if depth2 == 0:
                            regions.append((m.start(), m.end() + tok2.end()))
                            break
                        depth2 -= 1
                break
    return regions


def dead_fraction(text: str) -> tuple[float, int, int]:
    code = _code_text(text)
    regions = dead_regions(code)
    covered: list[tuple[int, int]] = []
    for a, b in sorted(regions):
        if covered and a <= covered[-1][1]:
            covered[-1] = (covered[-1][0], max(covered[-1][1], b))
        else:
            covered.append((a, b))
    dead = sum(b - a for a, b in covered)
    total = len(text)
    return (dead / total if total else 0.0), dead, total


POSITIVE_CONTROL = """local function live(x)
    local acc = 0
    for i = 1, 10 do acc = acc + i * x end
    return acc
end
local function dead_branch(x)
    if false then
        local unused = {}
        for i = 1, 100 do unused[i] = i * x end
        return unused
    end
    return x
end
local function dead_tail(x)
    return x * 2
    local never = x + 1
    never = never * 3
    return never
end
print(live(3), dead_branch(4), dead_tail(5))
"""


def main() -> int:
    # positive control first: a file that is ~50% dead by construction
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        pc = tmp / "positive.lua"
        pc.write_text(POSITIVE_CONTROL)
        frac, dead, total = dead_fraction(pc.read_text())
        print(f"[positive-control] dead_bytes={dead}/{total} = {frac*100:.1f}% (must be > 0)")
        if frac <= 0:
            print("detector cannot find synthetic dead code; artifact numbers are meaningless")
            print(verdict("失败", "dead_code_elimination"))
            return 0

        rows = []
        for name in CASES:
            src_frac, sdead, stotal = dead_fraction(case_path(name).read_text())
            art = build(name, tmp / f"{name}.lua")
            art_frac, adead, atotal = dead_fraction(art.read_text())
            rows.append((name, src_frac, art_frac))
            print(
                f"[{name}] source dead={sdead}/{stotal} = {src_frac*100:.2f}%   "
                f"artifact dead={adead}/{atotal} = {art_frac*100:.2f}%"
            )
    mean_src = sum(r[1] for r in rows) / len(rows)
    mean_art = sum(r[2] for r in rows) / len(rows)
    excess = mean_art - mean_src
    print(
        f"TOTAL mean_source={mean_src*100:.2f}% mean_artifact={mean_art*100:.2f}% "
        f"excess={excess*100:.2f}%"
    )
    print(
        "NOTE the source column is the detector's own false-positive floor: if "
        "the artifact is not clearly above it, a DCE pass has nothing to remove."
    )
    if excess >= 0.10:
        level = "成功"
    elif excess > 0:
        level = "部分成功"
    else:
        level = "失败"
    print(verdict(level, "dead_code_elimination"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
