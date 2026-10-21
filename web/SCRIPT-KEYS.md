# 脚本授权 Key 验证（第一阶段草案）

> 当前实现的是项目与授权 Key 的签发、撤销、设备绑定和服务器验证 API。脚本上传、混淆发布版本和下载/执行 loader 还没有接入；验证成功只返回授权状态，不会返回脚本代码。

## 设计原则

- 每位客户使用**独立授权 Key**；loader 从 `getgenv().key`（不存在时回退 `_G.key`）读取。
- Key 由服务器生成，原文只在创建响应中返回一次；状态文件只保存 SHA-256 摘要，不保存明文 Key。
- Key 绑定某个 `scriptId`，可设置到期时间、撤销；首次验证时绑定客户端提供的 HWID 摘要，管理员可重置。
- 服务器 Secret、HMAC 密钥和 Turnstile Secret 都不进入客户端源码。
- HWID 是客户端提供的值，可以被伪造或因 executor 变化而改变；当前绑定只能提高滥用成本，不能作为可靠的硬件身份认证。

## 项目管理接口

项目管理接口需要登录 Obsucvim 账户，使用同源 HttpOnly Cookie。

### 创建项目

`POST /api/auth/scripts`

```json
{ "name": "My Luau Script", "description": "optional" }
```

返回服务器生成的 `scriptId`，格式类似 `scr_<hex>`。每个账户最多 50 个项目。

### 签发客户 Key

`POST /api/auth/licenses`

```json
{
  "scriptId": "scr_<script-id>",
  "label": "customer name",
  "expiresAt": "2027-01-01T00:00:00.000Z"
}
```

`expiresAt` 可省略或设为 `null` 表示不过期。响应中的 `licenseKey` 只显示这一次；请安全交付给客户并要求客户在自己的环境中设置 `getgenv().key`，不要把同一把 Key 写进公共脚本。

### 列出、撤销、重置设备绑定

- `GET /api/auth/scripts`：列出当前账户的项目。
- `GET /api/auth/licenses?scriptId=<script-id>`：列出 Key 的状态摘要，不返回 Key 原文或 HWID。
- `POST /api/auth/licenses/revoke`：`{ "licenseId": "lic_<id>" }`
- `POST /api/auth/licenses/reset-hwid`：`{ "licenseId": "lic_<id>" }`，下次成功验证时会重新绑定。

## 客户端验证接口

`POST /api/licenses/validate`，无需登录 Cookie，但需要客户 Key：

```json
{
  "scriptId": "scr_<script-id>",
  "key": "<customer-license-key>",
  "hwid": "<executor-provided-device-id>"
}
```

有效响应为 `{ "ok": true, "valid": true, ... }`。Key 错误、已撤销、过期或项目不匹配会统一返回 `LICENSE_INVALID`；同一 Key 换设备会返回 `DEVICE_MISMATCH`。验证端点按 IP 限流 60 次/分钟。

## 注入到用户源码的授权检查（已实现）

`tools/license-guard.luau` 是注入模板，`tools/inject-license.mjs` 负责填入项目 ID 与验证地址：

```bash
node tools/inject-license.mjs --source user.lua --script-id scr_<24 hex> --out guarded.lua
```

产物首部是授权检查，后面接用户原始代码；再把这个文件交给 v15 混淆，检查逻辑会与用户代码一起被保护。

注入后的行为：

- 从 `getgenv().key`（缺失时用 `_G.key`）读取客户 Key，缺失则立即报错退出。
- 用 `gethwid`/`syn.get_hwid` 取 HWID，取不到则拒绝运行。
- 使用 `request` / `http_request` / `http.request` / `syn.request` 发 POST 请求；不支持时拒绝运行，不会退回 GET。
- 请求带随机 nonce，响应必须同时满足：HTTP 200、`valid = true`、`scriptId` 一致、`nonce` 一致，否则报错。
- 只嵌入公开的 `scriptId` 和验证 URL；服务器 Secret、HMAC 密钥、Turnstile Secret 都不进入脚本。
- 通过后写入 `env.__obsucvim_license = { valid, expiresAt, checkedAt }`，供脚本后续判断。

限制与说明：

- 授权检查在脚本加载时执行一次。用户可在客户端改内存绕过本地检查；真正防止盗版仍要靠服务端在下载/发布端再次验证，这属于下一阶段。
- 模板保持较紧凑（约 2.7 KB）。早期约 6 KB 版本会让当前 WASI 混淆器崩溃（node 段错误）；当前版本已用完整 v15 参数混淆验证通过。
- HWID 由 executor 提供，可伪造、可变化；绑定只提高滥用成本。

## Luau loader 验证示意（旧版参考）

此示意只执行授权检查，不下载或执行远端代码；注入版已在上面实现，可直接使用。

```lua
local shared = (type(getgenv) == "function" and getgenv()) or _G
local licenseKey = shared and shared.key
if type(licenseKey) ~= "string" or licenseKey == "" then
    error("License key is required")
end

local deviceId = type(gethwid) == "function" and tostring(gethwid()) or ""
local httpRequest = request or http_request or (syn and syn.request)
if type(httpRequest) ~= "function" then
    error("This executor does not provide a supported HTTP POST API")
end

local HttpService = game:GetService("HttpService")
local response = httpRequest({
    Url = "https://obsucvim.l2.ink/api/licenses/validate",
    Method = "POST",
    Headers = { ["Content-Type"] = "application/json" },
    Body = HttpService:JSONEncode({
        scriptId = "scr_<script-id>",
        key = licenseKey,
        hwid = deviceId,
    }),
})

local status = tonumber(response.StatusCode or response.Status) or 0
local result = HttpService:JSONDecode(response.Body or "{}")
if status ~= 200 or result.valid ~= true then
    error("License verification failed")
end

print("License accepted")
```

## 下一阶段（托管流程）

- 上传源码后，服务端临时处理；混淆成功后删除原稿，只保存混淆版本和版本号。
- 发布的 loader 通过 `request`/`http_request`/`syn.request` POST 验证 Key，再向受保护的版本端点获取对应项目的混淆产物。
- 下载端点也必须重新验证授权；不能只依赖客户端的 `valid` 判断，因为客户端脚本可被修改。
- 当前为单机 JSON 状态文件；正式保存项目和大量授权记录前，应迁移至事务型数据库，并将版本文件存于独立数据目录。
