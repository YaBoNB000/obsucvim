--[[
  Obsucvim attestation — ALWAYS served in front of the hosted loader.
  --------------------------------------------------------------------------
  Proves at runtime that the code runs on a real Roblox client through a real
  executor, and records what it found. Every probe is pcall'd; a missing API is
  recorded as "absent", never thrown.

  Hard rules for this file:
    * never raises, never errors, never blocks the script;
    * the verdict is data (getgenv().OBS_Attested / OBS_Attest), not a gate;
    * it is injected by the server on every /files/<id>.lua request, with no
      switch to turn it off.

  It lives in the loader (plain text) rather than inside the obfuscated artifact
  on purpose: the WASI engine segfaults when the injected preamble grows more
  nested functions, and the loader is never obfuscated. See
  docs/HOSTING-V2-PLAN.md — "engine preamble cliff".
]]

local OBS_Attest_Report = { ok = false, score = 0, total = 0, missing = {}, checks = {}, details = {}, source = "loader" }
do
	local env = _G
	pcall(function()
		if type(getgenv) == "function" then
			local current = getgenv()
			if type(current) == "table" then env = current end
		end
	end)

	local function get(name)
		local ok, value = pcall(function()
			local holder = env[name]
			if holder == nil then holder = rawget(_G, name) end
			return holder
		end)
		if ok then return value end
		return nil
	end

	local function note(name, passed, detail)
		OBS_Attest_Report.total = OBS_Attest_Report.total + 1
		-- checks 里只放布尔值：Lua 里 0 是 truthy，混进数字会让调用方判断出错。
		if passed then
			OBS_Attest_Report.score = OBS_Attest_Report.score + 1
			OBS_Attest_Report.checks[name] = true
		else
			OBS_Attest_Report.checks[name] = false
			OBS_Attest_Report.missing[#OBS_Attest_Report.missing + 1] = name
			if detail ~= nil then OBS_Attest_Report.details[name] = detail end
		end
		return passed
	end

	local function kindOf(value)
		local ok, kind = pcall(function()
			local fn = get("typeof")
			if type(fn) == "function" then return fn(value) end
			return type(value)
		end)
		if ok and type(kind) == "string" then return kind end
		return type(value)
	end

	------------------------------------------------------------------
	-- 1. Executor surface
	------------------------------------------------------------------
	local executorApi = {
		"getgenv", "gethwid", "identifyexecutor", "checkcaller",
		"iscclosure", "islclosure", "newcclosure", "hookfunction",
		"getrawmetatable", "loadstring",
	}
	for _, name in ipairs(executorApi) do
		note("executor." .. name, type(get(name)) == "function")
	end

	local identify = get("identifyexecutor")
	if type(identify) == "function" then
		local ok, name = pcall(identify)
		if ok and type(name) == "string" and #name > 0 and #name < 128 then
			OBS_Attest_Report.executor = name
		end
	end
	note("executor.identifyexecutor.answers", OBS_Attest_Report.executor ~= nil)

	local hwidFn = get("gethwid")
	local hwid = ""
	if type(hwidFn) == "function" then
		local ok, value = pcall(hwidFn)
		if ok and type(value) == "string" and #value >= 8 and #value <= 256 then hwid = value end
	end
	note("executor.gethwid.answers", hwid ~= "")

	local isC, isL = get("iscclosure"), get("islclosure")
	local classifies = false
	if type(isC) == "function" and type(isL) == "function" then
		local okC, cResult = pcall(isC, print)
		local probeFunction = function() return 1 end
		local okL, lResult = pcall(isL, probeFunction)
		classifies = okC and cResult == true and okL and lResult == true
	end
	note("executor.classifies", classifies)

	local debugLib = get("debug")
	local debugDepth = 0
	if type(debugLib) == "table" then
		local probes = { "getconstants", "getupvalues", "setupvalue", "getproto", "getprotos" }
		for _, name in ipairs(probes) do
			local ok, fn = pcall(function() return debugLib[name] end)
			if ok and type(fn) == "function" then debugDepth = debugDepth + 1 end
		end
	end
	note("executor.debug.depth", debugDepth >= 2, debugDepth)

	------------------------------------------------------------------
	-- 2. Roblox environment
	------------------------------------------------------------------
	local game = get("game")
	local gameKind = kindOf(game)
	note("roblox.game", gameKind == "Instance" or type(game) == "userdata" or type(game) == "table", gameKind)
	note("roblox.typeof", type(get("typeof")) == "function")

	local vec3 = get("Vector3")
	local vec3Ok = false
	if type(vec3) == "table" and type(vec3.new) == "function" then
		local ok, made = pcall(vec3.new, 1, 2, 3)
		vec3Ok = ok and made ~= nil and kindOf(made) == "Vector3"
	end
	note("roblox.Vector3.new", vec3Ok)

	local vec2 = get("Vector2")
	local vec2Ok = false
	if type(vec2) == "table" and type(vec2.new) == "function" then
		local ok, made = pcall(vec2.new, 1, 2)
		vec2Ok = ok and made ~= nil and kindOf(made) == "Vector2"
	end
	note("roblox.Vector2.new", vec2Ok)

	local taskLib = get("task")
	note("roblox.task", type(taskLib) == "table" and type(taskLib.wait) == "function"
		and type(taskLib.spawn) == "function" and type(taskLib.defer) == "function")

	local players = get("Players")
	note("roblox.Players", players ~= nil and (kindOf(players) == "Instance" or type(players) == "userdata"
		or type(players) == "table"))

	local instance = get("Instance")
	note("roblox.Instance.new", type(instance) == "table" and type(instance.new) == "function")

	local workspaceObject = get("workspace")
	note("roblox.workspace", workspaceObject ~= nil and (kindOf(workspaceObject) == "Instance"
		or type(workspaceObject) == "userdata" or type(workspaceObject) == "table"))

	local serviceOk = false
	if game ~= nil then
		local ok, service = pcall(function()
			local getter = game.GetService
			if type(getter) ~= "function" then return nil end
			return getter(game, "Players")
		end)
		serviceOk = ok and service ~= nil
	end
	note("roblox.game.GetService", serviceOk)

	------------------------------------------------------------------
	-- 3. Luau runtime the VM itself depends on
	------------------------------------------------------------------
	note("luau.buffer", type(get("buffer")) == "table")
	note("luau.bit32", type(get("bit32")) == "table")

	------------------------------------------------------------------
	-- Verdict (data only — nothing here decides whether the script runs)
	------------------------------------------------------------------
	OBS_Attest_Report.ok = OBS_Attest_Report.executor ~= nil and hwid ~= "" and serviceOk and vec3Ok
	OBS_Attest_Report.rating = OBS_Attest_Report.total > 0
		and (OBS_Attest_Report.score / OBS_Attest_Report.total) or 0

	pcall(function()
		env.OBS_Attest = OBS_Attest_Report
		env.OBS_Attested = OBS_Attest_Report.ok == true
	end)
end
