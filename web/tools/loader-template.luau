--[[
  Obsucvim hosted loader (template)
  Distributed by the script owner. Fill in __SCRIPT_ID__ and keep the URL on https.
  Flow: read getgenv().key + HWID -> POST /api/licenses/validate -> POST /api/releases/fetch
        -> loadstring the obfuscated artifact.
  The artifact itself is only released after the server accepts the license, so a
  modified loader still cannot download the script without a valid key.
]]
local SCRIPT_ID = "__SCRIPT_ID__"
local BASE = "https://obsucvim.l2.ink"

local env = _G
if type(getgenv) == "function" then
	local ok, value = pcall(getgenv)
	if ok and type(value) == "table" then env = value end
end

local key = env.key
if type(key) ~= "string" or #key < 16 then
	error("[Obsucvim] set getgenv().key before running the loader", 0)
end

local hwidFn = env.gethwid or _G.gethwid or (type(env.syn) == "table" and env.syn.get_hwid) or nil
if type(hwidFn) ~= "function" then
	error("[Obsucvim] this executor exposes no HWID function", 0)
end
local hwid = hwidFn()
if type(hwid) ~= "string" or #hwid < 8 then
	error("[Obsucvim] invalid HWID", 0)
end

local requestFn = env.request or env.http_request or _G.request or _G.http_request
if type(requestFn) ~= "function" and type(env.http) == "table" then requestFn = env.http.request end
if type(requestFn) ~= "function" and type(env.syn) == "table" then requestFn = env.syn.request end
if type(requestFn) ~= "function" then
	error("[Obsucvim] this executor exposes no HTTP POST API", 0)
end

local gameObject = _G.game
if type(gameObject) ~= "table" then
	error("[Obsucvim] game object unavailable", 0)
end
local http = gameObject:GetService("HttpService")

local function post(path, payload)
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
	local body = response.Body or response.body
	if type(body) ~= "string" then
		return nil, "empty response"
	end
	local decoded
	local decodedOk, value = pcall(function() return http:JSONDecode(body) end)
	if decodedOk then decoded = value end
	if type(decoded) ~= "table" then
		return nil, "malformed response (" .. tostring(status) .. ")"
	end
	return decoded, nil, status
end

local seed = tostring(os.time()) .. tostring(math.floor(os.clock() * 1000000))
local nonce = string.sub(seed .. string.rep("0", 32), 1, 32)

local verdict, failure = post("/api/licenses/validate", {
	scriptId = SCRIPT_ID,
	key = key,
	hwid = hwid,
	nonce = nonce,
})
if not verdict or verdict.valid ~= true or verdict.scriptId ~= SCRIPT_ID or verdict.nonce ~= nonce then
	error("[Obsucvim] " .. tostring((verdict and verdict.error) or failure or "license invalid"), 0)
end

local release, releaseFailure, releaseStatus = post("/api/releases/fetch", {
	scriptId = SCRIPT_ID,
	token = verdict.token,
})
if not release or type(release.code) ~= "string" or #release.code < 16 then
	error("[Obsucvim] " .. tostring((release and release.error) or releaseFailure or ("HTTP " .. tostring(releaseStatus))), 0)
end

local chunk, compileError = loadstring(release.code)
if type(chunk) ~= "function" then
	error("[Obsucvim] artifact failed to compile: " .. tostring(compileError), 0)
end
return chunk()
