#!/usr/bin/env python3
"""Plaintext-surface repair: release-artifact lexical scan + semantics.

The CCPD-era check (`grep ccpd`) only proved one module name was gone.
This suite is the general gate from the all-plaintext repair plan:

  A. A small Lua/Luau lexer splits every generated artifact into
     comments, string literals (decoded), identifiers and symbols.
     Release artifacts must contain:
       * no comments and no brand/version/audit text (`Obsucvim`,
         `OBSUCVIM_*`, `CCPD`, readable diagnostics);
       * no readable runtime string literal: type names, library or
         member names, metamethod keys, error-message text and format
         patterns must all ride the sealed numeric-code pool;
       * no host API identifier outside the documented bootstrap
         allowlist (see BOOTSTRAP_IDENTS below);
  B. User-program sentinel strings (normal, long, table-key, nested
     closure and binary/NUL) must be statically invisible in the
     artifact and byte-exact at runtime;
  C. The indirect global/library resolution must preserve dynamic
     host semantics: `print` replaced through the environment,
     environment tables with `__index`, and a wrapped `table.concat`
     must all be honoured at call time;
  D. Across a seed/minify/CCPD/super-insn/guard/proto-layout matrix.

Bootstrap allowlist (the minimum a standalone Luau file needs before
any sealed machinery can run; each entry is load-bearing):
  * `getfenv`, `_G`  - environment-table anchors for the prim
    bootstrap in template.rs and the v15 scaffold;
  * `type`, `pcall`  - the guard preamble's first two raw captures,
    taken before the env table exists so tampering with them is
    detectable (guard.rs);
  * the string literal `"char"` - obtains `string.char` from the
    empty-string metatable; every other name is numeric codes.

Usage: python3 tests/plaintext_surface_test.py
Exit code: 0 = all assertions hold, 1 = at least one failed.
"""
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)  # engine/
REPO = os.path.dirname(ROOT)
BIN = os.environ.get("OBSUCVIM_BIN", os.path.join(ROOT, "target", "release", "obsucvim"))
TOOLS_BIN = os.environ.get("OBSUCVIM_TOOLS", os.path.join(REPO, ".tools", "bin"))
LUAU = os.path.join(TOOLS_BIN, "luau")

checks = []


def check(name, ok, detail=""):
    checks.append((name, ok))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}{((' :: ' + detail) if detail and not ok else '')}")


# ---------------------------------------------------------------------------
# Lexer
# ---------------------------------------------------------------------------

def _long_bracket_level(text, i):
    """If text[i] == '[' opens a long bracket, return (level, content_start)."""
    if i >= len(text) or text[i] != "[":
        return None
    j = i + 1
    lvl = 0
    while j < len(text) and text[j] == "=":
        lvl += 1
        j += 1
    if j < len(text) and text[j] == "[":
        return lvl, j + 1
    return None


def _decode_short(raw):
    """Decode a short string body (between quotes) best-effort."""
    out = []
    i = 0
    while i < len(raw):
        c = raw[i]
        if c != "\\" or i + 1 >= len(raw):
            out.append(c)
            i += 1
            continue
        n = raw[i + 1]
        if n.isdigit():
            j = i + 1
            num = ""
            while j < len(raw) and raw[j].isdigit() and len(num) < 3:
                num += raw[j]
                j += 1
            out.append(chr(int(num) % 256))
            i = j
        elif n == "n":
            out.append("\n"); i += 2
        elif n == "t":
            out.append("\t"); i += 2
        elif n == "r":
            out.append("\r"); i += 2
        elif n == "x" and i + 3 < len(raw) + 1:
            try:
                out.append(chr(int(raw[i + 2:i + 4], 16)))
                i += 4
            except ValueError:
                out.append(n); i += 2
        elif n == "z":
            i += 2
            while i < len(raw) and raw[i] in " \t\r\n":
                i += 1
        else:
            out.append(n); i += 2
    return "".join(out)


def lex(text):
    """Yield (kind, value) tokens: comment/string/ident/number/symbol."""
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c in " \t\r\n":
            i += 1
            continue
        if text.startswith("--", i):
            lb = _long_bracket_level(text, i + 2)
            if lb:
                lvl, start = lb
                close = "]" + "=" * lvl + "]"
                j = text.find(close, start)
                j = n if j < 0 else j + len(close)
                yield ("comment", text[i:j])
                i = j
            else:
                j = text.find("\n", i)
                j = n if j < 0 else j
                yield ("comment", text[i:j])
                i = j
            continue
        lb = _long_bracket_level(text, i)
        if lb:
            lvl, start = lb
            close = "]" + "=" * lvl + "]"
            j = text.find(close, start)
            body = text[start:(n if j < 0 else j)]
            yield ("string", body)
            i = n if j < 0 else j + len(close)
            continue
        if c in "\"'":
            q = c
            j = i + 1
            raw = []
            while j < n:
                if text[j] == "\\" and j + 1 < n:
                    raw.append(text[j:j + 2])
                    j += 2
                    continue
                if text[j] == q:
                    break
                raw.append(text[j])
                j += 1
            yield ("string", _decode_short("".join(raw)))
            i = j + 1
            continue
        if c.isalpha() or c == "_":
            j = i
            while j < n and (text[j].isalnum() or text[j] == "_"):
                j += 1
            yield ("ident", text[i:j])
            i = j
            continue
        if c.isdigit():
            j = i
            while j < n and (text[j].isalnum() or text[j] in "._"):
                j += 1
            yield ("number", text[i:j])
            i = j
            continue
        if text.startswith("...", i):
            yield ("symbol", "...")
            i += 3
            continue
        if text.startswith("..", i):
            yield ("symbol", "..")
            i += 2
            continue
        yield ("symbol", c)
        i += 1


# ---------------------------------------------------------------------------
# Policy
# ---------------------------------------------------------------------------

BOOTSTRAP_IDENTS = {"getfenv", "_G", "type", "pcall"}  # documented above
BOOTSTRAP_STRINGS = {"char", ""}  # string.char anchor + empty string

TYPE_NAMES = {"function", "table", "number", "string", "boolean", "thread", "userdata"}
LIB_NAMES = {
    "bit32", "math", "os", "debug", "buffer", "coroutine", "task", "utf8",
    "game", "workspace", "Instance", "Vector3", "Vector2", "Enum", "Random",
}
MEMBER_NAMES = {
    "byte", "sub", "concat", "pack", "unpack", "floor", "clock", "info",
    "bxor", "band", "lshift", "rshift", "extract", "rep", "len", "gmatch",
    "fromstring", "tostring", "readu8", "writeu8", "create", "defer", "new",
}
GLOBAL_NAMES = {
    "getfenv", "setfenv", "getmetatable", "setmetatable", "rawget", "rawset",
    "rawequal", "tostring", "tonumber", "select", "xpcall", "error", "print",
    "warn", "next", "typeof", "loadstring", "load", "newproxy", "collectgarbage",
    "pairs", "ipairs", "assert",
}
METAMETHODS = {
    "__index", "__newindex", "__call", "__add", "__sub", "__mul", "__div",
    "__mod", "__pow", "__idiv", "__unm", "__len", "__concat", "__lt",
    "__le", "__eq", "__tostring", "__metatable", "__mode", "__iter",
}
ERROR_FRAGMENTS = (
    "attempt to perform arithmetic", "attempt to index", "attempt to call",
    "attempt to concatenate", "attempt to compare", "invalid runtime payload",
    "invalid control transfer", "lazy field",
)

BANNED_IDENTS = (LIB_NAMES | MEMBER_NAMES | GLOBAL_NAMES | METAMETHODS) - BOOTSTRAP_IDENTS
BANNED_STRINGS = (
    TYPE_NAMES | LIB_NAMES | MEMBER_NAMES | GLOBAL_NAMES | METAMETHODS
) - BOOTSTRAP_STRINGS
RAW_BANS = ("obsucvim", "ccpd", "fn=", "variant=", "po=", "pl=")


def _declared_locals(toks):
    """Names bound by local/function/for, so random minifier names that
    happen to spell an API word are not mistaken for global references."""
    names = set()
    i = 0
    n = len(toks)
    while i < n:
        kind, val = toks[i]
        if kind == "ident" and val == "local":
            j = i + 1
            if j < n and toks[j] == ("ident", "function"):
                if j + 1 < n and toks[j + 1][0] == "ident":
                    names.add(toks[j + 1][1])
                j += 2
            while j < n and toks[j][0] == "ident":
                names.add(toks[j][1])
                j += 1
                if j < n and toks[j] == ("symbol", ","):
                    j += 1
                    continue
                break
            i = j
            continue
        if kind == "ident" and val == "function":
            # parameter list
            j = i + 1
            if j < n and toks[j][0] == "ident":
                j += 1  # method/function name
            if j < n and toks[j] == ("symbol", "("):
                depth = 0
                while j < n:
                    if toks[j] == ("symbol", "("):
                        depth += 1
                    elif toks[j] == ("symbol", ")"):
                        depth -= 1
                        if depth == 0:
                            break
                    elif depth == 1 and toks[j][0] == "ident":
                        names.add(toks[j][1])
                    j += 1
                i = j
            continue
        if kind == "ident" and val == "for":
            j = i + 1
            while j < n and toks[j][0] == "ident":
                names.add(toks[j][1])
                j += 1
                if j < n and toks[j] == ("symbol", ","):
                    j += 1
                    continue
                break
            i = j
            continue
        i += 1
    return names


def scan_artifact(name, text):
    toks = list(lex(text))
    comments = [v for k, v in toks if k == "comment"]
    check(f"{name}: no comments or watermark", not comments,
          f"comments={comments[:2]!r}")
    # Raw-marker scan with high-entropy payload literals masked: random
    # ciphertext can contain "fn="-shaped substrings by chance, while a
    # real diagnostic field would sit in code or a short string.
    masked_parts = []
    for k, v in toks:
        masked_parts.append('""' if k == "string" and len(v) > 160 else (v if k != "string" else '"' + v + '"'))
    masked = " ".join(masked_parts).lower()
    raw_hits = [b for b in RAW_BANS if b in masked]
    check(f"{name}: no brand/internal raw markers", not raw_hits,
          f"hits={raw_hits}")
    bad_strings = sorted({
        v for k, v in toks
        if k == "string" and (
            v in BANNED_STRINGS
            or any(f in v for f in ERROR_FRAGMENTS)
            or (len(v) <= 96 and re.search(r"%[diouxXfeEgGcsqa]", v))
        )
    })
    check(f"{name}: no readable runtime string literals", not bad_strings,
          f"strings={[x[:40] for x in bad_strings[:6]]!r}")
    locals_ = _declared_locals(toks)
    bad_idents = set()
    for idx, (k, v) in enumerate(toks):
        if k != "ident":
            continue
        prev = toks[idx - 1] if idx else (None, None)
        if prev == ("symbol", ".") and (v in BANNED_IDENTS or v in METAMETHODS):
            bad_idents.add("." + v)
        elif v in METAMETHODS:
            bad_idents.add(v)
        elif v in BANNED_IDENTS and len(v) >= 3 and v not in locals_:
            bad_idents.add(v)
    check(f"{name}: no host API identifiers outside bootstrap allowlist",
          not bad_idents, f"idents={sorted(bad_idents)[:8]!r}")


# ---------------------------------------------------------------------------
# Drivers
# ---------------------------------------------------------------------------

def gen(src_path, out_path, seed, extra):
    proc = subprocess.run(
        [BIN, "--preset", "v15", "--dialect", "luau", "--seed", str(seed),
         str(src_path), str(out_path)] + extra,
        capture_output=True, text=True, timeout=180)
    return proc.returncode == 0, proc.stderr.strip()


def run_luau_bytes(path):
    proc = subprocess.run([LUAU, str(path)], capture_output=True, timeout=60)
    return proc.returncode, proc.stdout


def long_str(s):
    for lvl in range(1, 32):
        eq = "=" * lvl
        if f"]{eq}]" not in s:
            return f"[{eq}[\n{s}]{eq}]"
    raise RuntimeError("cannot wrap")


def run_in_sandbox(tmp, tag, art_txt, env_setup):
    harness = Path(tmp) / f"sb_{tag}.lua"
    harness.write_text(
        f"""local ART = {long_str(art_txt)}
local REAL_G = getfenv(0)
local ENV = {{}}
for k, v in pairs(REAL_G) do ENV[k] = v end
ENV._G = ENV
{env_setup}
local chunk = assert(REAL_G.loadstring(ART, {tag!r}))
setfenv(chunk, ENV)
chunk()
""")
    proc = subprocess.run([LUAU, str(harness)], capture_output=True, timeout=60)
    return proc.returncode, proc.stdout


RICH_SRC = r"""
local t = {}
local mt = { __add = function(a, b) return {v = a.v + b.v} end,
             __index = function(_, k) return "miss:" .. k end }
local a = setmetatable({v = 40}, mt)
local b = a + setmetatable({v = 2}, mt)
print(b.v, a.nope)
local co = coroutine.create(function(x) local y = coroutine.yield(x + 1) return y * 2 end)
local ok1, r1 = coroutine.resume(co, 10)
local ok2, r2 = coroutine.resume(co, 21)
print(ok1, r1, ok2, r2)
local parts = {}
for i = 1, 5 do parts[i] = string.rep("ab", i) end
print(table.concat(parts, "-"), string.sub("hello world", 7), #parts)
print(math.floor(9.7), math.max(3, 9), type(t), type(print), type(nil))
local pok, perr = pcall(function() error("user boom") end)
print(pok, perr ~= nil)
for k, v in pairs({x = 1}) do print(k, v) end
"""

SENTINEL_SRC = r"""
local a = "USER_LITERAL_SENTINEL_A7F3"
local b = [[LONG_LITERAL_SENTINEL_B91C
second line here]]
local t = { TABLE_KEY_SENTINEL_C42D = 7 }
t[a] = 1
local function mk()
  return function() return "NESTED_LITERAL_SENTINEL_D610" end
end
local bin = "\0\1\2\254\255BIN"
print(a, #b, t.TABLE_KEY_SENTINEL_C42D, t[a], mk()(), #bin, string.byte(bin, 7))
"""
SENTINELS = [
    "USER_LITERAL_SENTINEL_A7F3", "LONG_LITERAL_SENTINEL_B91C",
    "TABLE_KEY_SENTINEL_C42D", "NESTED_LITERAL_SENTINEL_D610",
]


def main():
    with tempfile.TemporaryDirectory(prefix="plaintext_") as td:
        tmp = Path(td)
        rich = tmp / "rich.lua"
        rich.write_text(RICH_SRC)
        rc0, ref = run_luau_bytes(rich)
        check("oracle rich program runs", rc0 == 0)

        matrix = []
        for seed in (7, 42, 123, 777):
            matrix.append((f"seed{seed}", seed, ["--ccpd"]))
            matrix.append((f"seed{seed}", seed, []))
        matrix += [
            ("seed42-nominify", 42, ["--ccpd", "--no-minify"]),
            ("seed42-super", 42, ["--ccpd", "--super-insn"]),
            ("seed42-noguard", 42, ["--ccpd", "--no-guard"]),
            ("seed42-noprotolayout", 42, ["--ccpd", "--no-proto-layout"]),
            ("seed42-envcheck", 42, ["--ccpd", "--env-check"]),
            ("seed42-guard", 42, ["--ccpd", "--guard"]),
        ]
        for tag, seed, extra in matrix:
            out = tmp / f"rich_{tag}.lua"
            ok, err = gen(rich, out, seed, extra)
            if not ok:
                check(f"matrix {tag} generates", False, err[:200])
                continue
            text = out.read_text(errors="surrogateescape")
            scan_artifact(f"matrix {tag}", text)
            rc, got = run_luau_bytes(out)
            check(f"matrix {tag} runtime matches oracle",
                  rc == rc0 and got == ref, f"rc={rc}")

        # Legacy VM preset (same generate() prim path) in scan matrix.
        vm_out = tmp / "rich_vm.lua"
        proc = subprocess.run(
            [BIN, "--preset", "vm", "--dialect", "luau", "--seed", "42",
             str(rich), str(vm_out)], capture_output=True, text=True, timeout=180)
        check("preset vm generates", proc.returncode == 0, proc.stderr[:200])
        if proc.returncode == 0:
            scan_artifact("preset vm", vm_out.read_text(errors="surrogateescape"))
            rc, got = run_luau_bytes(vm_out)
            check("preset vm runtime matches oracle", rc == rc0 and got == ref)

        # Sentinel coverage.
        sent = tmp / "sentinel.lua"
        sent.write_text(SENTINEL_SRC)
        rc_s, ref_s = run_luau_bytes(sent)
        for tag, extra in (("ccpd", ["--ccpd"]), ("off", []),
                           ("nominify", ["--ccpd", "--no-minify"])):
            out = tmp / f"sentinel_{tag}.lua"
            ok, err = gen(sent, out, 42, extra)
            if not ok:
                check(f"sentinel {tag} generates", False, err[:200])
                continue
            text = out.read_text(errors="surrogateescape")
            leaked = [s for s in SENTINELS if s in text]
            leaked += [s for s in SENTINELS
                       if any(k == "string" and s in v for k, v in lex(text))]
            check(f"sentinel {tag} statically invisible", not leaked,
                  f"leaked={leaked}")
            rc, got = run_luau_bytes(out)
            check(f"sentinel {tag} runtime byte-exact",
                  rc == rc_s and got == ref_s, f"rc={rc} got={got!r} ref={ref_s!r}")

        # Dynamic host semantics through the indirect resolution.
        art = tmp / "rich_dyn.lua"
        ok, err = gen(rich, art, 42, ["--ccpd"])
        check("dynamic-semantics artifact generates", ok, err[:200])
        if ok:
            art_txt = art.read_text(errors="surrogateescape")
            # The VM binds the payload's global table from the
            # thread-global environment (the guard stages are what
            # detect a substituted chunk env), so replacing `print`
            # in the real global env must be honoured at call time:
            rc, got = run_in_sandbox(tmp, "wrapped_print", art_txt, r"""
local real_print = REAL_G.print
REAL_G.print = function(...)
  local args = {...}
  for i = 1, select("#", ...) do args[i] = tostring(args[i]) end
  real_print("WRAPPED", table.concat(args, ","))
end
""")
            check("dynamic print replacement honoured at call time",
                  rc == 0 and got.startswith(b"WRAPPED"), f"rc={rc} got={got[:80]!r}")

            # Standard-library replacement: wrapping table.concat in
            # the global env must be visible to payload calls.
            rc, got = run_in_sandbox(tmp, "wrapped_concat", art_txt, r"""
local real_concat = REAL_G.table.concat
local calls = 0
REAL_G.table.concat = function(...)
  calls = calls + 1
  return real_concat(...)
end
local real_print = REAL_G.print
REAL_G.print = function(...)
  if select(1, ...) == 42 then real_print("CONCAT_CALLS", calls) end
  return real_print(...)
end
""")
            # Count includes VM-internal concat calls during boot;
            # >= 1 proves the replaced function (not a cached original)
            # is what actually runs.
            check("dynamic stdlib replacement honoured at call time",
                  rc == 0 and got.startswith(b"CONCAT_CALLS\t")
                  and int(got.split(b"\t", 1)[1].split(b"\n", 1)[0]) >= 1,
                  f"rc={rc} got={got[:80]!r}")

            # Custom environment compatibility: a bare chunk env that
            # only chains to the real globals via __index must still
            # run the artifact correctly.
            rc, got = run_in_sandbox(tmp, "index_env", art_txt, r"""
for k in pairs(ENV) do ENV[k] = nil end
ENV._G = ENV
setmetatable(ENV, { __index = REAL_G })
""")
            check("custom environment (__index chain) runs payload",
                  rc == 0 and got == ref, f"rc={rc} got={got[:80]!r}")

        # CCPD opaque sentinels present, readable diagnostics absent.
        ccpd_art = tmp / "rich_ccpd.lua"
        ok, _ = gen(rich, ccpd_art, 42, ["--ccpd", "--no-minify"])
        if ok:
            txt = ccpd_art.read_text(errors="surrogateescape")
            check("ccpd failure sentinels are opaque numeric raises",
                  re.search(r"\[16\]\(0,\s*0\)", txt) is not None
                  and "invalid runtime payload" not in txt
                  and "invalid control transfer" not in txt)

    failed = [n for n, ok in checks if not ok]
    print(f"plaintext_surface_test: {len(checks) - len(failed)}/{len(checks)} checks passed")
    if failed:
        print("failed checks:")
        for n in failed:
            print(f"  - {n}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
