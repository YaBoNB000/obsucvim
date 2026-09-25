#!/usr/bin/env python3
"""run7: 提取被保护程序的常量池字符串 + 原型统计, 限时运行观察行为。"""
import sys
SRC="sample_luraph15.lua"; OUT="harness_run7.lua"
src=open(SRC).read()
i=src.index("return setmetatable({"); head,rest=src[:i],src[i:]
rest=rest.replace("return setmetatable({","local __T = setmetatable({",1)
j=rest.rindex("},{}):FC()(...)"); body=rest[:j]
preamble=open("harness_run1.lua").read().split("--== 样本原文 ==")[0]
INSTR=r"""
local __seen={}
local function __strings(v,path,depth)
  depth=depth or 0
  if depth>4 then return end
  local tv=type(v)
  if tv=="table" then
    if __seen[v] then return end
    __seen[v]=true
    local n=0
    for _ in pairs(v) do n=n+1 end
    if n<=400 then
      for k,vv in pairs(v) do
        if type(vv)=="string" then
          print("CONST " .. path .. "[" .. tostring(k) .. "] str#" .. #vv .. " :: " .. vv:sub(1,80):gsub("[\n\r]","\\n"))
        elseif type(vv)=="number" then
          -- skip bulk numbers
        elseif type(vv)=="table" then
          __strings(vv, path .. "[" .. tostring(k) .. "]", depth+1)
        end
      end
    else
      print("CONST " .. path .. " BIGTABLE(" .. n .. ")")
    end
  end
end
local __fc=0
local orig18=__T[18]
__T[18]=function(...)
  __fc=__fc+1
  if __fc==1 then
    print("PROTO#1 arg scan")
    for a=2,select("#",...) do
      local v=select(a,...)
      if type(v)=="table" then __strings(v,"a"..a) end
    end
  end
  return orig18(...)
end
local fn=__T:FC()
print("FC ok")
-- 限时跑载荷, 用协程+时钟无法抢占, 改为直接 pcall 并外部 timeout
local co=coroutine.create(function() return fn() end)
local t0=os.clock()
while true do
  local ok,res=coroutine.resume(co)
  if not ok then print("ERR "..tostring(res)) break end
  if coroutine.status(co)=="dead" then print("DONE ret="..tostring(res)) break end
  if os.clock()-t0>15 then print("BUDGET-OUT elapsed="..string.format("%.1f",os.clock()-t0)) break end
end
return
"""
open(OUT,"w").write(head+preamble+body+"},{});\n"+INSTR)
print("wrote",OUT)
