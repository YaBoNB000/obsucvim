-- ============================================================
-- 诊断脚本 A（纯明文，不混淆）：executor char 自举环境普查
-- 用途：在执行器里跑一次，把全部输出贴回来。
-- 安全：所有探测都经 pcall / coroutine.wrap 隔离，不报错、不挂起。
-- ============================================================
local OUT = {}
local function say(tag, msg)
	OUT[#OUT + 1] = tag .. ": " .. tostring(msg)
end

local HASCO = type(coroutine) == "table" and type(coroutine.wrap) == "function"
-- 调用任意函数测 f(65)：协程隔离（防 task.wait 类让出函数卡死）
local function probe65(f)
	if HASCO then
		local ok, r = pcall(coroutine.wrap(function() return f(65) end))
		if ok then return r end
		return "ERR:" .. tostring(r)
	else
		local ok, r = pcall(f, 65)
		if ok then return r end
		return "ERR:" .. tostring(r)
	end
end
-- ---- A1: 脚本环境基础原语存在性 --------------------------------
do
	local G = getfenv and getfenv(0) or _G
	local names = { "type","pairs","next","pcall","getfenv","getmetatable",
		"coroutine","string","utf8","loadstring","buffer","select",
		"rawget","rawset","tostring","tonumber" }
	local line = {}
	for i = 1, #names do
		local n = names[i]
		local v = rawget(G, n)
		if v == nil then
			local ok2, v2 = pcall(function() return G[n] end)
			if ok2 then v = v2 end
		end
		line[#line + 1] = n .. "=" .. type(v)
	end
	say("A1-basics", table.concat(line, " "))
end

-- ---- A2: getmetatable("") 形态 ---------------------------------
local MTOK, MT = pcall(getmetatable, "")
say("A2-getmetatable", MTOK and type(MT) or ("ERR " .. tostring(MT)))
if MTOK and type(MT) == "table" then
	local n = 0
	local keys = {}
	local pok = pcall(function()
		for k, v in pairs(MT) do
			n = n + 1
			if n <= 20 then keys[#keys + 1] = tostring(k) .. ":" .. type(v) end
		end
	end)
	say("A2-mt-pairs", (pok and "" or "ITER-ERR ") .. "count=" .. n
		.. " [" .. table.concat(keys, " ") .. "]")
	local nok, nk = pcall(next, MT)
	say("A2-mt-next", nok and tostring(nk) or "ERR")
	local ri = rawget(MT, "__index")
	say("A2-mt-rawget-index", type(ri))
end

-- ---- A3: 元表（及嵌套表）里的 char 候选 ------------------------
local function scantab(t, label)
	local fn = 0
	local found = {}
	local pok = pcall(function()
		for k, v in pairs(t) do
			if type(v) == "function" then
				fn = fn + 1
				if probe65(v) == "A" then found[#found + 1] = tostring(k) end
			end
		end
	end)
	say(label, (pok and "" or "ITER-ERR ") .. "funcs=" .. fn
		.. " char-like=" .. (#found > 0 and table.concat(found, ",") or "NONE"))
end
if MTOK and type(MT) == "table" then
	scantab(MT, "A3-mt-scan")
	local ri = rawget(MT, "__index")
	if type(ri) == "table" then scantab(ri, "A3-mt-index-scan") end
end

-- ---- A4: getfenv(0) 全局环境普查 --------------------------------
local EOK, ENV = pcall(getfenv, 0)
say("A4-getfenv0", EOK and type(ENV) or ("ERR " .. tostring(ENV)))
if EOK and type(ENV) == "table" then
	local total = 0
	local tabs = {}
	local pok = pcall(function()
		for k, v in pairs(ENV) do
			total = total + 1
			if type(v) == "table" then tabs[#tabs + 1] = k end
		end
	end)
	say("A4-env-pairs", (pok and "" or "ITER-ERR ") .. "total=" .. total
		.. " tables=" .. #tabs .. " [" .. table.concat(tabs, " ") .. "]")
	local nok, nk = pcall(next, ENV)
	say("A4-env-next", nok and tostring(nk) or "ERR")
	-- 逐表找 char 候选（最多扫 30 张表防慢）
	local scanned = 0
	for i = 1, #tabs do
		if scanned >= 30 then break end
		local tk = tabs[i]
		local okg, t = pcall(function() return ENV[tk] end)
		if okg and type(t) == "table" then
			scanned = scanned + 1
			scantab(t, "A4-tab-" .. tk)
		end
	end
	-- 不迭代、纯索引探测
	local idxnames = { "string", "utf8", "buffer", "table", "math" }
	for i = 1, #idxnames do
		local n = idxnames[i]
		local r1 = rawget(ENV, n)
		local okx, r2 = pcall(function() return ENV[n] end)
		say("A4-idx-" .. n, "rawget=" .. type(r1)
			.. " index=" .. (okx and type(r2) or "ERR"))
	end
end

-- ---- A5: 明文凭据直查（诊断脚本特权，守卫不能这么干） ----------
do
	local ok1, r1 = pcall(function() return string.char(65) end)
	say("A5-string-char65", ok1 and tostring(r1) or ("ERR " .. tostring(r1)))
	local ok2, r2 = pcall(function() return utf8.char(65) end)
	say("A5-utf8-char65", ok2 and tostring(r2) or ("ERR " .. tostring(r2)))
	local ok3, r3 = pcall(function() return loadstring("return 65")() end)
	say("A5-loadstring", ok3 and tostring(r3) or ("ERR " .. tostring(r3)))
	local ok4, r4 = pcall(function() return ("x").char end)
	say("A5-literal-index-char", ok4 and type(r4) or ("ERR " .. tostring(r4)))
end

-- ---- A6: loadstring 块视角（守卫碎片就活在这种块里） ------------
do
	local okc, f = pcall(loadstring,
		"return type(pcall) .. '|' .. type(pairs) .. '|' .. type(coroutine)"
		.. " .. '|' .. type(getmetatable) .. '|' .. type(getfenv)"
		.. " .. '|' .. type(string) .. '|' .. type(utf8)")
	if not okc or type(f) ~= "function" then
		say("A6-chunk-globals", "LOADSTR-ERR " .. tostring(f))
	else
		local okr, r = pcall(f)
		say("A6-chunk-globals", okr and r or ("CALL-ERR " .. tostring(r)))
	end
	-- loadstring 块的 getfenv(0) 与脚本环境是否同一张表
	local okd, fd = pcall(loadstring, "return getfenv(0)")
	if okd and type(fd) == "function" then
		local oke, fe = pcall(fd)
		say("A6-chunk-fenv-same", oke and tostring(fe == ENV) or "ERR")
	else
		say("A6-chunk-fenv-same", "LOADSTR-ERR")
	end
	-- loadstring 块内能否直接取到 char 行为函数
	local okf, ff = pcall(loadstring,
		"for k, v in pairs(getfenv(0)) do"
		.. " if type(v) == 'table' then"
		.. "  for k2, f in pairs(v) do"
		.. "   if type(f) == 'function' then"
		.. "    local ok, r = pcall(coroutine.wrap(function() return f(65) end))"
		.. "    if ok and r == 'A' then return k .. '.' .. k2 end"
		.. "   end"
		.. "  end"
		.. " end"
		.. " end return 'NONE'")
	if okf and type(ff) == "function" then
		local okg, fg = pcall(ff)
		say("A6-chunk-char-search", okg and tostring(fg) or ("ERR " .. tostring(fg)))
	else
		say("A6-chunk-char-search", "LOADSTR-ERR")
	end
end

-- ---- 输出 -------------------------------------------------------
for i = 1, #OUT do
	print(OUT[i])
end
print("DIAG-A-DONE")

-- ---- 复制到剪切板（执行器剪贴板 API 多形态兼容） -----------------
do
	local report = table.concat(OUT, "\n") .. "\nDIAG-A-DONE"
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
		-- clipboard.set 表形态
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
		-- 最后兜底：部分执行器把剪贴板函数藏在 getgenv() 里
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
