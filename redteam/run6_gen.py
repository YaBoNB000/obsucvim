#!/usr/bin/env python3
"""run6: 精简仪表——结构摘要(大数组只报规模), 观察被保护程序真实行为。"""
import sys

SRC = "sample_luraph15.lua"
OUT = "harness_run6.lua"

src = open(SRC).read()
i = src.index("return setmetatable({")
head, rest = src[:i], src[i:]
rest = rest.replace("return setmetatable({", "local __T = setmetatable({", 1)
j = rest.rindex("},{}):FC()(...)")
body = rest[:j]
preamble = open("harness_run1.lua").read().split("--== 样本原文 ==")[0]

INSTR = r"""
--== run6 ==
local function __size(t)
  local n = 0
  for _ in pairs(t) do n = n + 1 end
  return n
end
local function __brief(v, path)
  local tv = type(v)
  if tv == "table" then
    local n = __size(v)
    if n > 50 then print("BRIEF " .. path .. " table(" .. n .. ")") return end
    for k, vv in pairs(v) do
      local tk = type(vv)
      if tk == "number" or tk == "boolean" then
        print("BRIEF " .. path .. "[" .. tostring(k) .. "]=" .. tostring(vv))
      elseif tk == "string" then
        print("BRIEF " .. path .. "[" .. tostring(k) .. "]=str#" .. #vv .. " " .. vv:sub(1, 60):gsub("\n", "\\n"))
      elseif tk == "table" then
        __brief(vv, path .. "[" .. tostring(k) .. "]")
      else
        print("BRIEF " .. path .. "[" .. tostring(k) .. "]=" .. tk)
      end
    end
  end
end
local __fcalls = 0
local function wrapfactory(key)
  local orig = __T[key]
  __T[key] = function(...)
    __fcalls = __fcalls + 1
    local n = select("#", ...)
    if __fcalls <= 3 or __fcalls % 50 == 0 then
      print("FACTORY[" .. key .. "]#" .. __fcalls .. " args=" .. n)
      for a = 2, n do
        local v = select(a, ...)
        if type(v) == "table" then __brief(v, "arg" .. a) end
      end
    end
    local r = orig(...)
    if type(r) == "function" and __fcalls <= 30 then
      local rr, id = r, __fcalls
      r = function(...)
        local t0 = os.clock()
        print("VM#" .. id .. " call nargs=" .. tostring(select("#", ...)))
        local res = table.pack(rr(...))
        print("VM#" .. id .. " ret " .. res.n .. " in " .. string.format("%.3fs", os.clock() - t0))
        return table.unpack(res, 1, res.n)
      end
    end
    return r
  end
end
wrapfactory(18)
wrapfactory(73)
local __oprint = print
local print = function(...)
  local args = {...}
  local s = {}
  for k = 1, select("#", ...) do s[k] = tostring(args[k]) end
  __oprint("PROG-OUT> " .. table.concat(s, "\t"))
end
local fn = __T:FC()
__oprint("FC ok")
local pok, perr = pcall(fn, ...)
__oprint("payload ok=" .. tostring(pok) .. " err=" .. tostring(perr))
__oprint("factory_calls=" .. tostring(__fcalls))
return
"""
open(OUT, "w").write(head + preamble + body + "},{});\n" + INSTR)
print("wrote", OUT)
