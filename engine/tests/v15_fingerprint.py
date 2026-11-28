#!/usr/bin/env python3
"""v15 结构指纹检查器（P0 脚手架，路线 A 已拍板 2026-08-25）。

统计当前 v15 输出的 32 条结构形态指标（F1–F32），覆盖文件壳、表布局、分发器、
常量/字节路径与随机化辅助结构。它是静态形态回归，不是安全、不可逆性或性能证明；
历史定义原在 v15 结构对齐档案 §0.1/§0.2，该档案已于 2026-11-15 删除。

用法：
    python3 tests/v15_fingerprint.py <file.lua> [--quiet]
退出码：0 = 32 条全部通过；1 = 有失败。

解析口径（二轮详读实测踩过的坑，全部已处理）：
  * 统计前先剥掉长字符串（RC blob）区间；
  * 块计数必须排除 Luau **if 表达式**（`=if ...then...else...` 无 end，
    不排除则 end 平衡差 17、模块表字段识别全错）；
  * 模块表字段按**函数深度 0** 识别（`function...end` 不改花括号深度，
    纯括号深度会把 handler 体内的赋值误判为表字段）。

旧黄金数对应 2026-08-25 的输出壳，不能直接套用当前 emitter。2026-11-21 起按现行
root table/VM 调用壳重新解释，并用 seed 42、772、888、999、4242、31337、999999
七个固定样本校准下限；计数是形态信号，不构成安全证明。随机化形态项设置宽容区间，
语义正确性与防篡改由官方矩阵及安全指纹单独验证。
"""
import re
import sys

PUSHKW = frozenset(("function", "if", "do", "repeat"))
POPKW = frozenset(("end", "until"))
PRIM_PREFIXES = (
    "buffer.", "bit32.", "string.", "table.", "coroutine.", "Vector",
    "vector.", "typeof", "setfenv", "getfenv", "pcall", "xpcall",
    "error", "next", "select", "unpack", "tonumber", "tostring",
    "rawget", "rawset", "assert", "type", "setmetatable", "getmetatable",
)


def tokenize(src):
    """剥字符串/注释/长字符串。返回:
    code      —— 同长度文本，字符串内容与注释被空格替换（位置保真）
    strings   —— [(start, text)] 双引号字符串（长字符串之外）
    longstrs  —— [(start, end)] 长字符串区间
    """
    n = len(src)
    code = list(src)
    strings = []
    longstrs = []
    i = 0
    in_str = None      # None | '"' | "'" | ('long', level)
    esc = False
    while i < n:
        ch = src[i]
        if in_str is not None:
            if isinstance(in_str, tuple):  # long string
                lvl = in_str[1]
                if src.startswith("]" + "=" * lvl + "]", i):
                    for k in range(i, i + lvl + 2):
                        code[k] = " "
                    longstrs.append((start, i + lvl + 2))
                    in_str = None
                    i += lvl + 2
                    continue
                code[i] = " "
                i += 1
                continue
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif (in_str == '"' and ch == '"') or (in_str == "'" and ch == "'"):
                strings.append((start, src[start:i + 1]))
                in_str = None
            else:
                code[i] = " "
            i += 1
            continue
        if ch == '"':
            in_str = '"'
            start = i
        elif ch == "'":
            in_str = "'"
            start = i
        elif ch == "[":
            m = re.match(r"\[(=*)\[", src[i:i + 12])
            if m:
                in_str = ("long", len(m.group(1)))
                start = i
                i += len(m.group(0))
                for k in range(start, i):
                    code[k] = " "
                continue
        elif ch == "-" and src.startswith("--", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                code[k] = " "
            i = j
            continue
        i += 1
    return "".join(code), strings, longstrs


def is_if_expr(code, i):
    """True when `if` at index `i` is a Luau if-expression (`=if`, `(if`, `,if`, etc.)
    rather than an `if ... then ... end` statement."""
    p = i - 1
    while p >= 0 and code[p] == " ":
        p -= 1
    return p >= 0 and code[p] in "=(,+-[{"


def block_scan(code, cb):
    """关键字级块扫描（if 表达式修正）。cb(kind, word, i, blk)。"""
    n = len(code)
    i = 0
    blk = 0
    while i < n:
        ch = code[i]
        if ch.isalpha() or ch == "_":
            j = i
            while j < n and (code[j].isalnum() or code[j] == "_"):
                j += 1
            w = code[i:j]
            if w in PUSHKW:
                if w == "if" and is_if_expr(code, i):
                    cb("kw", w, i, blk)  # if 表达式：不计块
                else:
                    blk += 1
                    cb("kw", w, i, blk)
            elif w in POPKW:
                cb("kw", w, i, blk)
                blk -= 1
            else:
                cb("kw", w, i, blk)
            i = j
            continue
        cb("ch", ch, i, blk)
        i += 1


def module_table_start(code):
    """Locate the emitted root module table across current and legacy shells."""
    m = re.search(r"\blocal\s+[A-Za-z_][A-Za-z_0-9]*\s*=\s*\{", code)
    if not m:
        m = re.search(r"setmetatable\(\{", code)
    return m.end() - 1 if m else -1


def module_fields(code):
    """模块表字段识别。返回 (numslots, named_fns, named_consts, tstart, tend)。"""
    tstart = module_table_start(code)
    if tstart < 0:
        return {}, [], {}, -1, -1
    # 找匹配的右括号（字符串已剥掉，直接数括号）
    d = 0
    tend = -1
    for i in range(tstart, len(code)):
        if code[i] == "{":
            d += 1
        elif code[i] == "}":
            d -= 1
            if d == 0:
                tend = i
                break
    if tend < 0:
        return {}, [], {}, -1, -1
    # 函数范围（模块级 =function 起，blk 回到 0 止）
    numslots = {}
    named_fns = []
    named_consts = {}
    blk = 0
    i = tstart
    n = len(code)

    def key_before(eqpos):
        j = eqpos - 1
        while j >= 0 and code[j] == " ":
            j -= 1
        if j >= 0 and code[j] == "]":
            k2 = code.rfind("[", 0, j)
            if k2 >= 0 and re.fullmatch(r"\d+", code[k2 + 1:j]):
                return int(code[k2 + 1:j])
            return None
        m2 = re.search(r"([A-Za-z_][A-Za-z_0-9]*)$", code[:j + 1])
        if not m2:
            return None
        st = m2.start()
        before = code[st - 1] if st > 0 else ""
        if before in "{,":
            return m2.group(1)
        return None

    fn_stack = []
    i = tstart
    blk = 0
    pending_key = None
    while i < tend:
        ch = code[i]
        if ch.isalpha() or ch == "_":
            j = i
            while j < n and (code[j].isalnum() or code[j] == "_"):
                j += 1
            w = code[i:j]
            if w in PUSHKW:
                if not (w == "if" and is_if_expr(code, i)):
                    blk += 1
                if w == "function" and blk == 1 and i > 0 and code[i - 1] == "=":
                    fn_stack.append(key_before(i))
            elif w in POPKW:
                blk -= 1
                if blk == 0 and fn_stack:
                    fn_stack.pop()
            i = j
            continue
        if ch == "=" and blk == 0 and i + 1 < tend and code[i + 1] != "=" \
                and code[i - 1] not in "<>~":
            key = key_before(i)
            val = code[i + 1:i + 80].split(",")[0].strip()
            if isinstance(key, int):
                numslots[key] = val
            elif isinstance(key, str):
                if val.startswith("function"):
                    named_fns.append(key)
                else:
                    named_consts[key] = val
        i += 1
    return numslots, named_fns, named_consts, tstart, tend


def function_ranges(code):
    """模块级函数 (name, start, end) 列表。"""
    tstart = module_table_start(code)
    if tstart < 0:
        return []
    ranges = []
    starts = []
    n = len(code)
    blk = 0
    i = tstart
    while i < n:
        ch = code[i]
        if ch.isalpha() or ch == "_":
            j = i
            while j < n and (code[j].isalnum() or code[j] == "_"):
                j += 1
            w = code[i:j]
            if w in PUSHKW:
                if not (w == "if" and is_if_expr(code, i)):
                    blk += 1
                if w == "function" and blk == 1 and i > 0 and code[i - 1] == "=":
                    j2 = i - 2
                    while j2 >= 0 and code[j2] == " ":
                        j2 -= 1
                    key = None
                    if code[j2] == "]":
                        k2 = code.rfind("[", 0, j2)
                        if k2 >= 0:
                            key = code[k2 + 1:j2]
                    else:
                        m2 = re.search(r"([A-Za-z_][A-Za-z_0-9]*)$", code[:j2 + 1])
                        if m2:
                            key = m2.group(1)
                    starts.append((key, i))
            elif w in POPKW:
                blk -= 1
                if blk == 0 and starts:
                    key, s = starts.pop()
                    ranges.append((key, s, i))
            i = j
            continue
        i += 1
    return ranges


def check(name, desc, measured, golden, ok):
    return {"id": name, "desc": desc, "measured": measured,
            "golden": golden, "ok": bool(ok)}


def analyze(path):
    src = open(path, encoding="utf-8", errors="replace").read()
    code, strings, longstrs = tokenize(src)
    lines = src.count("\n") + (0 if src.endswith("\n") else 1)
    phys = src.split("\n")
    numslots, named_fns, named_consts, tstart, tend = module_fields(code)
    franges = function_ranges(code)
    F = []

    # ---- F1 文件形态：watermark + 空行 + root module-table initializer.
    table_line = re.compile(r"local\s+[A-Za-z_][A-Za-z_0-9]*\s*=\s*\{")
    if lines == 3:
        ok1 = phys[0].startswith("--") and phys[1].strip() == "" \
            and bool(table_line.match(phys[2]))
    elif lines == 2:
        ok1 = bool(table_line.match(phys[1]))
    else:
        ok1 = False
    F.append(check("F1", "watermark/空行/root 表初始化形态", lines, 3, ok1))

    # ---- F2 root module table + terminal VM invocation.
    tail = src.rstrip()
    ok2 = tstart >= 0 and tstart < 512 and \
        bool(re.search(r":\w+\(\)\(\.\.\.\);\s*$", tail))
    F.append(check("F2", "root 表入口 + 结尾 VM 调用", 1 if ok2 else 0, 1, ok2))

    # ---- F3 first top-level table field count; RNG-selected layouts vary.
    # Seven fixed seeds observed 6..153 fields, so the gate only rejects a
    # missing/trivial table rather than requiring an obsolete 227-field shell.
    total_fields = len(numslots) + len(named_fns) + len(named_consts)
    F.append(check("F3", "root table 字段数", total_fields, 128, total_fields >= 5))

    # ---- F4 =function( 总数 + 具名函数数
    eqfn = len(re.findall(r"=function\(", code))
    F.append(check("F4", "=function( / 具名函数", (eqfn, len(named_fns)),
                   (190, 65), eqfn >= 100 and len(named_fns) >= 1))

    # ---- F5 状态返回 / 独立状态 ID / 方法调用
    rets = re.findall(r"return (\d+),", code)
    nret, nids = len(rets), len(set(rets))
    mcalls = len(re.findall(r"\b\w+:\w+\(", code))
    F.append(check("F5", "(return N, / 独立 ID / 方法调用)",
                   (nret, nids, mcalls), (106, 105, 111),
                   nret >= 100 and nids >= 30 and mcalls >= 100))

    # ---- F6 宽参数(≥7)函数数
    wide = 0
    for _m in re.finditer(r"=function\(([^)]*)\)", code):
        plist = [p for p in _m.group(1).split(",") if p.strip()]
        if len(plist) >= 7:
            wide += 1
    F.append(check("F6", "参数≥7 的函数", wide, 112, wide >= 50))

    # ---- F7 continue total + maximum inside the detected table (informational).
    # The randomized shell can place the large dispatcher outside that table.
    cont = re.findall(r"\bcontinue\b", code)
    per_fn = {}
    for _m in re.finditer(r"\bcontinue\b", code):
        for key, s, e in franges:
            if s < _m.start() < e:
                per_fn[key] = per_fn.get(key, 0) + 1
                break
    mx = max(per_fn.values()) if per_fn else 0
    F.append(check("F7", "continue 总数 / table 最大单函数(参考)", (len(cont), mx),
                   (104, 104), len(cont) >= 20))

    # ---- F8 大长串 + pC 形转义表（单字符键 → 5 字符值）
    maxlong = max((e - s for s, e in longstrs), default=0)
    pc = len(re.findall(r'\[\s*"(?:\\.|[^"\\])"\s*\]\s*=\s*"(?:\\.|[^"\\]){5}"', code))
    F.append(check("F8", "最长长串字节 / pC 形条目", (maxlong, pc),
                   (10506, 10), maxlong >= 10000 and pc >= 8))

    # ---- F9 numeric slots + primitive slots; floors come from seven seeds.
    prim_slots = sum(1 for v in numslots.values()
                     if v.startswith(PRIM_PREFIXES)
                     or v.startswith('_G[') or v.startswith('("")['))
    F.append(check("F9", "数字槽 / 其中原语槽", (len(numslots), prim_slots),
                   (57, 39), len(numslots) >= 3 and prim_slots >= 1))

    # ---- F10 blob 外字符串字面量数; observed 109..122 over seven fixed seeds.
    nstr = len(strings)
    F.append(check("F10", "短字符串字面量数", nstr, 112, 80 <= nstr <= 150))

    # ---- F11 fetch 循环 local X=T[Y];if X
    fetch = len(re.findall(r"local (\w{1,2})=(\w{1,2})\[(\w{1,2})\];if \1", code))
    F.append(check("F11", "fetch 循环数", fetch, 14, fetch >= 4))

    # ---- F12 pcall 包帧 + 包装闭包
    pcallf = len(re.findall(r"\w\(function\(", code))
    wrap = len(re.findall(r"=\d+,function\(\.\.\.\)", code))
    F.append(check("F12", "pcall(function / N,function(...) 包装",
                   (pcallf, wrap), (4, 1), pcallf >= 1 and wrap >= 1))

    # ---- F13 -128 偏置 + 128 进制重建
    # 增量⑭ 起：解析器整体搬入加密引导碎片，可见区只剩单一 r16
    # varint 解码器（样本族形态保留一档）；带按新架构校准。
    mb = code.count("-128")
    base = code.count("*128") + code.count("*16384") + code.count("*2097152")
    F.append(check("F13", "-128 偏置 / 128 进制乘加", (mb, base),
                   (14, 12), mb >= 5 and base >= 3))

    # ---- F14 SoA 常量自写回
    # ㊶ 偏差记录: 旧计数的大头 = 引导层拼串码表（ASCII 值纸老虎），
    # 已随保护代码隐藏搬入掩码层；写回形态仍存（C 状态写/派生表写），
    # 阈值放宽。
    sow = re.findall(r";(\w{1,2})\[(\w+)\]=(\d+);", code)
    F.append(check("F14", "SoA 常量写回数", len(sow), 1, len(sow) >= 1))

    # ---- F15 LCG 形态 + 嵌套槽状态写
    lcg = code.count("%268435456")
    nest = len(re.findall(r"\w\[\d+\]\[\d+\]\[", code))
    F.append(check("F15", "%268435456 / 嵌套槽写入", (lcg, nest),
                   (362, 20), lcg >= 2 and nest >= 10))

    # ---- F16 %256 位置密钥 + bxor
    m256 = code.count("%256")
    # Sealed emission: bit32.bxor may appear as numeric char codes.
    bxor = code.count("bit32.bxor") + len(re.findall(r"\(98,\s*120,\s*111,\s*114\)", code))
    F.append(check("F16", "%256 / bit32.bxor", (m256, bxor),
                   (406, 65), m256 >= 5 and bxor >= 1))

    # ---- F17 行号重写正则（密封形态：明文模式必须不存在，码序存在）
    lnre = 1 if (":(%d+)" not in src
                 and re.search(r"58,\s*40,\s*37,\s*100,\s*43,\s*41", code)) else 0
    F.append(check("F17", "行号重写正则密封存在", lnre, 1, lnre >= 1))

    # ---- F18 无 os.clock / 无 loadstring
    osc = len(re.findall(r"\bos\.clock\b", code))
    lds = len(re.findall(r"\bloadstring\b", code))
    F.append(check("F18", "os.clock=0 且 loadstring=0", (osc, lds),
                   (0, 0), osc == 0 and lds == 0))

    # ---- F19 按需能力裁剪（P1 起默认构建不再发射未使用的 buffer./typeof/vector 诱饵；
    # 仅 --env-check / --bind-env roblox 按需引入对应宿主 API）
    buf = len(re.findall(r"\bbuffer\.", code))
    tof = len(re.findall(r"\btypeof\b", code))
    vec = len(re.findall(r"\bvector\.|\bVector[23]\b", code))
    ok19 = (buf == 0 and tof == 0 and vec == 0) \
        or (buf >= 1 and vec == 0) \
        or (vec >= 1 and tof >= 1)
    F.append(check("F19", "按需能力裁剪(buffer./typeof/vector)", (buf, tof, vec),
                   (0, 0, 0), ok19))

    # ---- F20 while 数量
    whl = len(re.findall(r"\bwhile\b", code))
    F.append(check("F20", "while 数", whl, 38, whl >= 8))

    # ---- F21 name shortening ratio; table function count is RNG-layout dependent.
    short = sum(1 for k in named_fns if len(k) <= 2)
    pct = 100 * short // len(named_fns) if named_fns else 0
    F.append(check("F21", "具名函数≤2字符% / 具名数", (pct, len(named_fns)),
                   (100, 65), pct >= 90 and len(named_fns) >= 1))

    # ---- F22 if expressions; current output family emits at least one.
    ife = len(re.findall(r"=if ", code))
    F.append(check("F22", "=if 表达式数", ife, 1, ife >= 1))

    # ---- F23 fused conditions; seven fixed seeds observed 33..47.
    fus = len(re.findall(r"\band \d+ or \d+", code))
    F.append(check("F23", "and N or M 数", fus, 33, fus >= 25))

    # ---- F24 复合赋值
    cmp_ = len(re.findall(r"[+\-*/^]=[^=]", code))
    F.append(check("F24", "复合赋值数", cmp_, 211, cmp_ >= 20))

    # ---- F25 duplicate formal-parameter names; zero is the current invariant.
    shadow = 0
    for _m in re.finditer(r"=function\(([^)]*)\)", code):
        plist = [p.strip() for p in _m.group(1).split(",") if p.strip()]
        if len(plist) != len(set(plist)):
            shadow += 1
    F.append(check("F25", "重复形参名函数数", shadow, 0, shadow == 0))

    # ---- F26 runtime slot mutation. Named-field writes vary by randomized layout,
    # so the acceptance gate tracks numeric-slot mutations only.
    slotw = len(re.findall(r"\w\[\d+\]=\w", code)) - len(numslots)
    named_w = len(re.findall(r"\b\w\.[A-Za-z_]\w*=[^=]", code))
    F.append(check("F26", "槽自变异 / 命名字段写", (max(slotw, 0), named_w),
                   (28, 1), max(slotw, 0) >= 10))

    # ---- F27 自修改命中的数组多样性
    # ㊶ 偏差记录: 同 F14——拼串码表数组已入掩码层。
    arrays = set(a for a, _k, _v in sow)
    F.append(check("F27", "自修改数组种类", len(arrays), 1, len(arrays) >= 1))

    # ---- F28 optional randomized decoy LCG slots (zero occurrences are valid).
    decoy = 0
    for key, s, e in franges:
        if key and re.fullmatch(r"\d+", str(key)) and "268435456" in code[s:e]:
            ext = len(re.findall(r"\w\[%s\]" % key, code[:s] + code[e:]))
            if ext == 0:
                decoy += 1
    F.append(check("F28", "随机诱饵 LCG 槽(0..10)", decoy, 3, 0 <= decoy <= 10))

    # ---- F29 大常数(≥1e8)字面量
    big = len(re.findall(r"\b\d{9,}\b", code))
    F.append(check("F29", "≥1e8 常数字面量", big, 965, big >= 20))

    # ---- F30 upvalue cell 布局 [4]= / [7]=
    c4 = len(re.findall(r"\[4\]=", code))
    c7 = len(re.findall(r"\[7\]=", code))
    F.append(check("F30", "[4]= / [7]= 布局", (c4, c7), (6, 5),
                   c4 >= 3 and c7 >= 3))

    # ---- F31 optional tuple-slot makefn variant selected by the randomized shell.
    mkfn = len(re.findall(r"\w\[\w\[4\]\]\(", code))
    F.append(check("F31", "随机元组槽 makefn (0..5)", mkfn, 1, 0 <= mkfn <= 5))

    # ---- F32 初始化器 return true,N,nil
    init = len(re.findall(r"return true,\d+,nil", code))
    F.append(check("F32", "初始化器数", init, 3, init >= 2))

    return F


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    quiet = "--quiet" in sys.argv
    if not args:
        print(__doc__)
        sys.exit(2)
    F = analyze(args[0])
    npass = sum(1 for f in F if f["ok"])
    if not quiet:
        print("v15 结构指纹：%s" % args[0])
        print("%-4s %-34s %-18s %-14s %s" %
              ("ID", "指纹", "实测", "参考(772)", "结果"))
        for f in F:
            print("%-4s %-34s %-18s %-14s %s" %
                  (f["id"], f["desc"], str(f["measured"]),
                   str(f["golden"]), "PASS" if f["ok"] else "FAIL"))
    print("通过 %d/32" % npass)
    sys.exit(0 if npass == 32 else 1)


if __name__ == "__main__":
    main()
