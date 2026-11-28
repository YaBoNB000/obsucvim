#!/usr/bin/env python3
"""CCPD plan 11.3: Rust / Luau selector consistency.

The generated Luau runtime carries hand-written mirrors of the
`runtime_mask` derivations (`CCPDM` = derive_mask at entry_domain 0,
`CCPDV` = derive_variant_id, `CCPDR` = derive_root_context,
`CCPDO` = derive_closure_origin, `CCPDI` = derive_invocation_context,
`CCPDPM` = operand_perm_from_code). A silent drift between the Rust
compiler and these mirrors would not fail the semantic differential
(the variant selector only runs at runtime), so this test pins them
against each other explicitly:

  1. A Python re-implementation of the derivations is anchored to the
     pinned vectors asserted in `runtime_mask.rs` (including the
     nonzero-entry-domain `derive_mask` vectors the Luau mirror never
     evaluates, since payload masks run at domain 0 by design).
  2. The actual `CCPD_HELPERS_LUA` text is extracted from
     `template.rs`, executed under the repo Luau, and every output is
     compared against the anchored Python implementation over pinned,
     boundary, and wrap-around inputs.
  3. `variant_id` outputs are additionally asserted to stay in [0, 4).

Usage: python3 tests/ccpd_mirror_test.py
Exit code: 0 = all assertions hold, 1 = at least one failed.
"""
import os
import re
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)  # engine/
REPO = os.path.dirname(ROOT)
TOOLS_BIN = os.environ.get("OBSUCVIM_TOOLS", os.path.join(REPO, ".tools", "bin"))
LUAU = os.path.join(TOOLS_BIN, "luau")
TEMPLATE_RS = os.path.join(ROOT, "src", "vmgen", "template.rs")

MOD28 = 268435456

checks = []


def check(name, ok, detail=""):
    checks.append((name, ok))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}{(' :: ' + detail) if detail and not ok else ''}")


# ---------------------------------------------------------------------------
# Python re-implementation of runtime_mask.rs (exact integer arithmetic).
# ---------------------------------------------------------------------------

def mix28(s, v):
    return (s * 31 + (v % MOD28)) % MOD28


def avalanche28(s):
    s = (s + 0x9E3779B9 % MOD28) % MOD28
    s = (s * 0x5BD1E995) % MOD28
    s = (s + ((s >> 13) * 40503)) % MOD28
    s = (s * 0x85EBCA6B) % MOD28
    return s


def derive_mask(nonce, proto, block, lane, domain, epoch):
    s = nonce % MOD28
    s = mix28(s, proto)
    s = mix28(s, block)
    s = (s * 31 + lane) % MOD28
    s = (s * 31 + domain) % MOD28
    s = mix28(s, epoch)
    return avalanche28(s)


def derive_variant_id(fctx, edge, block, visit, salt):
    s = salt % MOD28
    s = mix28(s, fctx)
    s = mix28(s, edge)
    s = mix28(s, block)
    s = mix28(s, visit)
    return avalanche28(s) % 4


def derive_root_context(salt, proto):
    s = salt % MOD28
    s = mix28(s, proto)
    s = mix28(s, 0)  # EntryTransition::Fallthrough
    return avalanche28(s)


def derive_closure_origin(fctx, edge, block, proto, site, ordinal, child, salt):
    s = salt % MOD28
    for v in (fctx, edge, block, proto, site, ordinal, child, 7):  # 7 = ClosureInvocation
        s = mix28(s, v)
    return avalanche28(s)


def derive_invocation_context(org, ordinal, child, salt):
    s = salt % MOD28
    for v in (org, ordinal, child):
        s = mix28(s, v)
    return avalanche28(s)


def perm_from_code(code):
    rem = code % 24
    pool = [0, 1, 2, 3]
    out = []
    for slot, fact in enumerate((6, 2, 1, 1)):
        pick = 0 if slot == 3 else rem // fact
        if slot != 3:
            rem %= fact
        out.append(pool.pop(pick))
    return out


# Pinned vectors copied verbatim from runtime_mask.rs::pinned_vectors.
PINNED_MASK = [
    # (nonce, proto, block, lane, domain, epoch, expected)
    (42, 3, 7, 2, 5, 9, 129762251),
    (0, 0, 0, 0, 0, 0, 250714540),
    (268435455, 65535, 65535, 255, 65535, 4294967295, 148471878),
    (777, 12, 34, 4, 513, 99991, 68024606),
]


def anchor_python():
    for (nonce, p, b, l, d, e, want) in PINNED_MASK:
        got = derive_mask(nonce, p, b, l, d, e)
        check(f"python anchor derive_mask({nonce},{p},{b},{l},{d},{e})", got == want,
              f"got {got} want {want}")
    check("python anchor derive_variant_id", derive_variant_id(123456, 5, 3, 7, 42) == 2)
    check("python anchor derive_root_context", derive_root_context(42, 3) == 62904812)
    check("python anchor derive_closure_origin",
          derive_closure_origin(11, 2, 1, 3, 9, 1, 4, 42) == 194110675)
    check("python anchor derive_invocation_context",
          derive_invocation_context(12345, 7, 4, 42) == 142615952)
    check("python anchor perm code 0 is identity", perm_from_code(0) == [0, 1, 2, 3])
    seen = {tuple(perm_from_code(c)) for c in range(24)}
    check("python anchor all 24 perm codes distinct", len(seen) == 24)


# ---------------------------------------------------------------------------
# Luau mirror extraction + driver.
# ---------------------------------------------------------------------------

def extract_helpers():
    src = open(TEMPLATE_RS, encoding="utf-8").read()
    m = re.search(r'const CCPD_HELPERS_LUA: &str = "', src)
    if not m:
        raise SystemExit("CCPD_HELPERS_LUA not found in template.rs")
    i = m.end()
    out = []
    while i < len(src):
        c = src[i]
        if c == "\\":
            nxt = src[i + 1]
            out.append({"n": "\n", "t": "\t", '"': '"', "\\": "\\"}.get(nxt, nxt))
            i += 2
        elif c == '"':
            return "".join(out)
        else:
            out.append(c)
            i += 1
    raise SystemExit("unterminated CCPD_HELPERS_LUA literal")


def build_driver(helpers):
    lines = ["local FLOOR = math.floor", helpers]
    cases = []  # (tag, expected)

    def emit(tag, lua_expr, expected):
        lines.append(f'print("{tag}", {lua_expr})')
        cases.append((tag, expected))

    # CCPDM == derive_mask(nonce, proto, block, lane, domain=0, epoch).
    mask_inputs = [
        (42, 3, 7, 2, 9),
        (0, 0, 0, 0, 0),
        (268435455, 65535, 65535, 255, 4294967295),
        (777, 12, 34, 4, 99991),
        (1, 1, 1, 1, 1),
        (268435455, 0, 0, 0, 268435455),
        (123456789, 4242, 777, 130, 305419896),
    ]
    for (nonce, p, b, l, e) in mask_inputs:
        emit(f"M{nonce}_{p}_{b}_{l}_{e}", f"CCPDM({nonce}, {p}, {b}, {l}, {e})",
             derive_mask(nonce, p, b, l, 0, e))

    # CCPDV == derive_variant_id(fctx, edge, block, visit, salt).
    variant_inputs = [
        (123456, 5, 3, 7, 42),
        (0, 0, 0, 0, 0),
        (268435455, 65535, 65535, 4294967295, 268435455),
        (1, 1, 1, 1, 1),
        (999999937, 17, 23, 100000, 777),
    ]
    for salt in (0, 1, 42, 268435455):
        for visit in (0, 1, 2, 3, 57):
            variant_inputs.append((1000 + visit, 3, 9, visit, salt))
    for (f, ed, b, v, s) in variant_inputs:
        emit(f"V{f}_{ed}_{b}_{v}_{s}", f"CCPDV({f}, {ed}, {b}, {v}, {s})",
             derive_variant_id(f, ed, b, v, s))

    # CCPDR == derive_root_context(salt, proto).
    emit("R42_3", "CCPDR(42, 3)", derive_root_context(42, 3))
    emit("R0_0", "CCPDR(0, 0)", derive_root_context(0, 0))
    emit("Rmax", "CCPDR(268435455, 65535)", derive_root_context(268435455, 65535))

    # CCPDO == derive_closure_origin(fctx, edge, block, proto, site, ord, child, salt).
    emit("O_pinned", "CCPDO(11, 2, 1, 3, 9, 1, 4, 42)",
         derive_closure_origin(11, 2, 1, 3, 9, 1, 4, 42))
    emit("O_zero", "CCPDO(0, 0, 0, 0, 0, 0, 0, 0)",
         derive_closure_origin(0, 0, 0, 0, 0, 0, 0, 0))
    emit("O_wrap", "CCPDO(4294967295, 65535, 65535, 65535, 65535, 65535, 65535, 268435455)",
         derive_closure_origin(4294967295, 65535, 65535, 65535, 65535, 65535, 65535, 268435455))

    # CCPDI == derive_invocation_context(org, ordinal, child, salt).
    emit("I_pinned", "CCPDI(12345, 7, 4, 42)",
         derive_invocation_context(12345, 7, 4, 42))
    emit("I_zero", "CCPDI(0, 0, 0, 0)", derive_invocation_context(0, 0, 0, 0))
    emit("I_wrap", "CCPDI(268435455, 4294967295, 65535, 268435455)",
         derive_invocation_context(268435455, 4294967295, 65535, 268435455))

    # CCPDPM == operand_perm_from_code for every code 0..23.
    for code in range(24):
        tag = f"P{code}"
        lines.append(f'local pm{code} = CCPDPM({code})')
        lines.append(f'print("{tag}", pm{code}[1] .. "," .. pm{code}[2] .. "," .. pm{code}[3] .. "," .. pm{code}[4])')
        cases.append((tag, ",".join(str(x) for x in perm_from_code(code))))

    return "\n".join(lines) + "\n", cases


def main():
    if not os.path.isfile(LUAU):
        print(f"luau not found at {LUAU}; run engine/tools/rebuild_tools.sh first")
        return 1
    print("ccpd_mirror_test: python anchor against runtime_mask pinned vectors")
    anchor_python()

    helpers = extract_helpers()
    check("helpers contain all six mirrors",
          all(name in helpers for name in ("CCPDM", "CCPDV", "CCPDR", "CCPDO", "CCPDI", "CCPDPM")))
    driver, cases = build_driver(helpers)
    with tempfile.TemporaryDirectory() as td:
        path = os.path.join(td, "mirror_driver.lua")
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(driver)
        proc = subprocess.run([LUAU, path], capture_output=True, text=True, timeout=120)
    if proc.returncode != 0:
        check("luau mirror driver runs", False, proc.stderr.strip()[:400])
        return 1
    got = {}
    for line in proc.stdout.splitlines():
        parts = line.split(None, 1)
        if len(parts) == 2:
            got[parts[0]] = parts[1].strip()
    mismatches = []
    bounded = True
    for tag, want in cases:
        val = got.get(tag)
        if val is None or str(want) != val:
            mismatches.append(f"{tag}: luau={val} want={want}")
        if tag.startswith("V"):
            try:
                if not (0 <= int(val) < 4):
                    bounded = False
            except (TypeError, ValueError):
                bounded = False
    check(f"luau mirrors match rust derivations on {len(cases)} vectors",
          not mismatches, "; ".join(mismatches[:6]))
    check("variant selector stays within [0, 4)", bounded)

    failed = [n for n, ok in checks if not ok]
    print(f"ccpd_mirror_test: {len(checks) - len(failed)}/{len(checks)} checks passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
