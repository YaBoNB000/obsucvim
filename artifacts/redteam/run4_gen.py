#!/usr/bin/env python3
"""run4: 让载荷真正跑起来——钩 print/错误, readu8 心跳限时, 观察被保护程序行为。"""
import sys

SRC = "sample_luraph15.lua"
OUT = "harness_run4.lua"
DEADLINE = sys.argv[1] if len(sys.argv) > 1 else "20"

src = open(SRC).read()
i = src.index("return setmetatable({")
head, rest = src[:i], src[i:]
rest = rest.replace("return setmetatable({", "local __T = setmetatable({", 1)
j = rest.rindex("},{}):FC()(...)")
body = rest[:j]
preamble = open("harness_run1.lua").read().split("--== 样本原文 ==")[0]

HOOKS = r"""
local __obuf = buffer
local __bufn = 0
local __bufs = {}
local __realbuffer = buffer
local buffer = setmetatable({}, { __index = __realbuffer })
local __oreadu8, __ocreate, __otostr = buffer.readu8, buffer.create, buffer.tostring
local __t0 = os.clock()
local __nreads = 0
buffer.readu8 = function(b, off)
  __nreads = __nreads + 1
  if __nreads % 262144 == 0 then
    if os.clock() - __t0 > DEADLINE then
      error("REDTEAM-DEADLINE reads=" .. __nreads, 0)
    end
  end
  return __oreadu8(b, off)
end
buffer.create = function(n)
  __bufn = __bufn + 1
  local r = __ocreate(n)
  __bufs[__bufn] = r
  return r
end
local __oprint = print
local print = function(...)
  local args = {...}
  local s = {}
  for k = 1, select("#", ...) do s[k] = tostring(args[k]) end
  __oprint("PROG-OUT> " .. table.concat(s, "\t"))
end
"""
HOOKS = HOOKS.replace("DEADLINE", DEADLINE)

INSTR = r"""
local fn = __T:FC()
print("FC ok")
local pok, perr = pcall(fn, ...)
print("payload ok=" .. tostring(pok) .. " err=" .. tostring(perr))
print("reads=" .. tostring(__nreads) .. " elapsed=" .. string.format("%.2f", os.clock() - __t0))
print("bufs_created=" .. tostring(__bufn))
for i = 1, math.min(__bufn, 200) do
  local b = __bufs[i]
  local ok, s = pcall(__otostr, b)
  if ok and #s > 0 and #s < 4096 then
    local hex = {}
    for j = 1, #s do hex[j] = string.format("%02x", string.byte(s, j)) end
    print("BUF " .. i .. " len=" .. #s .. " " .. table.concat(hex))
  elseif ok then
    print("BUF " .. i .. " len=" .. #s .. " (large)")
  end
end
return
"""
open(OUT, "w").write(head + preamble + HOOKS + body + "},{});\n" + INSTR)
print("wrote", OUT)
