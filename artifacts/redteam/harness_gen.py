#!/usr/bin/env python3
"""harness_gen.py — 生成「局部遮蔽」桩环境副本（阶段 A）。
用法: python3 harness_gen.py [--names Vector3,Vector2,...] [--shadow-getfenv]
读 sample_luraph15.lua，产出 harness_runN.lua。
原则: 不碰 _G（CLI 只读），在文件顶端声明 local 遮蔽整个 chunk。
"""
import sys, re

SRC = "sample_luraph15.lua"

STUB_PREAMBLE = r"""--== 红队桩环境（局部遮蔽，不动 _G）==
local __STUB
__STUB = function(name)
  local s = {}
  local mt = {}
  mt.__index = function(t, k)
    local v = __STUB((name or "?") .. "." .. tostring(k))
    rawset(t, k, v)
    return v
  end
  mt.__call = function() return __STUB((name or "?") .. "()") end
  mt.__add = function(a, b) return __STUB("add") end
  mt.__sub = function(a, b) return __STUB("sub") end
  mt.__mul = function(a, b) return __STUB("mul") end
  mt.__div = function(a, b) return __STUB("div") end
  mt.__mod = function(a, b) return __STUB("mod") end
  mt.__unm = function(a) return __STUB("unm") end
  mt.__lt = function() return false end
  mt.__le = function() return false end
  mt.__eq = function(a, b) return rawequal(a, b) end
  mt.__len = function() return 0 end
  mt.__concat = function(a, b) return "" end
  mt.__tostring = function() return name or "stub" end
  return setmetatable(s, mt)
end
local Vector3 = __STUB("Vector3")
local Vector2 = __STUB("Vector2")
local vector = __STUB("vector")
local task = __STUB("task")
local game = __STUB("game")
local workspace = __STUB("workspace")
local Instance = __STUB("Instance")
local Enum = __STUB("Enum")
local Color3 = __STUB("Color3")
local UDim2 = __STUB("UDim2")
local CFrame = __STUB("CFrame")
local script = __STUB("script")
local shared = {}
local __real_getfenv = getfenv
local __realG = __real_getfenv(0)
-- 桩环境: 真标准库兜底 + 未知键自动生长为桩（运行期拼名索引有处可落）
local __stubenv = setmetatable({}, { __index = function(t, k)
  local v = __realG[k]
  if v == nil then v = __STUB("env." .. tostring(k)) end
  rawset(t, k, v)
  return v
end })
local getfenv = function(level) return __stubenv end
local setfenv = function(f, env) return f end
--== 样本原文 ==
"""

def main():
    names = None
    out = "harness_run1.lua"
    shadow_getfenv = True
    args = sys.argv[1:]
    while args:
        a = args.pop(0)
        if a == "--names":
            names = args.pop(0).split(",")
        elif a == "--out":
            out = args.pop(0)
        elif a == "--no-getfenv":
            shadow_getfenv = False
    src = open(SRC).read()
    preamble = STUB_PREAMBLE
    if not shadow_getfenv:
        preamble = "\n".join(l for l in preamble.split("\n")
                             if not (l.startswith("local getfenv") or l.startswith("local setfenv") or l.strip().startswith("local env") or l.strip().startswith("return env")))
    open(out, "w").write(preamble + src)
    print(f"wrote {out} ({len(preamble)+len(src)} bytes)")

if __name__ == "__main__":
    main()
