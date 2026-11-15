# 交接文档（HANDOFF）

> **这是仓库里唯一的文档。** 2026-11-15 仓库规整：除本文件外的 88 个 markdown 全部删除
> （恢复方式见 §13）。设计、审计、逐轮台账、红队与托管文档都不在树里了，需要时从 git 取回，
> 不要凭记忆补写。
>
> 最后更新：2026-11-15（基于实际代码、发布件与沙箱实测重写）。
> 工作分支以会话说明为准；本仓库当前是 **shallow clone**（只有 1 个可见提交），
> 历史提交只能看到基线 `68b9187`。

---

## 1. 项目是什么

**obsucvim**：商业级 **Luau / Roblox 混淆器**（对标 Luraph）。Rust 引擎把用户脚本编译成
**私有字节码**，由随产物分发的「Lua 写的加密解释器」执行；外围是 React 控制台、
Node 托管服务（账户 / 卡密 / 发布 / 验卡）和 Discord `/obf` 适配。

- 产品线 **Luau-only**：Lua 5.1 目标自 2026-09-10 起停止支持，`--dialect 5.1` 直接报错。
- 引擎本体 **Rust，std-only，零第三方 crate**（沙箱到不了 crates.io，这是设计约束）。
- 交付形态：CLI + Web 控制台 + 托管服务（卡密 / 单产物发布 / 验卡）+ Discord 机器人（未部署）。

---

## 2. 当前发布态（2026-11-15 实测）

| 项 | 值 |
| --- | --- |
| 发布 wasm | `web/tools/obsucvim.wasm`，**1,302,319 B** / `a019dc83057ad7722100ca289915704a4cbc5fc6df4a6a4778b3ded59195ebc3` |
| 构建代号 | **R081**（绑定层恒开；wasm 二进制变了，输出锚点未变） |
| 自检版本 | `node web/tools/verify-engine-version.mjs` → **verdict: R080**（按输出锚点识别，R081 仍报 R080 属预期） |
| 锚点命令 | `print("suda") --preset v15 --env-check --bind-executor --seed 67067` |
| 锚点结果 | **179,643 B** / `1b1056221c386f5a7232153e5d5f6f63a2e68cc4c51f5e6e7165a46b8b5c7135` |
| 上一锚点 | R079：178,551 B / `ccf5d0839fef962a8a0c031d745d869dff7a861b6590fe8e267d93f5cf63c045` |
| 水印 | 每个产物第一行固定 `-- This file was obfuscated by Obsucvim.`（普通注释，不是防篡改） |

**确定性**：同一 `--seed` 逐字节一致（本沙箱连跑两次 sha256 相同）；不同 seed 全量重随机
（指令编号、槽位、载体、转义、钥匙表、命名都变）。

**绑定层**：`--bind-executor`（v15）在发布件里**恒开**，传 `--no-bind-executor` 会硬报错；
原生开发构建仍保留该旗标，仅供语义矩阵和示例生成，这类产物不上线。

**EVKEY（V19 运行时值入钥流）实际默认 = Bind**：完整链（v15 + executor guard + key-host）
下不设环境变量与 `OBSUCVIM_V19_EVKEY=1` 输出完全相同；`=0` 才会关闭（体积 177,542 B /
`8e94579b…`）。旧文档写「默认关」是过期结论，代码与实测为准
（`engine/src/vmgen/exguard.rs::resolve_ev_mode`、`engine/src/main.rs`）。

---

## 3. 仓库布局（规整后）

```
engine/                 Rust 引擎（CLI + L6 私有 VM）+ 语料 + 测试矩阵 + 示例
  src/                  73 个 .rs，约 2.05 万行（注释已于 2026-10-11 整体剥离）
  src/vmgen/            L6：compiler / template / v15 / isa / envcheck / exguard /
                        fuse / strseal / strpool / handlers（43 个指令 handler）
  tests/                37 个语料 + 矩阵/审计/多种子脚本 + 攻击面回归
  examples/             37 个混淆示例（改 pass 必须重生成）
  tools/                剥注释脚本、luau CLI 主程序源码
web/                    控制台 + 托管服务 + 引擎 WASM + 验证套件（部署契约路径，勿改名）
  src/                  React 控制台（工作台 / 脚本 / 历史 / 文档 / 账户 / #key 自助页）
  tools/                engine-server.mjs（单文件后端）、obsucvim.wasm、wasi-runner、
                        test-*.mjs / test-*.py、mistweave.py、rebuild-package.sh
  deploy/               Windows 一键包脚本与自更新器（raw URL 路径，勿移动）
services/discord_obf/   Discord HTTP /obf 适配（Flask + Rust 引擎子进程；未部署）
artifacts/              只增不改的证据链：targets（R007–R067 靶标）/ samples / redteam /
                        legacy-lph（早期 Lua 参考实现，仅存档）
docs/research/          研究脚本与数据（md 报告已删除，见 §13）
release-assets/         Windows 发布包 + sha256（部署契约路径，勿移动）
HANDOFF.md              本文件（唯一文档）
```

改名或移动 `web/`、`release-assets/` 会让已部署客户端的自更新 404。

---

## 4. 构建与运行

### 4.1 原生引擎

```bash
export PATH=/home/user/obsucvim/.tools/bin:$PATH   # 工具链 gitignored，可能不存在
cd engine
CARGO_NET_OFFLINE=true cargo build --release        # → engine/target/release/obsucvim
cargo test
```

工具链要求 `cargo/rustc ≥ 1.88`、`lua51`、`luau`、`luau-compile`。本沙箱（2026-11-15）
**没有** cargo/rustc/.tools/bin，也没装 `web/node_modules`，所以原生构建与矩阵跑不了。

### 4.2 WASM 引擎（控制台 / 托管用）

```bash
cd engine && cargo build --release --target wasm32-wasip1
node ../web/tools/verify-engine-version.mjs target/wasm32-wasip1/release/obsucvim.wasm
cp target/wasm32-wasip1/release/obsucvim.wasm ../web/tools/obsucvim.wasm
```

> **坑：`web/tools/build-engine-wasm.sh` 已过期，不要用。** 它仍指向旧目录
> `$ROOT/obsucvim`，并校验 R067 锚点。按上面三条命令手工做；锚点不匹配就不要安装，
> 否则线上版本与文档脱节。

### 4.3 沙箱内可跑的自检

```bash
node web/tools/verify-engine-version.mjs          # → R080（实测通过）
printf 'print("hello")\n' > /tmp/work/in.lua
node web/tools/wasi-runner.mjs '{"wasmPath":"'"$PWD"'/web/tools/obsucvim.wasm",
  "workDir":"/tmp/work","args":["--seed","42","/work/in.lua","/work/out.lua"]}'
# 实测：174,364 B，首行水印正确；同 seed 两次 sha256 一致
```

WASI 运行器支持 `cfg.env` 与 `cfg.preopens` 透传（托管服务不传）。

---

## 5. CLI 用法

零参数即产品形态（v15）：

```bash
./target/release/obsucvim --seed 42 in.lua out.lua
# 等价于
./target/release/obsucvim --preset v15 --seed 42 in.lua out.lua
```

| 预设 | 内容 | 说明 |
| --- | --- | --- |
| 默认 = `v15` | v15 全防护（Luau/Roblox） | 产品形态 |
| `low` | L1 + L2（legacy） | 名称/字符串不可读 |
| `medium` | low + L3 | 控制流打散 |
| `high` | medium + L4/L5/L7 | 整段密文 + 反篡改 |
| `vm` | high + L6 | 旧版 VM |
| `max` | 当前 = `vm` | legacy 最强档 |

常用旗标：`--env-check`、`--bind-env roblox`、`--bind-executor` / `--no-bind-executor`
（后者仅原生开发构建）、`--super-insn`（函数级超指令融合，默认关）、
`--vm-seal-strings`，以及 `--no-mangle/-minify/-strings/-flatten/-junk/-numbers/-body/-antidbg/-guard`。

- `--key-host delta` 自 R070 起恒开（仅兼容保留）；`--no-key-host` 已移除。
- 绑定态产物（`--bind-env` / `--env-check` / `--bind-executor`）**不进运行等价矩阵**：
  只做语法校验 + 裸 Luau「干净拒绝」验证；矩阵与示例用 `--no-bind-executor`。
- Luau 输入支持 `//`、`continue`、复合赋值、反引号插值、类型注解（解析期剥离）。
- 多值/变参、`return f(...)` 尾调用、`local function` 递归可见性都有语料覆盖，改动别破坏。

可选输入侧变换（MW-1「雾织」）：

```bash
python3 web/tools/mistweave.py in.lua weaved.lua --seed 20261025 --num-fog 0.55 --blocks 6
python3 web/tools/mistweave-selftest.py    # 门禁 11/11（需要 luau）
```

---

## 6. 验证矩阵（改引擎前后必看）

```bash
cd engine
cargo build --release
bash tests/run_tests.sh        # 官方矩阵：37 语料 × luau ×（high + vm + v15）
bash tests/run_presets.sh      # 预设 × 全语料
bash tests/multiseed.sh        # 改 vmgen/ 必跑：1 7 4242 31337 999999
bash tests/defense_audit.sh    # 防御审计 D1–D20
bash tests/gen_examples.sh     # 改 pass 必跑，重生成 examples/
python3 tests/security_fingerprint.py
python3 tests/v15_fingerprint.py
```

语义判据：混淆产物的 **stdout + 退出码**与源码逐字节一致（Luau 下）；绑定态产物按
「干净拒绝」判定。任意改动必须：矩阵 + 多种子 + 指纹/审计 + 重生成示例 + 刷新锚点 +
更新本文件。

托管 / 引擎锚点：

```bash
node web/tools/verify-engine-version.mjs
node web/tools/test-hosting.mjs
python3 web/tools/test-loader.py
```

**本沙箱跑不了这些**（缺工具链、解释器、node_modules）。2026-11-15 静态清点的用例数：
hosting 76 项、loader 24、attest 18、live-loader 13、artifact-runtime 16、engine-retry 10；
这些是脚本里的校验条数，不是本轮实测结果。

**已知红项（既有缺陷，不是回归）**：

1. `--vm` 相位：37 个语料的 VM 产物在本机 luau 下输出二进制垃圾 / SIGABRT；
   R079 / R080 / R081 三个构建失败集合完全一致。
2. `tests/wtf_port_tier2_test.sh` 的 `destroy-lock` 负向探针恒红（「difference stayed 0」），
   mock 探针未接通该组；其余 19 项绿。
3. 官方 luau CLI 的 `luaL_sandbox` 会让写全局的产物在只读全局下失败；测试侧用的是
   去掉 sandbox 的 `main_luau.cpp` 构建。

---

## 7. 控制台与托管服务（web/）

```bash
cd web
npm install
npm run engine     # 后端：账户 + 卡密 + 发布 + 验卡（默认 127.0.0.1:8787）
npm run dev        # Vite 控制台，/api 代理到 8787
```

后端是单文件 `web/tools/engine-server.mjs`（3,936 行，约 50 条路由）：注册/登录/验证码/
会话、项目与文件夹、卡密签发/撤销/封禁/解绑、异步发布作业、loader 与文件分发、验卡、
webhook、审计、指标、外部应用。引擎优先原生二进制，缺失时回落 WASI
（`web/tools/obsucvim.wasm` + `wasi-runner.mjs`）。

要点：

- 会话是 **HttpOnly + SameSite=Strict Cookie**；密码 scrypt；验证码/锁定/额度/限流都在服务端；
  CSP + `X-Frame-Options: DENY`；HTTPS 才下发 Secure/HSTS。
- 发布是异步作业：`POST /api/publish` → `GET /api/publish/<jobId>`；失败退额度。
- R081 起只出**加固单变体**：服务端固定 `--preset v15 --env-check --bind-executor --bind-env roblox`，
  `hardening` 入参保留但无效；旧 `safe` 变体已下线。
- 存储是单文档 `web/tools/.state.json` + 本地磁盘产物（已原子写）；多实例/大规模卡密需换数据库。
- 未配 SMTP 时是 **demo 邮件模式**，验证码会出现在响应里（仅演示，别公网裸奔）；
  `web/.env.example` 有 SMTP / Turnstile / 代理信任配置项，`SMTP_PASS` 只放服务端环境变量。
- 前端历史记录存在浏览器本地；控制台注释里的「浏览器本地执行 / wasm-bindgen / 源码不出本机」
  是旧架构文案，**实际源码发到服务端处理**。

---

## 8. 部署与发布

- Windows 便携包：`release-assets/obsucvim-web-win64/Obsucvim-Web-Windows-x64.zip`
  （35 MB，含 Node 22、node_modules、WASI 引擎、单文件 UI）。
- 自更新器固定从 `https://raw.githubusercontent.com/YaBoNB000/obsucvim/arena/01a038eb-luraph`
  拉取 `web/deploy/self-update.mjs` 与发布包；该远端分支仍存在。**改发布通道前先改
  `Update-Obsucvim.ps1` / `self-update.mjs` 的 `REPO_BRANCH`，否则老客户端 404。**
- 重新打包：

  ```bash
  cd web && npm run build
  ./tools/rebuild-package.sh --base ../release-assets/obsucvim-web-win64/Obsucvim-Web-Windows-x64.zip
  ```

  2026-11-15 起脚本对已删除的文档做「存在才复制」，不会再因缺 md 中断。

- **发布包与源码已漂移（未修）**：包内 wasm 是 1,175,136 B / `13916d0b…`，
  且 `tools/engine-server.mjs` 仍是带 `--no-bind-executor` 的 R081 前逻辑。
  包内 `BUILD-INFO.txt` 写的 WASM 哈希 `8032da5b…` 与 `SHA256SUMS.txt`/实际文件
  （`13916d0b…`）不一致——`rebuild-package.sh` 只刷新 Build date，不刷新该哈希行。
  下次打包必须同时刷新 BUILD-INFO 的 WASM 哈希并重算 SHA256SUMS。

---

## 9. Discord `/obf`

`services/discord_obf/`：Flask + Ed25519 验签（无需 Gateway/Bot Token）、单工作线程、
单任务/队列上限/冷却、输入 256 KiB、输出 7 MiB、超时 90 s、地址空间 256 MiB，只混淆不执行。
`render.yaml` 用 Docker 部署（Rust 构建阶段 → Python 运行阶段），健康检查 `/healthz`。
代码与本地测试完成，**未部署、未做真实 Discord 联调**；Bot Token 只用于本地注册命令，
不要发到聊天/日志/仓库。

---

## 10. 已知问题（本次整理未擅自修）

优先级从高到低，均有可复现证据：

1. **重建 WASM 的官方脚本失效**：`web/tools/build-engine-wasm.sh` 用旧路径 `obsucvim/`
   且校验 R067 锚点。按 §4.2 手工命令绕过。
2. **CI 模板失效**：`web/tools/ci/build-engine.yml` 仍用 `obsucvim/**`、R067 锚点，
   上传路径又写 `engine/...`；`.github/workflows/` 无写权限，需手工复制修正后的版本。
3. **发布包漂移**：见 §8（旧引擎 + 旧服务端 + BUILD-INFO 哈希错误）。
4. **`engine-server.mjs` 原生引擎默认路径过期**：`BIN` 仍拼 `REPO/obsucvim/target/release/obsucvim`，
   实际目录已是 `engine/`，因此永远回落 WASI。临时对策：部署时显式设 `OBSUCVIM_BIN`。
5. **控制台文案**：`web/index.html` 与 i18n 里「浏览器本地 / wasm-bindgen / 源码不出本机」
   与服务端混淆架构不符。
6. **测试计数口径**：旧文档写 hosting 78/78、engine-retry 9/9，脚本实际 76 / 10 项校验；
   没有重跑记录前不要对外引用旧数字。
7. **部署通道钉在老分支** `arena/01a038eb-luraph`（其 PR #1 仍开着）；与当前工作分支不同。

---

## 11. 诚实边界（必须保留这些口径）

1. **静态可解性未解决**：对 R079 等价件的纯静态审计（原 `docs/engine/AUDIT-20261025-v19evkey.md`）
   证明载体层密钥流是产物内常量的纯函数，60 行 Python 可解；V-01/V-03 未修，
   R080 只修了 V-06/07/08（诱饵与噪声的可折叠形态）。**不要宣称「静态不可拆」。**
2. **自包含脚本的流密码密钥必然可恢复** —— 壁垒是「每构建唯一 + 提取成本」，
   不是密码学不可破。
3. **真机验证依赖用户回传**：沙箱只有自编 Luau + 宿主 mock；Roblox / Delta / Synapse
   正向验证不在本地能力范围。EVKEY 失败形态是「静默失步挂起」，观测面可被同宿主旁录。
4. **HWID 是客户端提供的值**，可伪造；卡密绑定只是摩擦，不是硬件认证。
5. **绑定态产物在通用沙箱加载即失败**是设计目的，不是缺陷。
6. **未验证项必须标注**：云邮件、Cloudflare Turnstile、真实 Windows Server、Render 部署、
   Discord 真实联调、外部验卡前端、多节点等都没做。
7. 水印是可删除的普通注释，不是防篡改。

---

## 12. 工作规则（用户硬性要求）

1. **不擅自扩范围**；引擎改进多次被收放（P2/P3/P4/P5 已取消，PW-1 已删除），
   没有明确指令不要开新战场，也不要复活已删技术线。
2. 改混淆模块的强制工作流：官方矩阵 + （动 `vmgen/`）多种子 + 安全指纹/防御审计 +
   重生成 `engine/examples/` + 刷新锚点 + 更新本文件；任一项不过 = 未完成。
3. 混淆器本体只写 Rust，不改用其他语言；`artifacts/legacy-lph/` 仅存档。
4. 每条结论给可复现证据（命令、体积、sha256、测试计数）；未验证就写「未验证」。
5. 只动当前工作分支：commit + push 到当前分支，**不动 `main`，不 force push**。
6. 代码里没有注释（历史剥离）；设计看 git 历史文档，不要惊讶。
7. Shell 小坑：`/tmp/t_$s_$d.lua` 会把 `$s_` 当变量名，用 `${s}_${d}`。
8. 守卫/探针改动要过「净化后形态」：标识符会被改名，字符串字面量不会；
   外部对象成员只能 `["字符串键"]` 访问。

---

## 13. 2026-11-15 仓库整理

**做了什么**

- 删除除 `HANDOFF.md` 外的全部 88 个 markdown（根 README、docs 全部、engine/web/services/
  artifacts/release-assets 下的 README 与说明）。
- 保留非 md 的研究脚本/数据（`docs/research/**`）、证据产物（`artifacts/**`）、
  发布包与全部代码/测试。
- `web/tools/rebuild-package.sh` 改为「文档存在才复制」，避免打包因缺 md 失败。
- 本文件按当前代码与实测重写。

**影响**

- 设计史、逐轮台账（R067–R081）、审计细节、红队过程、托管计划、部署手册都不在树里了；
  需要时从 git 取回，不要重写历史结论。
- `docs/` 下只剩 `docs/research/` 的脚本与数据；没有 README 解释它们，使用前先看脚本头。
- 发布包里的旧 README 仍是包里那份（zip 未重建）。

**恢复单个文档**

```bash
git show 68b9187:docs/CURRENT-STATE.md            # 查看
git show 68b9187:README.md > /tmp/README.md       # 导出
git show --stat 68b9187 | head                    # 基线提交概览
```

`68b9187` 是本次整理前的基线（会话起点）。仓库是 shallow clone，取不到更早的历史；
需要更早的文档/恢复点时，去远端仓库或完整克隆里查。

---

## 14. 新任务开工顺序

1. 读本文件 §2（发布态）、§10（已知问题）、§11（边界）。
2. 明确要动的是引擎、托管、部署还是文档——**先确认在不在允许范围内**。
3. 引擎：确认基线 → 改 `engine/` → 按 §6 全工作流 → 刷锚 → 更新本文件。
4. 托管/控制台：改 `web/` → 跑对应套件（需 node_modules 与引擎）→ 更新本文件。
5. 部署/发布：先处理 §8 的包漂移与 §10-4 的 `OBSUCVIM_BIN`，再重建包并刷新哈希。
6. 收尾：`git status` 干净、套件有记录、结论可复现；不合入 `main`。
