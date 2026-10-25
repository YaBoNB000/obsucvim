#!/usr/bin/env python3
"""run8: 全量提取原型数组(离线反汇编用) + 钩 VM 环境里的 print。"""
import sys

SRC = "sample_luraph15.lua"
OUT = "harness_run8.lua"

src = open(SRC).read()
i = src.index("return setmetatable({")
head, rest = src[:i], src[i:]
rest = rest.replace("return setmetatable({", "local __T = setmetatable({", 1)
j = rest.rindex("},{}):FC()(...)")
body = rest[:j]
preamble = open("harness_run1.lua").read().split("--== 样本原文 ==")[0]

INSTR = r"""
--== run8 ==
local __proton = 0
local function __dumparr(t, tag)
  if type(t) ~= "table" then print("ARR " .. tag .. " NOTTABLE") return end
  local n = 0
  for _ in pairs(t) do n = n + 1 end
  local parts = {}
  for k = 1, n do parts[k] = tostring(t[k]) end
  -- 分块打印防单行过长
  local CH = 500
  for s = 1, n, CH do
    print("ARR " .. tag .. " " .. table.concat(parts, ",", s, math.min(s + CH - 1, n)))
  end
  print("ARRLEN " .. tag .. " " .. n)
end
local function __dumpenv(v, path, depth)
  if depth > 5 then return end
  if type(v) ~= "table" then return end
  for k, vv in pairs(v) do
    if vv == print then
      print("HOOKPRINT at " .. path .. "[" .. tostring(k) .. "]")
      v[k] = function(...)
        local args = {...}
        local s = {}
        for a = 1, select("#", ...) do s[a] = tostring(args[a]) end
        print("PROG-OUT> " .. table.concat(s, "\t"))
      end
    elseif type(vv) == "table" then
      __dumpenv(vv, path .. "[" .. tostring(k) .. "]", depth + 1)
    end
  end
end
local orig18 = __T[18]
__T[18] = function(...)
  __proton = __proton + 1
  local pn = __proton
  if pn <= 4 then
    local u = select(2, ...)
    if type(u) == "table" then
      print("PROTO#" .. pn .. " meta: [1]=" .. tostring(u[1]) .. " [3]=" .. tostring(u[3]) .. " [4]=" .. tostring(u[4]) .. " [5]=" .. tostring(u[5]) .. " [7]=" .. tostring(u[7]) .. " [12]=" .. tostring(u[12]))
      local fields = { [5] = "u5", [7] = "u7", [8] = "X", [9] = "x", [10] = "A", [11] = "R", [12] = "a", [13] = "t", [14] = "y", [15] = "m", [16] = "J" }
      for idx, name in pairs(fields) do
        local v = u[idx]
        if type(v) == "table" then
          local first = v[0] ~= nil and v[0] or v
          if type(first) == "table" and type(v[0]) == "table" then
            __dumparr(v[0], "P" .. pn .. "." .. name .. "[0]")
          else
            __dumparr(v, "P" .. pn .. "." .. name)
          end
        else
          print("ARR P" .. pn .. "." .. name .. " = " .. tostring(v))
        end
      end
      -- u[2] 环境与常量
      local two = u[2]
      if type(two) == "table" then
        __dumpenv(two, "P" .. pn .. ".env", 0)
        local four = two[4]
        if type(four) == "table" then
          local n4 = 0
          for _ in pairs(four) do n4 = n4 + 1 end
          print("P" .. pn .. ".env[4] size=" .. n4)
          for k, vv in pairs(four) do
            local tv = type(vv)
            if tv == "string" then
              print("ENV4 P" .. pn .. " [" .. tostring(k) .. "] str#" .. #vv .. " :: " .. vv:sub(1, 100):gsub("\n", "\\n"))
            elseif tv == "number" or tv == "boolean" then
              print("ENV4 P" .. pn .. " [" .. tostring(k) .. "] " .. tv .. " " .. tostring(vv))
            elseif tv == "table" then
              local nn = 0
              for _ in pairs(vv) do nn = nn + 1 end
              print("ENV4 P" .. pn .. " [" .. tostring(k) .. "] table(" .. nn .. ")")
            else
              print("ENV4 P" .. pn .. " [" .. tostring(k) .. "] " .. tv)
            end
          end
        end
      end
      local three = u[3]
      print("P" .. pn .. " u3=" .. tostring(three))
      if type(three) == "table" then
        for k, vv in pairs(three) do print("U3 P" .. pn .. " [" .. tostring(k) .. "]=" .. tostring(vv)) end
      end
    end
  end
  return orig18(...)
end
local fn = __T:FC()
print("FC ok, running payload...")
fn()
print("payload returned")
return
"""
open(OUT, "w").write(head + preamble + body + "},{});\n" + INSTR)
print("wrote", OUT)
