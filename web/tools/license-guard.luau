-- Obsucvim license guard v2 (injected at the top of user source before obfuscation)
-- tools/inject-license.mjs fills in the public project id and the validate URL.
-- The key itself is never embedded; the loader reads getgenv().key at runtime.
do
	local guardScript = "__OBSUCVIM_SCRIPT_ID__"
	local guardUrl = "__OBSUCVIM_ENDPOINT__"

	local env = _G
	if type(getgenv) == "function" then
		local ok, value = pcall(getgenv)
		if ok and type(value) == "table" then env = value end
	end

	local licenseKey = env.key
	if type(licenseKey) ~= "string" or #licenseKey < 16 then
		error("[Obsucvim] set getgenv().key before running this script", 0)
	end

	local hwidFn = env.gethwid or _G.gethwid
	local hwid = nil
	if type(hwidFn) == "function" then
		local ok, value = pcall(hwidFn)
		if ok and type(value) == "string" and #value >= 8 then hwid = value end
	end
	if hwid == nil then
		error("[Obsucvim] this executor exposes no HWID function", 0)
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
	local status = tonumber(response.StatusCode or response.Status or response.status) or 0
	local body = response.Body or response.body
	if type(body) ~= "string" then
		error("[Obsucvim] license server returned no body", 0)
	end
	local decoded = nil
	local decodeOk, value = pcall(function() return http:JSONDecode(body) end)
	if decodeOk then decoded = value end
	if type(decoded) ~= "table" or decoded.valid ~= true or decoded.scriptId ~= guardScript then
		local code = type(decoded) == "table" and decoded.error or "HTTP_" .. tostring(status)
		if code == "DEVICE_MISMATCH" then
			error("[Obsucvim] license is bound to another device", 0)
		end
		error("[Obsucvim] license invalid or expired", 0)
	end
	if decoded.nonce ~= nonce then
		error("[Obsucvim] license response mismatch", 0)
	end
	env.__obsucvim_license = { valid = true, expiresAt = decoded.expiresAt, checkedAt = os.time() }
end
