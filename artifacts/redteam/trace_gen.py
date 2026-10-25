#!/usr/bin/env python3
"""trace_gen.py — 源码补丁插桩: 在 [18] 的四个分派循环头插 __trace,
载荷按 buffer.create 预算炸停后转储执行轨迹 + 届时数组状态。
大发现记录: 四入口取操作码的数组不同! 251/99 从 J, 8 从 y, 140 从 R。
"""
import sys, re

SRC = "sample_luraph15.lua"
OUT = "harness_trace.lua"
BUDGET = int(sys.argv[1]) if len(sys.argv) > 1 else 400
CAP = int(sys.argv[2]) if len(sys.argv) > 2 else 400000
TR2_CAP = int(sys.argv[3]) if len(sys.argv) > 3 else 20000

src = open(SRC).read()
i = src.index("return setmetatable({")
head, rest = src[:i], src[i:]
rest = rest.replace("return setmetatable({", "local __T = setmetatable({", 1)
j = rest.rindex("},{}):FC()(...)")
body = rest[:j]

preamble = open("harness_run1.lua").read().split("--== 样本原文 ==")[0]
preamble += r"""
--== trace 仪表 ==
local __tr_n = 0
local __tr = {}
local __tr_full = false
local __tr_counts = {}
local __tr2_n = 0
local __tr2 = {}
local function __trace(e, Q, opc)
  __tr_n = __tr_n + 1
  local c = e * 1000 + opc
  __tr_counts[c] = (__tr_counts[c] or 0) + 1
  if __tr_n <= TR_CAP then
    __tr[__tr_n] = e * 16777216 + Q * 256 + (opc % 256)
  elseif not __tr_full then
    __tr_full = true
  end
end
-- 带操作数的全量轨迹(分开记, 供数据流分析)
local function __trace2(e, Q, opc, R, t, m, y, A, J)
  __tr2_n = __tr2_n + 1
  if __tr2_n <= TR2_CAP then
    local base = (__tr2_n - 1) * 8
    __tr2[base + 1] = e
    __tr2[base + 2] = Q
    __tr2[base + 3] = opc
    __tr2[base + 4] = R
    __tr2[base + 5] = t
    __tr2[base + 6] = m
    __tr2[base + 7] = y
    __tr2[base + 8] = A
  end
end
local function __safe_trace2(e,Q,opc)
  local ok,er=pcall(function()
    __trace2(e,Q,opc,R[Q],t[Q],m[Q],y[Q],A[Q],J[Q])
  end)
  if not ok and __tr2_n<5 then print("SAFE-TRACE-ERR at e="..e.." Q="..Q.." opc="..opc.." err="..tostring(er).." tb="..debug.traceback("",2)) end
end
local __mk_n = 0
local __realcreate0 = buffer.create
local buffer = setmetatable({}, { __index = buffer })
buffer.create = function(n)
  __mk_n = __mk_n + 1
  if __mk_n % 50 == 0 then print("MKBUF#" .. __mk_n) end
  if __mk_n > CREATE_BUDGET then
    error("REDTEAM-BUDGET-STOP", 0)
  end
  return __realcreate0(n)
end
""".replace("TR_CAP", str(CAP)).replace("TR2_CAP", str(TR2_CAP)).replace("CREATE_BUDGET", str(BUDGET))

# 四处循环头插桩
# 四处循环头插桩(含操作数全量记录; 各入口取码数组不同)
pats = [
    ("if S==251 then while true do local O=J[Q];",
     "if S==251 then while true do local O=J[Q];__trace(251,Q,O);__trace2(251,Q,O,R[Q],t[Q],m[Q],y[Q],A[Q],J[Q]);"),
    ("if S==99 then while true do local q=J[Q];",
     "if S==99 then while true do local q=J[Q];__trace(99,Q,q);__trace2(99,Q,q,R[Q],t[Q],m[Q],y[Q],A[Q],J[Q]);"),
    ("if S==8 then while true do local q=y[Q];",
     "if S==8 then while true do local q=y[Q];__trace(8,Q,q);__trace2(8,Q,q,R[Q],t[Q],m[Q],y[Q],A[Q],J[Q]);"),
    ("if S==140 then while true do local q=R[Q];",
     "if S==140 then while true do local q=R[Q];__trace(140,Q,q);__trace2(140,Q,q,R[Q],t[Q],m[Q],y[Q],A[Q],J[Q]);"),
    ("while true do local f=m[x];",
     "while true do local f=m[x];__trace(73,x,f);"),
]
for old, new in pats:
    assert body.count(old) == 1, old
    body = body.replace(old, new)

INSTR = r"""
--== 原型捕获 + 运行 + 转储 ==
local __protos = {}
local __np = 0
local orig18 = __T[18]
__T[18] = function(...)
  __np = __np + 1
  local u = select(2, ...)
  if type(u) == "table" then __protos[__np] = u end
  return orig18(...)
end
local fn = __T:FC()
print("FC ok; running payload (budget=" .. BUDGET .. ", cap=" .. CAP .. ")")
local function __errh(e) return debug.traceback(tostring(e),2) end
local pok, perr = xpcall(fn, __errh)
print("payload ok=" .. tostring(pok) .. " err=" .. tostring(perr) .. " creates=" .. __mk_n .. " traced=" .. __tr_n)
print("TRACE-COUNTS")
local ck = {}
for k in pairs(__tr_counts) do ck[#ck + 1] = k end
table.sort(ck)
for _, k in ipairs(ck) do print("TC " .. k .. " " .. __tr_counts[k]) end
print("TRACE-DUMP")
for s = 1, math.min(__tr_n, CAP), 400 do
  local parts = {}
  for k = s, math.min(s + 399, __tr_n, CAP) do parts[#parts + 1] = string.format("%d", __tr[k]) end
  print("TD " .. table.concat(parts, ","))
end
print("TRACE-END full=" .. tostring(__tr_full))
print("TRACE2-DUMP")
for s = 1, math.min(__tr2_n, TR2CAP) * 8, 400 do
  local parts = {}
  for k = s, math.min(s + 399, math.min(__tr2_n, TR2CAP) * 8) do parts[#parts + 1] = tostring(__tr2[k]) end
  print("T2 " .. table.concat(parts, ","))
end
print("TRACE2-END n=" .. tostring(__tr2_n))
for pn, u in ipairs(__protos) do
  if pn <= 2 then
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
end
return
"""
INSTR = INSTR.replace("BUDGET", str(BUDGET)).replace("CAP", str(CAP)).replace("TR2CAP", str(TR2_CAP))
open(OUT, "w").write(head + preamble + body + "},{});\n" + INSTR)
print("wrote", OUT)
