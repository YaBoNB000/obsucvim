#!/usr/bin/env python3
"""run9: 让载荷跑一段时间(以 buffer.create 次数为预算), 中途炸停, 转储
届时已解密的指令流/操作数数组 —— 观察块链解密推进范围。"""
import sys

SRC = "sample_luraph15.lua"
OUT = "harness_run9.lua"
BUDGET = sys.argv[1] if len(sys.argv) > 1 else "500"

src = open(SRC).read()
i = src.index("return setmetatable({")
head, rest = src[:i], src[i:]
rest = rest.replace("return setmetatable({", "local __T = setmetatable({", 1)
j = rest.rindex("},{}):FC()(...)")
body = rest[:j]
preamble = open("harness_run1.lua").read().split("--== 样本原文 ==")[0]
preamble += r"""
local __nc = 0
local __realcreate0 = buffer.create
local buffer = setmetatable({}, { __index = buffer })
buffer.create = function(n)
  __nc = __nc + 1
  if __nc % 100 == 0 then print("MKBUF#" .. __nc .. " size=" .. tostring(n)) end
  if __nc > CREATEBUDGET then
    error("REDTEAM-BUDGET-STOP", 0)
  end
  return __realcreate0(n)
end
""".replace("CREATEBUDGET", BUDGET)

INSTR = r"""
--== run9 ==
local __protos = {}
local __np = 0
local function __hookprints(v, path, depth)
  if depth > 6 or type(v) ~= "table" then return end
  for k, vv in pairs(v) do
    if vv == print then
      v[k] = function(...)
        local s = {}
        for a = 1, select("#", ...) do s[a] = tostring((select(a, ...))) end
        print("PROG-OUT> " .. table.concat(s, "\t"))
      end
      print("HOOKPRINT at " .. path .. "[" .. tostring(k) .. "]")
    elseif type(vv) == "table" then
      __hookprints(vv, path .. "[" .. tostring(k) .. "]", depth + 1)
    end
  end
end
local orig18 = __T[18]
__T[18] = function(...)
  __np = __np + 1
  local u = select(2, ...)
  if type(u) == "table" then
    __protos[__np] = u
    for a = 1, select("#", ...) do
      __hookprints(select(a, ...), "arg" .. a, 0)
    end
  end
  return orig18(...)
end
local fn = __T:FC()
print("FC ok, running payload")
local pok, perr = pcall(fn)
print("payload ok=" .. tostring(pok) .. " err=" .. tostring(perr) .. " creates=" .. __nc)
for pn, u in ipairs(__protos) do
  print("POST P" .. pn .. " [7]=" .. tostring(u[7]))
  local J = u[16]
  if type(J) == "table" then
    local n = 0
    for _ in pairs(J) do n = n + 1 end
    local parts = {}
    for k = 1, n do parts[k] = tostring(J[k]) end
    for s = 1, n, 400 do
      print("POSTJ P" .. pn .. " " .. table.concat(parts, ",", s, math.min(s + 399, n)))
    end
  end
end
return
"""
INSTR = INSTR.replace("BUDGET", BUDGET)
open(OUT, "w").write(head + preamble + body + "},{});\n" + INSTR)
print("wrote", OUT)
