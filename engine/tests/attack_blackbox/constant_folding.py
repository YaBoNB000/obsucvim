#!/usr/bin/env python3
"""Black-box attack #1: constant folding (常量折叠).

Public technique: find sub-expressions that only contain numeric literals and
arithmetic operators, evaluate them, and use the results to recover the
original program's literal constants.

This script deliberately knows nothing about the obfuscator. It:
  1. collects the numeric literals of the *original* corpus (the ground truth
     an attacker would like to recover),
  2. collects every constant-arithmetic sub-expression in the *artifact* and
     evaluates it with Lua semantics (`/` is float division, `%` is Lua mod),
  3. reports how many of the original literals a folding pass actually
     recovers -- **and** runs the identical match against a decoy literal set
     of the same size that is provably absent from the corpus. Without that
     control, "recovered" just means "this small integer also occurs somewhere
     in a 150 KB file", which is not recovery at all.

Verdicts are driven by the *excess* over the decoy control, not by the raw
match rate:
  成功     excess >= 50% of the original literals
  部分成功  0 < excess < 50%
  失败     excess == 0 (the decoy control reproduces the whole "recovery")
"""
from __future__ import annotations

import re
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _common import CASES, build, case_path, strip_comments, strip_long_strings, verdict  # noqa: E402

# Longest runs of "numbers and arithmetic only" (no identifiers, no strings).
CONST_EXPR = re.compile(r"(?<![\w.])(?:\d+(?:\.\d+)?|\s|[+\-*/%()]){5,}(?![\w.])")
NUM = re.compile(r"(?<![\w.])\d+(?:\.\d+)?(?![\w.])")


def lua_mod(a: float, b: float) -> float:
    if b == 0:
        return float("nan")
    return a - (a // b) * b


class LuaExpr:
    """Evaluate a numbers-only arithmetic expression with Lua semantics."""

    def __init__(self, text: str):
        self.t = text
        self.i = 0

    def peek(self) -> str:
        while self.i < len(self.t) and self.t[self.i] in " \t\n":
            self.i += 1
        return self.t[self.i] if self.i < len(self.t) else ""

    def parse(self) -> float:
        v = self.add()
        if self.peek():
            raise ValueError("trailing")
        return v

    def add(self) -> float:
        v = self.mul()
        while self.peek() and self.peek() in "+-":
            op = self.t[self.i]
            self.i += 1
            r = self.mul()
            v = v + r if op == "+" else v - r
        return v

    def mul(self) -> float:
        v = self.unary()
        while self.peek() and self.peek() in "*/%":
            op = self.t[self.i]
            self.i += 1
            r = self.unary()
            if op == "*":
                v = v * r
            elif op == "/":
                v = v / r
            else:
                v = lua_mod(v, r)
        return v

    def unary(self) -> float:
        if self.peek() and self.peek() in "+-":
            op = self.t[self.i]
            self.i += 1
            v = self.unary()
            return v if op == "+" else -v
        return self.atom()

    def atom(self) -> float:
        if self.peek() == "(":
            self.i += 1
            v = self.add()
            if self.peek() != ")":
                raise ValueError("paren")
            self.i += 1
            return v
        m = re.match(r"\d+(?:\.\d+)?", self.t[self.i :])
        if not m:
            raise ValueError("num")
        self.i += len(m.group(0))
        return float(m.group(0))


def lua_num(v: float) -> str:
    if v == int(v) and abs(v) < 1e15:
        return str(int(v))
    return repr(v)


def _numeric_text(text: str) -> str:
    text = strip_comments(strip_long_strings(text))
    text = re.sub(r'"(?:\\.|[^"\\])*"', " ", text)
    return re.sub(r"'(?:\\.|[^'\\])*'", " ", text)


def source_literals(name: str) -> set[str]:
    """Numeric literals of the original corpus, normalised the way Lua would
    print them (so `6*7` folding to 42 can be matched against `42`)."""
    return {lua_num(float(m.group(0))) for m in NUM.finditer(_numeric_text(case_path(name).read_text()))}


def decoy_literals(truth: set[str]) -> set[str]:
    """Same number of literals, **magnitude-matched**, provably absent from the
    corpus: each decoy is the nearest free integer above its truth counterpart.
    Magnitude matching matters -- an off-by-7919 decoy would never occur in any
    artifact and would therefore not measure the base rate we need to beat."""
    out: set[str] = set()
    for s in sorted(truth, key=lambda x: (len(x), x)):
        d = float(s) + 1.0
        while lua_num(d) in truth or lua_num(d) in out:
            d += 1.0
        out.add(lua_num(d))
    return out


def fold_artifact(text: str) -> tuple[set[str], int]:
    stripped = _numeric_text(text)
    values: set[str] = set()
    tried = 0
    for m in CONST_EXPR.finditer(stripped):
        expr = m.group(0)
        if not re.search(r"[+\-*/%]", expr.strip()):
            continue
        tried += 1
        try:
            values.add(lua_num(LuaExpr(expr).parse()))
        except (ValueError, ZeroDivisionError):
            continue
    return values, tried


def main() -> int:
    rows = []
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        for name in CASES:
            art = build(name, tmp / f"{name}.lua")
            folded, tried = fold_artifact(art.read_text())
            truth = source_literals(name)
            decoy = decoy_literals(truth)
            real_hit = truth & folded
            decoy_hit = decoy & folded
            excess = len(real_hit) - len(decoy_hit)
            rows.append(
                {
                    "case": name,
                    "truth": len(truth),
                    "folded": len(folded),
                    "tried": tried,
                    "real": len(real_hit),
                    "decoy": len(decoy_hit),
                    "excess": excess,
                }
            )
            print(
                f"[{name}] original_literals={len(truth)} constant_exprs_tried={tried} "
                f"folded_values={len(folded)}"
            )
            print(
                f"    match_original={len(real_hit)}/{len(truth)} "
                f"({len(real_hit)/len(truth)*100:.1f}%)   "
                f"match_decoy={len(decoy_hit)}/{len(decoy)} "
                f"({len(decoy_hit)/len(decoy)*100:.1f}%)   excess={excess}"
            )
            print(f"    matched_original_values={sorted(real_hit, key=lambda x: (len(x), x))[:10]}")
            print(f"    matched_decoy_values={sorted(decoy_hit, key=lambda x: (len(x), x))[:10]}")
    truth_total = sum(r["truth"] for r in rows)
    real_total = sum(r["real"] for r in rows)
    decoy_total = sum(r["decoy"] for r in rows)
    excess_total = sum(r["excess"] for r in rows)
    print(
        f"TOTAL original_literals={truth_total} match_original={real_total} "
        f"match_decoy={decoy_total} excess={excess_total}"
    )
    print(
        "NOTE a small integer like 0/1/2 occurs thousands of times in a 150 KB "
        "artifact, so a raw match proves nothing; only the excess over the "
        "decoy control is evidence of actual constant recovery."
    )
    if excess_total >= truth_total * 0.5:
        level = "成功"
    elif excess_total > 0:
        level = "部分成功"
    else:
        level = "失败"
    print(verdict(level, "constant_folding"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
