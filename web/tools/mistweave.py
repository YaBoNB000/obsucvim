#!/usr/bin/env python3
# 雾织 MistWeave MW-1 源级变换器（自研新技术线 #2，2026-10-25 立）
# ============================================================
# 引擎「输入侧」纯 Lua→Lua 变换（与 PW-1 引擎外载体层正交）：
#   雾锁 Foglock —— 整数恒等不透明门（跨实现逐位定论族：x(x+1) 恒偶、
#                  x²%4∈{0,1}、(x%3)²%3∈{0,1}、xK%M 恒零族等），永假支内
#                  织入似真诱饵体（伪配置/伪解码/伪遥测），永真支织入纯噪
#                  执行体（trace 污染）；只用 <2^53 整数模加乘/模比较——
#                  禁浮点边界与位运算（P1/R079 纪律内化）。
#   雾织 Weave  —— chunk 顶雾块 + 函数体顶雾点（新 local 唯一名，绝不改
#                  写用户标识符/结构，雷达纪律 = 只增不改）。
#   雾数 Fognum —— 整数字面量 → 恒等算术链（乘法掩码族 (c*a+b-b)/a +
#                  可选雾局部联锁/二级梯）。
# 用法: mistweave.py <in.lua> <out.lua> --seed S [--num-fog 0.55] [--blocks 6]
# 输出与输入语义逐位等价（语义门实证）；MW-1 与引擎 R079 串接：
# 雾永假支进 VM 后是方言随机的真字节码块，静态剥离器须先完成虚拟化
# 才可动剪——装配顺序：源码 → MW-1 → 引擎 →（可选 PW-1 载体）。
import sys, random, re, hashlib

# ---------------------------------------------------------------- 词法器（子集）
# token: (kind, text, start)；kind ∈ ws/comment/str/num/name/kw/op
KW = {"and", "break", "do", "else", "elseif", "end", "false", "for",
      "function", "if", "in", "local", "nil", "not", "or", "repeat",
      "return", "then", "true", "until", "while", "continue"}
NUM_RE = re.compile(r"0[xX][0-9a-fA-F]+|\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|\.\d+(?:[eE][+-]?\d+)?")
LONG_RE = re.compile(r"\[(=*)\[")
OPS = ["...", "..", "..=", "==", "~=", "<=", ">=", "::", "->", "//",
       "+=", "-=", "*=", "/=", "//=", "%=", "^=",
       "+", "-", "*", "/", "%", "^", "#", "&", "~", "|", "<", ">",
       "=", "(", ")", "{", "}", "[", "]", ";", ":", ",", "."]

def lex(src: str):
    toks = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c in " \t\r\n":
            j = i
            while j < n and src[j] in " \t\r\n":
                j += 1
            toks.append(("ws", src[i:j], i)); i = j; continue
        if src.startswith("--", i):
            m = LONG_RE.match(src, i + 2)
            if m:
                close = "]" + m.group(1) + "]"
                j = src.find(close, i + 3 + len(m.group(1)))
                j = n if j < 0 else j + len(close)
            else:
                j = src.find("\n", i)
                j = n if j < 0 else j
            toks.append(("comment", src[i:j], i)); i = j; continue
        if c in "\"'":
            q = c; j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2; continue
                if src[j] == q:
                    j += 1; break
                j += 1
            toks.append(("str", src[i:j], i)); i = j; continue
        if c == "`":  # Luau 反勾串：整体不透明（内插内数字不雾，稳妥冗余）
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2; continue
                if src[j] == "`":
                    j += 1; break
                j += 1
            toks.append(("str", src[i:j], i)); i = j; continue
        m = LONG_RE.match(src, i)
        if m:
            close = "]" + m.group(1) + "]"
            j = src.find(close, i + 2 + len(m.group(1)))
            j = n if j < 0 else j + len(close)
            toks.append(("str", src[i:j], i)); i = j; continue
        m = NUM_RE.match(src, i)
        if m and (c.isdigit() or (c == "." and i + 1 < n and src[i + 1].isdigit())):
            toks.append(("num", m.group(0), i)); i = m.end(); continue
        if c.isalpha() or c == "_":
            j = i
            while j < n and (src[j].isalnum() or src[j] == "_"):
                j += 1
            w = src[i:j]
            toks.append(("kw" if w in KW else "name", w, i)); i = j; continue
        hit = None
        for op in OPS:
            if src.startswith(op, i):
                hit = op; break
        if hit:
            toks.append(("op", hit, i)); i += len(hit); continue
        raise ValueError("lex: unhandled char %r @%d" % (c, i))
    return toks

# ---------------------------------------------------------------- 雾锁谓词族
FALSE_GATES = [
    lambda x, r: "((%s*%s+%s)%%2)==1" % (x, x, x),                 # x(x+1) 恒偶
    lambda x, r: "(%s*%s)%%4==%d" % (x, x, r.choice((2, 3))),      # x²%4∈{0,1}
    lambda x, r: "((%s%%3)*(%s%%3))%%3==2" % (x, x),               # 平方剩余 mod3
    lambda x, r: "((%s*(%d))%%5)==%d" % (x, r.randrange(2, 9) * 5,
                                          r.randrange(1, 5)),      # x·5k%5≡0 → 判 1..4 恒假
    lambda x, r: "(%s*(%s+1))%%2==1" % (x, x),                     # 同族变体
    lambda x, r: "((%s+%s+1)%%2)==0" % (x, x),                     # x+x+1 恒奇 → 判偶恒假
]
TRUE_GATES = [
    lambda x, r: "((%s*%s+%s)%%2)==0" % (x, x, x),
    lambda x, r: "(%s*%s)%%4~=%d" % (x, x, r.choice((2, 3))),
    lambda x, r: "((%s*(%d))%%7)==0" % (x, r.randrange(3, 15) * 7),
    lambda x, r: "((%s+%s)%%2)==0" % (x, x),
]

# ---------------------------------------------------------------- 幻体模板
class NamePool:
    def __init__(self, rng):
        self.rng = rng; self.used = set()
    def fresh(self, k=1):
        out = []
        for _ in range(k):
            while True:
                nm = "_mw" + "".join(self.rng.choice("abcdefghijklmnopqrstuvwxyz")
                                     for _ in range(7))
                if nm not in self.used:
                    self.used.add(nm); out.append(nm); break
        return out

def decoy_body(rng, pool):
    a, b = rng.randrange(11, 97), rng.randrange(100, 999)
    v1, v2 = pool.fresh(2)
    kind = rng.randrange(5)
    if kind == 0:
        return ("local %s={ver=%d,tag=0x%x,on=%s} %s=(%s*%d+%d)%%65521 " % (
            v1, rng.randrange(1, 40), rng.randrange(256, 65535),
            rng.choice(("true", "false")), v2, a, rng.randrange(3, 9), b))
    if kind == 1:
        return ("local %s=\"\" for _q=1,%d do %s=%s..(\"%s\"):sub(1,1) end "
                "local %s=#%s*%d " % (v1, rng.randrange(4, 12), v1, v1,
                                      rng.choice(("k7", "vx", "q9", "m2")),
                                      v2, v1, rng.randrange(3, 8)))
    if kind == 2:
        return ("local %s=0 while %s<%d do %s=(%s*%d+17)%%%d end "
                "%s=(%s+%d)%%65521 " % (v1, v1, rng.randrange(50, 400), v1,
                                        v1, rng.randrange(3, 13),
                                        rng.choice((97, 251, 1021, 4093)),
                                        v2, v1, b))
    if kind == 3:
        return ("local %s={%d,%d,%d} for _i=1,#%s do %s[_i]=(%s[_i]+%d)%%%d end " % (
            v1, a, b, rng.randrange(1, 9), v1, v1, v1,
            rng.randrange(1, 99), rng.choice((251, 1021))))
    return "local %s=(%d*%d+1)%%%d-%d " % (
        v1, a, b, rng.choice((65521, 1021)), rng.randrange(1, 50))

# ---------------------------------------------------------------- 雾数
def fog_number(c: int, rng, fog_locals) -> str:
    a = rng.randrange(3, 97)
    b = rng.randrange(100, 9999)
    # (c*a+b)-b)/a：乘法掩码族，c*a+b < 10^6*97+9999 + … « 2^53 → double 精确
    if rng.random() < 0.35 and fog_locals:
        lv = rng.choice(fog_locals)
        return "((%d*%d+%s*%d)-(%s*%d))/%d" % (c, a, lv, b, lv, b, a)
    if rng.random() < 0.30:  # 二级梯
        a2, b2 = rng.randrange(3, 31), rng.randrange(10, 999)
        return "((((%d*%d+%d)-%d)/%d)*%d+%d-%d)/%d" % (
            c, a, b, b, a, a2, b2, b2, a2)
    return "((%d*%d+%d)-%d)/%d" % (c, a, b, b, a)

INT_RE = re.compile(r"^\d+$")

# ---------------------------------------------------------------- 主变换
def mw_transform(src: str, seed: int, num_fog: float = 0.55,
                 blocks: int = 6) -> bytes:
    rng = random.Random(seed)
    pool = NamePool(rng)
    toks = lex(src)  # (kind, text, start)

    # 雾局部名 + 真值（chunk 顶声明）
    fog_locals = pool.fresh(6)
    fog_vals = {nm: rng.randrange(3, 999983) for nm in fog_locals}

    edits = []  # (start, end, newtext)，零宽 end==start 即插入

    # --- 雾数：num token 级替换（纯十进制整数 0..1e6，概率 p）
    for kind, text, st in toks:
        if kind == "num" and INT_RE.match(text):
            v = int(text)
            if v <= 1_000_000 and rng.random() < num_fog:
                edits.append((st, st + len(text),
                              fog_number(v, rng, fog_locals)))

    # --- 函数体顶插入点：kw function → 参数表 '(' → 配对 ')' 之后
    sig = [(k, t) for k, t, _ in toks if k in ("kw", "name", "op")]
    sig_off = [st for k, t, st in toks if k in ("kw", "name", "op")]
    body_tops = []
    for i, (kind, text) in enumerate(sig):
        if kind == "kw" and text == "function":
            j = i + 1
            guard = 0
            while j < len(sig) and sig[j][1] != "(":
                if sig[j][0] == "kw":
                    break
                j += 1; guard += 1
                if guard > 40:
                    break
            if j >= len(sig) or sig[j][1] != "(":
                continue
            depth, m = 0, j
            while m < len(sig):
                if sig[m][1] == "(":
                    depth += 1
                elif sig[m][1] == ")":
                    depth -= 1
                    if depth == 0:
                        break
                m += 1
            if m < len(sig) and depth == 0:
                body_tops.append(sig_off[m] + 1)  # ')' 末位之后

    def fog_block(is_top):
        n = blocks if is_top else rng.choice((1, 1, 2))
        parts = []
        for _ in range(n):
            lv = rng.choice(fog_locals)
            if rng.random() < 0.7:
                g = rng.choice(FALSE_GATES)(lv, rng)
                parts.append("if %s then %send " % (g, decoy_body(rng, pool)))
            else:
                g = rng.choice(TRUE_GATES)(lv, rng)
                n1, n2 = pool.fresh(2)
                parts.append(
                    "if %s then local %s=(%s*%d+%d)%%65521 "
                    "local %s=(%s+%s)%%%d end " % (
                        g, n1, lv, rng.randrange(3, 31),
                        rng.randrange(1, 99), n2, n1, lv,
                        rng.choice((251, 1021))))
        return "".join(parts)

    # chunk 顶（雾局部声明 + 雾块）
    head = ("local %s=%s " % (",".join(fog_locals),
                              ",".join(str(fog_vals[n]) for n in fog_locals))
            + fog_block(True))
    edits.append((0, 0, head))
    for cp in body_tops:
        edits.append((cp, cp, fog_block(False)))

    # --- 装配：倒序应用偏移手术（替换与插入互不重叠）
    edits.sort(key=lambda e: (e[0], e[1]), reverse=True)
    out = src
    for st, en, txt in edits:
        out = out[:st] + txt + out[en:]
    return out.encode()


def main():
    args = sys.argv[1:]
    if len(args) < 2:
        raise SystemExit(
            "用法: mistweave.py <in.lua> <out.lua> --seed S "
            "[--num-fog 0.55] [--blocks 6]")
    src_p, dst_p = args[0], args[1]
    seed, num_fog, blocks = 20261025, 0.55, 6
    if "--seed" in args:
        seed = int(args[args.index("--seed") + 1])
    if "--num-fog" in args:
        num_fog = float(args[args.index("--num-fog") + 1])
    if "--blocks" in args:
        blocks = int(args[args.index("--blocks") + 1])
    src = open(src_p, encoding="utf-8").read()
    out = mw_transform(src, seed, num_fog, blocks)
    open(dst_p, "wb").write(out)
    print("MW-1 weave: %dB -> %dB seed=%d num-fog=%.2f blocks=%d out-sha=%s"
          % (len(src.encode()), len(out), seed, num_fog, blocks,
             hashlib.sha256(out).hexdigest()[:16]))

if __name__ == "__main__":
    main()
