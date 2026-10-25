#!/usr/bin/env python3
# Obsucvim wasm 数据段等长手术 v2（锚点按 R077 wasm 0f8dc517 实测字节）
# S1: EP SALT 三源派生+时钟门 -> R076 单源派生（核 + do-end 闭合，尾空格垫等长）
# S2: 三车道 lane NL 像 -> 恒等线性（piece1 去括号翻转、piece2 尾巴置空格；
#     bx_ep/bx_rt 共用 120B piece2（_sv 名）、ct_ep/ct_rt 共用 129B（_svk 名）、
#     km_ep 独有 129B（_svq 名）——实测数据段字面量合并布局）
# 用法: wasm-data-surgery.py <in.wasm> <out.wasm> s1|s2|s1s2|f1|f2
import sys, hashlib

SRC, DST, MODE = sys.argv[1], sys.argv[2], sys.argv[3]
data = bytearray(open(SRC, 'rb').read())
log = []

def patch_unique(old: bytes, new: bytes, tag: str):
    assert len(old) == len(new), (tag, len(old), len(new))
    i = data.find(old)
    if i < 0:
        raise SystemExit(f"[{tag}] 原文未找到: {old[:60]!r}")
    if data.find(old, i + 1) >= 0:
        raise SystemExit(f"[{tag}] 原文多处出现 @{i}（放弃保险）")
    data[i:i + len(old)] = new
    log.append(f"[{tag}] @{i} {len(old)}B")

# ---------- S1 ----------
EP_S1_OLD = (b'local _sn=0 for _si=1,3 do '
             b'local _so=(_si<3) and {} or function() end '
             b'local _ts=TOSTR(_so) '
             b'for _i=1,#_ts do '
             b'_sn=(_sn*31+BYTE(_ts,_i))%268435456 '
             b'_sn=(_sn+(_sn%16384)*((_sn-(_sn%16384))/16384))%268435456 '
             b'end end '
             b'SALT=(_sn*257+((_sn-(_sn%65536))/65536))%268435456 '
             b'if TYP(CLK)=="function" and TYP(PCAL)=="function" then '
             b'local _cko,_ckv=PCAL(CLK) '
             b'if _cko and TYP(_ckv)=="number" then '
             b'SALT=(SALT+(_ckv%65536))%268435456 end end end')

def do_s1():
    n = len(EP_S1_OLD)
    core = (b'local _st={} local _ts=TOSTR(_st) '
            b'for _i=1,#_ts do SALT=(SALT*31+BYTE(_ts,_i))%268435456 end end')
    assert len(core) <= n
    patch_unique(EP_S1_OLD, core + b' ' * (n - len(core)), "S1-salt-revert")

# ---------- S2 ----------
# piece1: 去 '(' 翻转成 BASE+( —— 等长
P1 = [
    (b'local _sv=(SALT+',  b'local _sv=SALT+('),
    (b'local _sv=(SLT+',   b'local _sv=SLT+('),
    (b'local _svk=(SALT+', b'local _svk=SALT+('),
    (b'local _svk=(SLTC+', b'local _svk=SLTC+('),
    (b'local _svq=(SALT+', b'local _svq=SALT+('),
]
# piece2（共用存储，名字硬编码在片内）: ')%268435456 <X>=(<NL链>)%268435456 <X>=(<NL2>)%268435456 '
def p2_old(nm: bytes) -> bytes:
    return (b')%268435456 ' + nm + b'=(' + nm + b'+(' + nm + b'%16384)*((' + nm
            + b'-(' + nm + b'%16384))/16384))%268435456 ' + nm + b'=(' + nm + b'*257+(('
            + nm + b'-(' + nm + b'%65536))/65536))%268435456 ')

def do_s2():
    for old, new in P1:
        assert len(old) == len(new), (old, len(old), len(new))
        patch_unique(old, new, f"S2-p1-{old[6:12].decode()}")
    for nm in (b'_sv', b'_svk', b'_svq'):
        old = p2_old(nm)
        core = b')%268435456 '
        new = core + b' ' * (len(old) - len(core))
        patch_unique(old, new, f"S2-p2-{nm.decode()}")

if 's1' in MODE:
    do_s1()
if 's2' in MODE:
    do_s2()
# ---------- F1/F2（P1 车道解剖；作用于 R077 病件 wasm） ----------
# piece2 结构:')%268435456 ' + FOLD1 + FOLD2（全等长改写：杀哪级就把哪段置空格）
def fold1(nm: bytes) -> bytes:
    return nm + b'=(' + nm + b'+(' + nm + b'%16384)*((' + nm + b'-(' + nm + b'%16384))/16384))%268435456 '
def fold2(nm: bytes) -> bytes:
    return nm + b'=(' + nm + b'*257+((' + nm + b'-(' + nm + b'%65536))/65536))%268435456 '

def do_f(mode):
    # 校验基件确是 R077 病件三车道形态
    for nm in (b'_sv', b'_svk', b'_svq'):
        old = b')%268435456 ' + fold1(nm) + fold2(nm)
        i = data.find(old)
        if i < 0:
            raise SystemExit(f"[{mode}] {nm.decode()} piece2 原文未找到")
        if data.find(old, i + 1) >= 0:
            raise SystemExit(f"[{mode}] {nm.decode()} piece2 多处出现 @{i}")
        f1, f2 = fold1(nm), fold2(nm)
        if mode == 'f1':
            new = b')%268435456 ' + f1 + b' ' * len(f2)
        else:
            new = b')%268435456 ' + b' ' * len(f1) + f2
        data[i:i + len(old)] = new
        log.append(f"[{mode}-{nm.decode()}] @{i} {len(old)}B")

if 'f1' in MODE:
    do_f('f1')
if 'f2' in MODE:
    do_f('f2')

open(DST, 'wb').write(bytes(data))
print('\n'.join(log))
print("out sha:", hashlib.sha256(bytes(data)).hexdigest()[:16], "size:", len(data))
