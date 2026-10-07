#!/usr/bin/env python3
"""R063 variable-name policy audit: generated locals/parameters/loop vars are <=2 letters."""
from __future__ import annotations

import re
import sys
from pathlib import Path

from v15_fingerprint import tokenize


def names_in_function_params(tokens: list[str], start: int) -> list[str]:
    # Skip an optional function name / dotted declaration target, then parse
    # the parameter list. The emitted VM uses plain function expressions; the
    # bounded scan also tolerates `function t.f(...)` declarations.
    j = start + 1
    while j < min(len(tokens), start + 12) and tokens[j] != "(":
        j += 1
    if j >= len(tokens) or tokens[j] != "(":
        return []
    j += 1
    depth = 0
    result: list[str] = []
    while j < len(tokens):
        t = tokens[j]
        if t == "(":
            depth += 1
        elif t == ")":
            if depth == 0:
                break
            depth -= 1
        elif depth == 0 and re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", t):
            if j == 0 or tokens[j - 1] in ("(", ","):
                result.append(t)
        j += 1
    return result


def audit(path: Path) -> tuple[int, list[str]]:
    source = path.read_text(encoding="utf-8")
    code, _, _ = tokenize(source)
    tokens = re.findall(r"[A-Za-z_][A-Za-z_0-9]*|\.\.\.|[(),=;]", code)
    variables: list[str] = []
    for i, token in enumerate(tokens):
        if token == "local":
            j = i + 1
            if j < len(tokens) and tokens[j] == "function":
                if j + 1 < len(tokens) and re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", tokens[j + 1]):
                    variables.append(tokens[j + 1])
            else:
                while j < len(tokens) and re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", tokens[j]):
                    variables.append(tokens[j])
                    j += 1
                    if j < len(tokens) and tokens[j] == ",":
                        j += 1
                    else:
                        break
        elif token == "function":
            variables.extend(names_in_function_params(tokens, i))
        elif token == "for":
            j = i + 1
            while j < len(tokens) and re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", tokens[j]):
                variables.append(tokens[j])
                j += 1
                if j < len(tokens) and tokens[j] == ",":
                    j += 1
                else:
                    break

    violations = sorted({
        name for name in variables
        if len(name) > 2 or not name.isascii() or not name.isalpha()
    })
    return len(variables), violations


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: v15_short_name_audit.py artifact.lua", file=sys.stderr)
        return 2
    path = Path(sys.argv[1])
    count, violations = audit(path)
    if violations:
        print(f"short-name audit: {len(violations)} long/non-alpha local names -> FAIL")
        print("  " + ", ".join(violations[:40]))
        return 1
    print(f"short-name audit: {count} local/parameter/loop identifiers, all <=2 letters -> PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
