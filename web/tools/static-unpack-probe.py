#!/usr/bin/env python3
"""静态拆包探针（原型）—— 只用产物内字面量，不执行任何 Lua。

用途：回答「不给运行时值，能不能解出明文」。当前实现覆盖 v15 载体层
（R[78]/R[73] 长串 → base94 → LCG/NL 折叠 → XOR），判定口径：
解出的字节流若含产物内的 Ng 转义表 token，或可打印率 ≥ 0.99，即判定
「静态可解」= 失守。

用法：
    python3 web/tools/static-unpack-probe.py <artifact.lua> [--dump out.bin]

诚实边界：仅覆盖已识别的载体车道；字节码车道（bdec/常量段）尚未实现，
所以本探针只能证明「存在静态可解路径」，不能证明「全部可解」。
"""
import re
import sys

MASK28 = 268435456
MASK32 = 4294967296


def literal(expr):
    """把产物里的 `a-b` / `a+b-c` 常量表达式折成整数。"""
    return int(eval(expr, {"__builtins__": {}}, {}))


def find_const(src, key):
    m = re.search(r'\[' + str(key) + r'\]=(\d+(?:[+-]\d+)+)', src)
    return literal(m.group(1)) if m else None


def find_table(src, key):
    i = src.find('[%d]={' % key)
    if i < 0:
        return None
    j = src.find('}', i)
    return [int(x) for x in src[i + len('[%d]={' % key):j].split(',') if x.strip()]


def find_long(src, key):
    m = re.search(r'\[' + str(key) + r'\]=\[(\[*)\]', src)
    m = re.search(r'\[' + str(key) + r'\]=(\[(=*)\[)', src)
    if not m:
        return None
    lvl, st = m.group(2), m.end()
    return src[st:src.find(']' + lvl + ']', st)]


def ng_tokens(src):
    m = re.search(r'Ng=\{([^}]*)\}', src)
    if not m:
        return []
    return re.findall(r'"(\w{5})"', m.group(1))


def build_stream(src):
    c16, c64 = find_const(src, 16), find_const(src, 64)
    a, b = find_table(src, 112), find_table(src, 118)
    if None in (c16, c64) or not a or not b:
        return None
    L = lambda t, i: t[i - 1]
    steps = [(L(a, 9) + L(b, 38), L(a, 15) + L(b, 26)),
             (L(a, 30) + L(b, 10), L(a, 31) + L(b, 41)),
             (L(a, 11) + L(b, 30), L(a, 45) + L(b, 9))]
    state = {'p': c16, 'pend': c64}

    def step():
        p = state['p']
        if state['pend'] is not None:
            p = (p + state['pend']) % MASK28
            state['pend'] = None
        for mul, add in steps:
            p = (p * mul + add) % MASK28
        state['p'] = p
        return p
    return step


def fold(u):
    u = (u + (u % 16384) * ((u - u % 16384) // 16384)) % MASK28
    u = (u * 257 + (u - u % 65536) // 65536) % MASK28
    return (u % 256 + ((u - u % 256) // 256) % 256
            + ((u - u % 65536) // 65536) % 256
            + ((u - u % 16777216) // 16777216)) % 256


def decode_window(blob, t0, groups, w, base, fl):
    out = bytearray()
    for j in range(groups):
        t = t0 + j
        idx = (t - 1) * 5
        ch = blob[idx:idx + 5]
        if len(ch) < 5:
            break
        v = 0
        for c in ch:
            v = (v * 94 + (ord(c) - 33)) % MASK32
        ve = (v - (base + t * fl) % MASK32) % MASK32
        for k in range(4):
            out.append(((ve >> (8 * k)) & 0xFF) ^ ((w + 1 + 4 * j + k) % 256))
    return bytes(out)


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    src = open(sys.argv[1], encoding='utf-8', errors='replace').read()
    dump = None
    if '--dump' in sys.argv:
        dump = sys.argv[sys.argv.index('--dump') + 1]

    step = build_stream(src)
    blob = find_long(src, 78)
    if not step or not blob:
        print('SKIP: 未命中 v15 载体签名（布局可能已变或非 v15 产物）')
        return 3

    base = (find_const(src, 61) + find_const(src, 20)) % MASK32
    fl = (find_const(src, 44) - find_const(src, 74)) % MASK32
    toks = ng_tokens(src)

    data = bytearray()
    st = build_stream(src)              # 重新起流，保证与首调用的 pending 语义一致
    for k in range((len(blob) // 115) + 1):
        w = fold(st())
        data += decode_window(blob, 1 + 23 * k, 23, w, base, fl)

    raw = bytes(data)
    hit = sum(raw.count(t.encode()) for t in toks)
    printable = sum(1 for x in raw if 32 <= x < 127 or x in (9, 10, 13)) / max(1, len(raw))
    verdict = 'STATIC BREAK' if (hit > 0 or printable > 0.99) else 'STILL SEALED'
    print(f'artifact : {sys.argv[1]}  ({len(src)} B)')
    print(f'carrier  : R[78] {len(blob)} chars -> {len(raw)} B decoded')
    print(f'ng hits  : {hit}/{len(toks)} tokens   printable={printable:.3f}')
    print(f'verdict  : {verdict}')
    if dump:
        open(dump, 'wb').write(raw)
        print(f'dumped   : {dump}')
    return 0 if verdict == 'STATIC BREAK' else 1


if __name__ == '__main__':
    sys.exit(main())
