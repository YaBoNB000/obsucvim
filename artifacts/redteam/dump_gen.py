#!/usr/bin/env python3
"""dump_gen.py — run3 转储型仪表: 只跟踪 buffer 创建, 零热路径开销。
FC() 返回后立即转储全部 buffer 十六进制(解码产物到手), 不调用载荷。
第二模式 --with-payload: FC 后转储一次, 再限时调用载荷, 结束后二次转储。
"""
import sys

SRC = "sample_luraph15.lua"
mode_payload = "--with-payload" in sys.argv
OUT = "harness_run3.lua"

src = open(SRC).read()
i = src.index("return setmetatable({")
head, rest = src[:i], src[i:]
rest = rest.replace("return setmetatable({", "local __T = setmetatable({", 1)
j = rest.rindex("},{}):FC()(...)")
body = rest[:j]

preamble = open("harness_run1.lua").read().split("--== 样本原文 ==")[0]
# buffer 包装必须作为 local 放在样本之前才能遮蔽模块表构造期的引用
BUFWRAP = r"""
local __bufmeta = {}
local __bufn = 0
local __bufs = {}
local function __bufid(b)
  local s = __bufmeta[b]
  if not s then
    __bufn = __bufn + 1
    s = "B" .. tostring(__bufn)
    __bufmeta[b] = s
    __bufs[__bufn] = b
  end
  return s
end
local __realbuffer = buffer
local __ocreate, __ofstr, __otostr = buffer.create, buffer.fromstring, buffer.tostring
local buffer = setmetatable({}, { __index = __realbuffer })
buffer.create = function(n)
  local r = __ocreate(n)
  print("MKBUF " .. __bufid(r) .. " size=" .. tostring(n))
  return r
end
buffer.fromstring = function(s)
  local r = __ofstr(s)
  print("FSTRBUF " .. __bufid(r) .. " len=" .. #s)
  return r
end
"""
preamble = preamble + BUFWRAP

INSTR = r"""
--== run3 转储仪表 ==
local function dumpall(tag)
  for i = 1, __bufn do
    local b = __bufs[i]
    local ok, s = pcall(__otostr, b)
    if ok then
      local hex = {}
      for j = 1, #s do hex[j] = string.format("%02x", string.byte(s, j)) end
      print("BUFDUMP " .. tag .. " B" .. i .. " len=" .. #s)
      print(table.concat(hex))
    end
  end
end
print("== FC ==")
local fn = __T:FC()
print("FC ok fn=" .. tostring(fn))
dumpall("T0")
if PAYLOAD then
  print("== payload ==")
  local pok, perr = pcall(fn, ...)
  print("payload ok=" .. tostring(pok) .. " err=" .. tostring(perr))
  dumpall("T1")
end
return
"""
INSTR = INSTR.replace("PAYLOAD", "true" if mode_payload else "false")
out = head + preamble + body + "},{});\n" + INSTR
open(OUT, "w").write(out)
print(f"wrote {OUT} ({len(out)} bytes)")
