#!/usr/bin/env python3
"""instrument_gen.py — 阶段 B 仪表化副本。
把样本头 `return setmetatable({` 改成捕获局部, 尾部 `},{}):FC()(...)`
拆开, 中间插仪表:
  - 按引用相等自动识别原语槽身份
  - 数字槽函数全包计数壳; buffer 族记录全部调用
  - pcall/xpcall/getfenv/setfenv/coroutine 族调用日志(实证运行时防御)
  - 进度心跳 + 最终日志落盘
"""
import re, sys

SRC = "sample_luraph15.lua"
OUT = sys.argv[1] if len(sys.argv) > 1 else "harness_run2.lua"

src = open(SRC).read()
assert src.startswith("-- This file")
# 定位第一处 "return setmetatable({"
i = src.index("return setmetatable({")
head, rest = src[:i], src[i:]
rest = rest.replace("return setmetatable({", "local __T = setmetatable({", 1)
# 尾部: ...},{}):FC()(...);
j = rest.rindex("},{}):FC()(...)")
body, tail = rest[:j], rest[j:]
assert tail.startswith("},{}):FC()(...)")

preamble = open("harness_run1.lua").read().split("--== 样本原文 ==")[0]

INSTR = r"""
--== 红队仪表 ==
local __log = {}
local __writes = {}   -- buffer 写日志: {bufid, off, val}
local __reads = {}
local __nwrites, __nreads = 0, 0
local __counts = {}
local __calllog = {}  -- 敏感原语调用记录
local __bufmeta = {}
local __bufn = 0
local __bufs = {}
local function __bufid(b)
  local s = __bufmeta[b]
  if not s then
    __bufn = __bufn + 1
    s = "B" .. tostring(__bufn)
    __bufmeta[b] = s
    __bufs[s] = b
  end
  return s
end
--(syntax fix applied)
local __known = {
  [buffer] = "buffer", [string] = "string", [table] = "table",
  [bit32] = "bit32", [coroutine] = "coroutine", [math] = "math",
}
local function __ident(v)
  for lib, name in pairs(__known) do
    for fn, f in pairs(lib) do
      if type(f) == "function" and f == v then return name .. "." .. tostring(fn) end
    end
  end
  if v == pcall then return "pcall" end
  if v == xpcall then return "xpcall" end
  if v == select then return "select" end
  if v == error then return "error" end
  return nil
end
local __idents = {}
for k, v in pairs(__T) do
  if type(k) == "number" and type(v) == "function" then
    __idents[k] = __ident(v) or ("fn#" .. tostring(k))
  end
end
local __orig = {}
for k, v in pairs(__T) do
  if type(k) == "number" and type(v) == "function" then
    __orig[k] = v
    local id = __idents[k]
    __T[k] = function(...)
      __counts[id] = (__counts[id] or 0) + 1
      if id == "buffer.writeu8" then
        local b, off, val = ...
        __nwrites = __nwrites + 1
        __writes[__nwrites] = __bufid(b) .. "," .. tostring(off) .. "," .. tostring(val)
        if __nwrites % 20000 == 0 then print("HB writes=" .. __nwrites) end
      elseif id == "buffer.readu8" then
        local b, off = ...
        __nreads = __nreads + 1
        if __nreads <= 40 or __nreads % 50000 == 0 then
          __reads[#__reads + 1] = __bufid(b) .. "," .. tostring(off)
        end
      elseif id == "buffer.create" then
        local r = v(...)
        print("MKBUF " .. __bufid(r) .. " size=" .. tostring((...)))
        return r
      elseif id == "buffer.fromstring" then
        local r = v(...)
        local s = (...)
        print("FSTRBUF " .. __bufid(r) .. " len=" .. tostring(#s))
        return r
      end
      if id == "pcall" or id == "xpcall" or id == "getfenv" or id == "setfenv"
         or (id:sub(1, 10) == "coroutine.") or id == "error" then
        __calllog[#__calllog + 1] = id .. " n=" .. tostring(__counts[id])
        if #__calllog <= 60 then print("SENS " .. id .. " call#" .. tostring(__counts[id])) end
      end
      return v(...)
    end
  end
end
-- 命名字段也包一层计数壳(只计数, 不记参), 找 FC 与高频 handler
for k, v in pairs(__T) do
  if type(k) == "string" and type(v) == "function" then
    __orig[k] = v
    __T[k] = function(...)
      __counts["@" .. k] = (__counts["@" .. k] or 0) + 1
      if __counts["@" .. k] == 1 then print("ENTER @" .. k) end
      return v(...)
    end
  end
end
print("IDENT MAP:")
local ks = {}
for k in pairs(__idents) do ks[#ks + 1] = k end
table.sort(ks)
for _, k in ipairs(ks) do print("  [" .. k .. "]=" .. __idents[k]) end
print("== calling FC ==")
local ok, fn = pcall(function() return __T:FC() end)
if not ok then
  print("FC FAILED: " .. tostring(fn))
else
  print("FC ok, fn=" .. tostring(fn) .. " calling payload...")
  local t0 = os and os.clock and os.clock() or 0
  local pok, perr = pcall(fn, ...)
  print("payload done ok=" .. tostring(pok) .. " err=" .. tostring(perr))
end
print("== COUNTS ==")
local ck = {}
for k in pairs(__counts) do ck[#ck + 1] = k end
table.sort(ck)
for _, k in ipairs(ck) do print(k .. " = " .. tostring(__counts[k])) end
print("writes=" .. __nwrites .. " reads=" .. __nreads)
-- 末态转储全部已跟踪 buffer（十六进制）
for i = 1, __bufn do
  local id = "B" .. i
  local b = __bufs[id]
  local ok, s = pcall(buffer.tostring, b)
  if ok then
    local hex = {}
    for j = 1, #s do hex[j] = string.format("%02x", string.byte(s, j)) end
    print("BUFDUMP " .. id .. " len=" .. #s)
    print(table.concat(hex))
  end
end
return
"""

out = head + preamble + body + "},{});\n" + INSTR
open(OUT, "w").write(out)
print(f"wrote {OUT} ({len(out)} bytes)")
