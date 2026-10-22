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
local verdict, failure, status = post("/api/licenses/validate", {
	scriptId = SCRIPT_ID,
	key = key,
	hwid = hwid,
	nonce = requestNonce,
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
	else
		fail(code or "licence check failed")
	end
end
if verdict.nonce ~= requestNonce then
	fail("licence response mismatch")
end

local release
if verdict.token then
	release = select(1, post("/api/releases/fetch", { scriptId = SCRIPT_ID, token = verdict.token }))
else
	release = select(1, post("/api/releases/fetch", {
		scriptId = SCRIPT_ID, key = "", hwid = hwid, nonce = nonce(),
	}))
end
if type(release) ~= "table" or type(release.code) ~= "string" or #release.code < 16 then
	fail((release and (release.code or release.error)) or "release unavailable")
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
	version = release.version,
}

local chunk, compileError = loadstring(release.code)
if type(chunk) ~= "function" then
	fail("artifact failed to compile: " .. tostring(compileError))
end
return chunk()
