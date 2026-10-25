--[[ ==========================================================================
  Obsucvim Doctor — 一条命令采集执行器全貌（不会报错，不会改任何东西）
  --------------------------------------------------------------------------
  在 Delta（或任何执行器）里整段粘贴运行。它会：
    1. 逐项探测执行器 API、Roblox 语义、Luau 运行时
    2. 打印一份可读报告
    3. 自动把整份报告复制到剪贴板（并明确告诉你复制成功与否）

  所有探测都在 pcall 里，缺什么记什么，绝不中断。
  输出以 [OBS] 开头，最后一行是 RESULT: 单行摘要（方便一次性粘贴）。
========================================================================== ]]

local R = {}
local function add(fmt, ...)
	local ok, line = pcall(string.format, fmt, ...)
	R[#R + 1] = ok and line or ("[format error] " .. tostring(fmt))
end

-- 安全探测：返回 ok, 结果
local function pc(fn, ...)
	local ok, a, b, c = pcall(fn, ...)
	if ok then return true, a, b, c end
	return false, a
end

local function typeOf(v)
	local ok, k = pcall(function()
		if type(typeof) == "function" then return typeof(v) end
		return type(v)
	end)
	if ok and type(k) == "string" then return k end
	return type(v)
end

-- 执行器环境：先 getgenv()，再回退 _G（这正是 R067.2 修的地方）
local ENV = nil
do
	local ok, g = pcall(function()
		if type(getgenv) == "function" then return getgenv() end
		return nil
	end)
	if ok and type(g) == "table" then ENV = g end
	if ENV == nil then ENV = _G end
end

-- 双环境查表：先执行器环境，再 _G；记录命中位置
local function lookup(name)
	local v = nil
	local where = "-"
	pcall(function() v = ENV[name] end)
	if v ~= nil then where = "genv" end
	if v == nil then
		pcall(function() v = _G[name] end)
		if v ~= nil then where = "_G" end
	end
	return v, where
end

add("==== Obsucvim Doctor ====")
add("time: %s", tostring(os and os.time and os.time() or "n/a"))
local genvEqG = "n/a"
do
	pcall(function()
		if type(getgenv) == "function" then genvEqG = tostring(getgenv() == _G) end
	end)
	add("getgenv()==_G : %s   (false = 执行器把 API 放在自己的环境里)", genvEqG)
end
local execName, execVer = "(no identifyexecutor)", "-"
do
	local fn = nil
	pcall(function() fn = ENV.identifyexecutor end)
	if type(fn) ~= "function" then pcall(function() fn = _G.identifyexecutor end) end
	if type(fn) == "function" then
		local ok, name, ver = pcall(fn)
		if ok then
			execName = tostring(name)
			execVer = tostring(ver)
			-- 一致性：连调两次必须一样（exguard 的第 5 号闸门）
			local ok2, name2 = pcall(fn)
			add("identifyexecutor x2: %s / %s   (期望完全一致)",
				tostring(name), ok2 and tostring(name2) or ("ERR " .. tostring(name2)))
		end
	end
	add("executor     : %s   version=%s", execName, execVer)
end
do
	local fn = nil
	pcall(function() fn = ENV.checkcaller end)
	if type(fn) ~= "function" then pcall(function() fn = _G.checkcaller end) end
	if type(fn) == "function" then
		local ok, v = pcall(fn)
		add("checkcaller() : %s   (期望 true)", ok and tostring(v) or ("ERR " .. tostring(v)))
	else
		add("checkcaller() : 不存在")
	end
end
do
	local fn = nil
	pcall(function() fn = ENV.getfenv end)
	if type(fn) ~= "function" then pcall(function() fn = _G.getfenv end) end
	if type(fn) == "function" then
		local ok, env0 = pcall(fn, 0)
		local sameAsGenv = false
		pcall(function()
			if ok and type(getgenv) == "function" then sameAsGenv = (env0 == getgenv()) end
		end)
		add("getfenv(0)    : ok=%s type=%s sameAsGenv=%s", tostring(ok), type(env0), tostring(sameAsGenv))
	else
		add("getfenv       : 不存在")
	end
end
do
	local writable = "n/a"
	pcall(function()
		local before = rawget(_G, "__obs_probe")
		rawset(_G, "__obs_probe", 1)
		writable = tostring(rawget(_G, "__obs_probe") == 1)
		rawset(_G, "__obs_probe", before)
	end)
	add("_G 可写       : %s   (false = 沙箱只读，tier-1 会命中这一点)", writable)
end
add("")

----------------------------------------------------------------------
add("---- 1. 执行器 API（层级：genv / _G）----")
local apiList = {
	"getgenv", "gethwid", "identifyexecutor", "checkcaller",
	"iscclosure", "islclosure", "newcclosure", "hookfunction",
	"clonefunction", "getrawmetatable", "setreadonly", "loadstring", "typeof",
}
local apiState = {}
for _, name in ipairs(apiList) do
	local v, where
	if name == "getgenv" then
		-- getgenv 本身是入口，不在它返回的表里
		local ok, fn = pcall(function() return getgenv end)
		v = ok and fn or nil
		where = type(v) == "function" and "global" or "-"
	else
		v, where = lookup(name)
	end
	local okType = type(v)
	apiState[name] = okType == "function"
	add("  %-18s %-10s at %s", name, okType, where)
end

----------------------------------------------------------------------
add("")
add("---- 2. 闭包分类（exguard 的第 9/14 号闸门就靠它）----")
do
	local isc, isl = nil, nil
	pcall(function() isc = ENV.iscclosure end)
	if type(isc) ~= "function" then pcall(function() isc = _G.iscclosure end) end
	pcall(function() isl = ENV.islclosure end)
	if type(isl) ~= "function" then pcall(function() isl = _G.islclosure end) end
	if type(isc) == "function" then
		local ok, v = pcall(isc, print)
		add("  iscclosure(print)      = %s   (期望 true)", ok and tostring(v) or ("ERR " .. tostring(v)))
		-- 关键：原语函数（C 闭包）必须被判成 C
		local prims = { ["string.char"] = string and string.char, ["buffer.len"] = buffer and buffer.len,
			["bit32.bxor"] = bit32 and bit32.bxor, ["pcall"] = pcall, ["select"] = select }
		for label, fn in pairs(prims) do
			if type(fn) == "function" then
				local okP, vP = pcall(isc, fn)
				add("  iscclosure(%-14s) = %s", label, okP and tostring(vP) or ("ERR " .. tostring(vP)))
			else
				add("  iscclosure(%-14s) = (缺失)", label)
			end
		end
	else
		add("  iscclosure 不存在")
	end
	if type(isl) == "function" then
		local ok, v = pcall(isl, function() return 1 end)
		add("  islclosure(lua fn)    = %s   (期望 true)", ok and tostring(v) or ("ERR " .. tostring(v)))
	end
end

----------------------------------------------------------------------
add("")
add("---- 3. debug 深内省（Synapse 级能力，exguard 硬要求）----")
do
	local dbg = nil
	pcall(function() dbg = ENV.debug end)
	if type(dbg) ~= "table" then pcall(function() dbg = _G.debug end) end
	add("  debug 库: %s", type(dbg))
	if type(dbg) == "table" then
		for _, name in ipairs({ "getconstants", "getupvalues", "setupvalue", "getproto", "getprotos", "info", "getinfo" }) do
			local v = nil
			pcall(function() v = dbg[name] end)
			add("  debug.%-14s %s", name, type(v))
		end
		-- 真实调用（不只是看类型）
		local function try(label, fn)
			local ok, a, b = pcall(fn)
			add("  %-34s ok=%s ret=%s%s", label, tostring(ok), typeOf(a),
				(type(a) == "table" and " n=" .. tostring(#a) or " " .. tostring(a)))
		end
		if type(dbg.getconstants) == "function" then
			try("debug.getconstants(print)", function() return dbg.getconstants(print) end)
		end
		if type(dbg.getupvalues) == "function" then
			local probe = function() return 1 end
			try("debug.getupvalues(lua fn)", function() return dbg.getupvalues(probe) end)
			try("debug.setupvalue(lua fn,1,0)", function() return dbg.setupvalue(probe, 1, 0) end)
		end
		if type(dbg.getproto) == "function" then
			try("debug.getproto(fn,1)", function() return dbg.getproto(function() end, 1) end)
		end
	end
end

----------------------------------------------------------------------
add("")
add("---- 4. Roblox 语义（tier-2 就在这里判定）----")
local T2 = {}
local function t2(name, pass) T2[name] = pass and true or false end
do
	-- Enum
	local EnumV = nil
	pcall(function() EnumV = ENV.Enum end)
	if EnumV == nil then pcall(function() EnumV = _G.Enum end) end
	add("  Enum type          : %s", type(EnumV))
	if EnumV ~= nil then
		local z1 = nil
		local ok1, e1 = pcall(function() return EnumV.HumanoidRigType end)
		z1 = ok1 and e1 or nil
		add("  Enum.HumanoidRigType: %s", typeOf(z1))
		if z1 ~= nil then
			local okA, z2 = pcall(function() return z1["FromName"](z1, "R6") end)
			local okB, z3 = pcall(function() return z1["FromName"](z1, "R6") end)
			local okC, z4 = pcall(function() return z1["FromValue"](z1, 0) end)
			local z5 = nil
			pcall(function() z5 = z1["R6"] end)
			add("  FromName('R6') ok=%s type=%s", tostring(okA), typeOf(z2))
			add("  ==FromValue(0)   : %s   (期望 true)", tostring(z2 ~= nil and z2 == z4))
			add("  ==idx['R6']      : %s   (期望 true)", tostring(z2 ~= nil and z2 == z5))
			local nameV, valV, etV = "?", "?", "?"
			pcall(function() nameV = tostring(z2["Name"]) end)
			pcall(function() valV = tostring(z2["Value"]) end)
			pcall(function() etV = tostring(z2["EnumType"]) end)
			add("  Name/Value/EnumType: %s / %s / %s   (期望 R6 / 0 / HumanoidRigType)", nameV, valV, etV)
			t2("enum_identity", z2 ~= nil and z2 == z3 and z2 == z4 and z2 == z5)
			t2("enum_fields", nameV == "R6" and valV == "0" and etV == tostring(z1))
			add("  typeof(Enum)     : %s   (期望 Enums)", typeOf(EnumV))
			add("  typeof(z1)       : %s   (期望 Enum)", typeOf(z1))
			add("  typeof(z2)       : %s   (期望 EnumItem)", typeOf(z2))
			t2("enum_typeof", typeOf(EnumV) == "Enums" and typeOf(z1) == "Enum" and typeOf(z2) == "EnumItem")
			local m1, m2, m3 = "?", "?", "?"
			pcall(function() m1 = tostring(getmetatable(EnumV)) end)
			pcall(function() m2 = tostring(getmetatable(z1)) end)
			pcall(function() m3 = tostring(getmetatable(z2)) end)
			add("  getmetatable(Enum) = %s", m1)
			add("  getmetatable(z1)   = %s", m2)
			add("  getmetatable(z2)   = %s", m3)
			add("  (tier-2 期望三者都是字符串 'The metatable is locked')")
			t2("enum_metatable", m1 == "The metatable is locked" and m2 == m1 and m3 == m1)
		end
	end

	-- Random（tier-2 要求精确浮点）
	local RandomV = nil
	pcall(function() RandomV = ENV.Random end)
	if RandomV == nil then pcall(function() RandomV = _G.Random end) end
	add("  Random type        : %s", type(RandomV))
	if type(RandomV) == "table" and type(RandomV.new) == "function" then
		-- 注意：R067.3 起不再比对精确浮点（执行器 PRNG 实现不同属正常），
		-- 只判定「是确定性 PRNG 且取值域正确」。
		local okN, r = pcall(RandomV.new, 51018400)
		if okN and r ~= nil then
			local ok1, v1 = pcall(function() return r["NextNumber"](r) end)
			local n1 = ok1 and string.format("%.17g", v1) or tostring(v1)
			add("  NextNumber #1      : %s   [0,1) ? %s", n1,
				tostring(ok1 and type(v1) == "number" and v1 >= 0 and v1 < 1))
			t2("random_range", ok1 and type(v1) == "number" and v1 >= 0 and v1 < 1)
			local ok2, v2 = pcall(function() return r["NextInteger"](r, 189, 1164) end)
			add("  NextInteger(189,1164): %s   integer & in-range ? %s",
				ok2 and tostring(v2) or ("ERR " .. tostring(v2)),
				tostring(ok2 and type(v2) == "number" and math.floor(v2) == v2 and v2 >= 189 and v2 <= 1164))
			t2("random_integer", ok2 and type(v2) == "number" and math.floor(v2) == v2 and v2 >= 189 and v2 <= 1164)
			local ok3, v3 = pcall(function() return r["NextNumber"](r) end)
			add("  NextNumber #2      : %s   advanced ? %s",
				ok3 and string.format("%.17g", v3) or tostring(v3),
				tostring(ok3 and type(v3) == "number" and v3 >= 0 and v3 < 1 and v3 ~= v1))
			t2("random_advances", ok3 and type(v3) == "number" and v3 >= 0 and v3 < 1 and v3 ~= v1)
			-- 同种子确定性 + Clone 语义 + 不同种子不同序列
			local sA = 1234567
			local dOk, rA, rB = pcall(function()
				local x = RandomV.new(sA)
				local y = RandomV.new(sA)
				return x, y
			end)
			local sameSeed = false
			if dOk and rA and rB then
				local v1ok, va = pcall(function() return rA["NextNumber"](rA) end)
				local v2ok, vb = pcall(function() return rB["NextNumber"](rB) end)
				sameSeed = v1ok and v2ok and va == vb
			end
			add("  same seed equal    : %s   (期望 true)", tostring(sameSeed))
			t2("random_deterministic", sameSeed)
			local sC = sA + 1
			local sOk, rC = pcall(RandomV.new, sC)
			local diff = false
			if sOk and rC then
				local v1ok, va = pcall(function() return RandomV.new(sA)["NextNumber"](RandomV.new(sA)) end)
				local v2ok, vb = pcall(function() return rC["NextNumber"](rC) end)
				diff = v1ok and v2ok and va ~= vb
			end
			add("  diff seed differs  : %s   (期望 true)", tostring(diff))
			t2("random_seed_sensitive", diff)
		else
			add("  Random.new(51018400) 失败: %s", tostring(r))
		end
	end

	-- task
	local taskV = nil
	pcall(function() taskV = ENV.task end)
	add("  task type          : %s", type(taskV))
	if type(taskV) == "table" then
		local zf = false
		local okS, th = pcall(taskV.spawn, function() zf = true end)
		add("  task.spawn         : ok=%s thread=%s ran=%s", tostring(okS), type(th), tostring(zf))
		local okD, dth = pcall(taskV.delay, 86400, function() end)
		add("  task.delay         : ok=%s thread=%s", tostring(okD), type(dth))
		local okC = false
		if okD then
			local okC2, cerr = pcall(taskV.cancel, dth)
			okC = okC2
			add("  task.cancel        : ok=%s err=%s", tostring(okC2), tostring(cerr))
		end
		local okW, werr = pcall(taskV.wait, {})
		add("  task.wait(table)   : ok=%s   (期望 ok=false，会报错)", tostring(okW))
		t2("task_spawn", okS and type(th) == "thread" and zf == true)
		t2("task_delay_cancel", okD and type(dth) == "thread" and okC)
		t2("task_wait_errors", okW == false)
	end

	-- game / Instance
	local gameV = nil
	pcall(function() gameV = ENV.game end)
	if gameV == nil then pcall(function() gameV = _G.game end) end
	add("  game type          : %s (typeof=%s)", type(gameV), typeOf(gameV))
	if gameV ~= nil then
		local okG, svc = pcall(function()
			local gs = gameV.GetService
			if type(gs) ~= "function" then return nil end
			return gs(gameV, "RunService")
		end)
		add("  game:GetService    : ok=%s type=%s", tostring(okG), type(svc))
		t2("game_getservice", okG and svc ~= nil)
		-- 方法必须用冒号：点调用传 nil self 必须报错；冒号调用必须成功
		local dotOk, dotErr = pcall(function() return svc["IsStudio"](nil) end)
		add("  Service.IsStudio(nil) : ok=%s err_type=%s   (期望 ok=false, string)",
			tostring(dotOk), type(dotErr))
		t2("member_dot_errors", (not dotOk) and type(dotErr) == "string" and #tostring(dotErr) > 0)
		local colonOk, colonVal = pcall(function() return svc:IsStudio() end)
		add("  Service:IsStudio()    : ok=%s type=%s   (期望 ok=true, boolean)",
			tostring(colonOk), type(colonVal))
		t2("member_colon_works", colonOk and type(colonVal) == "boolean")
	end
	local InstanceV = nil
	pcall(function() InstanceV = ENV.Instance end)
	add("  Instance type      : %s", type(InstanceV))
	if type(InstanceV) == "table" and type(InstanceV.new) == "function" then
		local okI, folder = pcall(InstanceV.new, "Folder")
		add("  Instance.new       : ok=%s type=%s", tostring(okI), typeOf(folder))
		if okI and folder ~= nil then
			local okC, kids = pcall(function() return folder["GetChildren"](folder) end)
			add("  GetChildren        : ok=%s n=%s", tostring(okC), tostring(type(kids) == "table" and #kids or "n/a"))
			local okD = pcall(function() folder["Destroy"](folder) end)
			add("  Destroy            : ok=%s", tostring(okD))
			t2("instance_folder", okI and okC and type(kids) == "table" and #kids == 0)
			-- 销毁后重新挂父必须报错（不再校验错误文案，只看是否拒绝）
			local lockOk, lockErr = pcall(function() folder["Parent"] = workspace end)
			add("  reparent destroyed : ok=%s err_type=%s   (期望 ok=false, string)",
				tostring(lockOk), type(lockErr))
			t2("instance_destroy_locked", (not lockOk) and type(lockErr) == "string")
		end
	end
	local wsV = nil
	pcall(function() wsV = ENV.workspace end)
	add("  workspace          : %s", type(wsV))
end

----------------------------------------------------------------------
add("")
add("---- 5. Luau 运行时（tier-1 靠这些）----")
for _, name in ipairs({ "buffer", "bit32", "utf8", "string", "table", "math", "coroutine" }) do
	local v = rawget(_G, name)
	if v == nil then pcall(function() v = ENV[name] end) end
	add("  %-10s %s", name, type(v))
end
do
	local ok, f = pcall(function() return loadstring("return 1+1") end)
	add("  loadstring      : ok=%s type=%s", tostring(ok), type(f))
	if ok and type(f) == "function" then
		local ok2, v = pcall(f)
		add("  loadstring run  : ok=%s value=%s", tostring(ok2), tostring(v))
	end
	local okE, fBad, errBad = pcall(function() return loadstring("local =") end)
	add("  loadstring(坏源) : call_ok=%s returns_nil=%s err=%s   (期望 call_ok=true, returns_nil=true)",
		tostring(okE), tostring(okE and fBad == nil), tostring(errBad))
end

----------------------------------------------------------------------
add("")
add("---- 6. 关键结论 ----")
local missing = {}
for _, name in ipairs(apiList) do
	if not apiState[name] then missing[#missing + 1] = name end
end
local missingDeep = {}
do
	local dbg = nil
	pcall(function() dbg = ENV.debug end)
	if type(dbg) ~= "table" then pcall(function() dbg = _G.debug end) end
	for _, name in ipairs({ "getconstants", "getupvalues", "setupvalue" }) do
		local v = nil
		pcall(function() v = type(dbg) == "table" and dbg[name] end)
		if type(v) ~= "function" then missingDeep[#missingDeep + 1] = name end
	end
end
local t2fail = {}
for _, name in ipairs({ "enum_identity", "enum_fields", "enum_typeof", "enum_metatable",
	"random_range", "random_integer", "random_advances", "random_deterministic", "random_seed_sensitive",
	"task_spawn", "task_delay_cancel", "task_wait_errors",
	"game_getservice", "member_dot_errors", "member_colon_works",
	"instance_folder", "instance_destroy_locked" }) do
	if T2[name] ~= true then t2fail[#t2fail + 1] = name end
end
add("  tier-2 未通过项 : %s", #t2fail > 0 and table.concat(t2fail, ",") or "(全通过)")
add("  缺失执行器 API : %s", #missing > 0 and table.concat(missing, ",") or "(无)")
add("  缺失 debug 深内省: %s", #missingDeep > 0 and table.concat(missingDeep, ",") or "(无)")
add("  → hard 版需要：执行器 API 齐全 + debug 深内省齐全")

local summary = string.format(
	"RESULT: executor=%s ver=%s genv_eq_G=%s tier2_fail=[%s] missing_api=[%s] missing_debug=[%s]",
	execName, execVer, genvEqG,
	#t2fail > 0 and table.concat(t2fail, ",") or "-",
	#missing > 0 and table.concat(missing, ",") or "-",
	#missingDeep > 0 and table.concat(missingDeep, ",") or "-")

----------------------------------------------------------------------
-- 输出
local full = table.concat(R, "\n")
print("======================================")
print(full)
print("======================================")
print(summary)
pcall(function() warn(summary) end)

-- 剪贴板
local payload = full .. "\n" .. summary
local copied = false
local used = "-"
local setters = {
	{ "setclipboard", function() return setclipboard end },
	{ "toclipboard", function() return toclipboard end },
	{ "writeclipboard", function() return writeclipboard end },
	{ "set_clipboard", function() return set_clipboard end },
}
for _, entry in ipairs(setters) do
	if not copied then
		local ok, fn = pcall(entry[2])
		if ok and type(fn) == "function" then
			local ok2 = pcall(fn, payload)
			if ok2 then copied = true; used = entry[1] end
		end
	end
end
if not copied then
	pcall(function()
		if type(Clipboard) == "table" and type(Clipboard.set) == "function" then
			Clipboard.set(payload); copied = true; used = "Clipboard.set"
		end
	end)
end
print(copied and ("[Obsucvim] 已复制到剪贴板（" .. used .. "），直接粘贴发给我即可。")
	or "[Obsucvim] 剪贴板 API 不可用：请手动全选上面的报告复制，或截图发我。")

return full
