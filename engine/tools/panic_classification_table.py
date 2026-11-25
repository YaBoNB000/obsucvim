#!/usr/bin/env python3
"""Generate the P0-2 attachment table (panic 位置 | 归类 | 判定依据) for HANDOFF §18.

Run from engine/: python3 tools/panic_classification_table.py
Every production panic-capable site (unwrap/expect/panic!/unreachable!/assert!)
gets one row; test-module sites are counted but not listed.
"""
import glob
import re

EXT = re.compile(
    r"(?<![\w.])\.unwrap\(\)|(?<!self)\.expect\(|(?<![\w.])panic!|(?<![\w.])unreachable!"
    r"|(?<![\w.])assert!\(|(?<![\w.])assert_eq!\("
)
PLAN = re.compile(r"\.unwrap\(\)|\.expect\(|panic!")

# (file regex, line-content regex, basis) -- content is matched against the
# site line plus the two following lines so multi-line assert!(...) matches.
B = [
    (r"flatten\.rs", r"assert!\(ctx\.inner, \"break", "内部不变量：parser.rs stat() 在 loops==0 时拒绝，funcbody 把 loops 清零后再解析函数体（P4 审查修复），Break 只会出现在循环上下文"),
    (r"flatten\.rs", r"assert!\(ctx\.inner, \"continue", "内部不变量：同上（continue）"),
    (r"flatten\.rs", r"make_loop called on a non-loop", "内部不变量：make_loop 只由 Stmt::While/Repeat/ForNum/ForGen 分支调用"),
    (r"flatten\.rs", r"_ => unreachable!\(\)", "内部不变量：对循环语句枚举的全部分支做了穷尽匹配，`_` 仅剩非循环节点"),
    (r"flatten\.rs", r"loop node must have a single successor", "内部不变量：build_graph 为循环节点固定生成单一后继"),
    (r"flatten\.rs", r"machine has at least one block", "内部不变量：build_graph 先 push 入口块再 dispatch_chain"),
    (r"flatten\.rs", r"continue must be rewired", "内部不变量：lower 前的 rewire 阶段把全部 Succ::Cont 改写为目标块；parser 保证 continue 仅在循环内"),
    (r"mangle\.rs", r"root name scope", "内部不变量：SymbolTable::new 压入根作用域，push/pop 在同一代码路径成对"),
    (r"parser\.rs", r"self\.toks\.last\(\)\.unwrap\(\)", "内部不变量：lexer 总是追加 EOF token，toks 非空"),
    (r"parser\.rs", r"self\.expect\(", "**非 panic**：`Parser::expect` 是返回 `Result<Token, ParseError>` 的方法（语法错误走干净 `error:`），任务书 grep 把同名方法误计为 `Option::expect`"),
    (r"symtab\.rs", r"scopes\.last_mut\(\)\.unwrap\(\)", "内部不变量：resolve() 以根作用域开始，每个 block 的 push/pop 成对"),
    (r"compiler\.rs", r"scopes\.last_mut\(\)\.unwrap\(\)\.insert", "内部不变量：Ctx::new* 压入根作用域，compile_block 的 push/pop 成对"),
    (r"compiler\.rs", r"vocab pairs are whitelist-only", "内部不变量：查表键是编译期 opcode 名，白名单为静态表（fuse.rs FUSE_WHITELIST）"),
    (r"compiler\.rs", r"perm\.try_into\(\)\.unwrap\(\)", "内部不变量：perm 由 `vec![0,1,2,3]` 构造，长度恒为 4"),
    (r"compiler\.rs", r"tag_pool\[\.\.8\]\.try_into\(\)\.unwrap\(\)", "内部不变量：tag_pool 为静态 8 元素池"),
    (r"compiler\.rs", r"values\[nv - 1\]\.as_ref\(\)\.unwrap\(\)", "内部不变量：上一行 `last_is_call` 由 values[nv-1] 为 Some 推出，且 nv>0"),
    (r"compiler\.rs", r"values\[nv - 1\]\.clone\(\)", "内部不变量：同上（赋值路径）"),
    (r"compiler\.rs", r"expect\(\"break outside loop\"\)", "内部不变量（P4 审查前**为假**）：原注释称 parser 已拒绝循环外 break，但 loops 计数器曾泄漏进嵌套函数体，`while true do local function f() break end end` 实际触发此 panic（rc=101）。已修 parser.rs funcbody，并加 robustness G 场景 + 单测钉住"),
    (r"compiler\.rs", r"expect\(\"continue outside loop\"\)", "内部不变量：同上（continue，Luau 方言）"),
    (r"compiler\.rs", r"is_call\(es\.last\(\)\.unwrap\(\)\)", "内部不变量：`es.is_empty()` 分支已提前 return"),
    (r"compiler\.rs", r"es\.last\(\)\.unwrap\(\)\.clone\(\)", "内部不变量：所在的 `else if is_call(es.last().unwrap())` 分支已证明末元素存在"),
    (r"compiler\.rs", r"matches!\(es\.last\(\)\.unwrap\(\), Expr::Vararg\)", "内部不变量：同上"),
    (r"compiler\.rs", r"_ => unreachable!\(\)", "内部不变量：对 AST Expr/Target 枚举的全部分支穷尽匹配，`_` 仅剩该处已在上层处理的变体"),
    (r"compiler\.rs", r"call_tail\(e\)\.unwrap\(\)", "内部不变量：仅在 `is_call(e)` 为真的分支内调用，call_tail 对同一谓词返回 Some"),
    (r"compiler\.rs", r"parent upvalue permutation missing", "内部不变量：up_perm 由 derive_upvalue_permutation(ups.len()) 为本帧构造，索引 u < ups.len()"),
    (r"compiler\.rs", r"upvalue permutation length mismatch", "内部不变量：同上"),
    (r"compiler\.rs", r"H3-6 invariant violated", "内部不变量：H3-6 噪声放置器的自检（算法自身性质，不来自输入数值）"),
    (r"handlers/mod\.rs", r"unknown opcode name", "内部不变量：name 来自 isa.rs 静态 OP_NAMES"),
    (r"handlers/mod\.rs", r"unknown arithmetic metamethod", "内部不变量：对静态 BinOp 枚举穷尽匹配"),
    (r"handlers/mod\.rs", r"comparison operator", "内部不变量：native 为模板内静态比较表达式字符串"),
    (r"isa\.rs", r"active_count > 0 && active_count <= N_OPS", "内部不变量：两个调用点传入 isa::N_OPS / N_OPS-1（编译期常量）"),
    (r"isa\.rs", r"glyphs\[1\.\.\]\.try_into\(\)\.unwrap\(\)", "内部不变量：glyphs 为固定 95 字节静态表"),
    (r"template\.rs", r"pool\.iter\(\)\.position\(\|&x\| x == b\)\.unwrap\(\)", "内部不变量：pool 由同一静态 boot 名单构造，b 必在其中"),
    (r"template\.rs", r"pool\[\.\.M_COUNT\]\.try_into\(\)\.unwrap\(\)", "内部不变量：M_COUNT 为编译期常量且 < 244"),
    (r"template\.rs", r"KeyEmitter needs the main carrier", "内部不变量：唯一调用点传入 program.fns 的长度列表，永不为空"),
    (r"template\.rs", r"multi-record micro recipe missing superoperator", "内部不变量：name 来自静态微码表，MICRO_SUPER_NAMES 与之一一对应（模板测试钉住）"),
    (r"template\.rs", r"superoperator index below base", "内部不变量：唯一调用方 micro_op_body 仅在 index >= M_SUPER_BASE 时进入"),
    (r"template\.rs", r"superoperator index out of range", "内部不变量：index < M_COUNT = M_SUPER_BASE + MICRO_SUPER_NAMES.len()"),
    (r"template\.rs", r"superoperator recipe missing", "内部不变量：每个 MICRO_SUPER_NAMES 条目在静态表里都有配方（模板测试钉住）"),
    (r"template\.rs", r"_ => unreachable!\(\)", "内部不变量：micro_op_body 的 match 覆盖 0..M_SUPER_BASE 内全部静态 M_* 微码，index >= M_SUPER_BASE 已在函数入口分流"),
    (r"v15\.rs", r"split_once\('\.'\)\.unwrap\(\)", "内部不变量：path 为 include_str 静态模板里的库.函数名"),
    (r"v15\.rs", r"panic!\(\"v15 static source\"\)", "内部不变量：仅当 include_str 的静态模板解析失败，与用户输入无关"),
    (r"v15\.rs", r"block\.stmts\.pop\(\)\.unwrap\(\)", "内部不变量：静态模板源码非空"),
    (r"v15\.rs", r"values\.pop\(\)\.unwrap\(\)\.unwrap\(\)", "内部不变量：静态模板的 local 语句恒有初值"),
    (r"v15\.rs", r"_ => unreachable!\(\)", "内部不变量：对静态模板语句的 match 仅接受 Local，其余分支在静态源里不存在"),
    (r"v15\.rs", r"it\.next\(\)\.unwrap\(\)", "内部不变量：静态模板迭代器元素个数固定"),
    (r"v15\.rs", r"not enough BW slots", "内部不变量：all_words 与 bw_slots 均来自 include_str 静态模板"),
    (r"v15\.rs", r"out\.try_into\(\)\.unwrap\(\)", "内部不变量：循环恰好压入 6 个名字"),
    (r"v15\.rs", r"leaves\.last\(\)\.unwrap\(\)\.0", "内部不变量：静态模板至少产生一个叶子"),
    (r"rng_check\.rs", r"assert_eq!\(a, b\)", "内部不变量：独立自检 bin（rng_check），不读任何输入"),
]

rows = []
plan_total = plan_test = plan_self = 0
for f in sorted(glob.glob("src/**/*.rs", recursive=True)):
    lines = open(f).read().split("\n")
    intest = False
    for i, l in enumerate(lines):
        if re.match(r"\s*#\[cfg\(test\)\]", l):
            intest = True
        s = l.strip()
        if s.startswith("//") or s.startswith("#[cfg"):
            continue
        planhit = bool(PLAN.search(l))
        if planhit:
            plan_total += 1
            if intest:
                plan_test += 1
            elif "self.expect(" in l:
                plan_self += 1
        if intest or not EXT.search(l):
            continue
        ctx = "\n".join(lines[i : i + 14])
        basis = None
        for fr, lr, b in B:
            if re.search(fr, f) and re.search(lr, ctx):
                basis = b
                break
        rows.append((f, i + 1, s, basis))

miss = [r for r in rows if r[3] is None]
for r in miss:
    print("UNMATCHED", r[0], r[1], r[2], file=__import__("sys").stderr)
print(f"<!-- production sites={len(rows)} plan-pattern lines={plan_total} (test={plan_test}, Parser::expect method={plan_self}) -->")
print("| panic 位置 | 归类 | 判定依据 |")
print("|---|---|---|")
for f, n, s, b in rows:
    cat = "Parser::expect 方法（非 panic，grep 误报）" if (b or "").startswith("**非 panic**") else "内部不变量"
    code = s.replace("|", "\\|")
    print(f"| `engine/{f}:{n}` | {cat} | {(b or 'UNMATCHED')} <br>`{code[:80]}` |")
raise SystemExit(1 if miss else 0)
