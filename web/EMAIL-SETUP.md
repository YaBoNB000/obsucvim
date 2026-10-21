# 接入 163 邮箱验证码（Windows Server 2019）

Web 后端已经实现 SMTP 发码。配置有效时，注册/重发验证码会由 `obsucvim@163.com` 发送，接口不返回 `demoCode`；未配置授权码时使用演示模式（验证码显示在页面上）。

## 1. 163 邮箱准备

登录 `obsucvim@163.com` 的 163 邮箱网页：

1. 进入 **设置 → POP3/SMTP/IMAP**（具体菜单名称可能随邮箱界面调整）。
2. 开启 SMTP 服务。
3. 按页面提示完成短信/安全验证并生成**客户端授权码**。
4. 复制授权码；这是 SMTP 密码，**不是邮箱网页登录密码**。

本项目默认连接参数：

- Host：`smtp.163.com`
- Port：`465`（SSL/TLS）
- User / From：`obsucvim@163.com`
- Password：163 邮箱客户端授权码

## 2. Windows Server 配置

在解压后的部署目录打开 PowerShell。只在当前启动进程中设置密码，不要发到聊天或提交到仓库：

```powershell
$env:SMTP_HOST = "smtp.163.com"
$env:SMTP_PORT = "465"
$env:SMTP_USER = "obsucvim@163.com"
$env:SMTP_FROM = "obsucvim@163.com"
$secure = Read-Host "163 邮箱客户端授权码" -AsSecureString
$ptr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
try { $env:SMTP_PASS = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($ptr) }
finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($ptr) }
.\Start-Obsucvim.ps1
```

密码只存在当前进程环境中（同机同权限进程可能读取）。长期以 Windows 服务运行时，应在服务管理器的私密环境变量中配置 SMTP_PASS，再重启服务；不要把它写进 `.ps1`、ZIP、Git、聊天或日志。SMTP 验证失败只会返回通用 `MAIL_SEND_FAILED`。

## 3. 确认配置生效

访问 `http://localhost:8787/api/health`，应看到 `"mail":"smtp"`。若是 `demo`，服务进程没有读到 SMTP_USER/SMTP_PASS，检查环境变量并重启。

用一个**尚未注册**的测试邮箱注册：真实邮件模式下页面不会显示演示码；验证码应送达邮箱，输入后即可完成注册。当前沙箱不能连接外部 163 SMTP，故我无法替你发送真实邮件或验证授权码；已通过本地 SMTP 模拟器验证真实发送代码路径和验证码确认逻辑。

## 4. 额度与服务器安全

- 每个账号每天 **5 次**，后端强制，按服务器本地时区午夜重置。
- 状态文件 `data\state.json` 保留用户、会话、额度；请定期备份并限制 Windows 文件 ACL。
- 单机状态文件适合单实例部署；多实例/高并发时迁移到数据库。
- 公网部署请放在 HTTPS 反向代理之后，并在 Windows/云防火墙中只开放需要的端口。
