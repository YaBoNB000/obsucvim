#!/usr/bin/env python3
"""run5: 包工厂函数 [18]/[73], 在闭包创建时刻深转储原型状态表;
返回的 VM 闭包再包一层调用日志; 热路径零包装, 载荷原生速度跑。"""
import sys

SRC = "sample_luraph15.lua"
OUT = "harness_run5.lua"

src = open(SRC).read()
i = src.index("return setmetatable({")
head, rest = src[:i], src[i:]
rest = rest.replace("return setmetatable({", "local __T = setmetatable({", 1)
j = rest.rindex("},{}):FC()(...)")
body = rest[:j]
preamble = open("harness_run1.lua").read().split("--== 样本原文 ==")[0]

INSTR = r"""
--== run5 仪表 ==
local function __dump(v, depth, seen, path)
  depth = depth or 0
  seen = seen or {}
  path = path or "u"
  local t = type(v)
  if t == "table" then
    if seen[v] or depth >= 3 then
      print("DUMP " .. path .. " = <table" .. (seen[v] and " cycle" or " deep") .. ">")
      return
    end
    seen[v] = true
    local n = 0
    for k in pairs(v) do n = n + 1 end
    print("DUMP " .. path .. " = table(" .. n .. ")")
    if depth < 3 then
      local ks = {}
      for k in pairs(v) do ks[#ks + 1] = k end
      table.sort(ks, function(a, b) return tostring(a) < tostring(b) end)
      for _, k in ipairs(ks) do
        local vv = v[k]
        local tv = type(vv)
        if tv == "number" or tv == "boolean" then
          print("DUMP   [" .. tostring(k) .. "] " .. tv .. " " .. tostring(vv))
        elseif tv == "string" then
          print("DUMP   [" .. tostring(k) .. "] string #" .. #vv .. " " .. string.format("%q", vv):sub(1, 120))
        elseif tv == "table" then
          __dump(vv, depth + 1, seen, path .. "[" .. tostring(k) .. "]")
        elseif tv == "function" then
          print("DUMP   [" .. tostring(k) .. "] function")
        else
          print("DUMP   [" .. tostring(k) .. "] " .. tv)
        end
      end
    end
  else
    print("DUMP " .. path .. " = " .. tostring(v))
  end
end
local __vmcalls = 0
local function wrapfactory(key)
  local orig = __T[key]
  __T[key] = function(...)
    local args = table.pack(...)
    print("FACTORY[" .. key .. "] called, " .. args.n .. " args")
    for a = 1, args.n do
      __dump(args[a], 0, nil, "arg" .. a)
    end
    local r = orig(...)
    if type(r) == "function" then
      local rr = r
      r = function(...)
        __vmcalls = __vmcalls + 1
        print("VM-CLOSURE#" .. __vmcalls .. " invoked (factory " .. key .. ") nargs=" .. tostring(select("#", ...)))
        local res = table.pack(rr(...))
        print("VM-CLOSURE#" .. __vmcalls .. " returned " .. res.n .. " vals")
        return table.unpack(res, 1, res.n)
      end
    else
      print("FACTORY[" .. key .. "] returned " .. type(r) .. " count=" .. tostring((function(...) return select("#", ...) end)(r)))
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
return
"""
open(OUT, "w").write(head + preamble + body + "},{});\n" + INSTR)
print("wrote", OUT)
