#!/usr/bin/env python3
"""Research-only token-aware Luau whitespace compactor.

No VM/runtime wrapper, no encryption changes: only removes whitespace that
is not required to preserve the exact token stream. Strings, long strings,
comments/directives, lifetimes/type punctuation are untouched. Always validate
output with luau-compile and an unbound semantic run before proposing adoption.
"""
from __future__ import annotations
import re
import sys
from dataclasses import dataclass
from pathlib import Path

OPS = sorted([
    "...", "..=", "//=", "<<=", ">>=", "::", "==", "~=", "<=", ">=", "..", "//",
    "+=", "-=", "*=", "/=", "%=", "^=", "%=", "->", "<<", ">>", "&&", "||",
], key=len, reverse=True)
NUM_HEX = re.compile(r"0[xX][0-9A-Fa-f_]+(?:\.(?!\.)[0-9A-Fa-f_]*)?(?:[pP][+-]?[0-9_]+)?")
NUM_DEC = re.compile(r"(?:[0-9][0-9_]*(?:\.(?!\.)[0-9_]*)?|\.[0-9][0-9_]*)(?:[eE][+-]?[0-9_]+)?")

@dataclass(frozen=True)
class Tok:
    kind: str
    text: str


def long_bracket_end(s: str, i: int):
    # s[i] == '['; recognize [=[ ... ]=] at any equals depth
    j = i + 1
    while j < len(s) and s[j] == "=":
        j += 1
    if j >= len(s) or s[j] != "[":
        return None
    eq = s[i + 1:j]
    close = "]" + eq + "]"
    end = s.find(close, j + 1)
    return len(s) if end < 0 else end + len(close)


def scan_one(s: str, i: int):
    n = len(s)
    c = s[i]
    # comments before operators
    if c == "-" and i + 1 < n and s[i + 1] == "-":
        if i + 2 < n and s[i + 2] == "[":
            end = long_bracket_end(s, i + 2)
            if end is not None:
                return Tok("block_comment", s[i:end]), end
        end = s.find("\n", i)
        if end < 0:
            end = n
        return Tok("line_comment", s[i:end]), end
    # quoted strings and byte-string prefixes
    start = i
    if c in "bB" and i + 1 < n and s[i + 1] in "\"'":
        q = s[i + 1]
        j = i + 2
    elif c in "\"'":
        q = c
        j = i + 1
    else:
        q = None
        j = i
    if q is not None:
        while j < n:
            if s[j] == "\\":
                j += 2
            elif s[j] == q:
                j += 1
                break
            else:
                j += 1
        return Tok("string", s[start:j]), j
    # long strings: [=[ ... ]=]
    if c == "[":
        end = long_bracket_end(s, i)
        if end is not None:
            return Tok("long_string", s[i:end]), end
    # identifiers; accept Unicode letters (Luau identifiers can be Unicode)
    if c == "_" or c.isalpha() or ord(c) >= 128:
        j = i + 1
        while j < n and (s[j] == "_" or s[j].isalnum() or ord(s[j]) >= 128):
            j += 1
        return Tok("ident", s[i:j]), j
    # numbers
    if c.isdigit() or (c == "." and i + 1 < n and s[i + 1].isdigit()):
        m = NUM_HEX.match(s, i) if s.startswith(("0x", "0X"), i) else NUM_DEC.match(s, i)
        if m:
            return Tok("number", m.group(0)), m.end()
    # operators, longest first
    for op in OPS:
        if s.startswith(op, i):
            return Tok("op", op), i + len(op)
    return Tok("punct", c), i + 1


def tokenize(s: str):
    out = []
    i = 0
    while i < len(s):
        if s[i].isspace():
            i += 1
            continue
        tok, i = scan_one(s, i)
        out.append(tok)
    return out


def needs_separator(a: Tok, b: Tok):
    if a.kind == "line_comment":
        return "\n"
    # Luau's numeric lexer treats `1..x` / `0x1..x` as a malformed number
    # unless whitespace separates the integer and concat operator. Similarly,
    # keep number/identifier boundaries conservative (e.g. `1e` can be read as
    # a malformed exponent by a dialect lexer even if our scanner splits it).
    if a.kind == "number" and (b.kind == "ident" or b.text.startswith(".")):
        return " "
    if b.kind == "number" and a.text.endswith("."):
        return " "
    joined = a.text + b.text
    toks = tokenize(joined)
    # Exact two-token preservation is the invariant. Comments count as tokens.
    if len(toks) == 2 and toks[0].text == a.text and toks[1].text == b.text:
        return ""
    # For block-comment boundaries, the scanner considers the comment a token.
    return " "


def compact(s: str):
    toks = tokenize(s)
    out = []
    prev = None
    for tok in toks:
        if prev is not None:
            sep = needs_separator(prev, tok)
            if sep:
                out.append(sep)
        out.append(tok.text)
        prev = tok
    return "".join(out)


def main():
    if len(sys.argv) < 2:
        raise SystemExit("usage: token_minify.py input.lua [output.lua]")
    src = Path(sys.argv[1]).read_text()
    out = compact(src)
    dest = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(sys.argv[1]).with_suffix(".compact.lua")
    dest.write_text(out)
    print(f"{len(src.encode())} -> {len(out.encode())} bytes ({100*(1-len(out)/max(1,len(src))):.2f}% smaller)")
    print(dest)

if __name__ == "__main__":
    main()
