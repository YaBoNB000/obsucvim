# 升级已部署的 Obsucvim（保留账户与卡密）

升级只替换程序文件，**`data/` 目录原样保留**：账户、卡密、发布产物、额度都不会丢。

## 方式一：一条命令（推荐）

在服务器上用**管理员 PowerShell** 进入部署目录（例如 `C:\Obsucvim`），执行：

```powershell
cd C:\Obsucvim
.\Update-Obsucvim.ps1 -Sha256 "<新版 SHA-256>"
```

脚本会：下载更新器 → 下载新包 → 校验 SHA-256 → 校验包内清单 → 备份 `data/` → 覆盖程序文件。
完成后按提示重启服务：

```powershell
.\Start-Obsucvim.ps1 -LocalOnly
```

> 如果目录里还没有 `Update-Obsucvim.ps1`（旧版包没有这个文件），先手动取一次：

```powershell
curl.exe -L -o Update-Obsucvim.ps1 "https://raw.githubusercontent.com/YaBoNB000/obsucvim/arena/01a038eb-luraph/web/deploy/Update-Obsucvim.ps1"
```

## 方式二：直接用更新器

```powershell
cd C:\Obsucvim
curl.exe -L -o "$env:TEMP\self-update.mjs" "https://raw.githubusercontent.com/YaBoNB000/obsucvim/arena/01a038eb-luraph/web/deploy/self-update.mjs"
.\runtime\node.exe "$env:TEMP\self-update.mjs" --dir . --url "<zip 地址>" --sha256 "<SHA-256>"
```

也可以先用浏览器/`curl` 下载 zip 到本机，再离线升级：

```powershell
.\runtime\node.exe "$env:TEMP\self-update.mjs" --dir . --zip D:\Obsucvim-Web-Windows-x64.zip --sha256 "<SHA-256>"
```

## 更新器做了什么

1. 下载（或读取本地）安装包
2. 校验 ZIP 的 SHA-256 —— 不匹配立即中止，**不修改任何文件**
3. 解压到临时目录，按包内 `SHA256SUMS.txt` 逐个校验 —— 任何文件被改过都中止
4. 把现有 `data/` 完整复制到 `backup\<时间戳>\data`（回滚用）
5. 只覆盖程序文件（`dist/`、`tools/`、`runtime/`、启动脚本、文档）
6. 临时目录清理

## 安全约束

- 拒绝 `../`、绝对路径等越界条目，不会写到安装目录之外
- 永不写入 `data/`，升级前后账户与卡密不变
- 不读取也不修改环境变量（`SMTP_PASS`、Turnstile 密钥等仍在服务/进程环境里）
- 失败时安装目录保持原状（失败发生在覆盖阶段之前）

## 回滚

```powershell
# 停服务后
Copy-Item -Recurse -Force .\backup\<时间戳>\data\* .\data\
# 如需回退程序文件，重新解压对应旧版 ZIP 覆盖，再重启
```

## 当前版本

见 `BUILD-INFO.txt`。发布页同时提供同名 `.sha256` 文件，建议每次升级都带上 `--sha256`，避免下载到被替换的安装包。
