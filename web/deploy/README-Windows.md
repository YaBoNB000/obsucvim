# Obsucvim Web — Windows Server 2019 x64 一键包

## 启动

1. 解压 ZIP 到一个有写权限的目录（例如 `C:\Obsucvim`）。
2. 双击 **`Start-Obsucvim.bat`**。
3. 在服务器上先打开 `http://localhost:8787` 测试。
4. 从其他电脑访问时，请先配置 HTTPS 反向代理（Caddy/IIS/ARR），**不要把明文 HTTP 登录直接暴露到公网**。

`Start-Obsucvim.cmd` 仍然可用，它会转交给 `.bat`。

### 菜单选项

双击 `.bat` 会出现菜单：

| 选项 | 作用 |
|---|---|
| 1 | 仅本机启动，绑定 `127.0.0.1`，适合首次测试 |
| 2 | 允许网络访问（`0.0.0.0`），仅配合 HTTPS 反向代理使用 |
| 3 | 启动前用隐藏输入读取 163 客户端授权码，启用真实邮件 |
| 4 | 只做自检：文件是否齐全、端口是否被占用、数据与邮件状态 |
| 5 | 运行带校验的自更新 |
| 6 | 打开 `data` 目录 |
| 7 | 打开安装目录 |
| 8 | 退出 |

### 命令行参数

```bat
Start-Obsucvim.bat /local          仅本机
Start-Obsucvim.bat /public         允许网络访问
Start-Obsucvim.bat /smtp           提示输入授权码后启动
Start-Obsucvim.bat /check          只自检，不启动
Start-Obsucvim.bat /update         运行自更新
Start-Obsucvim.bat /open           启动后自动打开浏览器
Start-Obsucvim.bat /local 9090     指定端口
```

自检失败或端口被占用时会给出明确提示，不会静默退出。

### 启动器自己做了什么

- 校验 `runtime\node.exe`、`tools\engine-server.mjs`、`tools\obsucvim.wasm`、`dist\index.html` 是否存在，缺什么列什么
- 自动创建 `data\`（首次运行）
- 启动前检测端口占用，并提示占用进程 PID
- 设置全部运行时环境变量（引擎路径、数据目录、代理信任、SMTP 默认值）
- 只在你输入授权码时设置 `SMTP_PASS`，且不写盘
- 服务退出后显示退出码并保留窗口，方便看错误

服务由内置 Node.js 22.22.3 x64 启动，Rust 引擎以 WASI Preview 1 在 Node 内运行；无需安装 Rust、Visual Studio、MSVC、Python 或额外的运行时。Web 静态文件与 `/api/*` 同源提供。登录会话使用 `HttpOnly; SameSite=Strict` Cookie，JavaScript 不再读取会话令牌；旧版可读会话升级后会失效一次，用户需重新登录。公开部署必须使用 HTTPS，HTTPS 请求会启用 Secure Cookie 与 HSTS。页面响应还包含哈希白名单 CSP、`frame-ancestors 'none'` 和 `X-Frame-Options: DENY`。

Caddy/IIS 在本机终止 TLS 时，启动脚本默认信任 loopback 代理 (`127.0.0.1,::1`) 并验证其 `X-Forwarded-Proto`；其他代理需将 `OBSUCVIM_TRUSTED_PROXY_IPS` 设为精确代理 IP。不要把 `X-Forwarded-For` 信任给任意来源。

## 升级已部署的实例

见 `README-UPDATE.md`。一句话：`cd C:\Obsucvim` 后执行 `Update-Obsucvim.ps1 -Sha256 "<版本哈希>"`，程序文件被替换、`data/` 原样保留，然后重启服务。

## Windows 防火墙

默认不自动开防火墙端口。若确认需要允许 Domain/Private 网络入站，可在管理员 PowerShell 启动：

```powershell
.\Start-Obsucvim.ps1 -OpenFirewall
```

仅 localhost 测试：

```powershell
.\Start-Obsucvim.ps1 -LocalOnly
```

`Start-Obsucvim.ps1` 只是一层薄包装：负责可选的防火墙规则，然后把参数转交 `Start-Obsucvim.bat`，启动逻辑只有一处实现。

停止服务按 Ctrl+C。

## 接入真正 163 邮箱验证

不需要把密码发给我，也不要放进压缩包、Git、命令行参数或截图。 网易 163 邮箱：

- SMTP host：`smtp.163.com`
- Port：`465`（SSL/TLS）
- SMTP_USER / SMTP_FROM：`obsucvim@163.com`
- SMTP_PASS：163 邮箱设置中开启 SMTP 服务后生成的**客户端授权码**（不是网页登录密码）

SMTP 验证码需要从**启动服务的同一 PowerShell 进程环境**传给 Node。可双击启动进入演示模式；要在当前会话测试真实发信，可用：

```powershell
$env:SMTP_USER = "obsucvim@163.com"
$env:SMTP_FROM = "obsucvim@163.com"
$env:SMTP_HOST = "smtp.163.com"
$env:SMTP_PORT = "465"
$secure = Read-Host "163 邮箱客户端授权码" -AsSecureString
$ptr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
try { $env:SMTP_PASS = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($ptr) }
finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($ptr) }
.\Start-Obsucvim.ps1
```

部署为 Windows Service 时，在服务管理器为服务配置上述环境变量，然后重启服务。真实 SMTP 模式下网页**不会显示验证码**，码只发送到邮箱。服务启动控制台会打印 `smtp: configured`；若显示 `demo fallback`，说明 SMTP_USER/SMTP_PASS 未进入 Node 进程环境。产品不提供公开的健康检查或调试 API。

在 163 邮箱网页设置中开启 SMTP 服务并生成客户端授权码。沙箱无法连外部 SMTP，所以本 ZIP 的真实外发没有在此环境实测；本地 SMTP 模拟器已验证 Nodemailer 发送、邮件验证码确认的整条程序路径。

## 授权 Key 验证原型

当前版本包含项目与客户授权 Key 的服务端 API，以及注入到用户源码的授权检查：

- Key 原文仅在签发响应出现一次，服务器只保存摘要；首次验证绑定 executor 提供的 HWID 摘要，拥有者可撤销 Key 或重置绑定。
- `tools/inject-license.mjs` 会把 `tools/license-guard.luau` 插到用户源码首部，再从 `getgenv().key` 读取 Key 做服务端验证；混淆时检查逻辑与用户代码一起被保护。
- 网页端「脚本托管 → 发布版本 → 上传源码」可一键完成注入+混淆+发布，原稿不落盘；命令行等价用法：`node tools/inject-license.mjs --source user.lua --script-id scr_<id> --out guarded.lua`。
- 卡密支持三种有效期：永久、指定日期、首次激活后 N 天计时。
- 项目文件夹：一把 Key 解锁文件夹内全部脚本，适合做脚本 hub；支持批量签发（单次最多 100）与 CSV 导出。
- 可配置执行日志 / 告警 Webhook（仅限 https 的 Discord 地址）。
- 客户可用 `#key` 公开页查询卡密状态；项目开启后支持自助重置设备（带冷却）。
- 脚本内可读取 `OBS_IsPremium` / `OBS_UserNote` / `OBS_ScriptName` / `OBS_SecondsLeft` / `OBS_TotalExecutions` 做 UI 分层（可被客户端篡改，不可用于安全判断）。
- 尚未包含网页端上传/发布流水线和受授权保护的脚本下载端点；HWID 由客户端提供，可伪造，仅作尽力绑定。

接口、注入说明与 Luau 示例见 `SCRIPT-KEYS.md`。

## Cloudflare Turnstile（可选，启用托管人机验证）

1. 在 Cloudflare Turnstile 控制台为 `obsucvim.l2.ink` 创建 Widget，推荐 Managed 模式，取 Site Key 与 Secret Key。
2. 测试时在启动服务的 PowerShell 窗口设置（Site Key 是公开值；Secret 仅在本机私密输入）：

   ```powershell
   $env:CLOUDFLARE_TURNSTILE_SITE_KEY = "<Site Key>"
   $env:CLOUDFLARE_TURNSTILE_HOSTNAME = "obsucvim.l2.ink"
   $secure = Read-Host "Cloudflare Turnstile Secret Key" -AsSecureString
   $ptr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
   try { $env:CLOUDFLARE_TURNSTILE_SECRET_KEY = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($ptr) }
   finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($ptr) }
   $env:OBSUCVIM_TRUSTED_PROXY_IPS = "127.0.0.1,::1"
   .\Start-Obsucvim.ps1 -LocalOnly
   ```

   长期作为 Windows 服务运行时，把这三个 Turnstile 变量放进服务管理器的私密环境变量；不要把 Secret 写入 ZIP、Git、聊天或日志。
3. 重启服务。启动日志出现 `captcha: Cloudflare Turnstile` 即已切换。浏览器会加载 Cloudflare 验证组件；验证令牌由服务器发往 `https://challenges.cloudflare.com/turnstile/v0/siteverify` 校验，并随验证请求发送服务端解析出的访客 IP (`remoteip`)。

两把密钥都不设置时仍使用本地拼图；只设置其中一把时服务会 fail closed，不会静默退回较弱验证。请确保服务器和访客浏览器都能访问 Cloudflare Turnstile。

## 额度与账户

- 每个邮箱（不区分大小写）只能注册一个账户；每个 IP 只能完成一个新账户注册。
- 更新后完成注册的 IP 限制会保存为 HMAC 索引（不存原始 IP）；更新前的账户没有历史 IP，无法追溯补录。
- 注册与登录需拖动图片拼图完成服务端人机验证；同一 IP 每分钟最多签发 4 张挑战图，注册另需邮箱验证码。
- 升级后旧版浏览器可读会话会失效一次；账户资料保留，用户首次访问需重新登录。
- 每账号每日 5 次（服务端强制；00:00 本地服务器时区重置）。
- 状态保存在 `data\state.json`（密码使用 scrypt 哈希，不存明文）；备份此文件以保留账户与 IP 限制记录。
- 当前为单机状态文件；多进程/多服务器生产部署应迁移到共享数据库。
- 直连时使用 Node 收到的 TCP 对端 IP。若 IIS/ARR 等反向代理在本机，需要在启动环境变量中设置 `OBSUCVIM_TRUSTED_PROXY_IPS` 为精确代理 IP（例：`127.0.0.1,::1`），并确保代理覆盖或在 `X-Forwarded-For` 末尾追加真实客户端 IP；否则所有访客会共用代理 IP 的一个注册名额。不要信任任意外部 XFF。

## 包内文件/版本

- `dist/`：单文件 React/Vite 控制台与黑洞图片
- `tools/engine-server.mjs`：本地 Web + API + SMTP 服务
- `tools/wasi-runner.mjs`：Node WASI 运行器
- `tools/obsucvim.wasm`：Rust 引擎（WASI Preview 1）
- `runtime/node.exe`：Node.js v22.22.3 Windows x64（来自 npm `node-win-x64@22.22.3`）
- `tools/node-runtime-notice.txt`：Node 运行时许可说明
- `SCRIPT-KEYS.md`：脚本项目/授权 Key 验证 API 原型与 Luau 调用示例（尚未包含源码托管/loader 下载）

Node/obsucvim 版本信息详见 `BUILD-INFO.txt` 与 `SHA256SUMS.txt`。
