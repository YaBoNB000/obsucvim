#!/usr/bin/env python3
# 径织 PathWeave PW-1 载体生成器（自研新技术线，2026-10-25 立；同日紊壳改版）
# ============================================================
# 三模块：
#   径锁 Pathlock —— 真实载荷帧（"PW01"+djb2 校验+u32 长）加法流加密 +
#                   定制 base85 入串；探针电池 8 项全为跨实现逐位定论的
#                   语言/宿主形制探针（禁浮点/时钟/位运算——P1 教训：
#                   只用 <2^53 整数模加乘与 string/table 原语）。
#   幻径 Mirage  —— N-1 条诱饵车道：同质加密、同 alphabet 外观的假程序；
#                   电池失配或真帧校验失败一律静默失步到幻径（不报错），
#                   绞杀 envlog 沙箱重跑与 --no-devirt 秒级 trace 收割。
#   织构 Weave   —— 生成器确定性（同种子逐字节恒等）；车道数/种子/标签可调。
#
# 紊壳 Snarl（2026-10-25 改版，应用户反馈「外壳太规整」）：
#   外壳恒为单行致密件（minify 化），四路种子化紊化——
#     名紊：locals 整池换名（短 minify 风 / 下划线随机风 / 混合三式随种）；
#     序紊：8 探针洗牌 + == 两侧随机换位 + if / and-or 双形态随机 + 表声明洗牌；
#     量紊：255/5381/65536/16777216/4294967296 魔数拆精确整数表达式（<2^53）；
#     串紊：alphabet 播种洗牌（顺序不再固定=签名杀灭）+ blob 串随机段 .. 拼接。
#   语义逐字节等价（门禁 11/11 复绿）；规整合成壳废止，无回退开关。
#
# 用法: pathweave.py <in.lua> <out.lua> --lanes N --seed S [--label TEXT]
# 诚实边界：PW-1 不创造白盒秘密（静态完美模拟仍可逐层推导），定位 =
#   自动化解混淆管线的摩擦层 + 欺骗层；真钥层仍由引擎 EVKEY/exguard 承担。
#   紊壳只抬眼检/签名/规则提取摩擦，不抬密码学强度。
import sys, random, hashlib

# ---------------------------------------------------------------- b85
# 字母表 85 字符：双引号串内免转义（无 " ' \ 控制符）。
# 注：发射到外壳的 alphabet 是经种子洗牌的，本节为标准顺序。
B85 = ("0123456789"
       "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
       "abcdefghijklmnopqrstuvwxyz"
       "!#$%&()*+,-./:;<=>?@[]^")
assert len(B85) == 85

def b85enc(data: bytes, alph: str = B85) -> str:
    pad = (-len(data)) % 4
    if pad:
        data = data + b"\x00" * pad
    out = []
    for i in range(0, len(data), 4):
        v = int.from_bytes(data[i:i + 4], "big")
        ds = [0] * 5
        for j in range(4, -1, -1):
            ds[j] = v % 85
            v //= 85
        out.extend(alph[d] for d in ds)
    return "".join(out)

# ---------------------------------------------------------------- 加法流加密
def stream_crypt(data: bytes, seed: int) -> bytes:
    r = seed % 2147483647 or 1
    out = bytearray()
    for i, b in enumerate(data):
        r = (r * 48271 + (i * 97) % 2147483646) % 2147483647
        out.append((b + r % 251) % 256)
    return bytes(out)

def djb2(data: bytes, mod: int = 4294967296) -> int:
    h = 5381
    for b in data:
        h = (h * 33 + b) % mod
    return h

def frame(payload: bytes) -> bytes:
    """真实载荷帧：PW01 | djb2(payload) u32 LE | payload。"""
    v = djb2(payload)
    return (b"PW01" + bytes((v & 0xFF, (v >> 8) & 0xFF,
                             (v >> 16) & 0xFF, (v >> 24) & 0xFF))
            + payload)

# ---------------------------------------------------------------- 幻径诱饵池
def decoy_program(rng: random.Random, label: str) -> bytes:
    kind = rng.randrange(5)
    if kind == 0:  # 伪斐波那契（起始/步进扰动）
        a, b = rng.randrange(2, 9), rng.randrange(1, 5)
        n, m = rng.randrange(8, 14), rng.randrange(2, 4)
        src = ("local a,b=%d,%d local o={tostring(a)} "
               "for i=2,%d do a,b=b,(a+b*%d)%%100000 "
               "o[#o+1]=tostring(a) end "
               "print(\"%s:\"..table.concat(o,\",\"))") % (a, b, n, m, label)
    elif kind == 1:  # 伪心跳累计
        t = rng.randrange(1000, 9999)
        src = ("local s=0 for i=1,%d do s=(s+i*7)%%65521 end "
               "print(\"%s hb:\"..s..\" ok\")") % (t, label)
    elif kind == 2:  # 伪配置表装载
        ks = [rng.choice(("alpha", "beta", "gamma", "delta", "kappa"))
              for _ in range(4)]
        src = ("local c={} " +
               "".join("c.%s=%d " % (k, rng.randrange(10, 999)) for k in ks) +
               "for k in pairs(c) do end print(\"%s cfg:\"..#c)") % label
    elif kind == 3:  # 伪字符串装配
        w = rng.choice(("module", "bundle", "shard", "realm"))
        src = ("local p={\"%s\",\"%d\"} local s=\"\" "
               "for i=1,#p[1] do s=s..\"%d.\" end "
               "print(\"%s \"..p[1]..\"/\"..s..p[2])") % (
               w, rng.randrange(10, 99), rng.randrange(10, 99), label)
    else:  # 伪校验环
        m = rng.randrange(3, 9)
        src = ("local x=%d for i=1,%d do x=(x*3+1)%%%d end "
               "print(\"%s chk:\"..x)") % (
               rng.randrange(11, 97), rng.randrange(5, 30), m ** 5, label)
    return src.encode()

# ---------------------------------------------------------------- 紊壳部件
LUA_KEYWORDS = {"and", "break", "continue", "do", "else", "elseif", "end",
                "false", "for", "function", "goto", "if", "in", "local",
                "nil", "not", "or", "repeat", "return", "then", "true",
                "until", "while"}
# 外壳引用的全局/调用面——名池必须避让（影子遮蔽会改语义）
RESERVED_GLOBALS = {"game", "type", "pcall", "select", "error", "tostring",
                    "loadstring", "load", "string", "table", "Vector3",
                    "task", "identifyexecutor", "checkcaller", "pairs"}

class NameGen:
    """种子化名发生器：三风格随种 0=短 minify 风 1=_随机长名 2=混合。"""

    def __init__(self, rng: random.Random):
        self.rng = rng
        self.style = rng.randrange(3)
        self.used = set()
        self._pool = None
        self._pi = 0

    def _mk(self) -> str:
        rng = self.rng
        letters = ("abcdefghijklmnopqrstuvwxyz"
                   "ABCDEFGHIJKLMNOPQRSTUVWXYZ")
        if self.style == 0:
            if self._pool is None:
                pool = [c for c in letters]
                pool += [a + b for a in letters for b in letters]
                pool = [w for w in pool
                        if w not in LUA_KEYWORDS and w not in RESERVED_GLOBALS]
                rng.shuffle(pool)
                self._pool = pool
            w = self._pool[self._pi]
            self._pi += 1
            return w
        if self.style == 1:
            return "_" + "".join(rng.choice(letters + "0123456789")
                                 for _ in range(rng.randrange(8, 15)))
        if rng.random() < 0.5:
            return "_" + "".join(rng.choice(letters + "0123456789")
                                 for _ in range(rng.randrange(4, 9)))
        return "".join(rng.choice(letters) for _ in range(rng.randrange(1, 5)))

    def take(self, n: int = 1):
        out = []
        while len(out) < n:
            w = self._mk()
            if w in LUA_KEYWORDS or w in RESERVED_GLOBALS or w in self.used:
                continue
            self.used.add(w)
            out.append(w)
        return out[0] if n == 1 else out

def split_num(rng: random.Random, v: int) -> str:
    """整数拆正两项（精确 double，<2^53）；小值不拆。"""
    if v <= 7:
        return str(v)
    a = rng.randrange(1, v // 2 + 1)
    return "(%d+%d)" % (a, v - a)

def split_str(rng: random.Random, s: str):
    """串切随机段（85% 长段 140..760，15% 碎段 3..40），字符全 alphabet 安全。"""
    out, i = [], 0
    while i < len(s):
        if rng.random() < 0.15:
            ln = rng.randrange(3, 41)
        else:
            ln = rng.randrange(140, 761)
        out.append(s[i:i + ln])
        i += ln
    return out

def quoted_segments(rng: random.Random, s: str) -> str:
    """分段拼接表达式。Luau 编译器对一条右结合 .. 长链每操作数各占一寄存器，
    裸长链会撞 255 寄存器墙（Out of registers）——两级括号分组：
    内层每组 ≤64 段、外层每链 ≤48 组（峰值 ~112），超限再递归一层。"""
    segs = split_str(rng, s)
    groups = []
    for i in range(0, len(segs), 64):
        grp = segs[i:i + 64]
        e = "..".join('"%s"' % g for g in grp)
        groups.append("(%s)" % e if len(grp) > 1 else '"%s"' % grp[0])
    while len(groups) > 48:
        nxt = []
        for i in range(0, len(groups), 48):
            grp = groups[i:i + 48]
            e = "..".join(grp)
            nxt.append("(%s)" % e if len(grp) > 1 else grp[0])
        groups = nxt
    return "..".join(groups)

def probes_block(rng: random.Random, nE: str):
    """8 探针洗牌 + if/and-or 双形态 + == 两侧换位 + 权重偶发拆项。"""
    def w_(w):
        return split_num(rng, w) if w > 7 and rng.random() < 0.6 else str(w)

    def eq(lhs, rhs):  # == 可换序（两侧皆纯表达式，不调函数外副作用）
        return (lhs + "==" + rhs) if rng.random() < 0.5 else (rhs + "==" + lhs)

    conds = [
        (eq("type(game)", '"userdata"'), 1),
        ('type(identifyexecutor)=="function" and '
         + eq('identifyexecutor()', '"Delta"'), 2),
        (eq("type(checkcaller)", '"function"'), 4),
        (eq('(select(2,pcall(function() error("x",0) end)))', '"x"'), 8),
        ('tostring(function() end):find(": 0x")', 16),
        ('type(Vector3)=="table" and ' + eq("type(Vector3.new)", '"function"'), 32),
        ('type(task)=="table" and ' + eq("type(task.spawn)", '"function"'), 64),
        (eq("tostring(nil)", '"nil"') + " and " + eq("tostring(true)", '"true"'), 128),
    ]
    rng.shuffle(conds)
    out = []
    for c, w in conds:
        if rng.random() < 0.5:
            out.append("if %s then %s=%s+%s end" % (c, nE, nE, w_(w)))
        else:
            out.append("%s=%s+(%s and %s or 0)" % (nE, nE, c, w_(w)))
    return out

def build_shell(rng: random.Random, blobs85, seeds, flens, alph: str) -> str:
    N = NameGen(rng)
    nB, nS, nL = N.take(3)          # 车道串表 / 种子表 / 原始长表
    nA, nC, nD = N.take(3)          # alphabet / 解码映射 / 解码函数
    pS, pSd, pLn, pR = N.take(4)    # fD 参数与流态
    pW, pAcc, pCnt, pOu = N.take(4)
    pI, pK, pM, pPb = N.take(4)
    nE = N.take()                   # 电池累计
    nF, nH = N.take(2)              # 车道1 串 / 串 djb2
    nM, mL, mR = N.take(3)          # 幻径函数与其局部
    nJ, nK, nV, nBody, nH2, nFf = N.take(6)   # 真帧解码/校验/执行
    iT = N.take()                   # 映射表 for 变量
    iH1, iH2 = N.take(2)            # 两条 djb2 环变量

    decl_b = "local %s={%s}" % (
        nB, ",".join(quoted_segments(rng, c) for c in blobs85))
    decl_s = "local %s={%s}" % (nS, ",".join(str(s) for s in seeds))
    decl_l = "local %s={%s}" % (nL, ",".join(str(x) for x in flens))
    decls = [decl_b, decl_s, decl_l]
    rng.shuffle(decls)

    stmts = list(decls)
    naseg = rng.randrange(2, 6)
    step = max(1, (len(alph) + naseg - 1) // naseg)
    stmts.append("local %s=%s" % (
        nA, "..".join('"%s"' % alph[i:i + step]
                      for i in range(0, len(alph), step))))
    stmts.append("local %s={} for %s=1,#%s do %s[%s:sub(%s,%s)]=%s-1 end" % (
        nC, iT, nA, nC, nA, iT, iT, iT))

    init3 = (("local %s,%s,%s=0,0,0" % (pW, pAcc, pCnt)) if rng.random() < 0.5
             else ("local %s=0 local %s=0 local %s=0" % (pW, pAcc, pCnt)))
    stmts.append(
        ("local function %s(%s,%s,%s) local %s=%s%%2147483647 "
         "if %s==0 then %s=1 end %s local %s={} "
         "for %s=1,#%s do %s=%s*85+(%s[%s:sub(%s,%s)]or 0) %s=%s+1 "
         "if %s==5 then for %s=3,0,-1 do local %s=2^(%s*8) "
         "local %s=(%s-(%s%%%s))/%s%%256 "
         "%s=(%s*48271+(%s*97)%%2147483646)%%2147483647 "
         "%s[#%s+1]=string.char((%s-%s%%251)%%256) %s=%s+1 "
         "if %s>=%s then break end end %s=0 %s=0 end "
         "if %s>=%s then break end end return table.concat(%s,\"\",1,%s) end")
        % (nD, pS, pSd, pLn, pR, pSd, pR, pR, init3, pOu,
           pI, pS, pAcc, pAcc, nC, pS, pI, pI, pCnt, pCnt,
           pCnt, pK, pM, pK,
           pPb, pAcc, pAcc, pM, pM,
           pR, pR, pW,
           pOu, pOu, pPb, pR, pW, pW,
           pW, pLn, pAcc, pCnt,
           pW, pLn, pOu, pLn))

    stmts.append("local %s=0" % nE)
    stmts += probes_block(rng, nE)

    stmts.append(
        "local %s=%s[1] local %s=%s for %s=1,#%s do "
        "%s=(%s*33+string.byte(%s,%s))%%(65536*65536) end" % (
            nF, nB, nH, split_num(rng, 5381), iH1, nF,
            nH, nH, nF, iH1))
    stmts.append(
        "local function %s() local %s=2+((%s*31+%s)%%(%d-1)) "
        "local %s=%s(%s[%s],%s[%s],%s[%s]) "
        "return (loadstring or load)(%s)() end" % (
            nM, mL, nE, nH, len(blobs85), mR, nD, nB, mL, nS, mL, nL, mL, mR))

    tail = [
        "if %s==%s then" % (nE, split_num(rng, 255)),
        "local %s=%s(%s,%s[1],%s[1])" % (nJ, nD, nF, nS, nL),
        'local %s=%s:sub(1,4)=="PW01"' % (nK, nJ),
        "if %s then" % nK,
        ("local %s=string.byte(%s,5)+string.byte(%s,6)*256"
         "+string.byte(%s,7)*(256*256)+string.byte(%s,8)*(4096*4096)")
        % (nV, nJ, nJ, nJ, nJ),
        "local %s=%s:sub(9)" % (nBody, nJ),
        "local %s=%s for %s=1,#%s do "
        "%s=(%s*33+string.byte(%s,%s))%%(65536*65536) end" % (
            nH2, split_num(rng, 5381), iH2, nBody, nH2, nH2, nBody, iH2),
        "%s=%s==%s" % (nK, nV, nH2),
        "if %s then %s=nil %s=nil %s=nil "
        "local %s=(loadstring or load)(%s) return %s() end" % (
            nK, nB, nA, nC, nFf, nBody, nFf),
        "end end",
        "return %s()" % nM,
    ]
    return " ".join(stmts + tail) + "\n"

def wrap(payload: bytes, lanes: int, seed: int, label: str) -> bytes:
    assert lanes >= 2
    rng = random.Random(seed)
    alph_l = list(B85)
    rng.shuffle(alph_l)          # 串紊：alphabet 播种洗牌（编解码同源一致）
    alph = "".join(alph_l)

    blobs_payload = [frame(payload)]
    seen = set()
    while len(blobs_payload) < lanes:
        d = decoy_program(rng, label)
        if d not in seen:
            seen.add(d)
            blobs_payload.append(d)

    seeds = [rng.randrange(1, 2147483646) for _ in range(lanes)]
    ciphers = [stream_crypt(p, s) for p, s in zip(blobs_payload, seeds)]
    blob85 = [b85enc(c, alph) for c in ciphers]
    flens = [len(p) for p in blobs_payload]

    return build_shell(rng, blob85, seeds, flens, alph).encode()

def main():
    args = sys.argv[1:]
    if len(args) < 2:
        raise SystemExit(
            "用法: pathweave.py <in.lua> <out.lua> --lanes N --seed S [--label TEXT]")
    src, dst = args[0], args[1]
    lanes, seed, label = 6, 20261025, "pw"
    if "--lanes" in args:
        lanes = int(args[args.index("--lanes") + 1])
    if "--seed" in args:
        seed = int(args[args.index("--seed") + 1])
    if "--label" in args:
        label = args[args.index("--label") + 1]
    payload = open(src, "rb").read()
    out = wrap(payload, lanes, seed, label)
    open(dst, "wb").write(out)
    print("PW-1 snarled wrap: %dB -> %dB lanes=%d seed=%d out-sha=%s"
          % (len(payload), len(out), lanes, seed,
             hashlib.sha256(out).hexdigest()[:16]))

if __name__ == "__main__":
    main()
