-- ============================================================
-- 诊断脚本 H（纯明文，不混淆）：executor 宿主语义深挖（守卫 H 门）
-- 针对现象：直接索引 game["GetService"] 能拿到，守卫读取函数拿不到
-- 全探测隔离（不报错不挂起），跑完自动复制报告到剪切板
-- ============================================================
local OUT = {}
local function say(tag, msg)
	OUT[#OUT + 1] = tag .. ": " .. tostring(msg)
end
say("DIAG", "H-v1")

-- ---- H1: 裸全局同一性（脚本上下文的 pcall/type/rawget 与环境里的是不是同一个）
do
	local ENV = nil
	local ok0, e0 = pcall(getfenv, 0)
	if ok0 and type(e0) == "table" then ENV = e0 end
	local gg = nil
	if ENV then
		local okx, gx = pcall(function() return ENV["getgenv"] end)
		if okx then gg = gx end
	end
	local CENV = nil
	if type(gg) == "function" then
		local okc, ce = pcall(gg)
		if okc and type(ce) == "table" then CENV = ce end
	end
	say("H1-env", "ENV=" .. type(ENV) .. " CENV=" .. type(CENV))
	local function idx(t, k)
		if t == nil then return nil end
		local ok, v = pcall(function() return t[k] end)
		if ok then return v end
		return nil
	end
	local pE, pC = idx(ENV, "pcall"), idx(CENV, "pcall")
	local tE, tC = idx(ENV, "type"), idx(CENV, "type")
	local rE, rC = idx(ENV, "rawget"), idx(CENV, "rawget")
	say("H1-pcall", "script=" .. type(pcall) .. " ENV=" .. type(pE) .. " CENV=" .. type(pC)
		.. " script==ENV:" .. tostring(pcall == pE)
		.. " script==CENV:" .. tostring(pcall == pC)
		.. " ENV==CENV:" .. tostring(pE == pC))
	say("H1-type", "script=" .. type(type) .. " ENV=" .. type(tE) .. " CENV=" .. type(tC)
		.. " script==ENV:" .. tostring(type == tE)
		.. " script==CENV:" .. tostring(type == tC)
		.. " ENV==CENV:" .. tostring(tE == tC))
	say("H1-rawget", "script=" .. type(rawget) .. " ENV=" .. type(rE) .. " CENV=" .. type(rC)
		.. " script==ENV:" .. tostring(rawget == rE)
		.. " script==CENV:" .. tostring(rawget == rC)
		.. " ENV==CENV:" .. tostring(rE == rC))
end

-- ---- H2: game 的三个来源 + 类型 -----------------------------------
local gS, gE, gC = nil, nil, nil
do
	gS = game
	local ok0, e0 = pcall(getfenv, 0)
	local ENV = ok0 and e0 or nil
	if type(ENV) == "table" then
		local ok1, v1 = pcall(function() return ENV["game"] end)
		if ok1 then gE = v1 end
		local gg = nil
		local ok2, v2 = pcall(function() return ENV["getgenv"] end)
		if ok2 then gg = v2 end
		if type(gg) == "function" then
			local ok3, ce = pcall(gg)
			if ok3 and type(ce) == "table" then
				local ok4, v4 = pcall(function() return ce["game"] end)
				if ok4 then gC = v4 end
			end
		end
	end
	say("H2-types", "gS=" .. type(gS) .. " gE=" .. type(gE) .. " gC=" .. type(gC))
	local ts = typeof and typeof(gS) or "no-typeof"
	local te = typeof and typeof(gE) or "no-typeof"
	local tc = typeof and typeof(gC) or "no-typeof"
	say("H2-typeof", "gS=" .. tostring(ts) .. " gE=" .. tostring(te) .. " gC=" .. tostring(tc))
	say("H2-same", "S==E:" .. tostring(gS == gE) .. " S==C:" .. tostring(gS == gC) .. " E==C:" .. tostring(gE == gC))
end

-- ---- H3: 每个 game 候选的 GetService 三姿势矩阵 ------------------
local function gs_probe(g, label)
	if g == nil then
		say("H3-" .. label, "nil")
		return
	end
	local rr, ri, rc
	local ok1, v1 = pcall(rawget, g, "GetService")
	rr = ok1 and type(v1) or ("ERR:" .. tostring(v1))
	local ok2, v2 = pcall(function() return g["GetService"] end)
	ri = ok2 and type(v2) or ("ERR:" .. tostring(v2))
	local ok3, v3 = pcall(function() return g:GetService("Players") end)
	rc = ok3 and type(v3) or ("ERR:" .. tostring(v3))
	local cls = "nil"
	if ok3 and v3 ~= nil then
		local ok4, v4 = pcall(function() return v3.ClassName end)
		if ok4 then cls = tostring(v4) end
	end
	say("H3-" .. label, "rawget=" .. tostring(rr) .. " index=" .. tostring(ri)
		.. " call=" .. tostring(rc) .. " ClassName=" .. cls)
end
gs_probe(gS, "gS")
gs_probe(gE, "gE")
gs_probe(gC, "gC")

-- ---- H4: loadstring 块视角（守卫碎片真实生存环境） ----------------
do
	local src = [[
return function(sp, st, sr)
	local r = {}
	r.tg = type(game)
	r.ttg = tostring(typeof and typeof(game))
	r.pe = tostring(pcall == sp)
	r.te = tostring(type == st)
	r.re = tostring(rawget == sr)
	local ok1, v1 = pcall(function() return game["GetService"] end)
	r.gsi = ok1 and type(v1) or ("ERR:" .. tostring(v1))
	local ok2, v2 = pcall(function() return game:GetService("Players") end)
	r.gsc = ok2 and type(v2) or ("ERR:" .. tostring(v2))
	local ok3, v3 = pcall(rawget, game, "GetService")
	r.gsr = ok3 and type(v3) or ("ERR:" .. tostring(v3))
	return r
end
]]
	local okf, maker = pcall(loadstring, src)
	if not okf or type(maker) ~= "function" then
		say("H4-chunk", "LOADSTR-ERR " .. tostring(maker))
	else
		local okm, f = pcall(maker)
		if not okm or type(f) ~= "function" then
			say("H4-chunk", "MAKE-ERR " .. tostring(f))
		else
			local okr, r = pcall(f, pcall, type, rawget)
			if okr and type(r) == "table" then
				say("H4-chunk", "type(game)=" .. tostring(r.tg)
					.. " typeof=" .. tostring(r.ttg)
					.. " pcall==脚本:" .. tostring(r.pe)
					.. " type==脚本:" .. tostring(r.te)
					.. " rawget==脚本:" .. tostring(r.re))
				say("H4-gs", "rawget=" .. tostring(r.gsr)
					.. " index=" .. tostring(r.gsi)
					.. " call=" .. tostring(r.gsc))
			else
				say("H4-chunk", "RUN-ERR " .. tostring(r))
			end
		end
	end
end

-- ---- 输出 -------------------------------------------------------
for i = 1, #OUT do
	print(OUT[i])
end
print("DIAG-H-DONE")

-- ---- 复制到剪切板 -------------------------------------------------
do
	local report = table.concat(OUT, "\n") .. "\nDIAG-H-DONE"
	local G = getfenv and getfenv(0) or _G
	local clipnames = { "setclipboard", "toclipboard", "copyclipboard",
		"writeclipboard" }
	local done = false
	for i = 1, #clipnames do
		local nm = clipnames[i]
		local okf, fn = pcall(function() return G[nm] end)
		if okf and type(fn) == "function" then
			local okc = pcall(fn, report)
			if okc then
				print("CLIPBOARD-OK: " .. nm)
				done = true
				break
			end
		end
	end
	if not done then
		local okt, ct = pcall(function() return G.clipboard end)
		if okt and type(ct) == "table" then
			local oks, fn = pcall(function() return ct.set end)
			if oks and type(fn) == "function" then
				local okc = pcall(fn, report)
				if okc then
					print("CLIPBOARD-OK: clipboard.set")
					done = true
				end
			end
		end
	end
	if not done then
		local okg, genv = pcall(function() return G.getgenv and G.getgenv() end)
		if okg and type(genv) == "table" then
			for i = 1, #clipnames do
				local okf, fn = pcall(function() return genv[clipnames[i]] end)
				if okf and type(fn) == "function" then
					local okc = pcall(fn, report)
					if okc then
						print("CLIPBOARD-OK: getgenv()." .. clipnames[i])
						done = true
						break
					end
				end
			end
		end
	end
	if not done then
		print("CLIPBOARD-NONE: 无可用剪贴板 API，请从控制台手动复制上面输出")
	end
end
