--[[
  Obsucvim license guard v3 (injected at the top of user source before obfuscation)
  tools/inject-license.mjs fills in the public project id and the validate URL.
  No secret is embedded. A blank key is allowed only for Free for All scripts;
  the server decides. If getgenv().key is missing the guard first calls
  getgenv().obsucvim_on_missing_key (optionally) so the host can show a key GUI.
]]
do
	local guardScript = "__OBSUCVIM_SCRIPT_ID__"
	local guardUrl = "__OBSUCVIM_ENDPOINT__"

	local env = _G
	if type(getgenv) == "function" then
		local ok, value = pcall(getgenv)
		if ok and type(value) == "table" then env = value end
	end

	local licenseKey = env.key
	if type(licenseKey) ~= "string" then
		local hook = env.obsucvim_on_missing_key
		if type(hook) == "function" then
			pcall(hook)
			licenseKey = env.key
		end
	end
	if type(licenseKey) ~= "string" then licenseKey = "" end

	local hwid = ""
	local hwidFn = env.gethwid or _G.gethwid
	if type(hwidFn) == "function" then
		local ok, value = pcall(hwidFn)
		if ok and type(value) == "string" and #value >= 8 then hwid = value end
	end
	if hwid == "" and type(env.syn) == "table" and type(env.syn.get_hwid) == "function" then
		local ok, value = pcall(env.syn.get_hwid)
		if ok and type(value) == "string" and #value >= 8 then hwid = value end
	end

	local requestFn = env.request or env.http_request or _G.request or _G.http_request
	if type(requestFn) ~= "function" and type(env.http) == "table" then
		requestFn = env.http.request
	end
	if type(requestFn) ~= "function" and type(env.syn) == "table" then
		requestFn = env.syn.request
	end
	if type(requestFn) ~= "function" then
		error("[Obsucvim] this executor exposes no HTTP POST API", 0)
	end

	local gameObject = _G.game
	if type(gameObject) ~= "table" then
		error("[Obsucvim] game object unavailable", 0)
	end
	local http = gameObject:GetService("HttpService")
	local seed = tostring(os.time()) .. tostring(math.floor(os.clock() * 1000000))
	local nonce = string.sub(seed .. string.rep("0", 32), 1, 32)
	local payload = http:JSONEncode({
		scriptId = guardScript,
		key = licenseKey,
		hwid = hwid,
		nonce = nonce,
	})
	local ok, response = pcall(requestFn, {
		Url = guardUrl,
		Method = "POST",
		Body = payload,
		Headers = { ["Content-Type"] = "application/json" },
	})
	if not ok or type(response) ~= "table" then
		error("[Obsucvim] license server unreachable", 0)
	end
	local body = response.Body or response.body
	if type(body) ~= "string" then
		error("[Obsucvim] license server returned no body", 0)
	end
	local decoded = nil
	local decodeOk, value = pcall(function() return http:JSONDecode(body) end)
	if decodeOk then decoded = value end
	if type(decoded) ~= "table" or decoded.valid ~= true or decoded.scriptId ~= guardScript then
		local code = "UNKNOWN"
		if type(decoded) == "table" then code = decoded.code or decoded.error or code end
		if code == "KEY_BANNED" then
			local reason = decoded and decoded.banReason
			if type(reason) == "string" and #reason > 0 then
				error("[Obsucvim] license banned: " .. reason, 0)
			end
			error("[Obsucvim] license banned", 0)
		elseif code == "KEY_HWID_LOCKED" or code == "DEVICE_MISMATCH" then
			error("[Obsucvim] license is bound to another device", 0)
		elseif code == "KEY_MISSING" then
			error("[Obsucvim] set getgenv().key before running this script", 0)
		elseif code == "HWID_MISSING" then
			error("[Obsucvim] this executor exposes no HWID function", 0)
		elseif code == "KEY_EXPIRED" then
			error("[Obsucvim] license expired", 0)
		elseif code == "KEY_LOCKED" or code == "RATE_LIMITED" then
			error("[Obsucvim] too many attempts, retry shortly", 0)
		elseif code == "NONCE_REPLAY" then
			error("[Obsucvim] license response replayed", 0)
		elseif code == "SCRIPT_NOT_FOUND" then
			error("[Obsucvim] script is not published", 0)
		end
		error("[Obsucvim] license invalid or revoked", 0)
	end
	if decoded.nonce ~= nonce then
		error("[Obsucvim] license response mismatch", 0)
	end
	local runtime = decoded.runtime
	if type(runtime) ~= "table" then runtime = {} end
	local license = {
		valid = true,
		free = decoded.free == true,
		expiresAt = decoded.expiresAt,
		checkedAt = os.time(),
		premium = runtime.premium == true,
		note = tostring(runtime.note or ""),
		scriptName = tostring(runtime.scriptName or ""),
		folderName = tostring(runtime.folderName or ""),
		secondsLeft = tonumber(runtime.secondsLeft) or math.huge,
		executions = tonumber(runtime.totalExecutions) or 0,
	}
	env.__obsucvim_license = license
	env.OBS_License = license
	env.OBS_IsPremium = license.premium
	env.OBS_UserNote = license.note
	env.OBS_ScriptName = license.scriptName
	env.OBS_SecondsLeft = license.secondsLeft
	env.OBS_TotalExecutions = license.executions
end
