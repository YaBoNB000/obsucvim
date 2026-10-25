# Discord `/obf` → Render 免费 Web 服务

> 实现状态（2026-10-07）：代码和本地测试完成，**尚未部署到 Render，尚未做真实 Discord 联调**。
> 本环境不能访问 Discord / Render，也没有 Docker，因此没有声称已验证容器构建或平台免费额度。
> 免费套餐是否可用、是否要求绑卡、资源/出站流量限制，以部署时平台显示为准；不接受付费升级就不要确认付费套餐。

## 这版提供什么

```text
/obf file:<上传的 .lua 或 .luau>
  → Discord 签名 HTTP 请求
  → 立即 defer（仅自己可见的“处理中”）
  → 单工作线程下载附件、调用 Rust obsucvim
  → 编辑原始回复，附上 obfuscated.luau
```

- 默认仅允许 `ALLOWED_USER_IDS` 中的用户，配置为空会拒绝启动，不会变成公开服务。
- 固定 `--preset v15 --dialect luau`，每次随机 64 位 seed。不开放任意 CLI 参数。
- 新构建引擎回传的产物第一行为 `-- This file was obfuscated by Obsucvim.`（2026-10-08）；由 Rust 输出层统一添加，服务无需再次添加。
- 输入可用 Lua / Luau 语法，但**当前引擎输出仅支持 Luau，不支持 Lua 5.1 目标**。
- 最大输入 256 KiB，最大结果 7 MiB；单任务混淆最长 90 秒、子进程地址空间上限 256 MiB。
- 同时只混淆一个任务，最多三个等待；每用户一个未完成任务，提交间隔至少 30 秒。
- **只编译混淆，不运行源码或结果。** 不安装 Lua 解释器，不测试上传代码的运行结果。
- 不需要 Gateway 长连接、Message Content Intent、公网 IP、自己的域名或网页。
- 运行中的 HTTP 服务**不需要 Bot Token**。Public Key 用于验签；Bot Token 只在本地注册命令时用。

## 1. 创建 Discord Application

在浏览器打开 Discord Developer Portal：<https://discord.com/developers/applications>。

1. 创建一个 Application，例如 `obsucvim`。
2. 在 **General Information** 记录 **Application ID** 和 **Public Key**。
3. Discord 客户端中开启「设置 → 高级 → 开发者模式」，右键自己的头像 → 复制用户 ID。
4. 准备一个测试服务器；复制其服务器 ID，供下一步注册测试命令。
5. 在应用的 Installation / OAuth2 设置中启用服务器安装（Guild Install），用 `applications.commands` scope 安装到测试服务器。只做 HTTP 命令不需要管理员权限；如 UI 同时添加 `bot` scope，也不要给 Administrator 权限。

此模式不通过 Gateway 上线，因此 Bot 没有在线状态并不代表 `/obf` 不可用。

**不要把 Bot Token 发到聊天、截图、仓库、Render 日志或命令行参数里。** Token 和 Public Key 是两个不同的东西。

## 2. 将当前代码放到 GitHub 工作分支

Render 从远端仓库构建，不会读取本地/Arena 尚未提交的文件。

需要确保远端 `arena/01a038eb-luraph` 上至少存在：

```text
obsucvim/Cargo.toml
obsucvim/Cargo.lock
obsucvim/src/**
services/discord_obf/**
.dockerignore
render.yaml
```

本轮未自动提交、推送现有大量改名/修改，以免把会话前的其他工作混入部署提交。推送前审查工作树和敏感信息；不要提交 `.env`、`.venv`、`.tools`、`target` 或真实上传文件。

本会话固定使用 `arena/01a038eb-luraph`，不要改用其他分支。

## 3. 部署到 Render

### 推荐：新建 Web Service

在 Render 仪表盘操作：

1. New → **Web Service**，连接 `YaBoNB000/luraph`。
2. Branch 选 **`arena/01a038eb-luraph`**。
3. Language/Runtime 选 **Docker**。
4. **Root Directory 留空**（构建上下文必须是仓库根目录）。
5. Dockerfile Path 填 **`services/discord_obf/Dockerfile`**。
6. 选择 **Free** 实例；如果当前账户/服务没有 Free 选项，不要误选付费档。
7. Health Check Path 填 **`/healthz`**；建议关闭自动部署，避免提交触发重启丢任务。
8. 填以下三个环境变量：

| 名称 | 填什么 |
|---|---|
| `DISCORD_APPLICATION_ID` | General Information 中的 Application ID |
| `DISCORD_PUBLIC_KEY` | General Information 中的 Public Key，64 个十六进制字符 |
| `ALLOWED_USER_IDS` | 你的用户 ID；多个用户用英文逗号分隔 |

**不要在 Render 填 `DISCORD_BOT_TOKEN`，运行服务用不到。**

确认部署后，Render 应提供 `https://你的服务名.onrender.com`。访问 `/healthz`，应返回：

```json
{"status":"ok"}
```

也提供根目录 `render.yaml` 供 Blueprint 使用；创建时同样检查部署分支、Docker 上下文和 Free 套餐。不要把 API endpoint 当成网页：根路径没有上传界面。

### 绑定 Discord Endpoint

回到应用的 **General Information → Interactions Endpoint URL**：

```text
https://你的服务名.onrender.com/interactions
```

保存时 Discord 会发签名 PING，服务验签后返回 PONG。若免费实例正在冷启动，先在浏览器手动打开 `/healthz`，等返回 `ok` 后再保存 Endpoint。不要设置定时刷请求来规避平台休眠规则。

## 4. 注册 `/obf`（本地只需运行一次）

在能连接 Discord 的电脑上运行。下面是 Bash 示例；Windows 可用 WSL，或用 PowerShell 设置相同环境变量。

```bash
# 从仓库根目录运行
python3 -m venv .venv
.venv/bin/python -m pip install -r services/discord_obf/requirements.txt

export DISCORD_APPLICATION_ID='填应用ID'
export DISCORD_GUILD_ID='填测试服务器ID'

# 通过隐藏输入读取 Bot Token，不写入命令历史，不回显，不保存到文件。
.venv/bin/python - <<'PY'
import getpass
import os
from services.discord_obf.register_command import main
os.environ['DISCORD_BOT_TOKEN'] = getpass.getpass('Discord Bot Token（隐藏输入）: ')
raise SystemExit(main())
PY
```

Bot Token 从 Developer Portal → Bot 页面生成/重置；如意外泄露，立即重置。

- 指定 `DISCORD_GUILD_ID`：只在测试服务器注册，适合首次试用。
- 不设置 `DISCORD_GUILD_ID`：注册全局命令，显示可能有传播延迟。
- 脚本 POST/upsert 单个 `/obf`，**不会 bulk overwrite 删除其他命令**。
- 从服务器命令转为全局命令时注意清理旧的服务器 `/obf`，避免两个同名命令。
- `.env` 文件已加入 Git 忽略，但服务/脚本**不会自动读取 `.env`**；必须通过环境变量传入。

## 5. 在 Discord 试用

在安装了应用的测试服务器输入 `/obf`，在 `file` 选项上传这个内容的 `test.lua`：

```lua
local a = "bysuda"
```

预期收到仅自己可见的处理中状态，完成后附上 `obfuscated.luau`。

然后验证：

- 换成非白名单用户 → 拒绝。
- 上传 `.exe` 或超过 256 KiB 的文件 → 拒绝。
- 同一用户再提交一个任务 → 忙碌或频率提示。
- 上传语法错误文件 → 通用失败提示，不泄露源码/编译器 stderr。

如果 Discord 显示“应用未响应”，可能是冷启动未在 Discord 的首次响应时限内完成。打开 `/healthz` 等服务启动，再重新提交命令。HTTP `defer` 只能解决**服务已启动后的长任务**，不能消除平台冷启动。

## 免费托管边界和故障说明

- Discord 首次交互通常要求 3 秒内回应，交互 token 通常有效 15 分钟。服务收到请求后立即返回 type 5，任务在后台进行；队列有上限，但网络慢、平台暂停/重启等仍可导致任务失败。
- 免费服务可能闲置休眠、资源不足、暂停或限制出站流量，**不承诺全天稳定在线**；确认平台允许服务在 defer 后继续运行后台线程。
- 队列、去重、限频均保存在**单进程内存**。必须保持 `--workers 1`，禁止 `--preload`，不要多实例横向扩容。重启后队列丢失，正在处理的任务不会自动恢复；重新提交即可。
- 大脚本即使没超过 256 KiB，也可能因编译复杂度触发内存/时间限制。免费方案适合小脚本试用，不适合无白名单开放。
- 平台出站被阻止或 Discord 限流时，附件下载/回传会失败。回传有有限重试；如果连失败消息都无法回传，界面可能一直“处理中”，日志只记录失败任务 ID。
- 本版 `/healthz` 仅用于进程存活检查，不代表 Discord 网络和任务处理端到端可用。

## 隐私和防护边界

- 输入和结果位于独立临时目录，正常成功/失败均删除；不会记录源码、附件签名 URL、Webhook Token 或编译器 stderr。
- 回复是 ephemeral（仅调用者可见），但**附件仍经过 Discord 和云托管服务**，不是端到端保密存储。不要上传你无权处理的代码或不允许经过这些平台的机密源码。
- `SIGKILL` / 机器崩溃可能跳过临时目录清理，不承诺安全擦除。不要挂载持久磁盘保存临时目录；排查残留时先停止任务再处理。
- Ed25519 验证原始 body + timestamp；拒绝错误/过期签名，校验应用 ID、用户白名单和附件选项。
- 仅从指定 Discord CDN 的 `/attachments/...` HTTPS URL 下载，不跟随重定向；流式限制输入字节数；只用固定本地文件名。
- Rust 子进程通过独立 launcher 设置 CPU/地址空间/输出大小限制，清理继承环境，不接收 Bot Token；Docker 使用非 root 用户。
- **这些是资源限制，不是完整的恶意代码沙箱。** 不执行用户脚本，但解析器仍接触不可信输入。生产公开服务还需要更强隔离、持久队列、监控、滥用处置和更完善的隐私政策。

## 本地开发 / 测试

```bash
# 在仓库根目录；需 Linux、Rust 1.88+
cargo build --release --locked --offline --manifest-path obsucvim/Cargo.toml
python3 -m venv .venv
.venv/bin/python -m pip install -r services/discord_obf/requirements.txt pytest==8.4.2

TEST_OBSUCVIM_BIN="$PWD/engine/target/release/obsucvim" \
  .venv/bin/python -m pytest services/discord_obf/tests -q
```

若省略 `TEST_OBSUCVIM_BIN`，真实引擎的三个集成测试会跳过，不要把这种运行说成完成了真实引擎集成测试。

2026-10-07 本地验证：**55 passed**，包含原始请求验签、PING、私密 defer、授权/输入校验、重放去重、并发去重、限频、队列上限、SSRF URL 拒绝、流式大小限制、下载错误、子进程参数/环境隔离、真实 OS rlimit、真实子进程超时、回传重试、成功/失败临时目录清理，以及真实引擎 canary、编译不执行、语法错误路径。Discord HTTP 使用模拟响应；另 `cargo test` **39 passed / 1 ignored**；实际 Gunicorn 启动、`/healthz`（含代理 Host）和无签名请求 401 检查均通过，测试进程已停止。本轮没有修改混淆器核心，没有重跑 Lua/Luau 全矩阵或重新生成既有示例。

启动 API（先设置前述三个环境变量）：

```bash
export OBSUCVIM_BIN="$PWD/engine/target/release/obsucvim"
.venv/bin/gunicorn 'services.discord_obf.app:create_app()' \
  --bind 0.0.0.0:10000 --workers 1 --threads 4 --timeout 30
```

本地 HTTP 不能直接填写为 Discord Endpoint；Discord 需要公网 HTTPS URL。Render 自带 HTTPS。

有 Docker 的环境可验证镜像（在仓库根目录）：

```bash
docker build -f services/discord_obf/Dockerfile -t obsucvim-discord .
docker run --rm -p 10000:10000 \
  -e DISCORD_APPLICATION_ID -e DISCORD_PUBLIC_KEY -e ALLOWED_USER_IDS \
  obsucvim-discord
```
