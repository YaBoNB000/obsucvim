# Obsucvim Web — Windows Server 2019 x64 一键包

## 启动

1. 解压 ZIP 到一个有写权限的目录（例如 `C:\Obsucvim`）。
2. 双击 `Start-Obsucvim.cmd`。
3. 在服务器上先打开 `http://localhost:8787` 测试。
4. 从其他电脑访问 `http://<服务器内网或公网 IP>:8787`。若要从公网访问，请先配置 HTTPS 反向代理（IIS/ARR 等），**不要把明文 HTTP 登录直接暴露到公网**。

服务由内置 Node.js 22.22.3 x64 启动，Rust 引擎以 WASI Preview 1 在 Node 内运行；无需安装 Rust、Visual Studio、MSVC、Python 或额外的运行时。Web 静态文件与 `/api/*` 同源提供。

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

部署为 Windows Service 时，在服务管理器为服务配置上述环境变量，然后重启服务。用邮箱注册一次：真实 SMTP 模式下网页**不会显示验证码**，码只送到邮箱；`/api/health` 显示 `mail: "smtp"` 即已启用。若显示 `demo`，说明 SMTP_USER/SMTP_PASS 未进入 Node 进程环境。

在 163 邮箱网页设置中开启 SMTP 服务并生成客户端授权码。沙箱无法连外部 SMTP，所以本 ZIP 的真实外发没有在此环境实测；本地 SMTP 模拟器已验证 Nodemailer 发送、邮件验证码确认的整条程序路径。

## 额度与账户

- 每账号每日 5 次（服务端强制；00:00 本地服务器时区重置）
- 注册需人机验证与邮箱验证码
- 状态保存在 `data\state.json`（密码使用 scrypt 哈希，不存明文）
- 当前为单机状态文件；多进程/多服务器生产部署应迁移到数据库

## 包内文件/版本

- `dist/`：单文件 React/Vite 控制台与黑洞图片
- `tools/engine-server.mjs`：本地 Web + API + SMTP 服务
- `tools/wasi-runner.mjs`：Node WASI 运行器
- `tools/obsucvim.wasm`：Rust 引擎（WASI Preview 1）
- `runtime/node.exe`：Node.js v22.22.3 Windows x64（来自 npm `node-win-x64@22.22.3`）
- `tools/node-runtime-notice.txt`：Node 运行时许可说明

Node/obsucvim 版本信息详见 `BUILD-INFO.txt` 与 `SHA256SUMS.txt`。
