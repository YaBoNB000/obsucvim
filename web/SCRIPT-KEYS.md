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

有效期有两种模式，二选一：

- `keyDays: 7` —— **相对到期**，首次成功激活才开始计时（可先囤货后出售）
- `expiresAt: "2027-01-01T00:00:00.000Z"` —— 绝对到期时间
- 两者都省略表示永久有效

```json
{ "scriptId": "scr_<id>", "label": "stock", "keyDays": 7 }
```

同时传 `keyDays` 和 `expiresAt` 会返回 `BAD_EXPIRY`。响应中的 `licenseKey` 只显示这一次；请安全交付给客户并要求客户在自己的环境中设置 `getgenv().key`，不要把同一把 Key 写进公共脚本。

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

### 一条龙发布（上传源码 → 注入 → 混淆 → 存储）

`POST /api/publish`（项目拥有者，登录 Cookie）

```json
{ "scriptId": "scr_<id>", "version": "v2", "source": "<原始 Luau 源码>" }
```

服务端会：注入卡密校验 → 用 v15 全保护混淆 → 校验产物 → 写入 `data/releases/<id>/<version>.lua`。
**原始源码只存在于内存和引擎临时目录，不落盘、不进状态文件，请求结束即删除。** 每次消耗 1 次每日额度。

**异步作业模型**（v2）：提交立刻返回 `202 { jobId, stage, progress, quota }`，混淆在后台继续；
前端轮询 `GET /api/publish/<jobId>`（需要登录，且只能查自己的作业）：

```json
{ "ok": true, "status": "processing|completed|failed", "stage": "queued|injecting|obfuscating|storing|completed",
  "progress": 0, "elapsedMs": 1234, "result": { "version", "sha256", "size" }, "error": "ENGINE_FAIL" }
```

- 额度在提交时**预留**，失败自动退还，不会出现「报错还扣次数」
- 同账号最多 2 个并发作业，超出返回 `409 PUBLISH_BUSY`
- 作业只在内存中，10 分钟后过期；服务重启后轮询会得到 `404 JOB_NOT_FOUND`，提示重试即可
- 作业不存在 / 不是自己的，一律返回同一个 404，避免撞库探测

错误码：`BAD_SOURCE`、`BAD_VERSION`、`RELEASE_LIMIT`、`RELEASE_TOO_LARGE`、`ENGINE_BUSY`、`ENGINE_TIMEOUT`、`ENGINE_FAIL`、`QUOTA_EXCEEDED`、`PUBLISH_BUSY`、`JOB_NOT_FOUND`。

### 直接上传已混淆产物

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

## 路线图与已定决策

### P1 接口

- `GET /api/auth/folders` / `POST /api/auth/folders` / `POST /api/auth/folders/update`
  - `update` 可改 `name`、`logWebhook`、`alertWebhook`
  - Webhook **只接受 https 且主机为 discord.com / discordapp.com**，防止被当作 SSRF 跳板
  - 日志与告警各自 5 秒最多一条，投递失败静默忽略，不影响验证
- 项目归属：`POST /api/auth/scripts` 增加 `folderId`；旧数据自动迁移到每个账户的 Default 文件夹
- 签发：`POST /api/auth/licenses` 支持 `folderId` + `count`（1–100）
  - 单个：返回 `{ licenseKey, license }`
  - 批量：返回 `{ count, licenses: [{ licenseKey, license }] }`
- 导出：`GET /api/auth/licenses/export?folderId=` 返回 text/csv 附件
- 验证：Key 现在按**文件夹**判定，同一文件夹内所有脚本共用一把 Key，跨文件夹无效

### 脚本托管（对齐 Luarmor 的核心能力）

**稳定 loader URL**

```
https://obsucvim.l2.ink/files/<scriptId>.lua
```

- 地址**永远不变**：重新发布只换服务端内容，用户不用改 loadstring
- 返回的是一个 bootstrap，里面只嵌 `scriptId` 与站点地址，不含任何密钥
- 用户只需一行：

```lua
loadstring(game:HttpGet("https://obsucvim.l2.ink/files/<scriptId>.lua"))()
```

- 未知或已停用的脚本返回 404；该端点按 IP 限流

**免卡密模式（FFA）**

`POST /api/auth/scripts/update` → `{ "scriptId": "scr_...", "ffa": true }`

- 开启后 `validate` 允许空 key，返回 `{ valid: true, free: true }`
- `runtime.premium = false`，脚本内可用 `OBS_IsPremium` 区分免费/付费功能
- 无 key 调用不绑定设备、不累计执行次数

**心跳复核（heartbeat）**

`POST /api/auth/scripts/update` → `{ "scriptId": "scr_...", "heartbeat": true }`

- 服务端把开关写进 loader（`local HEARTBEAT = true`），下次请求 `/files/<id>.lua` 即生效
- 开启后 loader 每 ~90 秒用同一把 key 重新校验一次，成功则刷新 `OBS_SecondsLeft` / `OBS_TotalExecutions`
- 复核失败（撤销/封禁/过期）：把 `OBS_License.valid` 置 `false`、写 `OBS_License.reason`，
  调用可选的 `getgenv().obsucvim_on_heartbeat_fail(code)`，并在控制台 `warn`（`silent` 时不 warn）
- **边界**：客户端无法可靠杀死不是自己创建的线程，所以心跳只能「通知 + 让脚本自行退出」。
  长驻脚本应自己检查 `OBS_License.valid`
- FFA（无 key）没有可复核的对象，心跳自动跳过

**静默模式（silent）**

字段保留（`silent` 会传给 loader），但我们的 loader 本身不打印任何内容，**没有可静默的输出**，
因此控制台不提供这个开关（Luarmor 的 silent 是针对它自己的控制台输出）。这一点在 UI 里有明确说明。

**卡密迁移**

- `GET /api/auth/licenses/export.json?folderId=` → 元数据（`licenseId`、备注、到期、设备绑定、封禁、执行数），**永不含卡密原文**（服务端只存 HMAC 摘要）
- `POST /api/auth/licenses/import` → `{ folderId, entries: [{ label, keyDays|expiresAt }] }`，一次最多 100 条，
  只生成**新卡密**并明文返回一次；任一条不合法则整批拒绝，不做半批导入

**封禁（完整版）**

`POST /api/auth/licenses/ban` → `{ licenseId, reason?, banDays? | banExpireAt? }`

- 不带时间 = 永久封禁；带 `banDays`（1–3650）或 `banExpireAt` = 到期自动解除
- 同时把该卡密当前绑定的设备写入设备黑名单，**换一把新卡密在同一台机器上同样被拒**
- 返回 `unbanToken`，用户可凭它自助解封
- `POST /api/auth/licenses/unban`（拥有者）或 `POST /api/key/unban`（`{ key, token }`，公开）
- 封禁会立刻让该卡密已签发的会话令牌失效

**分页与筛选**

`GET /api/auth/licenses?folderId=&from=&until=&search=&status=`

返回 `{ licenses, total, from, until }`；`status` 支持 `active` / `expired` / `revoked` / `banned`；`search` 匹配备注与卡密 ID。

### P2 接口

**Key Check API（无需登录，凭卡密本身鉴权）**

`POST /api/key/status` → `{ key: "osk_..." }`

返回 `state`（active/expired/revoked）、`note`、`scriptName`、`folderName`、`expiresAt`、`secondsLeft`、`deviceBound`、`totalExecutions`、`selfReset`、`resetCooldownDays`、`resetAvailableIn`。
不消耗 nonce、不计入执行次数，仅限流（与验证共享 60 次/分钟/IP）。卡密不存在统一返回 404 `LICENSE_INVALID`。

**自助重置设备**

`POST /api/key/reset-hwid` → `{ key }`

需要项目文件夹开启 `selfReset`，否则 403 `SELF_RESET_DISABLED`。冷却期内返回 429 `RESET_COOLDOWN` + `retryAfter`。
重置成功后清空设备绑定、记录 `lastHwidResetAt`，并向项目告警 Webhook 推送一条事件。

文件夹设置新增 `selfReset`（默认 `false`）与 `resetCooldownDays`（默认 7，0–365）。

**运行时变量（guard 注入）**

验证成功后 guard 在脚本环境里设置：

| 变量 | 含义 |
|---|---|
| `OBS_License` | 完整表：valid / expiresAt / premium / note / scriptName / folderName / secondsLeft / executions |
| `OBS_IsPremium` | 布尔，等价于授权有效 |
| `OBS_UserNote` | 卡密备注 |
| `OBS_ScriptName` | 脚本名 |
| `OBS_SecondsLeft` | 剩余秒数，永久卡密为 `math.huge` |
| `OBS_TotalExecutions` | 该卡密累计成功验证次数 |

**Key 缺失钩子**：guard 发现 `getgenv().key` 不存在时，会先调用 `getgenv().obsucvim_on_missing_key`（如果宿主提供了），再重新读取 `key`。这样脚本作者可以做"输入卡密"的 GUI，而不是让用户直接看到报错：

```lua
getgenv().obsucvim_on_missing_key = function()
    getgenv().key = showKeyPrompt()   -- 你自己实现的输入框
end
```

没有提供钩子、且脚本不是 FFA 时，才会报 `set getgenv().key before running this script`。

这些变量由客户端接收，**可被 hook 篡改**，只适合做 UI 分层或提示，不要用于安全判断。

已完成：

- P0-1 一条龙发布：网页端上传源码 → 服务端注入 guard → v15 混淆 → 存盘；原稿不落盘。端点 `POST /api/publish`。
- P0-2 相对到期：`keyDays` 首次激活才计时，可与 `expiresAt`（绝对日期）二选一。

待办：

| 优先级 | 项目 | 说明 |
|---|---|---|
| ✅ | 项目文件夹 | 已完成：Key 属于文件夹，一个 Key 解锁文件夹内所有脚本 |
| ✅ | 批量生成 Key + CSV 导出 | 已完成：单次最多 100 个；CSV 只含元数据，永远不含明文 Key |
| ✅ | 执行日志 / 告警 Webhook | 已完成：仅接受 https 的 Discord webhook |
| ✅ | 稳定 loader URL | 已完成：`GET /files/<scriptId>.lua`，地址永不变 |
| ✅ | 免卡密模式（FFA） | 已完成：脚本级开关，keyless 可运行 |
| ✅ | 封禁体系 | 已完成：永久/临时封禁 + 设备黑名单 + 解封 token |
| ✅ | 卡密分页 / 搜索 | 已完成：from/until/search/status |
| ✅ | 用户自助页 | 已完成：`#key` 公开页，查状态 + 自助重置设备 |
| ✅ | 运行时变量 | 已完成：guard 注入 `OBS_*` 变量 |
| ✅ | Key Check API | 已完成：`POST /api/key/status` |

**已取消（用户决定，不实现）**：

- ~~Discord 机器人~~ —— 取消，人工操作保留在网页控制台。
- ~~广告奖励系统（Linkvertise / Lootlabs / Work.ink）与反绕过~~ —— 取消，不做广告变现。

长期注意事项：

- 当前为单机 JSON 状态文件 + 本地文件目录；正式多实例部署应迁移到事务型数据库和对象存储。
- 客户端始终可能被改内存绕过；能守住的边界是“没有有效授权就拿不到产物”，而不是“客户端一定诚实执行检查”。


## 托管 v2 新增接口（Phase 2–6）

### 批量与审计

```text
POST /api/auth/publish/batch      { folderId, version, source, scriptIds? }   # ≤5 个脚本，每个独立 jobId
POST /api/auth/licenses/batch     { action: ban|unban|reset-hwid|revoke, licenseIds[], reason?, banDays? }
GET  /api/auth/audit?limit=50     # 最近操作（IP 只存 HMAC 前 16 位）
GET  /api/auth/scripts            # 现在额外返回 releaseCount / latestVersion / lastPublishedAt / licenseCount
```

### Key Check（Luarmor 兼容）

```text
POST /api/key/check               { scriptId, key, hwid }  → { code, message, data? }
GET  /library.lua                 → { script_id, check_key, load_script, purge_cache }
```

状态码：`KEY_VALID` `KEY_EXPIRED` `KEY_BANNED` `KEY_HWID_LOCKED` `KEY_INCORRECT` `KEY_INVALID`
`KEY_LOCKED` `SCRIPT_ID_INVALID` `SCRIPT_ID_INCORRECT` `INVALID_EXECUTOR` `SECURITY_ERROR`
`TIME_ERROR` `UNKNOWN_ERROR`。`check_key` 是只读的（不绑定设备、不累计执行次数）。

```lua
local api = loadstring(game:HttpGet("https://obsucvim.l2.ink/library.lua"))()
api.script_id = "scr_..."
local status = api.check_key(getgenv().key)
if status.code == "KEY_VALID" then
	print("剩余秒数", status.data.auth_expire - os.time())
	api.load_script()
else
	print(status.code, status.message)
end
```

### 指标与投递日志

```text
GET /api/auth/metrics?days=7          # [{ date, validations, rejected, executions, devices }]
GET /api/auth/webhooks/deliveries     # 最近 80 条投递结果（含失败原因）
```

Webhook 现在带签名与重试：

```text
POST <你的 discord webhook>
X-Obsucvim-Signature: sha256=<HMAC-SHA256(文件夹 secret, 原始 body)>
```

失败按 250/500ms 退避重试 3 次，结果写入投递日志。

### 受保护的 webhook（脚本内发送）

```text
POST /api/auth/webhooks        { folderId, name, url, template, sanitizers: { name: "[A-Za-z0-9_]{3,20}" } }
GET  /api/auth/webhooks        # 只回 host 与变量名，URL 永不回显
POST /api/auth/webhooks/delete { webhookId }
POST /api/webhooks/send        { token: "lst_...", webhookId, values: { name: "Player_1" } }
```

模板里可用 `{{name}}`（客户端传入，必须匹配该变量的正则）与
`%SCRIPT_NAME%` `%SCRIPT_ID%` `%CLIENT_IP%` `%USER_NOTE%` `%LICENSE_ID%` `%TOTAL_EXECUTIONS%` `%COUNTRY_CODE%`（服务端渲染，客户端改不了）。
限制：30 req/min/IP、最多 3 个 embeds、渲染后 ≤7000 字节。

### 外部验卡（第三方应用）

```text
GET /api/auth/external-apps        POST { name } → { appId, secret }（secret 仅此一次）
GET /external/sync                 → { st, nodes, algorithm: "hmac-sha256" }
GET /external/check_key?by=<appId>&key=<卡密>
    headers: clienttime, clientnonce, clienthwid, externalsignature
    externalsignature = HMAC-SHA256(secret, clientnonce|key|clienttime|clienthwid) 的 hex
```

与 Luarmor 的 SHA-1 方案相比这里用 HMAC-SHA256，并且响应同样带
`signature = HMAC-SHA256(secret, clientnonce|code)`，客户端可验签防篡改。`clienttime` 与服务器相差超过 5 分钟返回 `TIME_ERROR`。
