# Obsucvim Web — Windows Server 2019 x64 一键包

## 启动

1. 解压 ZIP 到一个有写权限的目录（例如 `C:\Obsucvim`）。
2. 双击 `Start-Obsucvim.cmd`。
3. 在服务器上先打开 `http://localhost:8787` 测试。
4. 从其他电脑访问 `http://<服务器内网或公网 IP>:8787`。若要从公网访问，请先配置 HTTPS 反向代理（IIS/ARR 等），**不要把明文 HTTP 登录直接暴露到公网**。

服务由内置 Node.js 22.22.3 x64 启动，Rust 引擎以 WASI Preview 1 在 Node 内运行；无需安装 Rust、Visual Studio、MSVC、Python 或额外的运行时。Web 静态文件与 `/api/*` 同源提供。登录会话使用 `HttpOnly; SameSite=Strict` Cookie，JavaScript 不再读取会话令牌；旧版可读会话升级后会失效一次，用户需重新登录。公开部署必须使用 HTTPS，HTTPS 请求会启用 Secure Cookie 与 HSTS。页面响应还包含哈希白名单 CSP、`frame-ancestors 'none'` 和 `X-Frame-Options: DENY`。

Caddy/IIS 在本机终止 TLS 时，启动脚本默认信任 loopback 代理 (`127.0.0.1,::1`) 并验证其 `X-Forwarded-Proto`；其他代理需将 `OBSUCVIM_TRUSTED_PROXY_IPS` 设为精确代理 IP。不要把 `X-Forwarded-For` 信任给任意来源。

## Windows 防火墙

默认不自动开防火墙端口。若确认需要允许 Domain/Private 网络入站，可在管理员 PowerShell 启动：

```powershell
.\Start-Obsucvim.ps1 -OpenFirewall
```

仅 localhost 测试：

```powershell
.\Start-Obsucvim.ps1 -LocalOnly
```

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

Node/obsucvim 版本信息详见 `BUILD-INFO.txt` 与 `SHA256SUMS.txt`。
