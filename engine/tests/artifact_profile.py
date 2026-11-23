#!/usr/bin/env python3
"""Obsucvim v15 artifact profiler & baseline auditor (P0).

Parses Luau source/artifacts into a real lexical-scoped AST (not regex-only)
and reports:
  - Environment metadata (branch, HEAD, git status, rustc/luau versions)
  - Input bytes, output bytes, SHA-256, line count
  - Token count, AST node count, function count, branch count, loop count
  - Unbound global API references (deduplicated set + count) & string-metatable primitives
  - AST literal constants (total, string count, max string length) + optional
    compile-time VM constant/opcode manifest (via OBSUCVIM_PROFILE_AUDIT when
    available in debug/audit builds; never embedded in output artifacts)
  - Handler wrapper structural signatures (outer staging wrappers & inner HW2
    VM opcode wrappers: signature count, top-signature share)
  - Conservative AST-verified provably constant predicates (real-algebraic
    tautologies/contradictions, integer-parity identities, identical-arm
    branches, and domain-dependent candidates reported separately)
  - Multi-seed determinism check (byte-for-byte identity on repeated seed) &
    cross-seed structural variance + semantic equivalence under Luau
  - Build wall time, execution wall time, and child peak RSS (via os.wait4)

Usage:
  # Profile standalone artifact(s) such as example.txt:
  python3 engine/tests/artifact_profile.py --artifact example.txt

  # Run full diagnostic suite across seeds (default: 1,7,42,4242,31337,999999):
  python3 engine/tests/artifact_profile.py --suite --out /tmp/obsucvim_p0_profile.json
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import os
import pathlib
import re
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any

ENGINE_ROOT = pathlib.Path(__file__).resolve().parent.parent
REPO_ROOT = ENGINE_ROOT.parent


def find_tools_bin() -> pathlib.Path:
    for cand in (
        REPO_ROOT / ".tools" / "bin",
        pathlib.Path("/home/user/tools/bin"),
    ):
        if (cand / "luau").is_file() and os.access(cand / "luau", os.X_OK):
            return cand
    raise RuntimeError(
        "Luau toolchain missing: expected .tools/bin/luau (run `bash engine/tools/rebuild_tools.sh`)"
    )


def find_obsucvim_bin(prefer_audit: bool = False) -> pathlib.Path:
    env_bin = os.environ.get("OBSUCVIM_BIN")
    if env_bin:
        p = pathlib.Path(env_bin)
        if p.is_file() and os.access(p, os.X_OK):
            return p
        raise RuntimeError(f"OBSUCVIM_BIN={env_bin} is not an executable file")
    candidates = []
    if prefer_audit:
        candidates.append(ENGINE_ROOT / "target" / "debug" / "obsucvim")
    candidates.append(ENGINE_ROOT / "target" / "release" / "obsucvim")
    candidates.append(ENGINE_ROOT / "target" / "debug" / "obsucvim")
    for p in candidates:
        if p.is_file() and os.access(p, os.X_OK):
            return p
    raise RuntimeError(
        f"obsucvim binary missing under {ENGINE_ROOT / 'target'} (run `cargo build --release`)"
    )


def run_measured(
    cmd: list[str],
    env: dict[str, str] | None = None,
    timeout_sec: float = 60.0,
) -> dict[str, Any]:
    """Run subprocess while capturing wall time, stdout/stderr, exit code, and peak RSS."""
    with tempfile.TemporaryFile() as f_out, tempfile.TemporaryFile() as f_err:
        t0 = time.perf_counter()
        proc = subprocess.Popen(
            cmd,
            stdout=f_out,
            stderr=f_err,
            env=env,
            cwd=str(REPO_ROOT),
        )
        timed_out = False

        def kill_proc() -> None:
            nonlocal timed_out
            timed_out = True
            try:
                proc.kill()
            except OSError:
                pass

        timer = threading.Timer(timeout_sec, kill_proc)
        timer.start()
        maxrss_kb: int | None = None
        try:
            if hasattr(os, "wait4"):
                _, status, rusage = os.wait4(proc.pid, 0)
                if os.WIFEXITED(status):
                    rc = os.WEXITSTATUS(status)
                elif os.WIFSIGNALED(status):
                    rc = -os.WTERMSIG(status)
                else:
                    rc = 1
                proc.returncode = rc
                maxrss_kb = int(getattr(rusage, "ru_maxrss", 0)) or None
            else:
                rc = proc.wait()
        finally:
            timer.cancel()
        dt = time.perf_counter() - t0
        f_out.seek(0)
        f_err.seek(0)
        stdout = f_out.read()
        stderr = f_err.read()
        if timed_out:
            rc = 124
        return {
            "rc": rc,
            "timed_out": timed_out,
            "wall_ms": round(dt * 1000.0, 3),
            "peak_rss_kb": maxrss_kb if maxrss_kb is not None else "unavailable",
            "stdout": stdout,
            "stderr": stderr,
        }


# ---------------------------------------------------------------------------
# Luau Lexer & Recursive-Descent AST Parser (for accurate AST analysis)
# ---------------------------------------------------------------------------

KEYWORDS = frozenset(
    {
        "and",
        "break",
        "do",
        "else",
        "elseif",
        "end",
        "false",
        "for",
        "function",
        "if",
        "in",
        "local",
        "nil",
        "not",
        "or",
        "repeat",
        "return",
        "then",
        "true",
        "until",
        "while",
    }
)

COMPOUND_OPS = frozenset({"+=", "-=", "*=", "/=", "//=", "%=", "^=", "..="})


class Token:
    __slots__ = ("kind", "val", "pos")

    def __init__(self, kind: str, val: Any, pos: int) -> None:
        self.kind = kind
        self.val = val
        self.pos = pos

    def __repr__(self) -> str:
        return f"Token({self.kind!r}, {self.val!r}@{self.pos})"


def decode_lua_short_str(raw: str) -> bytes:
    out = bytearray()
    i = 1
    n = len(raw) - 1
    while i < n:
        ch = raw[i]
        if ch != "\\":
            out.extend(ch.encode("utf-8", errors="surrogateescape"))
            i += 1
            continue
        i += 1
        if i >= n:
            break
        esc = raw[i]
        if esc == "a":
            out.append(7)
            i += 1
        elif esc == "b":
            out.append(8)
            i += 1
        elif esc == "f":
            out.append(12)
            i += 1
        elif esc == "n":
            out.append(10)
            i += 1
        elif esc == "r":
            out.append(13)
            i += 1
        elif esc == "t":
            out.append(9)
            i += 1
        elif esc == "v":
            out.append(11)
            i += 1
        elif esc in ('\\', '"', "'", "\n"):
            out.extend(esc.encode("latin1", errors="replace"))
            i += 1
        elif esc == "x" and i + 2 < n:
            try:
                out.append(int(raw[i + 1 : i + 3], 16))
                i += 3
            except ValueError:
                out.append(ord("x"))
                i += 1
        elif esc == "z":
            i += 1
            while i < n and raw[i] in " \t\r\n":
                i += 1
        elif esc.isdigit():
            j = i
            while j < min(n, i + 3) and raw[j].isdigit():
                j += 1
            out.append(int(raw[i:j], 10) & 0xFF)
            i = j
        else:
            out.extend(esc.encode("utf-8", errors="surrogateescape"))
            i += 1
    return bytes(out)


def lex_luau(src: str) -> list[Token]:
    tokens: list[Token] = []
    i = 0
    n = len(src)
    while i < n:
        c = src[i]
        if c in " \t\r\n":
            i += 1
            continue
        # Comment
        if c == "-" and i + 1 < n and src[i + 1] == "-":
            if i + 2 < n and src[i + 2] == "[":
                m = re.match(r"\[(=*)\[", src[i + 2 : i + 20])
                if m:
                    close = "]" + ("=" * len(m.group(1))) + "]"
                    end = src.find(close, i + 2 + len(m.group(0)))
                    i = n if end < 0 else end + len(close)
                    continue
            end = src.find("\n", i + 2)
            i = n if end < 0 else end + 1
            continue
        # Long string
        if c == "[" and i + 1 < n and src[i + 1] in "=[":
            m = re.match(r"\[(=*)\[", src[i : i + 20])
            if m:
                hdr_len = len(m.group(0))
                close = "]" + ("=" * len(m.group(1))) + "]"
                end = src.find(close, i + hdr_len)
                if end < 0:
                    raise SyntaxError(f"unclosed long string at {i}")
                body = src[i + hdr_len : end]
                if body.startswith("\n"):
                    body = body[1:]
                tokens.append(Token("LONGSTR", body.encode("utf-8", errors="surrogateescape"), i))
                i = end + len(close)
                continue
        # Quoted string
        if c in ('"', "'"):
            q = c
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == q:
                    break
                j += 1
            if j >= n:
                raise SyntaxError(f"unclosed string at {i}")
            raw = src[i : j + 1]
            tokens.append(Token("STR", decode_lua_short_str(raw), i))
            i = j + 1
            continue
        # Backtick string (simple fallback for Luau interpolated literal)
        if c == "`":
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == "`":
                    break
                j += 1
            raw_bt = src[i + 1 : j]
            tokens.append(Token("STR", raw_bt.encode("utf-8"), i))
            i = j + 1
            continue
        # Number
        if c.isdigit() or (c == "." and i + 1 < n and src[i + 1].isdigit()):
            m = re.match(
                r"(?:0[xX][0-9a-fA-F_]+(?:\.[0-9a-fA-F_]+)?(?:[pP][+-]?[0-9_]+)?|"
                r"0[bB][01_]+|"
                r"(?:\d[\d_]*(?:\.[\d_]*)?|\.\d[\d_]*)(?:[eE][+-]?[\d_]+)?)",
                src[i:],
            )
            assert m is not None
            raw_num = m.group(0).replace("_", "")
            try:
                if raw_num.lower().startswith("0x"):
                    val = float.fromhex(raw_num) if ("." in raw_num or "p" in raw_num.lower()) else float(int(raw_num, 16))
                elif raw_num.lower().startswith("0b"):
                    val = float(int(raw_num[2:], 2))
                else:
                    val = float(raw_num)
            except ValueError:
                val = 0.0
            tokens.append(Token("NUM", val, i))
            i += len(m.group(0))
            continue
        # Identifier or keyword
        if c.isalpha() or c == "_":
            j = i + 1
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            w = src[i:j]
            if w in KEYWORDS:
                tokens.append(Token("KW", w, i))
            else:
                tokens.append(Token("IDENT", w, i))
            i = j
            continue
        # 3-char punct
        if src.startswith("...", i):
            tokens.append(Token("OP", "...", i))
            i += 3
            continue
        if src.startswith("..=", i) or src.startswith("//=", i):
            tokens.append(Token("OP", src[i : i + 3], i))
            i += 3
            continue
        # 2-char punct
        two = src[i : i + 2]
        if two in ("==", "~=", "<=", ">=", "..", "//", "::", "->", "+=", "-=", "*=", "/=", "%=", "^="):
            tokens.append(Token("OP", two, i))
            i += 2
            continue
        tokens.append(Token("OP", c, i))
        i += 1
    tokens.append(Token("EOF", "", n))
    return tokens


BIN_PREC: dict[str, tuple[int, int]] = {
    "or": (1, 1),
    "and": (2, 2),
    "<": (3, 3),
    ">": (3, 3),
    "<=": (3, 3),
    ">=": (3, 3),
    "~=": (3, 3),
    "==": (3, 3),
    "..": (4, 3),  # right-associative
    "+": (5, 5),
    "-": (5, 5),
    "*": (6, 6),
    "/": (6, 6),
    "//": (6, 6),
    "%": (6, 6),
    "^": (8, 7),   # right-associative (higher than unary prec 7)
}


class Parser:
    def __init__(self, tokens: list[Token]) -> None:
        self.toks = tokens
        self.p = 0

    def cur(self) -> Token:
        return self.toks[self.p]

    def peek(self, k: int = 1) -> Token:
        idx = min(self.p + k, len(self.toks) - 1)
        return self.toks[idx]

    def at_kw(self, w: str) -> bool:
        t = self.cur()
        return t.kind == "KW" and t.val == w

    def at_op(self, op: str) -> bool:
        t = self.cur()
        return t.kind == "OP" and t.val == op

    def eat_kw(self, w: str) -> None:
        t = self.cur()
        if not (t.kind == "KW" and t.val == w):
            raise SyntaxError(f"expected keyword {w!r}, got {t}")
        self.p += 1

    def eat_op(self, op: str) -> None:
        t = self.cur()
        if not (t.kind == "OP" and t.val == op):
            raise SyntaxError(f"expected op {op!r}, got {t}")
        self.p += 1

    def eat_ident(self) -> str:
        t = self.cur()
        if t.kind == "IDENT":
            self.p += 1
            return t.val
        raise SyntaxError(f"expected IDENT, got {t}")

    def skip_type_annot(self) -> None:
        """Skip basic Luau type annotations when present in source inputs."""
        depth = 0
        while self.cur().kind != "EOF":
            t = self.cur()
            if t.kind == "OP" and t.val in ("(", "{", "<", "["):
                depth += 1
                self.p += 1
                continue
            if t.kind == "OP" and t.val in (")", "}", ">", "]"):
                if depth == 0:
                    break
                depth -= 1
                self.p += 1
                continue
            if depth == 0:
                if t.kind == "OP" and t.val in (",", "=", ";"):
                    break
                if t.kind == "KW" and t.val not in ("nil", "true", "false"):
                    break
                # Stop if next token starts a new statement at depth 0 after a simple type
                if t.kind in ("IDENT",) and self.peek().kind == "OP" and self.peek().val in ("?", "|", "&", ".", "->"):
                    self.p += 1
                    if self.cur().kind == "OP" and self.cur().val in ("?", "|", "&", ".", "->"):
                        self.p += 1
                        continue
                if t.kind == "IDENT":
                    self.p += 1
                    if self.cur().kind == "OP" and self.cur().val in ("<",):
                        continue
                    if self.cur().kind == "OP" and self.cur().val in ("?", "|", "&", "->"):
                        self.p += 1
                        continue
                    break
            self.p += 1

    def parse_block(self, terminators: set[str]) -> dict[str, Any]:
        stmts: list[dict[str, Any]] = []
        while self.cur().kind != "EOF":
            while self.at_op(";"):
                self.p += 1
            t = self.cur()
            if t.kind == "EOF" or (t.kind == "KW" and t.val in terminators):
                break
            stmts.append(self.parse_stmt())
            while self.at_op(";"):
                self.p += 1
        return {"t": "Block", "stmts": stmts}

    def parse_stmt(self) -> dict[str, Any]:
        t = self.cur()
        if t.kind == "KW":
            w = t.val
            if w == "local":
                self.p += 1
                if self.at_kw("function"):
                    self.p += 1
                    name = self.eat_ident()
                    fn = self.parse_func_body()
                    return {"t": "LocalFunc", "name": name, "fn": fn}
                names = [self.eat_ident()]
                if self.at_op(":"):
                    self.p += 1
                    self.skip_type_annot()
                while self.at_op(","):
                    self.p += 1
                    names.append(self.eat_ident())
                    if self.at_op(":"):
                        self.p += 1
                        self.skip_type_annot()
                vals: list[dict[str, Any]] = []
                if self.at_op("="):
                    self.p += 1
                    vals = self.parse_expr_list()
                return {"t": "Local", "names": names, "vals": vals}
            if w == "function":
                self.p += 1
                base = self.eat_ident()
                chain = [base]
                method: str | None = None
                while self.at_op("."):
                    self.p += 1
                    chain.append(self.eat_ident())
                if self.at_op(":"):
                    self.p += 1
                    method = self.eat_ident()
                fn = self.parse_func_body(implicit_self=method is not None)
                return {"t": "FuncDecl", "chain": chain, "method": method, "fn": fn}
            if w == "if":
                return self.parse_if_stmt()
            if w == "while":
                self.p += 1
                cond = self.parse_expr()
                self.eat_kw("do")
                body = self.parse_block({"end"})
                self.eat_kw("end")
                return {"t": "While", "cond": cond, "body": body}
            if w == "repeat":
                self.p += 1
                body = self.parse_block({"until"})
                self.eat_kw("until")
                cond = self.parse_expr()
                return {"t": "Repeat", "body": body, "cond": cond}
            if w == "do":
                self.p += 1
                body = self.parse_block({"end"})
                self.eat_kw("end")
                return {"t": "Do", "body": body}
            if w == "for":
                return self.parse_for_stmt()
            if w == "return":
                self.p += 1
                vals = []
                nt = self.cur()
                if nt.kind != "EOF" and not (nt.kind == "OP" and nt.val == ";") and not (
                    nt.kind == "KW" and nt.val in ("end", "else", "elseif", "until")
                ):
                    vals = self.parse_expr_list()
                return {"t": "Return", "vals": vals}
            if w == "break":
                self.p += 1
                return {"t": "Break"}
        # Luau contextual keyword `continue` or `type` / `export`
        if t.kind == "IDENT" and t.val == "continue":
            nxt = self.peek()
            if not (nxt.kind == "OP" and nxt.val in ("=", ",", "(", ".", "[", ":")) and nxt.kind not in ("STR", "LONGSTR"):
                self.p += 1
                return {"t": "Continue"}
        if t.kind == "IDENT" and t.val in ("type", "export"):
            if t.val == "export" and self.peek().kind == "IDENT" and self.peek().val == "type":
                self.p += 2
                self.eat_ident()
                if self.at_op("="):
                    self.p += 1
                    self.skip_type_annot()
                return {"t": "TypeDecl"}
            if t.val == "type" and self.peek().kind == "IDENT":
                self.p += 1
                self.eat_ident()
                if self.at_op("<"):
                    self.skip_type_annot()
                if self.at_op("="):
                    self.p += 1
                    self.skip_type_annot()
                return {"t": "TypeDecl"}
        # Primary/suffixed expression -> call statement, compound assign, or multi-assign
        first = self.parse_suffixed_expr()
        if self.cur().kind == "OP" and self.cur().val in COMPOUND_OPS:
            op = self.cur().val
            self.p += 1
            rhs = self.parse_expr()
            return {"t": "CompoundAssign", "op": op, "lhs": first, "rhs": rhs}
        if self.at_op("=") or self.at_op(","):
            targets = [first]
            while self.at_op(","):
                self.p += 1
                targets.append(self.parse_suffixed_expr())
            self.eat_op("=")
            vals = self.parse_expr_list()
            return {"t": "Assign", "targets": targets, "vals": vals}
        return {"t": "ExprStmt", "expr": first}

    def parse_if_stmt(self) -> dict[str, Any]:
        self.eat_kw("if")
        cond = self.parse_expr()
        self.eat_kw("then")
        thenb = self.parse_block({"elseif", "else", "end"})
        elsifs: list[tuple[dict[str, Any], dict[str, Any]]] = []
        while self.at_kw("elseif"):
            self.p += 1
            ec = self.parse_expr()
            self.eat_kw("then")
            eb = self.parse_block({"elseif", "else", "end"})
            elsifs.append((ec, eb))
        elseb: dict[str, Any] | None = None
        if self.at_kw("else"):
            self.p += 1
            elseb = self.parse_block({"end"})
        self.eat_kw("end")
        return {"t": "If", "cond": cond, "thenb": thenb, "elsifs": elsifs, "elseb": elseb}

    def parse_for_stmt(self) -> dict[str, Any]:
        self.eat_kw("for")
        v1 = self.eat_ident()
        if self.at_op(":"):
            self.p += 1
            self.skip_type_annot()
        if self.at_op("="):
            self.p += 1
            start = self.parse_expr()
            self.eat_op(",")
            limit = self.parse_expr()
            step = None
            if self.at_op(","):
                self.p += 1
                step = self.parse_expr()
            self.eat_kw("do")
            body = self.parse_block({"end"})
            self.eat_kw("end")
            return {"t": "ForNum", "var": v1, "start": start, "limit": limit, "step": step, "body": body}
        vars_ = [v1]
        while self.at_op(","):
            self.p += 1
            vars_.append(self.eat_ident())
            if self.at_op(":"):
                self.p += 1
                self.skip_type_annot()
        self.eat_kw("in")
        iters = self.parse_expr_list()
        self.eat_kw("do")
        body = self.parse_block({"end"})
        self.eat_kw("end")
        return {"t": "ForGen", "vars": vars_, "iters": iters, "body": body}

    def parse_func_body(self, implicit_self: bool = False) -> dict[str, Any]:
        if self.at_op("<"):
            self.skip_type_annot()
        self.eat_op("(")
        params: list[str] = ["self"] if implicit_self else []
        vararg = False
        if not self.at_op(")"):
            while True:
                if self.at_op("..."):
                    self.p += 1
                    vararg = True
                    if self.at_op(":"):
                        self.p += 1
                        self.skip_type_annot()
                    break
                params.append(self.eat_ident())
                if self.at_op(":"):
                    self.p += 1
                    self.skip_type_annot()
                if not self.at_op(","):
                    break
                self.p += 1
        self.eat_op(")")
        if self.at_op(":"):
            self.p += 1
            self.skip_type_annot()
        body = self.parse_block({"end"})
        self.eat_kw("end")
        return {"t": "Function", "params": params, "vararg": vararg, "body": body}

    def parse_expr_list(self) -> list[dict[str, Any]]:
        out = [self.parse_expr()]
        while self.at_op(","):
            self.p += 1
            out.append(self.parse_expr())
        return out

    def parse_expr(self, min_prec: int = 1) -> dict[str, Any]:
        lhs = self.parse_unary()
        while True:
            t = self.cur()
            if t.kind == "OP" and t.val == "::":
                self.p += 1
                self.skip_type_annot()
                continue
            op = t.val if t.kind in ("OP", "KW") else None
            if op not in BIN_PREC:
                break
            lprec, rprec = BIN_PREC[op]
            if lprec < min_prec:
                break
            self.p += 1
            rhs = self.parse_expr(rprec + 1 if lprec == rprec else rprec)
            lhs = {"t": "Bin", "op": op, "l": lhs, "r": rhs}
        return lhs

    def parse_unary(self) -> dict[str, Any]:
        t = self.cur()
        if (t.kind == "KW" and t.val == "not") or (t.kind == "OP" and t.val in ("-", "#")):
            op = t.val
            self.p += 1
            e = self.parse_expr(7)
            return {"t": "Un", "op": op, "e": e}
        return self.parse_suffixed_expr()

    def parse_primary(self) -> dict[str, Any]:
        t = self.cur()
        if t.kind == "KW":
            if t.val == "nil":
                self.p += 1
                return {"t": "Nil"}
            if t.val == "true":
                self.p += 1
                return {"t": "Bool", "v": True}
            if t.val == "false":
                self.p += 1
                return {"t": "Bool", "v": False}
            if t.val == "function":
                self.p += 1
                return self.parse_func_body()
            if t.val == "if":
                return self.parse_if_expr()
        if t.kind == "NUM":
            self.p += 1
            return {"t": "Num", "v": t.val}
        if t.kind == "STR":
            self.p += 1
            return {"t": "Str", "b": t.val, "long": False}
        if t.kind == "LONGSTR":
            self.p += 1
            return {"t": "Str", "b": t.val, "long": True}
        if t.kind == "IDENT":
            self.p += 1
            return {"t": "Ident", "name": t.val}
        if self.at_op("..."):
            self.p += 1
            return {"t": "Vararg"}
        if self.at_op("("):
            self.p += 1
            e = self.parse_expr()
            self.eat_op(")")
            return {"t": "Paren", "e": e}
        if self.at_op("{"):
            return self.parse_table()
        raise SyntaxError(f"unexpected token in expression: {t}")

    def parse_if_expr(self) -> dict[str, Any]:
        self.eat_kw("if")
        c0 = self.parse_expr()
        self.eat_kw("then")
        v0 = self.parse_expr()
        arms = [(c0, v0)]
        while self.at_kw("elseif"):
            self.p += 1
            ck = self.parse_expr()
            self.eat_kw("then")
            vk = self.parse_expr()
            arms.append((ck, vk))
        self.eat_kw("else")
        elsev = self.parse_expr()
        return {"t": "IfExpr", "arms": arms, "else": elsev}

    def parse_table(self) -> dict[str, Any]:
        self.eat_op("{")
        fields: list[dict[str, Any]] = []
        while not self.at_op("}") and self.cur().kind != "EOF":
            if self.at_op("["):
                self.p += 1
                k = self.parse_expr()
                self.eat_op("]")
                self.eat_op("=")
                v = self.parse_expr()
                fields.append({"kind": "Key", "k": k, "v": v})
            elif self.cur().kind == "IDENT" and self.peek().kind == "OP" and self.peek().val == "=":
                kname = self.eat_ident()
                self.eat_op("=")
                v = self.parse_expr()
                fields.append({"kind": "Key", "k": {"t": "Str", "b": kname.encode("utf-8"), "long": False}, "v": v})
            else:
                v = self.parse_expr()
                fields.append({"kind": "Array", "v": v})
            if self.at_op(",") or self.at_op(";"):
                self.p += 1
            else:
                break
        self.eat_op("}")
        return {"t": "Table", "fields": fields}

    def parse_call_args(self) -> list[dict[str, Any]]:
        if self.at_op("("):
            self.p += 1
            args: list[dict[str, Any]] = []
            if not self.at_op(")"):
                args = self.parse_expr_list()
            self.eat_op(")")
            return args
        if self.at_op("{"):
            return [self.parse_table()]
        t = self.cur()
        if t.kind in ("STR", "LONGSTR"):
            self.p += 1
            return [{"t": "Str", "b": t.val, "long": t.kind == "LONGSTR"}]
        raise SyntaxError(f"expected call args, got {t}")

    def parse_suffixed_expr(self) -> dict[str, Any]:
        e = self.parse_primary()
        while True:
            t = self.cur()
            if self.at_op("."):
                self.p += 1
                # Member name may be keyword-like identifier in some dialects, normally IDENT
                tok = self.cur()
                if tok.kind in ("IDENT", "KW"):
                    self.p += 1
                    e = {"t": "Dot", "obj": e, "name": tok.val}
                else:
                    raise SyntaxError(f"expected member name after '.', got {tok}")
            elif self.at_op("["):
                self.p += 1
                idx = self.parse_expr()
                self.eat_op("]")
                e = {"t": "Index", "obj": e, "idx": idx}
            elif self.at_op(":"):
                self.p += 1
                tok = self.cur()
                if tok.kind in ("IDENT", "KW"):
                    self.p += 1
                    mname = tok.val
                else:
                    raise SyntaxError(f"expected method name after ':', got {tok}")
                args = self.parse_call_args()
                e = {"t": "Method", "obj": e, "name": mname, "args": args}
            elif (t.kind == "OP" and t.val in ("(", "{")) or t.kind in ("STR", "LONGSTR"):
                args = self.parse_call_args()
                e = {"t": "Call", "fn": e, "args": args}
            else:
                break
        return e


# ---------------------------------------------------------------------------
# AST Structural Analysis, Scope Resolution, Predicate Prover & Clustering
# ---------------------------------------------------------------------------

def strip_parens(e: dict[str, Any]) -> dict[str, Any]:
    while e["t"] == "Paren":
        e = e["e"]
    return e


def canon_expr(e: dict[str, Any], var_map: dict[str, str] | None = None) -> str:
    """Canonical expression representation for algebraic equivalence & structural signature."""
    e = strip_parens(e)
    et = e["t"]
    if et == "Nil":
        return "nil"
    if et == "Bool":
        return "true" if e["v"] else "false"
    if et == "Num":
        if var_map is not None:
            return "N"
        v = e["v"]
        return str(int(v)) if v.is_integer() else repr(v)
    if et == "Str":
        if var_map is not None:
            return "LSTR" if e["long"] else "STR"
        return f"S({e['b']!r})"
    if et == "Vararg":
        return "..."
    if et == "Ident":
        nm = e["name"]
        if var_map is not None:
            return var_map.get(nm, "G")
        return f"Id({nm})"
    if et == "Un":
        return f"({e['op']} {canon_expr(e['e'], var_map)})"
    if et == "Bin":
        op = e["op"]
        l = canon_expr(e["l"], var_map)
        r = canon_expr(e["r"], var_map)
        if op == ">":
            op, l, r = "<", r, l
        elif op == ">=":
            op, l, r = "<=", r, l
        elif op in ("+", "*", "==", "~=") and l > r:
            l, r = r, l
        return f"({l} {op} {r})"
    if et == "Dot":
        obj = canon_expr(e["obj"], var_map)
        fld = "F" if var_map is not None else e["name"]
        return f"{obj}.{fld}"
    if et == "Index":
        return f"{canon_expr(e['obj'], var_map)}[{canon_expr(e['idx'], var_map)}]"
    if et == "Call":
        args = ",".join(canon_expr(a, var_map) for a in e["args"])
        return f"Call({canon_expr(e['fn'], var_map)};{args})"
    if et == "Method":
        args = ",".join(canon_expr(a, var_map) for a in e["args"])
        mn = "M" if var_map is not None else e["name"]
        return f"Meth({canon_expr(e['obj'], var_map)}:{mn};{args})"
    if et == "Table":
        items = []
        for f in e["fields"]:
            if f["kind"] == "Array":
                items.append(canon_expr(f["v"], var_map))
            else:
                items.append(f"[{canon_expr(f['k'], var_map)}]={canon_expr(f['v'], var_map)}")
        return "{" + ",".join(items) + "}"
    if et == "Function":
        return func_structural_sig(e)
    if et == "IfExpr":
        arms = ";".join(
            f"{canon_expr(c, var_map)}?{canon_expr(v, var_map)}" for c, v in e["arms"]
        )
        return f"IfE({arms}:{canon_expr(e['else'], var_map)})"
    return et


def stmt_structural_sig(st: dict[str, Any], var_map: dict[str, str]) -> str:
    t = st["t"]
    if t == "Local":
        for nm in st["names"]:
            if nm not in var_map:
                var_map[nm] = f"L{len(var_map)}"
        vs = ",".join(canon_expr(v, var_map) for v in st["vals"])
        return f"Loc({len(st['names'])}=[{vs}])"
    if t == "LocalFunc":
        var_map[st["name"]] = f"L{len(var_map)}"
        return f"LocFn({func_structural_sig(st['fn'])})"
    if t == "FuncDecl":
        return f"FnDecl({func_structural_sig(st['fn'])})"
    if t == "Assign":
        lhs = ",".join(canon_expr(x, var_map) for x in st["targets"])
        rhs = ",".join(canon_expr(x, var_map) for x in st["vals"])
        return f"Asg([{lhs}]=[{rhs}])"
    if t == "CompoundAssign":
        return f"CAsg({st['op']};{canon_expr(st['lhs'], var_map)};{canon_expr(st['rhs'], var_map)})"
    if t == "ExprStmt":
        return f"Exp({canon_expr(st['expr'], var_map)})"
    if t == "Return":
        return "Ret(" + ",".join(canon_expr(v, var_map) for v in st["vals"]) + ")"
    if t == "Break":
        return "Brk"
    if t == "Continue":
        return "Cont"
    if t == "Do":
        inner = dict(var_map)
        return "Do(" + ";".join(stmt_structural_sig(s, inner) for s in st["body"]["stmts"]) + ")"
    if t == "While":
        inner = dict(var_map)
        c = canon_expr(st["cond"], var_map)
        b = ";".join(stmt_structural_sig(s, inner) for s in st["body"]["stmts"])
        return f"While({c}){{{b}}}"
    if t == "Repeat":
        inner = dict(var_map)
        b = ";".join(stmt_structural_sig(s, inner) for s in st["body"]["stmts"])
        c = canon_expr(st["cond"], inner)
        return f"Repeat{{{b}}}Until({c})"
    if t == "ForNum":
        inner = dict(var_map)
        inner[st["var"]] = f"L{len(inner)}"
        step_s = canon_expr(st["step"], var_map) if st["step"] else "_"
        b = ";".join(stmt_structural_sig(s, inner) for s in st["body"]["stmts"])
        return f"ForN({canon_expr(st['start'], var_map)},{canon_expr(st['limit'], var_map)},{step_s}){{{b}}}"
    if t == "ForGen":
        inner = dict(var_map)
        for v in st["vars"]:
            inner[v] = f"L{len(inner)}"
        its = ",".join(canon_expr(x, var_map) for x in st["iters"])
        b = ";".join(stmt_structural_sig(s, inner) for s in st["body"]["stmts"])
        return f"ForG({len(st['vars'])} in {its}){{{b}}}"
    if t == "If":
        c = canon_expr(st["cond"], var_map)
        tb = ";".join(stmt_structural_sig(s, dict(var_map)) for s in st["thenb"]["stmts"])
        eifs = "".join(
            f"ElseIf({canon_expr(ec, var_map)}){{{';'.join(stmt_structural_sig(s, dict(var_map)) for s in eb['stmts'])}}}"
            for ec, eb in st["elsifs"]
        )
        eb_s = (
            "Else{" + ";".join(stmt_structural_sig(s, dict(var_map)) for s in st["elseb"]["stmts"]) + "}"
            if st["elseb"] is not None
            else ""
        )
        return f"If({c}){{{tb}}}{eifs}{eb_s}"
    return t


def func_structural_sig(fn: dict[str, Any]) -> str:
    var_map: dict[str, str] = {}
    for idx, p in enumerate(fn["params"]):
        var_map[p] = f"P{idx}"
    va = "+va" if fn["vararg"] else ""
    body_sig = ";".join(stmt_structural_sig(s, var_map) for s in fn["body"]["stmts"])
    return f"Fn({len(fn['params'])}{va}){{{body_sig}}}"


def is_pure_expr(e: dict[str, Any]) -> bool:
    """True when evaluating `e` has no function/method calls and is deterministic."""
    e = strip_parens(e)
    et = e["t"]
    if et in ("Nil", "Bool", "Num", "Str", "Ident"):
        return True
    if et == "Un":
        return is_pure_expr(e["e"])
    if et == "Bin":
        return is_pure_expr(e["l"]) and is_pure_expr(e["r"])
    return False


def is_nonneg_real_expr(e: dict[str, Any]) -> bool:
    """Conservative AST check for expressions guaranteed >= 0 over finite real numbers."""
    e = strip_parens(e)
    if not is_pure_expr(e):
        return False
    if e["t"] == "Num":
        return e["v"] >= 0
    if e["t"] == "Un" and e["op"] == "#" and e["e"]["t"] == "Str":
        return True
    if e["t"] == "Bin":
        op = e["op"]
        l, r = strip_parens(e["l"]), strip_parens(e["r"])
        # Square X * X >= 0
        if op == "*" and canon_expr(l) == canon_expr(r):
            return True
        if op == "*" and is_nonneg_real_expr(l) and is_nonneg_real_expr(r):
            return True
        if op == "+" and is_nonneg_real_expr(l) and is_nonneg_real_expr(r):
            return True
        # Even integer power X ^ (2k) >= 0
        if op == "^" and r["t"] == "Num" and r["v"] > 0 and r["v"].is_integer() and int(r["v"]) % 2 == 0:
            return True
        # Luau `x % m` with positive literal modulus m > 0 is always in [0, m) >= 0!
        if op == "%" and r["t"] == "Num" and r["v"] > 0:
            return True
    return False


def is_consec_even_mod2(e: dict[str, Any]) -> bool:
    """Check `((v*v + v) % 2) == 0` / `((v + v*v) % 2) == 0` / `(v*(v+1) % 2) == 0`."""
    e = strip_parens(e)
    if e["t"] != "Bin" or e["op"] not in ("==", "~="):
        return False
    l, r = strip_parens(e["l"]), strip_parens(e["r"])
    if r["t"] == "Num" and r["v"] == 0:
        mod_expr = l
    elif l["t"] == "Num" and l["v"] == 0:
        mod_expr = r
    else:
        return False
    if mod_expr["t"] != "Bin" or mod_expr["op"] != "%":
        return False
    mr = strip_parens(mod_expr["r"])
    if mr["t"] != "Num" or mr["v"] != 2:
        return False
    ml = strip_parens(mod_expr["l"])
    if not is_pure_expr(ml):
        return False
    if ml["t"] == "Bin" and ml["op"] == "+":
        a, b = strip_parens(ml["l"]), strip_parens(ml["r"])
        if a["t"] == "Bin" and a["op"] == "*" and canon_expr(a["l"]) == canon_expr(a["r"]) == canon_expr(b):
            return True
        if b["t"] == "Bin" and b["op"] == "*" and canon_expr(b["l"]) == canon_expr(b["r"]) == canon_expr(a):
            return True
    return False


def is_shifted_square_cmp(e: dict[str, Any]) -> bool:
    """Detect `(v + 1)*(v + 1) >= v*v` (reduces to 2v + 1 >= 0; domain-dependent!)."""
    e = strip_parens(e)
    if e["t"] != "Bin" or e["op"] not in (">=", "<=", ">", "<"):
        return False
    l, r = strip_parens(e["l"]), strip_parens(e["r"])
    if e["op"] in ("<=", "<"):
        l, r = r, l
    if not (l["t"] == "Bin" and l["op"] == "*" and canon_expr(l["l"]) == canon_expr(l["r"])):
        return False
    if not (r["t"] == "Bin" and r["op"] == "*" and canon_expr(r["l"]) == canon_expr(r["r"])):
        return False
    vp1 = strip_parens(l["l"])
    v = strip_parens(r["l"])
    if vp1["t"] == "Bin" and vp1["op"] == "+":
        a, b = strip_parens(vp1["l"]), strip_parens(vp1["r"])
        if (canon_expr(a) == canon_expr(v) and b["t"] == "Num" and b["v"] > 0) or (
            canon_expr(b) == canon_expr(v) and a["t"] == "Num" and a["v"] > 0
        ):
            return True
    return False


def classify_condition_ast(e: dict[str, Any]) -> str | None:
    """Return predicate classification or None if genuinely runtime-dependent."""
    e = strip_parens(e)
    et = e["t"]
    if et in ("Bool", "Nil", "Num", "Str", "Table", "Function"):
        return "const_literal"
    if et == "Un" and e["op"] == "not":
        return classify_condition_ast(e["e"])
    if is_consec_even_mod2(e):
        return "const_int_parity"
    if is_shifted_square_cmp(e):
        return "domain_dependent_linear"
    if et == "Bin":
        op = e["op"]
        l, r = strip_parens(e["l"]), strip_parens(e["r"])
        if is_pure_expr(l) and is_pure_expr(r):
            if canon_expr(l) == canon_expr(r) and op in ("==", "~=", "<=", ">=", "<", ">"):
                return "const_self_cmp"
            if op == ">=" and r["t"] == "Num" and r["v"] <= 0 and is_nonneg_real_expr(l):
                return "const_nonneg_cmp"
            if op == "<=" and l["t"] == "Num" and l["v"] <= 0 and is_nonneg_real_expr(r):
                return "const_nonneg_cmp"
            if op == "<" and r["t"] == "Num" and r["v"] <= 0 and is_nonneg_real_expr(l):
                return "const_nonneg_cmp"
            if op == ">" and l["t"] == "Num" and l["v"] <= 0 and is_nonneg_real_expr(r):
                return "const_nonneg_cmp"
    return None


def summarize_signatures(sigs: list[str]) -> dict[str, Any]:
    total = len(sigs)
    if total == 0:
        return {"total": 0, "unique_signatures": 0, "top_signature_count": 0, "top_signature_ratio": 0.0}
    ctr = collections.Counter(sigs)
    _, top_cnt = ctr.most_common(1)[0]
    return {
        "total": total,
        "unique_signatures": len(ctr),
        "top_signature_count": top_cnt,
        "top_signature_ratio": round(top_cnt / total, 4),
    }


def analyze_luau_ast(src: str) -> dict[str, Any]:
    tokens = lex_luau(src)
    parser = Parser(tokens)
    root = parser.parse_block(set())
    if parser.cur().kind != "EOF":
        raise SyntaxError(f"trailing unparsed token: {parser.cur()}")

    metrics = {
        "token_count": len(tokens) - 1,  # exclude EOF
        "ast_nodes": 0,
        "functions": 0,
        "branches": 0,
        "loops": 0,
        "ast_const_total": 0,
        "ast_str_const_count": 0,
        "ast_max_str_len": 0,
        "ast_short_str_max_len": 0,
    }
    global_roots: set[str] = set()
    global_apis: set[str] = set()
    str_meta_prims: set[str] = set()
    pred_counts: collections.Counter[str] = collections.Counter()
    identical_branch_arms = 0
    staging_sigs: list[str] = []
    hw_wrapper_sigs: list[str] = []
    all_fn_sigs: list[str] = []

    def in_scopes(scopes: list[set[str]], name: str) -> bool:
        return any(name in sc for sc in reversed(scopes))

    def visit_expr(e: dict[str, Any], scopes: list[set[str]], parent_is_member: bool = False) -> None:
        metrics["ast_nodes"] += 1
        et = e["t"]
        if et == "Nil":
            metrics["ast_const_total"] += 1
        elif et in ("Bool", "Num"):
            metrics["ast_const_total"] += 1
        elif et == "Str":
            metrics["ast_const_total"] += 1
            metrics["ast_str_const_count"] += 1
            blen = len(e["b"])
            if blen > metrics["ast_max_str_len"]:
                metrics["ast_max_str_len"] = blen
            if not e["long"] and blen > metrics["ast_short_str_max_len"]:
                metrics["ast_short_str_max_len"] = blen
        elif et == "Ident":
            nm = e["name"]
            if not in_scopes(scopes, nm):
                global_roots.add(nm)
                if not parent_is_member:
                    global_apis.add(nm)
        elif et == "Paren":
            visit_expr(e["e"], scopes, False)
        elif et == "Un":
            visit_expr(e["e"], scopes, False)
        elif et == "Bin":
            visit_expr(e["l"], scopes, False)
            visit_expr(e["r"], scopes, False)
        elif et == "Dot":
            obj = strip_parens(e["obj"])
            if obj["t"] == "Ident" and not in_scopes(scopes, obj["name"]):
                global_roots.add(obj["name"])
                global_apis.add(f"{obj['name']}.{e['name']}")
                visit_expr(e["obj"], scopes, True)
            else:
                visit_expr(e["obj"], scopes, False)
        elif et == "Index":
            obj = strip_parens(e["obj"])
            idx = strip_parens(e["idx"])
            if (
                obj["t"] == "Str"
                and len(obj["b"]) == 0
                and idx["t"] == "Str"
            ):
                str_meta_prims.add(f'("")[{idx["b"].decode("utf-8", "replace")!r}]')
            if (
                obj["t"] == "Ident"
                and not in_scopes(scopes, obj["name"])
                and idx["t"] == "Str"
            ):
                member = idx["b"].decode("utf-8", "replace")
                global_roots.add(obj["name"])
                global_apis.add(f"{obj['name']}.{member}")
                visit_expr(e["obj"], scopes, True)
                visit_expr(e["idx"], scopes, False)
            else:
                visit_expr(e["obj"], scopes, False)
                visit_expr(e["idx"], scopes, False)
        elif et == "Call":
            visit_expr(e["fn"], scopes, False)
            for a in e["args"]:
                visit_expr(a, scopes, False)
        elif et == "Method":
            obj = strip_parens(e["obj"])
            if obj["t"] == "Ident" and not in_scopes(scopes, obj["name"]):
                global_roots.add(obj["name"])
                global_apis.add(f"{obj['name']}:{e['name']}")
                visit_expr(e["obj"], scopes, True)
            else:
                visit_expr(e["obj"], scopes, False)
            for a in e["args"]:
                visit_expr(a, scopes, False)
        elif et == "Table":
            for f in e["fields"]:
                if f["kind"] == "Key":
                    visit_expr(f["k"], scopes, False)
                visit_expr(f["v"], scopes, False)
        elif et == "IfExpr":
            metrics["branches"] += len(e["arms"])
            for c, v in e["arms"]:
                cls = classify_condition_ast(c)
                if cls:
                    pred_counts[cls] += 1
                visit_expr(c, scopes, False)
                visit_expr(v, scopes, False)
            visit_expr(e["else"], scopes, False)
        elif et == "Function":
            visit_function(e, scopes)

    def visit_function(fn: dict[str, Any], scopes: list[set[str]]) -> None:
        metrics["functions"] += 1
        sig = func_structural_sig(fn)
        all_fn_sigs.append(sig)
        # Outer v15 staging wrappers have 7 formal parameters: (b, C, r1..r5)
        if len(fn["params"]) == 7 and not fn["vararg"]:
            staging_sigs.append(sig)
        # Inner VM opcode wrappers in HW2 have 5 formal parameters: (E, a, b, c, d)
        if len(fn["params"]) == 5 and not fn["vararg"]:
            hw_wrapper_sigs.append(sig)
        fn_scope = set(fn["params"])
        scopes.append(fn_scope)
        visit_block(fn["body"], scopes)
        scopes.pop()

    def visit_block(blk: dict[str, Any], scopes: list[set[str]]) -> None:
        nonlocal identical_branch_arms
        scopes.append(set())
        for st in blk["stmts"]:
            metrics["ast_nodes"] += 1
            t = st["t"]
            if t == "Local":
                for v in st["vals"]:
                    visit_expr(v, scopes, False)
                scopes[-1].update(st["names"])
            elif t == "LocalFunc":
                scopes[-1].add(st["name"])
                visit_function(st["fn"], scopes)
            elif t == "FuncDecl":
                base = st["chain"][0]
                if not in_scopes(scopes, base):
                    global_roots.add(base)
                    global_apis.add(".".join(st["chain"]))
                visit_function(st["fn"], scopes)
            elif t == "Assign":
                for x in st["targets"]:
                    if x["t"] == "Ident" and not in_scopes(scopes, x["name"]):
                        global_roots.add(x["name"])
                        global_apis.add(x["name"])
                    else:
                        visit_expr(x, scopes, False)
                for v in st["vals"]:
                    visit_expr(v, scopes, False)
            elif t == "CompoundAssign":
                visit_expr(st["lhs"], scopes, False)
                visit_expr(st["rhs"], scopes, False)
            elif t == "ExprStmt":
                visit_expr(st["expr"], scopes, False)
            elif t == "Return":
                for v in st["vals"]:
                    visit_expr(v, scopes, False)
            elif t == "Do":
                visit_block(st["body"], scopes)
            elif t == "While":
                metrics["loops"] += 1
                cls = classify_condition_ast(st["cond"])
                if cls:
                    pred_counts[cls] += 1
                visit_expr(st["cond"], scopes, False)
                visit_block(st["body"], scopes)
            elif t == "Repeat":
                metrics["loops"] += 1
                scopes.append(set())
                for inner_st in st["body"]["stmts"]:
                    # Repeat scope extends into until condition!
                    metrics["ast_nodes"] += 1
                    if inner_st["t"] == "Local":
                        for v in inner_st["vals"]:
                            visit_expr(v, scopes, False)
                        scopes[-1].update(inner_st["names"])
                    else:
                        # delegate via temporary single-stmt block without losing scope
                        visit_single_in_current_scope(inner_st, scopes)
                cls = classify_condition_ast(st["cond"])
                if cls:
                    pred_counts[cls] += 1
                visit_expr(st["cond"], scopes, False)
                scopes.pop()
            elif t == "ForNum":
                metrics["loops"] += 1
                visit_expr(st["start"], scopes, False)
                visit_expr(st["limit"], scopes, False)
                if st["step"] is not None:
                    visit_expr(st["step"], scopes, False)
                scopes.append({st["var"]})
                visit_block(st["body"], scopes)
                scopes.pop()
            elif t == "ForGen":
                metrics["loops"] += 1
                for it in st["iters"]:
                    visit_expr(it, scopes, False)
                scopes.append(set(st["vars"]))
                visit_block(st["body"], scopes)
                scopes.pop()
            elif t == "If":
                metrics["branches"] += 1 + len(st["elsifs"])
                cls = classify_condition_ast(st["cond"])
                if cls:
                    pred_counts[cls] += 1
                if (
                    not st["elsifs"]
                    and st["elseb"] is not None
                    and stmt_structural_sig({"t": "Do", "body": st["thenb"]}, {})
                    == stmt_structural_sig({"t": "Do", "body": st["elseb"]}, {})
                ):
                    identical_branch_arms += 1
                visit_expr(st["cond"], scopes, False)
                visit_block(st["thenb"], scopes)
                for ec, eb in st["elsifs"]:
                    ecls = classify_condition_ast(ec)
                    if ecls:
                        pred_counts[ecls] += 1
                    visit_expr(ec, scopes, False)
                    visit_block(eb, scopes)
                if st["elseb"] is not None:
                    visit_block(st["elseb"], scopes)
        scopes.pop()

    def visit_single_in_current_scope(st: dict[str, Any], scopes: list[set[str]]) -> None:
        t = st["t"]
        if t == "LocalFunc":
            scopes[-1].add(st["name"])
            visit_function(st["fn"], scopes)
        elif t == "Assign":
            for x in st["targets"]:
                visit_expr(x, scopes, False)
            for v in st["vals"]:
                visit_expr(v, scopes, False)
        elif t == "CompoundAssign":
            visit_expr(st["lhs"], scopes, False)
            visit_expr(st["rhs"], scopes, False)
        elif t == "ExprStmt":
            visit_expr(st["expr"], scopes, False)
        elif t == "Return":
            for v in st["vals"]:
                visit_expr(v, scopes, False)
        elif t in ("If", "While", "Repeat", "ForNum", "ForGen", "Do"):
            visit_block({"t": "Block", "stmts": [st]}, scopes)

    visit_block(root, [])

    provably_const_real = (
        pred_counts["const_literal"]
        + pred_counts["const_self_cmp"]
        + pred_counts["const_nonneg_cmp"]
    )
    provably_const_int = pred_counts["const_int_parity"]

    return {
        "analysis_mode": "ast",
        **metrics,
        "global_api_roots": sorted(global_roots),
        "global_api_root_count": len(global_roots),
        "global_api_refs": sorted(global_apis),
        "global_api_count": len(global_apis),
        "string_metatable_primitives": sorted(str_meta_prims),
        "predicates": {
            "provably_const_real": provably_const_real,
            "provably_const_int_parity": provably_const_int,
            "provably_const_total": provably_const_real + provably_const_int,
            "domain_dependent_linear": pred_counts["domain_dependent_linear"],
            "identical_branch_arms": identical_branch_arms,
            "breakdown": dict(pred_counts),
        },
        "wrappers": {
            "staging_7arg": summarize_signatures(staging_sigs),
            "vm_hw_5arg": summarize_signatures(hw_wrapper_sigs),
            "all_functions": summarize_signatures(all_fn_sigs),
        },
    }


# ---------------------------------------------------------------------------
# Diagnostic Suite Inputs (covering all P0 required language constructs)
# ---------------------------------------------------------------------------

DIAGNOSTIC_CASES: dict[str, str] = {
    "01_simple_expr": """\
local a = 17
local b = 29
local c = (a * 3 + b) // 2 - (a % 5)
print(c, a + b, b ^ 2)
""",
    "02_string_fn": """\
local function greet(who, tag)
    local msg = "hello, " .. who .. " [" .. tag .. "]"
    return #msg, string.sub(msg, 1, 12), msg
end
local n, head, full = greet("obsucvim", "v15")
print(n, head, full)
""",
    "03_closure_upval": """\
local function make_acc(init)
    local total = init
    local step = 1
    local function add(x)
        total += x * step
        step += 1
        return total
    end
    return add
end
local f = make_acc(10)
print(f(3), f(4), f(5))
""",
    "04_loop_branch": """\
local acc = 0
for i = 1, 25 do
    if i % 7 == 0 then
        continue
    elseif i % 2 == 0 then
        acc += i
    else
        acc -= 1
    end
end
local k = 5
while k > 0 do
    acc += k
    k -= 1
end
repeat
    k += 2
until k >= 6
print(acc, k)
""",
    "05_table_key": """\
local t = { alpha = 10, beta = 20, [3] = "three" }
local dyn = "al" .. "pha"
t[dyn] = t[dyn] + 5
t["gam" .. "ma"] = t.beta * 2
local keys = 0
local sum = 0
for k, v in pairs(t) do
    keys += 1
    if type(v) == "number" then
        sum += v
    end
end
print(keys, sum, t[3], t.gamma)
""",
    "06_vararg_multival": """\
local function tail_pair(x, y)
    return x + y, x * y, x - y
end
local function pack_sum(head, ...)
    local n = select("#", ...)
    local s = head
    for i = 1, n do
        s += select(i, ...)
    end
    return s, n, tail_pair(head, n)
end
local a, b, c, d, e = pack_sum(10, 2, 4, 6, (tail_pair(5, 3)))
print(a, b, c, d, e)
""",
    "07_coroutine": """\
local co = coroutine.create(function(start)
    local cur = start
    for i = 1, 3 do
        cur = coroutine.yield(cur + i)
    end
    return cur * 10
end)
local ok1, v1 = coroutine.resume(co, 5)
local ok2, v2 = coroutine.resume(co, v1 * 2)
local ok3, v3 = coroutine.resume(co, v2 + 1)
local ok4, v4 = coroutine.resume(co, v3 - 3)
print(ok1, v1, ok2, v2, ok3, v3, ok4, v4, coroutine.status(co))
""",
}

DEFAULT_SEEDS = [1, 7, 42, 4242, 31337, 999999]


def collect_env_metadata(tools_bin: pathlib.Path, obsucvim_bin: pathlib.Path) -> dict[str, Any]:
    def cmd_out(args: list[str]) -> str:
        p = subprocess.run(args, cwd=str(REPO_ROOT), capture_output=True, text=True)
        return p.stdout.strip()

    env = os.environ.copy()
    env["PATH"] = f"{tools_bin}:{env.get('PATH', '')}"
    rustc_v = subprocess.run(["rustc", "--version"], env=env, capture_output=True, text=True).stdout.strip()
    cargo_v = subprocess.run(["cargo", "--version"], env=env, capture_output=True, text=True).stdout.strip()
    example_path = REPO_ROOT / "example.txt"
    ex_info: dict[str, Any] = {}
    if example_path.is_file():
        data = example_path.read_bytes()
        ex_info = {
            "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
        }
    return {
        "branch": cmd_out(["git", "branch", "--show-current"]),
        "head": cmd_out(["git", "rev-parse", "HEAD"]),
        "git_status_short": cmd_out(["git", "status", "--short"]),
        "rustc_version": rustc_v,
        "cargo_version": cargo_v,
        "luau_bin": str(tools_bin / "luau"),
        "obsucvim_bin": str(obsucvim_bin),
        "example_txt": ex_info,
    }


def profile_existing_artifact(
    artifact_path: pathlib.Path,
    tools_bin: pathlib.Path,
    seed: int | str = "unknown",
    input_bytes: int | str = "unknown",
    vm_manifest: dict[str, Any] | None = None,
    build_stats: dict[str, Any] | None = None,
) -> dict[str, Any]:
    if not artifact_path.is_file():
        raise FileNotFoundError(f"missing artifact file: {artifact_path}")
    raw = artifact_path.read_bytes()
    sha = hashlib.sha256(raw).hexdigest()
    text = raw.decode("utf-8", errors="surrogateescape")
    lines = text.count("\n") + (0 if text.endswith("\n") else 1)
    ast_stats = analyze_luau_ast(text)
    luau_run = run_measured([str(tools_bin / "luau"), str(artifact_path)], timeout_sec=60.0)
    luac_run = run_measured([str(tools_bin / "luau-compile"), str(artifact_path)], timeout_sec=30.0)

    vm_info = vm_manifest if vm_manifest is not None else {
        "status": "encrypted_in_carrier (use --suite with audit build for exact pre-encryption VM opcode/const manifest)",
        "observable_hw_dispatch_slots": ast_stats["wrappers"]["vm_hw_5arg"]["total"],
    }

    return {
        "path": str(artifact_path),
        "seed": seed,
        "input_bytes": input_bytes,
        "output_bytes": len(raw),
        "sha256": sha,
        "line_count": lines,
        "luau_compile_ok": luac_run["rc"] == 0,
        "run_rc": luau_run["rc"],
        "run_wall_ms": luau_run["wall_ms"],
        "run_peak_rss_kb": luau_run["peak_rss_kb"],
        "build_wall_ms": build_stats["wall_ms"] if build_stats else "unavailable",
        "build_peak_rss_kb": build_stats["peak_rss_kb"] if build_stats else "unavailable",
        "ast": ast_stats,
        "vm_manifest": vm_info,
    }


def run_suite(
    seeds: list[int],
    tools_bin: pathlib.Path,
    obsucvim_bin: pathlib.Path,
    audit_bin: pathlib.Path | None,
    extra_flags: list[str] | None = None,
) -> dict[str, Any]:
    extra_flags = extra_flags or []
    luau = str(tools_bin / "luau")
    suite_results: dict[str, Any] = {}

    with tempfile.TemporaryDirectory(prefix="obsucvim_profile_") as td:
        td_path = pathlib.Path(td)
        for case_name, src_code in DIAGNOSTIC_CASES.items():
            src_file = td_path / f"{case_name}.lua"
            src_bytes = src_code.encode("utf-8")
            src_file.write_bytes(src_bytes)

            ref_run = run_measured([luau, str(src_file)], timeout_sec=30.0)
            if ref_run["rc"] != 0:
                raise RuntimeError(
                    f"diagnostic source {case_name} failed under Luau: {ref_run['stderr']!r}"
                )

            per_seed: list[dict[str, Any]] = []
            sha_set: set[str] = set()
            determinism_ok = True

            for idx, seed in enumerate(seeds):
                out_file = td_path / f"{case_name}.s{seed}.lua"
                cmd = [
                    str(obsucvim_bin),
                    "--preset",
                    "v15",
                    "--dialect",
                    "luau",
                    "--seed",
                    str(seed),
                    *extra_flags,
                    str(src_file),
                    str(out_file),
                ]
                bstat = run_measured(cmd, timeout_sec=60.0)
                if bstat["rc"] != 0:
                    raise RuntimeError(
                        f"obsucvim build failed on {case_name} seed={seed}: {bstat['stderr'].decode('utf-8', 'replace')}"
                    )

                # Verify same-seed determinism on the first two seeds of every case
                if idx < 2:
                    out_dup = td_path / f"{case_name}.s{seed}.dup.lua"
                    cmd_dup = [*cmd[:-1], str(out_dup)]
                    bdup = run_measured(cmd_dup, timeout_sec=60.0)
                    if bdup["rc"] != 0 or out_file.read_bytes() != out_dup.read_bytes():
                        determinism_ok = False

                # Optional audit manifest (from debug/audit binary if supported)
                vm_manifest = None
                if audit_bin is not None:
                    mani_path = td_path / f"{case_name}.s{seed}.vm.json"
                    out_audit = td_path / f"{case_name}.s{seed}.audit.lua"
                    env = os.environ.copy()
                    env["OBSUCVIM_PROFILE_AUDIT"] = str(mani_path)
                    acmd = [
                        str(audit_bin),
                        "--preset",
                        "v15",
                        "--dialect",
                        "luau",
                        "--seed",
                        str(seed),
                        *extra_flags,
                        str(src_file),
                        str(out_audit),
                    ]
                    arun = run_measured(acmd, env=env, timeout_sec=60.0)
                    if arun["rc"] == 0 and mani_path.is_file():
                        # Audit manifest must NEVER change emitted bytes!
                        if out_audit.read_bytes() != out_file.read_bytes():
                            raise RuntimeError(
                                f"OBSUCVIM_PROFILE_AUDIT altered output bytes for {case_name} seed={seed}!"
                            )
                        vm_manifest = json.loads(mani_path.read_text(encoding="utf-8"))

                prof = profile_existing_artifact(
                    out_file,
                    tools_bin=tools_bin,
                    seed=seed,
                    input_bytes=len(src_bytes),
                    vm_manifest=vm_manifest,
                    build_stats=bstat,
                )
                # Verify semantic equivalence (stdout + exit code)
                run_res = run_measured([luau, str(out_file)], timeout_sec=60.0)
                sem_ok = (
                    run_res["rc"] == ref_run["rc"]
                    and run_res["stdout"] == ref_run["stdout"]
                    and prof["luau_compile_ok"]
                )
                prof["semantic_equivalent"] = sem_ok
                if not sem_ok:
                    raise RuntimeError(
                        f"semantic mismatch on {case_name} seed={seed}: "
                        f"rc {run_res['rc']} vs {ref_run['rc']}, "
                        f"out {run_res['stdout']!r} vs {ref_run['stdout']!r}"
                    )
                sha_set.add(prof["sha256"])
                per_seed.append(prof)

            suite_results[case_name] = {
                "input_bytes": len(src_bytes),
                "determinism_verified": determinism_ok,
                "distinct_seed_hashes": len(sha_set),
                "expected_seed_count": len(seeds),
                "all_seeds_randomized": len(sha_set) == len(seeds),
                "all_semantically_equivalent": all(p["semantic_equivalent"] for p in per_seed),
                "summary": {
                    "median_output_bytes": int(statistics.median(p["output_bytes"] for p in per_seed)),
                    "median_build_ms": round(statistics.median(p["build_wall_ms"] for p in per_seed), 2),
                    "median_run_ms": round(statistics.median(p["run_wall_ms"] for p in per_seed), 2),
                    "global_api_count_range": [
                        min(p["ast"]["global_api_count"] for p in per_seed),
                        max(p["ast"]["global_api_count"] for p in per_seed),
                    ],
                    "global_api_refs_seed42": next(
                        (p["ast"]["global_api_refs"] for p in per_seed if p["seed"] == 42),
                        per_seed[0]["ast"]["global_api_refs"],
                    ),
                    "provably_const_real_median": statistics.median(
                        p["ast"]["predicates"]["provably_const_real"] for p in per_seed
                    ),
                    "provably_const_total_median": statistics.median(
                        p["ast"]["predicates"]["provably_const_total"] for p in per_seed
                    ),
                    "staging_wrapper_top_share_median": round(
                        statistics.median(
                            p["ast"]["wrappers"]["staging_7arg"]["top_signature_ratio"]
                            for p in per_seed
                        ),
                        4,
                    ),
                    "vm_hw_wrapper_top_share_median": round(
                        statistics.median(
                            p["ast"]["wrappers"]["vm_hw_5arg"]["top_signature_ratio"]
                            for p in per_seed
                        ),
                        4,
                    ),
                },
                "seeds": per_seed,
            }

    return suite_results


def print_summary(report: dict[str, Any]) -> None:
    meta = report["environment"]
    print(f"== Obsucvim Artifact Profile ==")
    print(f"Branch : {meta['branch']} @ {meta['head'][:12]}")
    print(f"Rustc  : {meta['rustc_version']}")
    if meta.get("example_txt"):
        ex = meta["example_txt"]
        print(f"example.txt : {ex['bytes']} B  sha256={ex['sha256']}")

    for art in report.get("artifacts", []):
        a = art["ast"]
        p = a["predicates"]
        ws = a["wrappers"]["staging_7arg"]
        wh = a["wrappers"]["vm_hw_5arg"]
        print(f"\n[Artifact] {art['path']}")
        print(
            f"  bytes={art['output_bytes']} (in={art['input_bytes']})  lines={art['line_count']}  "
            f"sha256={art['sha256'][:16]}..."
        )
        print(
            f"  tokens={a['token_count']}  ast_nodes={a['ast_nodes']}  "
            f"fns={a['functions']}  branches={a['branches']}  loops={a['loops']}"
        )
        print(
            f"  global_apis={a['global_api_count']} (roots={a['global_api_root_count']})  "
            f"str_meta_prims={len(a['string_metatable_primitives'])}"
        )
        print(
            f"  ast_consts={a['ast_const_total']} (strs={a['ast_str_const_count']}, "
            f"max_str={a['ast_max_str_len']}, max_short_str={a['ast_short_str_max_len']})"
        )
        print(
            f"  predicates(AST): const_real={p['provably_const_real']}  "
            f"const_int_parity={p['provably_const_int_parity']}  "
            f"const_total={p['provably_const_total']}  "
            f"domain_linear={p['domain_dependent_linear']}  "
            f"ident_arms={p['identical_branch_arms']}"
        )
        print(
            f"  wrappers: staging_7arg={ws['unique_signatures']}/{ws['total']} "
            f"(top={ws['top_signature_ratio']*100:.1f}%)  "
            f"vm_hw_5arg={wh['unique_signatures']}/{wh['total']} "
            f"(top={wh['top_signature_ratio']*100:.1f}%)"
        )
        print(
            f"  run: rc={art['run_rc']}  wall={art['run_wall_ms']} ms  "
            f"peak_rss={art['run_peak_rss_kb']} KB"
        )

    if "suite" in report:
        print("\n[Diagnostic Suite Summary]")
        print(
            f"{'Case':<20} {'InB':>5} {'MedOutB':>8} {'APIs':>7} {'ConstP':>7} "
            f"{'StgTop%':>8} {'HWTop%':>8} {'BldMs':>7} {'RunMs':>7} {'Det/Rnd':>8}"
        )
        for name, c in report["suite"].items():
            s = c["summary"]
            apis = f"{s['global_api_count_range'][0]}..{s['global_api_count_range'][1]}"
            dr = f"{'Y' if c['determinism_verified'] else 'N'}/{c['distinct_seed_hashes']}"
            print(
                f"{name:<20} {c['input_bytes']:>5} {s['median_output_bytes']:>8} {apis:>7} "
                f"{s['provably_const_total_median']:>7.1f} "
                f"{s['staging_wrapper_top_share_median']*100:>7.1f}% "
                f"{s['vm_hw_wrapper_top_share_median']*100:>7.1f}% "
                f"{s['median_build_ms']:>7.1f} {s['median_run_ms']:>7.1f} {dr:>8}"
            )


def main() -> int:
    ap = argparse.ArgumentParser(description="Obsucvim v15 artifact profiler")
    ap.add_argument("--artifact", action="append", default=[], help="Existing artifact path to profile")
    ap.add_argument("--suite", action="store_true", help="Run 7-case multi-seed diagnostic suite")
    ap.add_argument(
        "--seeds",
        default=",".join(str(s) for s in DEFAULT_SEEDS),
        help="Comma-separated seed list for --suite",
    )
    ap.add_argument("--flag", action="append", default=[], help="Extra CLI flag for obsucvim in --suite")
    ap.add_argument("--out", default="", help="Optional JSON output path")
    ap.add_argument("--quiet", action="store_true", help="Suppress human-readable table")
    args = ap.parse_args()

    if not args.artifact and not args.suite:
        ap.error("specify --artifact <file> and/or --suite")

    try:
        tools_bin = find_tools_bin()
        obsucvim_bin = find_obsucvim_bin(prefer_audit=False)
    except RuntimeError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    audit_bin: pathlib.Path | None = None
    dbg_cand = ENGINE_ROOT / "target" / "debug" / "obsucvim"
    if dbg_cand.is_file() and os.access(dbg_cand, os.X_OK):
        audit_bin = dbg_cand
    else:
        audit_bin = obsucvim_bin

    report: dict[str, Any] = {
        "environment": collect_env_metadata(tools_bin, obsucvim_bin),
    }

    if args.artifact:
        arts = []
        for p in args.artifact:
            arts.append(profile_existing_artifact(pathlib.Path(p), tools_bin=tools_bin))
        report["artifacts"] = arts

    if args.suite:
        seeds = [int(x.strip()) for x in args.seeds.split(",") if x.strip()]
        report["suite"] = run_suite(
            seeds=seeds,
            tools_bin=tools_bin,
            obsucvim_bin=obsucvim_bin,
            audit_bin=audit_bin,
            extra_flags=args.flag,
        )

    if args.out:
        out_p = pathlib.Path(args.out)
        out_p.parent.mkdir(parents=True, exist_ok=True)
        out_p.write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8")

    if not args.quiet:
        print_summary(report)
    return 0


if __name__ == "__main__":
    sys.exit(main())
