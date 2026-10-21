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

请求必须带 16–64 位随机 `nonce`。有效响应包含 `valid: true`、回显的 `nonce`，以及一个 2 小时有效的 `token`（`lst_...`）。错误语义：

| 情况 | HTTP | error |
|---|---|---|
| Key 不存在 / 已撤销 / 已过期 / 项目不符 | 403 | `LICENSE_INVALID` |
| 同一 Key 换设备 | 403 | `DEVICE_MISMATCH` |
| `nonce` 在 5 分钟内被重复使用 | 409 | `NONCE_REPLAY` |
| 同一客户端对同一项目连续猜错 Key | 429 | `LICENSE_LOCKED` |

安全设计要点：

- Key 使用服务端 HMAC 密钥计算摘要存储，状态文件不含明文 Key；比较使用 `timingSafeEqual`，避免时序侧信道。
- `nonce` 服务端去重并保留 5 分钟，防止重放已捕获的合法请求。
- 猜错计数按「客户端 IP + 项目」统计，并且**只对未通过 Key 校验的请求生效**；合法 Key 不会被别人的失败尝试锁死（避免 DoS）。达到 10 次后该组合封禁 10 分钟。
- 验证端点整体另有限流 60 次/分钟/IP。
- 公开错误码不区分「Key 不存在」与「Key 错误」，避免枚举预言机。

## 受授权保护的发布与下载

真实防盗版的关键在这里：**混淆产物只在授权通过后才下发**。

`POST /api/auth/releases`（项目拥有者，登录 Cookie）

```json
{ "scriptId": "scr_<id>", "version": "v1", "code": "<已混淆产物>" }
```

产物写入 `data/releases/<scriptId>/<version>.lua`，状态文件只保存版本、SHA-256、大小和路径；单个项目最多 20 个版本，单文件上限 4 MB。

`POST /api/releases/fetch`（客户端）

```json
{ "scriptId": "scr_<id>", "token": "lst_..." }
```

也接受 `{ scriptId, key, hwid, nonce }` 现场验证。返回最新版本的 `code`、`version`、`sha256`。Key 被撤销后，之前签发的 token 立即失效。

`tools/loader-template.luau` 是配套 loader：校验 Key → 拿 token → 拉取产物 → `loadstring` 执行；没有有效授权时**连产物都拿不到**。

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
- 已实现受保护下载端点 `/api/releases/fetch`；仍需网页端上传/发布界面（目前用 API 或脚本调用）。
- 待办：把“上传源码 → 注入 → 混淆 → 发布”串成一条服务端流水线，并在发布时自动删除原稿。
- 当前为单机 JSON 状态文件 + 本地文件目录；正式多实例部署应迁移到事务型数据库和对象存储。
- 客户端始终可能被改内存绕过；能守住的边界是“没有有效授权就拿不到产物”，而不是“客户端一定诚实执行检查”。
