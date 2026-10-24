-- v19-check 诊断件（明文，不混淆）：把 EVK 探针的每个中间量打印出来。
-- 用法：直接在真 Delta 跑，把输出原样发回。
-- 判据：k2a=1143546169, k2b=1317214663, k3c=22 ⇒ EVK=4576808。
local function f31(s) local h = 7 for i = 1, #s do h = (h * 31 + s:byte(i)) % 2147483647 end return h end
local env0 = getfenv and getfenv(0)
local env1 = getfenv and getfenv(1)
local gg = getgenv and getgenv()
local function has(t, n) return type(t) == "table" and t[n] ~= nil and 1 or 0 end
print("[src] Vector3:", has(env0,"Vector3"), has(env1,"Vector3"), has(gg,"Vector3"),
  "| typeof:", has(env0,"typeof"), has(env1,"typeof"), has(gg,"typeof"),
  "| game:", has(env0,"game"), has(env1,"game"), has(gg,"game"),
  "| checkcaller:", has(env0,"checkcaller"), has(env1,"checkcaller"), has(gg,"checkcaller"),
  "| identifyexecutor:", has(env0,"identifyexecutor"), has(env1,"identifyexecutor"), has(gg,"identifyexecutor"))
local function lk(n)
  if gg and gg[n] ~= nil then return gg[n] end
  if env0 and env0[n] ~= nil then return env0[n] end
  if env1 and env1[n] ~= nil then return env1[n] end
  return nil
end
local vn = lk("Vector3")
if type(vn) ~= "table" then print("[fail] Vector3 missing everywhere") return end
local v = vn.new(1, 2, 3)
local k2a = f31(tostring(v))
print("[k2a] tostring=", tostring(v), "fold=", k2a, k2a == 1143546169 and "OK" or "MISMATCH")
local tf = lk("typeof")
local k2b = tf and f31(tf(v)) or -1
print("[k2b] typeof=", tf and tf(v) or "?", "fold=", k2b, k2b == 1317214663 and "OK" or "MISMATCH")
local bs, bc, nl = 0, 0, 0
local gm = lk("game")
if gm then
  local ok, rs = pcall(function() return gm:GetService("RunService") end)
  if ok and rs then local ok2, isv = pcall(function() return rs:IsStudio() end) bs = (ok2 and isv == true) and 1 or 0 end
end
local ckf = lk("checkcaller")
if type(ckf) == "function" then local ok, r = pcall(ckf) bc = (ok and r == true) and 1 or 0 end
local idf = lk("identifyexecutor")
if type(idf) == "function" then local ok, n1 = pcall(idf) if ok and type(n1) == "string" then nl = #n1 print("[name]", n1) end end
local k3c = bs + bc * 2 + nl * 4
print("[k3c] isStudio=", bs, "checkcaller=", bc, "namelen=", nl, "fold=", k3c, k3c == 22 and "OK" or "MISMATCH")
local evk = ((k2a * 31 + k2b) % 268435456 * 31 + k3c) % 268435456
print("[EVK]", evk, evk == 4576808 and "MATCH-4576808" or "MISMATCH expect 4576808")
