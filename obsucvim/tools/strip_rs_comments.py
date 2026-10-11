#!/usr/bin/env python3
"""Rust-aware comment stripper for obsucvim/src (research-safe: tokens only).

Handles: // and /// //! line comments, nested /* */ block comments, normal
and byte strings with escapes (incl. \ line continuation), raw strings
r".."/r#".."# (and br variants, incl. raw-identifier r#type which is NOT a
string), char literals vs lifetimes. Comments inside any literal are kept.

Line cleanup: a line whose surviving content is whitespace-only AND that
contained a comment is dropped entirely; a line with code + trailing comment
is rstripped.
"""
import sys
from pathlib import Path

IDENT = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_")


def comment_spans(src: str):
    spans = []
    n = len(src)
    i = 0
    while i < n:
        c = src[i]
        nxt = src[i + 1] if i + 1 < n else ""
        # ---- comments ----
        if c == "/" and nxt == "/":
            j = i
            while j < n and src[j] != "\n":
                j += 1
            spans.append((i, j))
            i = j
            continue
        if c == "/" and nxt == "*":
            depth, j = 1, i + 2
            while j < n and depth > 0:
                if src[j] == "/" and j + 1 < n and src[j + 1] == "*":
                    depth += 1
                    j += 2
                elif src[j] == "*" and j + 1 < n and src[j + 1] == "/":
                    depth -= 1
                    j += 2
                else:
                    j += 1
            spans.append((i, j))
            i = j
            continue
        # ---- strings ----
        if c == '"' or (c == "b" and nxt == '"'):
            j = i + (2 if c == "b" else 1)
            while j < n:
                if src[j] == "\\":
                    j += 2
                elif src[j] == '"':
                    j += 1
                    break
                else:
                    j += 1
            i = j
            continue
        # raw / raw-byte strings: r", r#", br", br# (identifier boundary)
        if (c == "r" or (c == "b" and nxt == "r")) and (i == 0 or src[i - 1] not in IDENT):
            j = i + (2 if c == "b" else 1)
            h = 0
            while j < n and src[j] == "#":
                h += 1
                j += 1
            if j < n and src[j] == '"':
                j += 1
                term = '"' + "#" * h
                while j < n:
                    if src.startswith(term, j):
                        j += len(term)
                        break
                    j += 1
                i = j
                continue
            # else: raw identifier like r#type — fall through as plain char
        # ---- char literals vs lifetimes ----
        if c == "'" or (c == "b" and nxt == "'"):
            q = i + (2 if c == "b" else 1)  # first payload char
            if q < n:
                if src[q] == "\\":
                    j = q + 1
                    if j < n and src[j] == "x":
                        j += 3
                    elif j < n and src[j] == "u":
                        while j < n and src[j] != "}":
                            j += 1
                        j += 1
                    else:
                        j += 1
                    if j < n and src[j] == "'":
                        i = j + 1
                        continue
                elif q + 1 < n and src[q + 1] == "'" and src[q] != "'":
                    i = q + 2
                    continue
            # lifetime / label — plain char
        i += 1
    return spans


def strip_comments(src: str) -> tuple[str, int]:
    spans = comment_spans(src)
    if not spans:
        return src, 0
    drop = bytearray(len(src))
    for a, b in spans:
        for k in range(a, b):
            drop[k] = 1
    # line cleanup: drop whitespace-only lines that contained a comment;
    # rstrip lines that keep code after removing a trailing comment
    line_start = 0
    lines = src.split("\n")
    # build line ranges over the original string (indices)
    pos = 0
    for ln in lines:
        start, end = pos, pos + len(ln)  # end excludes the \n
        pos = end + 1
        had_comment = any(not (b <= start or a >= end) for a, b in spans)
        if had_comment:
            rest_ws = all(
                (drop[k] or ln[k - start] in " \t\r") for k in range(start, end)
            )
            if rest_ws and end < len(src):
                for t in range(start, end):
                    drop[t] = 1  # kill leading indentation of comment-only lines
                drop[end] = 1  # kill the newline too -> whole line removed
            else:
                # find last surviving non-ws char; drop trailing ws/comment
                k = end - 1
                while k >= start and (drop[k] or ln[k - start] in " \t\r"):
                    k -= 1
                for t in range(k + 1, end):
                    drop[t] = 1
                # if everything before the first surviving non-ws char is
                # dropped-or-ws (line started with a comment), lstrip too
                k2 = start
                while k2 < end and (drop[k2] or ln[k2 - start] in " \t\r"):
                    k2 += 1
                for t in range(start, min(k2, end)):
                    if ln[t - start] in " \t\r":
                        drop[t] = 1
    out = "".join(ch for k, ch in enumerate(src) if not drop[k])
    return out, len(spans)


def main():
    if sys.argv[1] == "--selftest":
        torture = r'''// full line
	/// indented doc stays whole-line-dropped (tab must not leak)
/// doc line
//! inner doc
/* block */ let a = 5; // trailing
let nested = 1; /* outer /* inner */ still comment */ let b = 2;
let s = "http://x // not comment /* nope */";
let r = r#"raw // stay /* stay "#;
let rid = r#type; // raw ident above
let c = '/'; let c2 = '\''; let c3 = '\n';
fn f<'a>(x: &'a str) -> &'static str { x }
let multi = "a \
b // still string";
let bs = b"bytes\x00//";
let bc = b'x';
let div = a / b; let x = 4/2;
'''
        expect_keep = [
            "let a = 5;",
            'let s = "http://x // not comment /* nope */";',
            'let r = r#"raw // stay /* stay "#;',
            "let rid = r#type;",
            "let c = '/';",
            "let c2 = '\\'';",
            "fn f<'a>(x: &'a str) -> &'static str { x }",
            'let bs = b"bytes\\x00//";',
            "let bc = b'x';",
            "let div = a / b;",
        ]
        out, cnt = strip_comments(torture)
        ok = True
        for k in expect_keep:
            if k not in out:
                print(f"MISSING KEEP: {k}")
                ok = False
        for bad in ["full line", "doc line", "inner doc", "block", "outer", "trailing", "still comment"]:
            if bad in out.replace("still string", ""):
                print(f"LEAKED COMMENT: {bad}")
                ok = False
        print("--- output ---")
        print(out)
        print("--- selftest:", "PASS" if ok else "FAIL", f"({cnt} comments)")
        sys.exit(0 if ok else 1)
    total = 0
    for path in sys.argv[1:]:
        p = Path(path)
        src = p.read_text(encoding="utf-8")
        out, cnt = strip_comments(src)
        p.write_text(out, encoding="utf-8")
        total += cnt
        print(f"stripped {cnt:5d} comments  {path}")
    print(f"TOTAL {total}")


if __name__ == "__main__":
    main()
