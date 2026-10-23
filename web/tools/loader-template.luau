--[[
  Obsucvim hosted loader (served from /files/<scriptId>.lua, URL never changes)
  ------------------------------------------------------------------
  __SCRIPT_ID__ is filled in by the server. Nothing secret is embedded: the
  artifact is only released after the server accepts the licence, so editing
  this file still cannot download the script without a valid key.
  Scripts in Free for All mode work with no key at all.
]]

local SCRIPT_ID = "__SCRIPT_ID__"
local BASE = "__BASE_URL__"

-- Filled in from the project's script settings.
local HEARTBEAT = __HEARTBEAT__
local SILENT = __SILENT__

local env = _G
if type(getgenv) == "function" then
	local ok, value = pcall(getgenv)
	if ok and type(value) == "table" then env = value end
end

local function fail(message)
	error("[Obsucvim] " .. tostring(message), 0)
end

-- In Roblox `game` is a userdata (an Instance), not a table; a few executors
-- expose a table instead. Accept both, and only insist that GetService works.
local gameObject = rawget(_G, "game")
if type(gameObject) ~= "table" and type(gameObject) ~= "userdata" then gameObject = env.game end
if type(gameObject) ~= "table" and type(gameObject) ~= "userdata" then
	fail("game object unavailable")
end
local okService, http = pcall(function()
	return gameObject:GetService("HttpService")
end)
if not okService or type(http) ~= "table" and type(http) ~= "userdata" then
	fail("HttpService unavailable")
end

local function post(path, payload)
	local requestFn = env.request or env.http_request or rawget(_G, "request") or rawget(_G, "http_request")
	if type(requestFn) ~= "function" and type(env.http) == "table" then requestFn = env.http.request end
	if type(requestFn) ~= "function" and type(env.syn) == "table" then requestFn = env.syn.request end
	if type(requestFn) ~= "function" then
		return nil, "this executor exposes no HTTP POST API"
	end
	local ok, response = pcall(requestFn, {
		Url = BASE .. path,
		Method = "POST",
		Body = http:JSONEncode(payload),
		Headers = { ["Content-Type"] = "application/json" },
	})
	if not ok or type(response) ~= "table" then
		return nil, "server unreachable"
	end
	local status = tonumber(response.StatusCode or response.Status or response.status) or 0
	local raw = response.Body or response.body
	if type(raw) ~= "string" then
		return nil, "empty response (" .. tostring(status) .. ")"
	end
	local decoded
	local okDecode, value = pcall(function() return http:JSONDecode(raw) end)
	if okDecode then decoded = value end
	if type(decoded) ~= "table" then
		return nil, "malformed response (" .. tostring(status) .. ")"
	end
	return decoded, nil, status
end

-- Ask the host for a key when none was provided. The host can show a GUI here
-- and set getgenv().key; returning nothing simply means "run without a key",
-- which only works for Free for All scripts.
local key = env.key
if type(key) ~= "string" then
	local hook = env.obsucvim_on_missing_key
	if type(hook) == "function" then
		pcall(hook)
		key = env.key
	end
end
if type(key) ~= "string" then key = "" end
-- Copy/paste from the dashboard often brings spaces, tabs or newlines along,
-- and executors happily keep them. Keys never contain whitespace.
key = (string.gsub(key, "%s+", ""))

-- Catch a wrong-shaped key here instead of sending it and getting a generic
-- server error: users paste plain 32-character keys, licence ids, or the whole
-- `getgenv().key = "..."` line surprisingly often.
local function keyProblem(value)
	if value == "" then return nil end
	local digits = string.match(value, "^[Oo][Ss][Kk]_(%x+)$")
	if not digits then
		if string.match(value, "^lic_") then
			return "that looks like a licence id (lic_...), not a key (osk_...)"
		end
		return "a key starts with osk_ followed by 64 hex characters, but this one does not"
	end
	if #digits ~= 64 then
		return "a key has 64 hex characters after osk_, but this one has " .. tostring(#digits)
	end
	return nil
end
local keyIssue = keyProblem(key)
if keyIssue then
	fail("key format looks wrong: " .. keyIssue)
end
if key ~= "" then key = string.lower(key) end

local hwidFn = env.gethwid or rawget(_G, "gethwid")
if type(hwidFn) ~= "function" and type(env.syn) == "table" then hwidFn = env.syn.get_hwid end
local hwid = ""
if type(hwidFn) == "function" then
	local ok, value = pcall(hwidFn)
	if ok and type(value) == "string" then hwid = value end
end

local function nonce()
	local seed = tostring(os.time()) .. tostring(math.floor(os.clock() * 1000000))
	return string.sub(seed .. string.rep("0", 32), 1, 32)
end

local requestNonce = nonce()
-- attest.luau 总会先跑（服务端在每次 /files/<id>.lua 里拼在最前面）。
-- 这里只把结论当数据带上去，不参与任何放行判断。
local attestReport = env.OBS_Attest
local attestedFlag = env.OBS_Attested == true
local attestedScore = 0
local attestedTotal = 0
local attestedMissing = ""
if type(attestReport) == "table" then
	attestedScore = tonumber(attestReport.score) or 0
	attestedTotal = tonumber(attestReport.total) or 0
	attestedMissing = tostring(attestReport.missing or "")
	if attestReport.ok == true then attestedFlag = true end
end

local verdict, failure, status = post("/api/licenses/validate", {
	scriptId = SCRIPT_ID,
	key = key,
	hwid = hwid,
	nonce = requestNonce,
	attested = attestedFlag,
	attestScore = attestedScore,
	attestTotal = attestedTotal,
	attestMissing = attestedMissing,
})
if not verdict or verdict.valid ~= true then
	local code = verdict and (verdict.code or verdict.error) or failure
	if code == "KEY_BANNED" then
		local reason = verdict and verdict.banReason
		fail("licence banned" .. (type(reason) == "string" and #reason > 0 and (": " .. reason) or ""))
	elseif code == "KEY_HWID_LOCKED" then
		fail("this licence is bound to another device, reset it first")
	elseif code == "KEY_MISSING" then
		fail("no key provided: set getgenv().key before running the loader")
	elseif code == "KEY_EXPIRED" then
		fail("licence expired")
	elseif code == "KEY_INCORRECT" or code == "KEY_REVOKED" then
		fail("licence invalid or revoked")
	elseif code == "KEY_INVALID" then
		fail("key format looks wrong: expected osk_ followed by 64 hex characters")
	elseif code == "KEY_LOCKED" then
		fail("too many failed attempts from this address, retry in a few minutes")
	elseif code == "SCRIPT_ID_INVALID" or code == "SCRIPT_ID_INCORRECT" or code == "SCRIPT_NOT_FOUND" then
		fail("this loader points at a script that no longer exists")
	elseif code == "HWID_INVALID" then
		fail("this executor returned an unusable HWID (empty or longer than 256 characters)")
	elseif code == "NONCE_INVALID" then
		fail("internal: the loader sent a malformed nonce, please re-run")
	elseif code == "RATE_LIMITED" then
		fail("too many licence checks, retry in a minute")
	else
		fail(code or "licence check failed")
	end
end
if verdict.nonce ~= requestNonce then
	fail("licence response mismatch")
end

--[[ 变体选择：每个版本都有两份产物
	hard = 完整加固（引擎级执行器证明 + Roblox 环境绑定），只有在这台宿主真的
	       具备那套 API 时才取，否则一加载就自己崩掉；
	safe = 兼容版，任何人任何执行器都能跑。
	判断依据是上面证明层的实测结果（不是用户手填），谎报只会拿到硬版本然后崩在自己身上。 ]]
local attestChecks = type(attestReport) == "table" and attestReport.checks or nil
local wantsHard = false
if type(attestChecks) == "table" then
	wantsHard = attestChecks["executor.debug.depth"] == true
		and attestChecks["executor.identifyexecutor.answers"] == true
		and attestChecks["executor.classifies"] == true
		and attestChecks["roblox.Vector3.new"] == true
		and attestChecks["roblox.typeof"] == true
		and attestChecks["roblox.task"] == true
		and attestChecks["roblox.game.GetService"] == true
end

local release
local requestedVariant = wantsHard and "hard" or "safe"
if verdict.token then
	release = select(1, post("/api/releases/fetch", {
		scriptId = SCRIPT_ID, token = verdict.token, variant = requestedVariant,
	}))
else
	release = select(1, post("/api/releases/fetch", {
		scriptId = SCRIPT_ID, key = "", hwid = hwid, nonce = nonce(), variant = requestedVariant,
	}))
end
-- 硬版本取不到时回落到兼容版，绝不让用户卡死在「没有产物」。
if type(release) ~= "table" or type(release.code) ~= "string" then
	if wantsHard then
		notify("hardened build unavailable, using the compatible build")
		if verdict.token then
			release = select(1, post("/api/releases/fetch", {
				scriptId = SCRIPT_ID, token = verdict.token, variant = "safe",
			}))
		else
			release = select(1, post("/api/releases/fetch", {
				scriptId = SCRIPT_ID, key = "", hwid = hwid, nonce = nonce(), variant = "safe",
			}))
		end
	end
end
if type(release) ~= "table" or type(release.code) ~= "string" or #release.code < 16 then
	local code = release and (release.error or release.code)
	if code == "RELEASE_NOT_FOUND" then
		fail("this script has no published version yet")
	elseif code == "RELEASE_UNREADABLE" then
		fail("the published file is missing on the server, ask the owner to re-publish")
	elseif code == "LICENSE_INVALID" or code == "INVALID_SESSION" then
		fail("licence session expired, run the script again")
	else
		fail(code or "release unavailable")
	end
end

local function notify(message)
	if SILENT then return end
	local printer = rawget(_G, "warn") or rawget(_G, "print")
	if type(printer) == "function" then pcall(printer, "[Obsucvim] " .. tostring(message)) end
end

local runtime = verdict.runtime
if type(runtime) ~= "table" then runtime = {} end
env.__obsucvim_license = {
	valid = true,
	premium = runtime.premium == true,
	note = tostring(runtime.note or ""),
	scriptName = tostring(runtime.scriptName or ""),
	folderName = tostring(runtime.folderName or ""),
	secondsLeft = tonumber(runtime.secondsLeft) or math.huge,
	executions = tonumber(runtime.totalExecutions) or 0,
	expiresAt = tonumber(runtime.expiresAt) or nil,
	version = release.version,
}

local licence = env.__obsucvim_license
env.OBS_License = licence
env.OBS_IsPremium = licence.premium
env.OBS_UserNote = licence.note
env.OBS_ScriptName = licence.scriptName
env.OBS_FolderName = licence.folderName
env.OBS_SecondsLeft = licence.secondsLeft
env.OBS_TotalExecutions = licence.executions
env.OBS_ScriptVersion = licence.version
env.OBS_ExpiresAt = licence.expiresAt

--[[ Heartbeat: re-check the licence in the background so a revoked or banned
  key does not keep running forever. A client cannot reliably kill threads it
  did not create, so this only marks the licence invalid, calls the optional
  obsucvim_on_heartbeat_fail hook and warns; long-running scripts should check
  OBS_License.valid themselves.
  Free for All runs have no key, so there is nothing to re-check. ]]
if HEARTBEAT and key ~= "" then
	local sleeper
	if type(task) == "table" and type(task.wait) == "function" then
		sleeper = task.wait
	elseif type(wait) == "function" then
		sleeper = wait
	elseif type(env.task) == "table" and type(env.task.wait) == "function" then
		sleeper = env.task.wait
	end

	local function heartbeatTick()
		local again = select(1, post("/api/licenses/validate", {
			scriptId = SCRIPT_ID, key = key, hwid = hwid, nonce = nonce(),
		}))
		local licence = env.__obsucvim_license
		if type(again) == "table" and again.valid == true then
			local fresh = again.runtime
			if type(fresh) == "table" and type(licence) == "table" then
				licence.premium = fresh.premium == true
				licence.secondsLeft = tonumber(fresh.secondsLeft) or licence.secondsLeft
				licence.executions = tonumber(fresh.totalExecutions) or licence.executions
				env.OBS_SecondsLeft = licence.secondsLeft
				env.OBS_TotalExecutions = licence.executions
			end
			return true
		end
		local code = "HEARTBEAT_FAILED"
		if type(again) == "table" then code = tostring(again.code or again.error or code) end
		if type(licence) == "table" then
			licence.valid = false
			licence.reason = code
		end
		env.OBS_License = licence
		local hook = env.obsucvim_on_heartbeat_fail
		if type(hook) == "function" then pcall(hook, code) end
		notify("licence heartbeat failed: " .. tostring(code))
		return false
	end

	local function heartbeatLoop()
		while true do
			local slept, sleepError = pcall(sleeper, 90)
			if not slept then return end -- the host stopped yielding; stop quietly
			local licence = env.__obsucvim_license
			if type(licence) == "table" and licence.valid == false then return end
			if not heartbeatTick() then return end
		end
	end

	if sleeper then
		if type(task) == "table" and type(task.spawn) == "function" then
			pcall(task.spawn, heartbeatLoop)
		elseif type(env.task) == "table" and type(env.task.spawn) == "function" then
			pcall(env.task.spawn, heartbeatLoop)
		elseif type(coroutine) == "table" and type(coroutine.create) == "function" then
			pcall(function()
				local co = coroutine.create(heartbeatLoop)
				coroutine.resume(co)
			end)
		end
	end
end

local chunk, compileError = loadstring(release.code)
if type(chunk) ~= "function" then
	fail("artifact failed to compile: " .. tostring(compileError))
end
return chunk()
