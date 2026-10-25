-- Obsucvim GateProbe v2 -- R067.5 companion host probe (diagnostic only, no protection logic)
--
-- 用途：在目标执行器/宿主里【整段运行一次】，跑完全部输出自动复制到剪贴板，
-- 末尾 [XG-CLIP] 行报告复制结果（host 无 setclipboard/toclipboard 时会提示手动复制）。
-- Suda.hard-trace.lua 报的是「第一道炸掉的闸」（一次只给一道）；
-- 本探针按 R067.5 的 SCHALL 判定逻辑 1:1 复刻，【一次给出全部闸门的风险画像】，
-- 末尾 [PREDICT] 行列出预测会炸的闸字母（空集 = hard 产物预期直接跑通）。
--
-- 对应关系：行号 [XG-X] 的 X == trace 报告 (字母-xxxx) 的字母。
-- 不可测的闸：R/S/T/U 是引擎内部状态机闸（seal/转录/ATTEST/投毒链），
-- 宿主探针无从复制，只能由 trace 产物在真机给出；S 为保留号（产物里不可达）。
--
-- 安全说明：除 D 小节做一次“写入后立即还原”的 rawset 金丝雀外，全程只读 + pcall；
-- 不改全局、不联网、不依赖任何 Obsucvim 组件。

local risk = {}
-- 输出收集：所有报告行在打印的同时进缓冲区，跑完自动复制到剪贴板
local OUTBUF = {}
local rawprint = print
local function out(line)
	OUTBUF[#OUTBUF + 1] = tostring(line)
	rawprint(line)
end
local function emit(id, ok, detail)
	out("[XG-" .. id .. "] ok=" .. tostring(ok and true or false) .. " " .. tostring(detail))
end
local function mark(letter, ok, why)
	if not ok then risk[letter] = why or "" end
end
local function tcount(t)
	local n = 0
	for _, _ in next, t do n = n + 1 end
	return n
end
local function tcontains(t, exp)
	if type(t) ~= "table" then return false end
	for _, it in next, t do
		if it == exp then return true end
	end
	return false
end
-- 与引擎 ERD/LKP 完全同形：rawget 优先，退回索引，全 pcall
local function erd(e, k)
	if type(e) ~= "table" and type(e) ~= "userdata" then return nil end
	local v
	if type(e) == "table" then
		v = rawget(e, k)
		if v ~= nil then return v end
	end
	local ok, iv = pcall(function() return e[k] end)
	if ok then return iv end
	return nil
end

-- ===== 环境基线（引擎 ENV/CENV 绑定的同形复刻） =====
local ENV = nil
do
	local ok, e = pcall(getfenv, 0)
	if ok and type(e) == "table" then ENV = e end
end
local GGENV = erd(ENV, "getgenv")
local CENV = nil
if type(GGENV) == "function" then
	local ok, e = pcall(GGENV)
	if ok and type(e) == "table" then CENV = e end
end
local function lkp(k)
	local v = erd(CENV, k)
	if v == nil then v = erd(ENV, k) end
	return v
end
emit("ENV", ENV ~= nil and CENV ~= nil,
	"ENV=" .. type(ENV) .. " CENV=" .. type(CENV)
	.. " ENV==_G:" .. tostring(ENV ~= nil and rawequal(ENV, _G))
	.. " CENV==_G:" .. tostring(CENV ~= nil and rawequal(CENV, _G)))

-- ===== A: char-chain 解析（引擎的 VK 解码基元来源；真机过了 precheck 即隐含通过） =====
do
	local function isch(f)
		local o1, r1 = pcall(coroutine.wrap(function() return f(65) end))
		if not o1 or r1 ~= "A" then return false end
		local o2, r2 = pcall(coroutine.wrap(function() return f(90) end))
		if not o2 or r2 ~= "Z" then return false end
		local o3, r3 = pcall(coroutine.wrap(function() return f(48) end))
		if not o3 or r3 ~= "0" then return false end
		local o4, r4 = pcall(coroutine.wrap(function() return f(95) end))
		return o4 and r4 == "_"
	end
	local function scan(t)
		for _, v in pairs(t) do
			if type(v) == "function" and isch(v) then return v end
		end
		return nil
	end
	local via = "none"
	local ch = nil
	local mok, mt = pcall(getmetatable, "")
	if mok and type(mt) == "table" then
		local sok, sr = pcall(function()
			local r = scan(mt)
			if not r then
				for _, v in pairs(mt) do
					if type(v) == "table" then r = scan(v) if r then break end end
				end
			end
			return r
		end)
		if sok and sr then ch, via = sr, "M" end
	end
	if not ch and ENV then
		local sok2, sr2 = pcall(function()
			for _, tab in pairs(ENV) do
				if type(tab) == "table" then local r2 = scan(tab) if r2 then return r2 end end
			end
			return nil
		end)
		if sok2 and sr2 then ch, via = sr2, "E" end
	end
	if not ch then
		local lok, lv = pcall(function() return ("\0")["char"] end)
		if lok and type(lv) == "function" and isch(lv) then ch, via = lv, "L" end
	end
	emit("A", ch ~= nil, "via=" .. via .. " (M=string-mt E=env-scan L=lit-idx)")
end

-- ===== B: API 类型总检（与 precheck/SCHALL 第一道同一判定集） =====
local DTBL = lkp("debug")
local function dapi(n) if DTBL == nil then return nil end return erd(DTBL, n) end
local missing = {}
local sides = {}
local function need(name, v)
	if type(v) ~= "function" then table.insert(missing, name) end
end
local function side(name, v)
	local s = "-"
	if type(v) == "function" then
		local ing = erd(CENV, name) ~= nil
		local ine = erd(ENV, name) ~= nil
		s = (ing and ine) and "GE" or (ing and "G" or "E")
	end
	table.insert(sides, name .. ":" .. s)
end
local bset = {
	"identifyexecutor", "checkcaller", "iscclosure", "islclosure",
	"newcclosure", "loadstring", "typeof",
}
local bvals = {}
for _, n in ipairs(bset) do
	bvals[n] = lkp(n)
	need(n, bvals[n])
	side(n, bvals[n])
end
local INSPECT = dapi("info")
if type(INSPECT) ~= "function" then INSPECT = dapi("getinfo") end
if type(dapi("info")) ~= "function" and type(dapi("getinfo")) ~= "function" then
	table.insert(missing, "debug.info")
end
need("debug.getconstants", dapi("getconstants"))
need("debug.getupvalues", dapi("getupvalues"))
if type(dapi("getproto")) ~= "function" and type(dapi("getprotos")) ~= "function" then
	table.insert(missing, "debug.getproto")
end
need("debug.setupvalue", dapi("setupvalue"))
local bok = #missing == 0
emit("B", bok, "missing={" .. table.concat(missing, ",") .. "}")
emit("Bs", true, "side {" .. table.concat(sides, " ") .. "} debug:" ..
	(erd(CENV, "debug") ~= nil and (erd(ENV, "debug") ~= nil and "GE" or "G") or (erd(ENV, "debug") ~= nil and "E" or "-")))
mark("B", bok, "missing APIs")

local IDENTV, CHKCV, ISCV, ISLV, NEWCV, LSTRV, TYPHOF =
	bvals.identifyexecutor, bvals.checkcaller, bvals.iscclosure, bvals.islclosure,
	bvals.newcclosure, bvals.loadstring, bvals.typeof
local GCONSTV, GUPVV, GPROTOV, GPROTOSV, SUPVV =
	dapi("getconstants"), dapi("getupvalues"), dapi("getproto"), dapi("getprotos"), dapi("setupvalue")
local GAMEV, INSTV, VEC3V, TASKV = lkp("game"), lkp("Instance"), lkp("Vector3"), lkp("task")

-- ===== C: getgenv() ~= getfenv(0) =====
do
	local same = ENV ~= nil and CENV ~= nil and rawequal(ENV, CENV)
	local cok = ENV ~= nil and CENV ~= nil and not same
	emit("C", cok, "rawequal(ENV,CENV)=" .. tostring(same))
	mark("C", cok or (ENV == nil or CENV == nil), "env not separated (getgenv()==getfenv(0))")
end

-- ===== D: env 金丝雀（分离 + 持久，判定串与引擎逐比特一致） =====
do
	local CANARY = "__obsucvim_gp_canary__"
	local canok, sep, per, tr1, tr2 = false, false, false, false, false
	if ENV and CENV then
		local thold, cold = erd(ENV, CANARY), erd(CENV, CANARY)
		local tm, cm = {}, {}
		canok = pcall(function()
			rawset(ENV, CANARY, tm)
			sep = erd(CENV, CANARY) ~= tm
			rawset(CENV, CANARY, cm)
			local rok, renv = pcall(GGENV)
			per = rok and rawequal(renv, CENV) and erd(renv, CANARY) == cm
		end)
		tr1 = pcall(rawset, ENV, CANARY, thold)
		tr2 = pcall(rawset, CENV, CANARY, cold)
	end
	local dok = canok and tr1 and tr2 and sep and per
	emit("D", dok, string.format("canok=%s tr1=%s tr2=%s sep=%s per=%s",
		tostring(canok), tostring(tr1), tostring(tr2), tostring(sep), tostring(per)))
	mark("D", dok or (ENV == nil or CENV == nil), "canary separation/persistence failed")
end

-- ===== E: identifyexecutor 双调用一致性 =====
do
	local eok, det = false, "noapi"
	if type(IDENTV) == "function" then
		local i1ok, in1, iv1 = pcall(IDENTV)
		local i2ok, in2, iv2 = pcall(IDENTV)
		eok = i1ok and i2ok and type(in1) == "string" and type(iv1) == "string"
			and type(iv2) == "string" and #in1 >= 1 and #in1 <= 128
			and #iv1 <= 128 and in1 == in2 and iv1 == iv2
		det = "i1ok=" .. tostring(i1ok) .. " i2ok=" .. tostring(i2ok)
			.. " in1=" .. tostring(in1) .. " iv1=" .. tostring(iv1)
			.. " in2=" .. tostring(in2) .. " iv2=" .. tostring(iv2)
	end
	emit("E", eok, det)
	mark("E", eok or type(IDENTV) ~= "function", "identifyexecutor unstable/shape")
end

-- ===== F: checkcaller（闸只查 pcall 形态；coroutine 形态为参考值） =====
do
	local f_gate, det = false, "noapi"
	if type(CHKCV) == "function" then
		local ccok, cc = pcall(CHKCV)
		f_gate = ccok and cc == true
		local fpv = nil
		pcall(function()
			local o, v = pcall(CHKCV) fpv = (o and v == true) return fpv
		end)
		local coro_ok, coro_v = pcall(function()
			return coroutine.wrap(function()
				local o, v = pcall(CHKCV) return o and v == true
			end)()
		end)
		det = "gate=" .. tostring(ccok) .. "/" .. tostring(cc)
			.. " nested-pcall=" .. tostring(fpv)
			.. " coroutine=" .. tostring(coro_ok) .. "/" .. tostring(coro_v)
	end
	emit("F", f_gate, det)
	mark("F", f_gate or type(CHKCV) ~= "function", "checkcaller()~=true")
end

-- ===== G: Roblox 全局类型 =====
do
	local gok = (type(GAMEV) == "table" or type(GAMEV) == "userdata")
		and type(INSTV) == "table" and type(VEC3V) == "table" and type(TASKV) == "table"
	emit("G", gok, type(GAMEV) .. "/" .. type(INSTV) .. "/" .. type(VEC3V) .. "/" .. type(TASKV))
	mark("G", gok, "roblox globals shape")
end

-- ===== H: Roblox 深探（细节串字段与引擎 _hd 完全同名） =====
do
	local hd_unused = ""
	local hok, hver, hd = pcall(function()
		local d = "g:" .. type(GAMEV)
		local gs = erd(GAMEV, "GetService")
		local ok8, v8 = pcall(function() return GAMEV["GetService"] end)
		d = d .. ",rp:" .. tostring(ok8) .. "/" .. type(v8) .. ",er:" .. type(gs)
		if gs == nil and ok8 then gs = v8 end
		d = d .. ",gs:" .. type(gs)
		local pok, players = pcall(gs, GAMEV, "Players")
		d = d .. ",pl:" .. tostring(pok) .. "/" .. type(players)
		local cn = nil
		if players ~= nil then
			local cok2, cv = pcall(function() return erd(players, "ClassName") end)
			if cok2 then cn = cv end
			if cn == nil then
				local cok3, cv3 = pcall(function() return players["ClassName"] end)
				if cok3 then cn = cv3 end
			end
		end
		d = d .. ",cn:" .. tostring(cn)
		local vn = erd(VEC3V, "new")
		if vn == nil then
			local nkok, nkv = pcall(function() return VEC3V["new"] end)
			if nkok then vn = nkv end
		end
		d = d .. ",vn:" .. type(vn)
		local vok, vec = pcall(vn)
		local vty = (vok and TYPHOF ~= nil) and TYPHOF(vec) or type(vec)
		d = d .. ",v3:" .. tostring(vok) .. "/" .. tostring(vty)
		local tv = TYPHOF ~= nil and TYPHOF(setmetatable({}, {})) or "no-typeof"
		d = d .. ",smt:" .. tostring(tv)
		local inew = erd(INSTV, "new")
		if inew == nil then
			local iok2, iv2 = pcall(function() return INSTV["new"] end)
			if iok2 then inew = iv2 end
		end
		d = d .. ",in:" .. type(inew)
		local tw, tsp, tdf = erd(TASKV, "wait"), erd(TASKV, "spawn"), erd(TASKV, "defer")
		d = d .. ",tw:" .. type(tw) .. ",tsp:" .. type(tsp) .. ",tdf:" .. type(tdf)
		local ver = pok == true and players ~= nil and cn == "Players" and vok == true
			and TYPHOF ~= nil and TYPHOF(vec) == "Vector3" and tv == "table"
			and type(inew) == "function" and type(tw) == "function"
			and type(tsp) == "function" and type(tdf) == "function"
		return ver == true, d
	end)
	local hverv, hdv = false, "<error>"
	if hok then hverv, hdv = hver, hd else hdv = tostring(hver) end
	emit("H", hok and hverv == true, "hok=" .. tostring(hok) .. "/hver=" .. tostring(hverv) .. ":" .. hdv)
	mark("H", hok and hverv == true, "roblox deep probe")
end

-- ===== I: 原语 C 闭包分类（19 项 + Lua 反例） =====
local function classifies(f, expc)
	if type(f) ~= "function" then return false end
	if type(ISCV) ~= "function" or type(ISLV) ~= "function" then return false end
	local cok, cres = pcall(ISCV, f)
	local lok, lres = pcall(ISLV, f)
	return cok and lok and cres == expc and lres == (not expc)
end
do
	local STBL, TTBL, MTBL = lkp("string"), lkp("table"), lkp("math")
	local prims = {
		{ "string.byte", erd(STBL, "byte") }, { "string.char", erd(STBL, "char") },
		{ "string.sub", erd(STBL, "sub") }, { "table.concat", erd(TTBL, "concat") },
		{ "table.insert", erd(TTBL, "insert") }, { "math.ldexp", erd(MTBL, "ldexp") },
		{ "select", lkp("select") }, { "pcall", lkp("pcall") }, { "type", lkp("type") },
		{ "tostring", lkp("tostring") }, { "tonumber", lkp("tonumber") },
		{ "rawget", lkp("rawget") }, { "rawset", lkp("rawset") },
		{ "rawequal", lkp("rawequal") }, { "next", lkp("next") },
		{ "setmetatable", lkp("setmetatable") }, { "getmetatable", lkp("getmetatable") },
		{ "table.unpack|unpack", erd(TTBL, "unpack") or lkp("unpack") },
		{ "debug.inspect", INSPECT },
	}
	local badp = {}
	for _, p in ipairs(prims) do
		if not classifies(p[2], true) then table.insert(badp, p[1]) end
	end
	local lprobe = function(v) return v end
	if not classifies(lprobe, false) then table.insert(badp, "_lprobe(lua)") end
	local iok = #badp == 0
	local ipre = type(ISCV) == "function" and type(ISLV) == "function"
	emit("I", iok, ipre and ("bad={" .. table.concat(badp, ",") .. "}") or "noapi(iscclosure/islclosure missing)")
	mark("I", iok or not ipre, "C/Lua closure misclassification")
end

-- ===== J: 反向 debug 探针（对 string.byte 必须全部失败或空） =====
do
	local sb = erd(lkp("string"), "byte")
	local c1ok, c2ok, c2up, c3ok, c4ok, c5ok = false, false, nil, false, false, false
	if type(GCONSTV) == "function" and sb then c1ok = pcall(GCONSTV, sb) end
	if type(GUPVV) == "function" and sb then c2ok, c2up = pcall(GUPVV, sb) end
	if type(SUPVV) == "function" and sb then c3ok = pcall(SUPVV, sb, 1, 0) end
	if type(GPROTOV) == "function" and sb then c4ok = pcall(GPROTOV, sb, 1) end
	if type(GPROTOSV) == "function" and sb then c5ok = pcall(GPROTOSV, sb) end
	local c2empty = type(c2up) == "table" and next(c2up) == nil
	local jok = (not c1ok) and (not c3ok) and ((not c2ok) or c2empty) and (not c4ok) and (not c5ok)
	emit("J", jok, string.format(
		"getconstants(byte)=%s getupvalues(byte)=%s/%s setupvalue(byte)=%s getproto(byte)=%s getprotos(byte)=%s",
		tostring(c1ok), tostring(c2ok), c2ok and (type(c2up) .. ",empty=" .. tostring(c2empty)) or "-",
		tostring(c3ok), tostring(c4ok), tostring(c5ok)))
	mark("J", jok, "C-fn debug introspection leaked")
end

-- ===== K/L/M/N: getconstants/getupvalues/setupvalue/getproto(s) 对真 Lua 闭包 =====
local UPVV = 111
local function uprobe(v) UPVV = (UPVV + v) % 2147483647 return UPVV end
do
	local KCONST = 987654321
	local kok, kdet = false, "skip(no debug.getconstants)"
	if type(GCONSTV) == "function" then
		local function cprobe() return KCONST end
		local ok, t = pcall(GCONSTV, cprobe)
		kok = ok and type(t) == "table"
		kdet = "ok=" .. tostring(ok) .. " type=" .. type(t)
		if type(t) == "table" then
			kdet = kdet .. " n=" .. tcount(t) .. " hasConst=" .. tostring(tcontains(t, KCONST))
		end
	end
	emit("K", kok, kdet)
	mark("K", kok or type(GCONSTV) ~= "function", "getconstants on Lua fn failed")
end
do
	local lok, ldet = false, "skip(no debug.getupvalues)"
	if type(GUPVV) == "function" then
		local ok, t = pcall(GUPVV, uprobe)
		lok = ok and type(t) == "table"
		ldet = "ok=" .. tostring(ok) .. " type=" .. type(t)
		if type(t) == "table" then ldet = ldet .. " n=" .. tcount(t) end
	end
	emit("L", lok, ldet)
	mark("L", lok or type(GUPVV) ~= "function", "getupvalues on Lua fn failed")
end
do
	local mok, mdet = false, "skip(no debug.setupvalue)"
	if type(SUPVV) == "function" then
		local k3a = pcall(SUPVV, uprobe, 1, 999)
		local k3b = pcall(SUPVV, uprobe, 1, 111)
		local after = uprobe(0)
		mok = k3a and k3b and after == 111
		mdet = "set(999)=" .. tostring(k3a) .. " set(111)=" .. tostring(k3b) .. " uprobe(0)=" .. tostring(after) .. " exp=111"
	end
	emit("M", mok, mdet)
	mark("M", mok or type(SUPVV) ~= "function", "setupvalue real mutation failed")
end
do
	local nok, ndet = false, ""
	local fails = {}
	local function pprobe() local function pchild(v) return (v + 77) % 2147483647 end return pchild end
	local actp = pprobe()
	local aok, ares = pcall(actp, 13)
	if not (aok and ares == 90) then table.insert(fails, "child-call=" .. tostring(aok) .. "/" .. tostring(ares)) end
	if not classifies(actp, false) then table.insert(fails, "child-class") end
	local saw = false
	if type(GPROTOV) == "function" then
		saw = true
		local p1ok = pcall(GPROTOV, pprobe, 1)
		if not p1ok then
			local _, e1 = pcall(GPROTOV, pprobe, 1)
			table.insert(fails, "getproto:" .. tostring(e1))
		else
			local p2ok, p2 = pcall(GPROTOV, pprobe, 1, true)
			if not p2ok or type(p2) ~= "table" then table.insert(fails, "getproto-tbl=" .. tostring(p2ok) .. "/" .. type(p2)) end
		end
	end
	if type(GPROTOSV) == "function" then
		saw = true
		local p3ok, p3 = pcall(GPROTOSV, pprobe)
		if not p3ok or type(p3) ~= "table" then table.insert(fails, "getprotos=" .. tostring(p3ok) .. "/" .. type(p3)) end
	end
	if not saw then table.insert(fails, "no-proto-api") end
	nok = #fails == 0
	ndet = #fails == 0 and "all pass (child+getproto+getprotos)" or table.concat(fails, " | ")
	emit("N", nok, ndet)
	mark("N", nok or (type(GPROTOV) ~= "function" and type(GPROTOSV) ~= "function"), "getproto(s) enumeration failed")
end

-- ===== O/P: loadstring 拒坏源 / 载好源并核对常量 =====
do
	local ook, odet = false, "skip(no loadstring)"
	if type(LSTRV) == "function" then
		local ivok, ivf, ive = pcall(LSTRV, "!!!obsucvim!!!", "obsucvim-gp")
		ook = ivok and ivf == nil and type(ive) == "string" and #ive >= 1
		odet = "call=" .. tostring(ivok) .. " fn=" .. tostring(ivf) .. " err=" .. type(ive)
			.. (type(ive) == "string" and (" len=" .. #ive) or "")
	end
	emit("O", ook, odet)
	mark("O", ook or type(LSTRV) ~= "function", "loadstring accepted invalid source / bad err shape")
end
do
	local PCONST = 2468101214
	local pok, pdet = false, "skip(no loadstring)"
	if type(LSTRV) == "function" then
		local fails, skips = {}, {}
		local lok2, lf = pcall(LSTRV, "return " .. tostring(PCONST))
		if not lok2 or type(lf) ~= "function" then
			table.insert(fails, "load=" .. tostring(lok2) .. "/" .. type(lf))
		else
			local l2ok, lv = pcall(lf)
			if not l2ok or lv ~= PCONST then table.insert(fails, "call=" .. tostring(l2ok) .. "/" .. tostring(lv)) end
			if not classifies(lf, false) then
				table.insert(type(ISCV) == "function" and type(ISLV) == "function" and fails or skips, "class-not-lua")
			end
			if type(GCONSTV) == "function" then
				local l3ok, l3 = pcall(GCONSTV, lf)
				if not l3ok or not tcontains(l3, PCONST) then
					table.insert(fails, "consts=" .. tostring(l3ok) .. "/has=" .. tostring(tcontains(l3, PCONST)))
				end
			else
				table.insert(skips, "consts=skip(no-getconstants)")
			end
		end
		pok = #fails == 0
		pdet = (pok and "core-pass" or table.concat(fails, " | "))
			.. (#skips > 0 and (" | " .. table.concat(skips, " | ")) or "")
	end
	emit("P", pok, pdet)
	mark("P", pok or type(LSTRV) ~= "function" or type(GCONSTV) ~= "function", "loadstring+getconstants roundtrip failed")
end

-- ===== Q: newcclosure 包装 =====
do
	local qok, qdet = false, "skip(no newcclosure)"
	if type(NEWCV) == "function" then
		local fails, skips = {}, {}
		local function cbody(v) return math.abs(v) end
		local wok, wrapped = pcall(NEWCV, cbody)
		if not wok or type(wrapped) ~= "function" then
			table.insert(type(ISCV) == "function" and type(ISLV) == "function" and fails or skips, "wrap=" .. tostring(wok) .. "/" .. type(wrapped))
		else
			local w2ok, wv = pcall(wrapped, -37)
			if not w2ok or wv ~= 37 then table.insert(fails, "call=" .. tostring(w2ok) .. "/" .. tostring(wv)) end
			if not classifies(wrapped, true) then table.insert(fails, "class-not-c") end
			if type(GUPVV) == "function" then
				local w3ok, w3 = pcall(GUPVV, wrapped)
				if w3ok and not (type(w3) == "table" and next(w3) == nil) then
					table.insert(fails, "upv-leak n=" .. tostring(type(w3) == "table" and tcount(w3) or -1))
				end
			end
		end
		qok = #fails == 0
		qdet = (qok and "core-pass" or table.concat(fails, " | "))
			.. (#skips > 0 and (" | " .. table.concat(skips, " | ")) or "")
	end
	emit("Q", qok, qdet)
	mark("Q", qok or type(NEWCV) ~= "function" or type(ISCV) ~= "function" or type(ISLV) ~= "function", "newcclosure semantics")
end

-- ===== T2: tier-2 / bind-env 宿主语义（不进 PREDICT，供 L1/L2 判读） =====
do
	local rsDet, rsOk = "skip", false
	local okRS = pcall(function()
		local gsf = erd(GAMEV, "GetService")
		if gsf == nil then gsf = GAMEV["GetService"] end
		local svc = gsf(GAMEV, "RunService")
		local f = erd(svc, "IsStudio")
		if f == nil then f = svc["IsStudio"] end
		rsOk = f(svc)
		rsDet = "done"
	end)
	emit("T2-rs", okRS, "IsStudio=" .. tostring(rsOk) .. " (client=false / Studio=true) " .. rsDet)
	local stDet = {}
	local okV = pcall(function()
		local v3 = VEC3V.new(1, 2, 3)
		table.insert(stDet, "v3=" .. tostring(v3) .. " ty=" .. tostring(TYPHOF and TYPHOF(v3)))
		local V2 = lkp("Vector2")
		local v2 = V2.new(4, 5)
		table.insert(stDet, "v2=" .. tostring(v2) .. " ty=" .. tostring(TYPHOF and TYPHOF(v2)))
	end)
	emit("T2-vec", okV, table.concat(stDet, " ") .. " exp[v3=\"1, 2, 3\" v2=\"4, 5\" ty=Vector3/Vector2]")
	local okD = pcall(function()
		local d = erd(TASKV, "defer")
		local thr = d(function() end)
		return thr
	end)
	local hasV2new = pcall(function() return lkp("Vector2").new end)
	emit("T2-env", okD and hasV2new, "Vector2.new=fn:" .. tostring(hasV2new) .. " task.defer=" .. tostring(okD))
end

-- ===== K: v16 密钥源稳定性探针（仅测量，不进 PREDICT） =====
-- v16-a 会把这些运行期观测值编进载荷解码钥；每个值都必须在真 Delta 上稳定可见。
do
	local function fold31(str)
		local h = 7
		for i = 1, #str do h = (h * 31 + string.byte(str, i)) % 2147483647 end
		return h
	end
	-- K1: identifyexecutor 名字段长度与不稳定性（版本串不参与）
	local k1 = "noapi"
	if type(IDENTV) == "function" then
		local o1, n1 = pcall(IDENTV)
		local o2, n2 = pcall(IDENTV)
		local o3, n3 = pcall(IDENTV)
		k1 = string.format("name=%s len=%s stable=%s",
			tostring(n1), (type(n1) == "string" and #n1 or -1),
			tostring(o1 and o2 and o3 and n1 == n2 and n2 == n3))
	end
	emit("K1", true, k1)
	-- K2: Vector3 形态指纹（typeof 值 + tostring 格式的 fold）
	local k2 = "noapi"
	if VEC3V ~= nil then
		local ok, r = pcall(function()
			local v = VEC3V.new(1, 2, 3)
			local t = tostring(v)
			local y = TYPHOF and TYPHOF(v) or type(v)
			return string.format("typeof=%s tostring=%q fold31(v3str)=%d fold31(typeof)=%d",
				tostring(y), t, fold31(t), fold31(tostring(y)))
		end)
		k2 = ok and r or ("err:" .. tostring(r))
	end
	emit("K2", true, k2)
	-- K3: RunService IsStudio 布尔 + checkcaller 布尔的联合折叠
	local isStudio, ccv = "nil", "nil"
	pcall(function()
		local gsf = erd(GAMEV, "GetService"); if gsf == nil then gsf = GAMEV["GetService"] end
		local svc = gsf(GAMEV, "RunService")
		local f = erd(svc, "IsStudio"); if f == nil then f = svc["IsStudio"] end
		isStudio = tostring(f(svc))
	end)
	if type(CHKCV) == "function" then
		local okc, v = pcall(CHKCV); ccv = tostring(okc and v)
	end
	emit("K3", true, "IsStudio=" .. isStudio .. " checkcaller=" .. ccv
		.. " boolpack=" .. tostring((isStudio == "false" and 2 or 0) + (ccv == "true" and 1 or 0)))
	-- K4: HTTP 能力矩阵（为将来可选的联网形态摸底；纯登记）
	local hcap = {}
	for _, pair in ipairs({
		{ "request", lkp("request") }, { "http_request", lkp("http_request") },
		{ "syn.request", lkp("syn") and erd(lkp("syn"), "request") },
		{ "http.request", lkp("http") and erd(lkp("http"), "request") },
	}) do
		table.insert(hcap, pair[1] .. ":" .. type(pair[2]))
	end
	emit("K4", true, "{" .. table.concat(hcap, " ") .. "}")
	out("[XG-KF] combined-fold=" .. tostring(fold31(
		(type(IDENTV) == "function" and tostring(select(2, pcall(IDENTV)) or "" ) or "") .. "|"
		.. tostring(isStudio) .. "|" .. tostring(ccv))))
	-- KH: v16-a 引擎 hostfold 三项（= --key-host delta 编译期模型的运行期对应物）。
	-- a=fold31(tostring(Vector3.new(1,2,3))) b=fold31(typeof) c=IsStudio*1+checkcaller*2+名长*4。
	-- match=true 的宿主才能跑通 --key-host delta 产物；执行器升级后改 tostring 格式也会翻 false。
	do
		local kh = "skip(no Vector3)"
		if VEC3V ~= nil then
			local okr, r = pcall(function()
				local v = VEC3V.new(1, 2, 3)
				local a = fold31(tostring(v))
				local b = fold31((TYPHOF and TYPHOF(v) or type(v)))
				local bs = (isStudio == "true") and 1 or 0
				local bc = (ccv == "true") and 1 or 0
				local nl = -1
				if type(IDENTV) == "function" then
					local _, n2 = pcall(IDENTV)
					if type(n2) == "string" then nl = #n2 end
				end
				local c = bs + bc * 2 + nl * 4
				return string.format("a=%d b=%d c=%d match(delta a=1143546169 b=1317214663 c=22)=%s",
					a, b, c, tostring(a == 1143546169 and b == 1317214663 and c == 22))
			end)
			kh = okr and r or ("err:" .. tostring(r))
		end
		out("[XG-KH] " .. kh)
	end
end

-- ===== 汇总 =====
local ks = {}
for l in pairs(risk) do table.insert(ks, l) end
table.sort(ks)
out("[PREDICT] risk={" .. table.concat(ks, ",") .. "}")
out("[PREDICT] note: empty = hard-build gates A..Q expected to pass on this host;"
	.. " R/S/T/U are engine-internal gates and can only be observed via Suda.hard-trace.lua")
out("[END] Obsucvim GateProbe v2")

-- ===== 自动复制到剪贴板 =====
-- 复制内容 = 上面全部报告行（含 [PREDICT]/[END]）；本 [XG-CLIP] 状态行只打印、不进剪贴板。
local CLIPTEXT = table.concat(OUTBUF, "\n")
local clipfn, clipvia = nil, "none"
if type(setclipboard) == "function" then
	clipfn, clipvia = setclipboard, "setclipboard"
elseif type(toclipboard) == "function" then
	clipfn, clipvia = toclipboard, "toclipboard"
end
if clipfn then
	local okc, errc = pcall(clipfn, CLIPTEXT)
	print("[XG-CLIP] copied=" .. tostring(okc) .. " via=" .. clipvia .. " bytes=" .. tostring(#CLIPTEXT)
		.. (okc and "" or (" err=" .. tostring(errc))))
else
	print("[XG-CLIP] copied=false reason=no-clipboard-api (host has no setclipboard/toclipboard; copy the output manually)")
end
