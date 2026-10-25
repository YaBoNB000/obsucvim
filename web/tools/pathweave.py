#!/usr/bin/env python3
# 径织 PathWeave PW-1 载体生成器（自研新技术线，2026-10-25 立）
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
# 用法: pathweave.py <in.lua> <out.lua> --lanes N --seed S [--label TEXT]
# 诚实边界：PW-1 不创造白盒秘密（静态完美模拟仍可逐层推导），定位 =
#   自动化解混淆管线的摩擦层 + 欺骗层；真钥层仍由引擎 EVKEY/exguard 承担。
import sys, random, hashlib

# ---------------------------------------------------------------- b85
# 字母表 85 字符：双引号串内免转义（无 " ' \ 控制符）
B85 = ("0123456789"
       "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
       "abcdefghijklmnopqrstuvwxyz"
       "!#$%&()*+,-./:;<=>?@[]^")
assert len(B85) == 85

def b85enc(data: bytes) -> str:
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
        out.extend(B85[d] for d in ds)
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

# ---------------------------------------------------------------- Lua 装载器
# 段占位：@@BLOBS@@（车道密文 b85 串表）、@@SEEDS@@（车道种子表）、
#         @@FLENS@@（车道解密后原始长表）、@@NL@@（车道总数）
LOADER_TMPL = '''\
local _PWA=@@BLOBS@@
local _PWS=@@SEEDS@@
local _PWL=@@FLENS@@
local _PWB="@@ALPH@@"
local _PWC={}for _i=1,#_PWB do _PWC[_PWB:sub(_i,_i)]=_i-1 end
local function _PWD(s,sd,ln)local r=sd%2147483647 if r==0 then r=1 end local w=0 local acc=0 local cnt=0 local ou={}for i=1,#s do acc=acc*85+(_PWC[s:sub(i,i)]or 0)cnt=cnt+1 if cnt==5 then for k=3,0,-1 do local m=2^(k*8)local pb=(acc-(acc%m))/m%256 r=(r*48271+(w*97)%2147483646)%2147483647 ou[#ou+1]=string.char((pb-r%251)%256)w=w+1 if w>=ln then break end end acc=0 cnt=0 end if w>=ln then break end end return table.concat(ou,"",1,ln)end
local _PWE=0
if type(game)=="userdata" then _PWE=_PWE+1 end
if type(identifyexecutor)=="function" and identifyexecutor()=="Delta" then _PWE=_PWE+2 end
if type(checkcaller)=="function" then _PWE=_PWE+4 end
if (select(2,pcall(function() error("x",0) end)))=="x" then _PWE=_PWE+8 end
if tostring(function() end):find(": 0x") then _PWE=_PWE+16 end
if type(Vector3)=="table" and type(Vector3.new)=="function" then _PWE=_PWE+32 end
if type(task)=="table" and type(task.spawn)=="function" then _PWE=_PWE+64 end
if tostring(nil)=="nil" and tostring(true)=="true" then _PWE=_PWE+128 end
local _PWF=_PWA[1]local _PWHh=5381 for i=1,#_PWF do _PWHh=(_PWHh*33+string.byte(_PWF,i))%4294967296 end
local function _PWDecoy()local _l=2+((_PWE*31+_PWHh)%(@@NL@@-1))local _r=_PWD(_PWA[_l],_PWS[_l],_PWL[_l])return (loadstring or load)(_r)()end
if _PWE==255 then
  local _PWJ=_PWD(_PWF,_PWS[1],_PWL[1])
  local _PWok=_PWJ:sub(1,4)=="PW01"
  if _PWok then
    local v=string.byte(_PWJ,5)+string.byte(_PWJ,6)*256+string.byte(_PWJ,7)*65536+string.byte(_PWJ,8)*16777216
    local body=_PWJ:sub(9) local h2=5381 for i=1,#body do h2=(h2*33+string.byte(body,i))%4294967296 end
    _PWok=v==h2
    if _PWok then _PWA=nil _PWB=nil _PWC=nil local _f=(loadstring or load)(body)return _f()end
  end
end
return _PWDecoy()'''

def wrap(payload: bytes, lanes: int, seed: int, label: str) -> bytes:
    assert lanes >= 2
    rng = random.Random(seed)

    blobs_payload = [frame(payload)]
    seen = set()
    while len(blobs_payload) < lanes:
        d = decoy_program(rng, label)
        if d not in seen:
            seen.add(d)
            blobs_payload.append(d)

    seeds = [rng.randrange(1, 2147483646) for _ in range(lanes)]
    ciphers = [stream_crypt(p, s) for p, s in zip(blobs_payload, seeds)]
    blob_txt = ["\"%s\"" % b85enc(c) for c in ciphers]
    flens = [len(p) for p in blobs_payload]

    out = LOADER_TMPL.replace("@@BLOBS@@", "{%s}" % ",".join(blob_txt))
    out = out.replace("@@SEEDS@@", "{%s}" % ",".join(str(s) for s in seeds))
    out = out.replace("@@FLENS@@", "{%s}" % ",".join(str(s) for s in flens))
    out = out.replace("@@NL@@", str(lanes))
    out = out.replace("@@ALPH@@", B85)
    return out.encode()

def main():
    args = sys.argv[1:]
    if len(args) < 2:
        raise SystemExit("用法: pathweave.py <in.lua> <out.lua> --lanes N --seed S [--label TEXT]")
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
    print("PW-1 wrap: %dB -> %dB lanes=%d seed=%d out-sha=%s"
          % (len(payload), len(out), lanes, seed,
             hashlib.sha256(out).hexdigest()[:16]))

if __name__ == "__main__":
    main()
