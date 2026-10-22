# 远程升级已部署的 Obsucvim（保留账户与卡密）

升级只替换程序文件，**`data\` 目录原样保留**：账户、卡密、发布产物、额度都不会丢，升级前还会整份备份到 `backup\<时间戳>\data`。

## 一条命令（远程升级，推荐）

在服务器上打开 **PowerShell**（管理员），把 `C:\Obsucvim` 换成你的实际安装目录：

```powershell
cd C:\Obsucvim; iwr -UseBasicParsing https://raw.githubusercontent.com/YaBoNB000/obsucvim/arena/01a038eb-luraph/web/deploy/Update-Obsucvim.ps1 -OutFile .\Update-Obsucvim.ps1; .\Update-Obsucvim.ps1
```

从 **cmd** 里跑同一条（不需要进 PowerShell 会话）：

```cmd
powershell -NoProfile -ExecutionPolicy Bypass -Command "cd C:\Obsucvim; iwr -UseBasicParsing https://raw.githubusercontent.com/YaBoNB000/obsucvim/arena/01a038eb-luraph/web/deploy/Update-Obsucvim.ps1 -OutFile .\Update-Obsucvim.ps1; .\Update-Obsucvim.ps1"
```

它会：取最新更新器 → 从官方发布页下载新包 → 校验 SHA-256（没手填就自动取发布页的同名 `.sha256`）→ 校验包内 `SHA256SUMS.txt` → 备份 `data\` → 覆盖程序文件。
完成后按提示重启：`.\Start-Obsucvim.bat`。

## 装好之后：菜单里的「更新」

新版启动器菜单第 6 项就是升级，不再需要带参数：

```cmd
Start-Obsucvim.bat
```

## 想固定版本（防下载到被替换的包）

```powershell
.\Update-Obsucvim.ps1 -Sha256 "<发布页 .sha256 文件里那串哈希>"
```

## 其他情形

只检查、不动文件：

```powershell
.\Update-Obsucvim.ps1 -DryRun
```

用自己的镜像 / 官方地址连不上（例如服务器走国内出口）：

```powershell
.\Update-Obsucvim.ps1 -Url "https://你的域名/Obsucvim-Web-Windows-x64.zip"
```

离线包（先把 ~34 MB 的 zip 传到服务器）：

```powershell
.\Update-Obsucvim.ps1 -Zip "D:\Obsucvim-Web-Windows-x64.zip" -Sha256 "<哈希>"
```

不走 PowerShell，直接调更新器：

```powershell
cd C:\Obsucvim
curl.exe -L -o "$env:TEMP\self-update.mjs" "https://raw.githubusercontent.com/YaBoNB000/obsucvim/arena/01a038eb-luraph/web/deploy/self-update.mjs"
.\runtime\node.exe "$env:TEMP\self-update.mjs" --dir .
```

## 更新器做了什么

1. 下载（或读取本地）安装包
2. 校验 ZIP 的 SHA-256 —— 不匹配立即中止，**不修改任何文件**
3. 解压到临时目录，按包内 `SHA256SUMS.txt` 逐个校验 —— 任何文件被改过都中止
4. 把现有 `data\` 完整复制到 `backup\<时间戳>\data`（回滚用）
5. 只覆盖程序文件（`dist\`、`tools\`、`runtime\`、启动脚本、文档）
6. 清理临时目录

## 关于「程序正在运行」

`runtime\node.exe` 是正在执行的程序，Windows 不允许直接改写它。更新器会自动把它改名让开（`node.exe.old-<时间戳>`）再写入新文件，所以**边跑边升级也不会留下半截文件**；下一次启动时旧文件会被清掉。

如果某个文件仍然写不进去（例如你开了两个窗口），更新器会列出是哪个文件并返回非 0，安装目录其余部分保持已更新的状态，**关掉占用进程后重跑同一条命令即可**。

## 安全约束

- 拒绝 `../`、绝对路径等越界条目，不会写到安装目录之外
- 永不写入 `data\`，升级前后账户与卡密不变
- 不读取也不修改环境变量（`SMTP_PASS`、Turnstile 密钥等仍在服务/进程环境里）
- 下载地址必须 https

## 回滚

```powershell
# 停服务后
Copy-Item -Recurse -Force .\backup\<时间戳>\data\* .\data\
# 如需回退程序文件，重新解压对应旧版 ZIP 覆盖，再重启
```

## 当前版本

见 `BUILD-INFO.txt`；发布页同时提供同名 `.sha256`，每次升级建议带上 `-Sha256`。
