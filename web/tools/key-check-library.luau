--[[ Obsucvim key-check library (mirrors the Luarmor call shape)
  local api = loadstring(game:HttpGet("__BASE_URL__/library.lua"))()
  api.script_id = "scr_..."
  local status = api.check_key(script_key)   -- { code, message, data? }
  if status.code == "KEY_VALID" then api.load_script() end
]]
local BASE = "__BASE_URL__"
local env = _G
if type(getgenv) == "function" then
	local ok, value = pcall(getgenv)
	if ok and type(value) == "table" then env = value end
end
local function request()
	local fn = env.request or env.http_request or rawget(_G, "request") or rawget(_G, "http_request")
	if type(fn) ~= "function" and type(env.http) == "table" then fn = env.http.request end
	if type(fn) ~= "function" and type(env.syn) == "table" then fn = env.syn.request end
	return fn
end
local function hwid()
	local fn = env.gethwid or rawget(_G, "gethwid")
	if type(fn) ~= "function" and type(env.syn) == "table" then fn = env.syn.get_hwid end
	if type(fn) ~= "function" then return "" end
	local ok, value = pcall(fn)
	if ok and type(value) == "string" then return value end
	return ""
end
local api = {}
local scriptId = nil
setmetatable(api, {
	__newindex = function(t, k, v)
		if k == "script_id" then
			if type(v) ~= "string" then error("[Obsucvim] script_id must be a string", 0) end
			if not string.match(v, "^scr_[%da-f]+$") or #v ~= 28 then
				error("[Obsucvim] script_id is not a valid script id", 0)
			end
			scriptId = v
			rawset(t, "script_id", v)
			return
		end
		rawset(t, k, v)
	end,
})
local function post(path, payload)
	local fn = request()
	if not fn then return nil, "INVALID_EXECUTOR" end
	local gameObject = rawget(_G, "game")
	local ok, response = pcall(fn, {
		Url = BASE .. path,
		Method = "POST",
		Body = select(2, pcall(function() return gameObject:GetService("HttpService"):JSONEncode(payload) end)) or "",
		Headers = { ["Content-Type"] = "application/json" },
	})
	if not ok or type(response) ~= "table" then return nil, "UNKNOWN_ERROR" end
	local raw = response.Body or response.body
	if type(raw) ~= "string" then return nil, "UNKNOWN_ERROR" end
	local decoded
	local decodeOk, value = pcall(function()
		return gameObject:GetService("HttpService"):JSONDecode(raw)
	end)
	if decodeOk then decoded = value end
	if type(decoded) ~= "table" then return nil, "UNKNOWN_ERROR" end
	return decoded
end
function api.check_key(key)
	if type(scriptId) ~= "string" then
		return { code = "SCRIPT_ID_INVALID", message = "set api.script_id first" }
	end
	if type(key) ~= "string" or key == "" then
		return { code = "KEY_INVALID", message = "key is empty" }
	end
	local result, failure = post("/api/key/check", { scriptId = scriptId, key = key, hwid = hwid() })
	if type(result) ~= "table" then
		return { code = failure or "UNKNOWN_ERROR", message = "request failed" }
	end
	return {
		code = tostring(result.code or "UNKNOWN_ERROR"),
		message = tostring(result.message or ""),
		data = result.data,
	}
end
function api.load_script()
	if type(scriptId) ~= "string" then error("[Obsucvim] set api.script_id first", 0) end
	return loadstring(game:HttpGet(BASE .. "/files/" .. scriptId .. ".lua"))()
end
function api.purge_cache()
	-- 我们的 loader 不在本地留缓存文件，这里只保留 Luarmor 同名接口以便迁移脚本。
	return true
end
return api

