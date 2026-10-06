#!/usr/bin/env python3
"""H5-2C lean default-shell static audit."""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def strip_strings(src):
    out = list(src)
    i = 0
    while i < len(src):
        if src[i] == '[':
            m = re.match(r'\[(=*)\[', src[i:])
            if m:
                close = ']' + m.group(1) + ']'
                j = src.find(close, i + len(m.group(0)))
                if j >= 0:
                    e = j + len(close)
                    for k in range(i, e):
                        out[k] = ' '
                    i = e
                    continue
        if src[i] in "\"'":
            q = src[i]
            j = i + 1
            while j < len(src):
                if src[j] == '\\':
                    j += 2
                    continue
                if src[j] == q:
                    break
                j += 1
            for k in range(i, min(j + 1, len(src))):
                out[k] = ' '
            i = j + 1
            continue
        i += 1
    return ''.join(out)


def main():
    args = [x for x in sys.argv[1:] if not x.startswith('--')]
    if not args:
        print("usage: h5_shell_audit.py artifact.lua")
        return 2
    src = pathlib.Path(args[0]).read_text(errors="replace")
    code = strip_strings(src)
    main_rs = (ROOT / "src/main.rs").read_text()

    no_module_shell = not code.lstrip().startswith("return setmetatable(")
    no_numeric_self_runner = not re.search(r'\b\w+\[\d+\]\(\w+(?:,|\))', code)
    no_c15_concat = not re.search(r'\b(\w+)\[1\]\.\.\1\[2\].*\1\[15\]', code)
    no_outer_cps = len(re.findall(r'\bcontinue\b', code)) < 10
    lean_gate = ('if opts.bind_env.is_none() && !opts.bind_executor' in main_rs
                 and 'tblock.stmts.push(ast::Stmt::ExprStmt(vm_call))' in main_rs)
    bound_fallback = ('vmgen::v15::scaffold(' in main_rs
                      and 'opts.bind_env.as_deref()' in main_rs)
    no_loader = not re.search(r'\bloadstring\s*\(', code)
    carriers = len(re.findall(r'\[=*\[', src))
    compact = len(src) < 180000  # fixed basics audit artifact; old shell was >230KB

    checks = [no_module_shell, no_numeric_self_runner, no_c15_concat, no_outer_cps,
              lean_gate, bound_fallback, no_loader, carriers >= 1, compact]
    print("== H5-2C 默认外壳精简审计 ==")
    print(f"[0] setmetatable module shell 消失: {'YES' if no_module_shell else 'NO'}")
    print(f"[1] b[n](b)/C[1..15] 固定语法消失: {'YES' if no_numeric_self_runner and no_c15_concat else 'NO'}")
    print(f"[2] 外层 CPS continue forest 消失: {'YES' if no_outer_cps else 'NO'}")
    print(f"[3] 默认 lean gate + 绑定态兼容 fallback: {'YES' if lean_gate and bound_fallback else 'NO'}")
    print(f"[4] 无内部 loader: {'YES' if no_loader else 'NO'}")
    print(f"[5] carrier 保留且 basics 产物 <180KB: {'YES' if carriers >= 1 and compact else 'NO'} "
          f"(size={len(src)}, carriers={carriers})")
    ok = all(checks)
    print("\nH5-2C 审计: " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
