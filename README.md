# obsucvim

商业级 **Luau 混淆器**（对标 [Luraph](https://lura.ph/)）。Rust 引擎把用户脚本编译成
**私有字节码**，由随文件分发的「Lua 写的加密解释器」执行——标准反编译器拿不到原生
Luau 字节码，也还原不出源码结构。每个 `--seed` 的 VM（指令置换、派发树、操作数编码、
命名、载体字母表）都不同。

- **目标方言**：**Luau / Roblox**（产品线 Luau-only；Lua 5.1 目标自 2026-09-10 起停止支持）
- **本体**：Rust，std-only，零第三方 crate（`engine/`，71 个模块，~1.98 万行）
- **纵深**：L1 名称/压缩 → L2 字符串加密 → L3 控制流扁平化 → L4 数值 →
  L5 整段加密容器 → **L6 私有 VM** → L7 反篡改（金丝雀 / 校验和 / 行号重映射 / 时间陷阱）
  → 环境绑定 / 环境检查 / executor 认证守卫 / 宿主钥
- **交付形态**：CLI + React 控制台 + 托管服务（卡密、双产物发布、验卡）+ Discord `/obf`

> **当前状态以 [`docs/CURRENT-STATE.md`](docs/CURRENT-STATE.md) 为准**（引擎 **R080**、
> 锚点 179,643 B / `1b105622…`；上一版 R079 = 178,551 B / `ccf5d083…`）。
> 本文只讲产品与用法，不重复状态。

---

## 仓库布局

```
engine/          Rust 引擎（CLI + L6 私有 VM）、官方测试矩阵、示例
web/             控制台（React/Vite）+ 托管服务（Node）+ 引擎 WASM + 全部验证套件
docs/            文档：CURRENT-STATE / engine / hosting / redteam / research / archive
artifacts/       证据与产物：targets（历轮靶标）/ samples / redteam / legacy-lph
services/        Discord HTTP /obf 适配
release-assets/  Windows 发布包（部署契约路径，勿移动）
```

完整地图与阅读顺序：[`docs/README.md`](docs/README.md)。
接手干活先读 [`HANDOFF.md`](HANDOFF.md)。

---

## 构建

需要 `rustc`/`cargo` ≥ 1.88。仓库沙箱把工具链放在 gitignored 的 `.tools/bin`：

```bash
cd engine
export PATH=/home/user/obsucvim/.tools/bin:$PATH   # 使用仓库内置工具链时
CARGO_NET_OFFLINE=true cargo build --release
```

沙箱外：系统 Rust 即可，不必离线。重建 `.tools` 的步骤见 `HANDOFF.md` §4。

**引擎 WASM**（控制台/托管用）：`web/tools/build-engine-wasm.sh`（编译 + 锚点校验 +
安装，锚点不符拒装）；版本自检 `node web/tools/verify-engine-version.mjs`。

---

## 用法

所有新产物第一行固定为英文品牌水印（文件、stdout 和 Discord 回传一致）：

```lua
-- This file was obfuscated by Obsucvim.
```

水印在压缩后添加，`--no-minify` 也保留；代码正文不变。这是可删除的普通注释，
不是防篡改机制。

```bash
# 默认 = v15 管线（产品形态，Luau/Roblox）——零参数、直接可运行
./target/release/obsucvim --seed 42 in.lua out.lua
# 显式写法等价：
./target/release/obsucvim --preset v15 --seed 42 in.lua out.lua

# 旧版轻量/VM 预设（legacy，后面的 --no-* / --vm 会覆盖）
./target/release/obsucvim --preset low    --seed 42 in.lua out.lua
./target/release/obsucvim --preset medium --seed 42 in.lua out.lua
./target/release/obsucvim --preset high   --seed 42 in.lua out.lua
./target/release/obsucvim --preset vm     --seed 42 in.lua out.vm.lua
./target/release/obsucvim --preset max    --seed 42 in.lua out.vm.lua

# 加固开关（v15）
--env-check         环境检查组（tier-1 纯 Luau 语义；配合 --bind-executor 发射 tier-2）
--bind-executor     executor 认证绑定（v15 默认开启）
--no-bind-executor  R081 起仅原生开发构建可用（矩阵/示例生成）；发布件传入即报错
--bind-env roblox   环境绑定（产物只在目标运行时可加载）
--super-insn        函数级超指令融合（默认关）
```

| 预设 | 打开的层 | 典型体积 | 适用 |
|---|---|---|---|
| **默认（= `v15`）** | v15 架构全防护（Luau/Roblox） | ~150–180 KB | **产品形态**：零参数直接可运行 |
| `low` | L1 + L2（legacy） | ~源码量级 | 名称/字符串不可读即可 |
| `medium` | low + L3（legacy） | 数 KB–数十 KB | 还要打散控制流 |
| `high` | medium + L4 + L5 + L7（legacy） | 数十 KB | 商业级非 VM：整段密文 + 反篡改 |
| `vm` | high + L6（legacy） | ~130–160 KB | 旧版 VM |
| `max` | 当前 = `vm`（legacy） | 同 `vm` | 旧版最强档 |

同 `--seed` → 输出逐字节一致；不同 seed → 编码完全不同。
分层开关：`--no-mangle` / `--no-minify` / `--no-strings` / `--no-flatten` / `--no-junk` /
`--no-numbers` / `--no-body` / `--no-antidbg` / `--no-guard` / `--vm`。

Luau 输入支持 `//`、`continue`、复合赋值、反引号插值、类型注解（解析期剥离）。

### 输入侧加层：MW-1「雾织」

可选的自研源级变换器，串接在引擎之前（`源码 → MW-1 → 引擎 v15`）：

```bash
python3 web/tools/mistweave.py in.lua weaved.lua --seed 20261025 --num-fog 0.55 --blocks 6
python3 web/tools/mistweave-selftest.py          # 门禁 11/11
```

详见 [`docs/engine/MW1-MISTWEAVE.md`](docs/engine/MW1-MISTWEAVE.md)。

---

## 控制台 / 托管

```bash
cd web
npm install
npm run engine     # 引擎+账户+卡密+发布服务（默认 127.0.0.1:8787）
npm run dev        # 控制台（Vite，/api 代理到 8787）
```

接口与部署见 [`docs/hosting/SCRIPT-KEYS.md`](docs/hosting/SCRIPT-KEYS.md)、
[`docs/hosting/HOSTING-V2-PLAN.md`](docs/hosting/HOSTING-V2-PLAN.md)、
`web/deploy/README-Windows.md`（Windows 一键包）。

Discord 机器人（HTTP 交互、Render Docker、免费托管试用）：`services/discord_obf/`
——本地测试完成，**尚未部署 / 未做真实 Discord 联调**。

---

## 验证

```bash
cd engine
bash tests/run_tests.sh      # 官方矩阵：全语料 × 方言 ×（high + vm + v15）
bash tests/run_presets.sh    # 预设 × 全语料
bash tests/multiseed.sh      # VM/编码改动必跑
bash tests/defense_audit.sh  # 防御审计 D1–D20
bash tests/gen_examples.sh   # 重生成 engine/examples/

cd ../web
node tools/verify-engine-version.mjs   # 引擎锚点（R080）
node tools/test-hosting.mjs            # 托管套件
python3 tools/test-loader.py           # loader 套件
```

最近一次记录的全绿口径：官方矩阵、预设矩阵、多种子失败 0、防御审计、轮廓/安全指纹，
以及托管八套件（数字见 `docs/CURRENT-STATE.md` §7）。
**本沙箱没有 Rust 工具链与 Lua 解释器**，只能跑 WASI 引擎与锚点校验（见同一文档 §8）。

---

## 文档

| 入口 | 内容 |
|---|---|
| [`docs/CURRENT-STATE.md`](docs/CURRENT-STATE.md) | **唯一权威现状**：版本、锚点、收摊令、V19/MW-1、已知边界 |
| [`docs/README.md`](docs/README.md) | 文档总索引 + 目录布局 + 历史路径映射 |
| [`HANDOFF.md`](HANDOFF.md) | 交接：用户硬性要求、环境、验收口径、坑 |
| [`docs/engine/ENGINE-VERSION.md`](docs/engine/ENGINE-VERSION.md) | 引擎逐轮台账（R0xx 决策与证据） |
| [`docs/engine/implementation-plan.md`](docs/engine/implementation-plan.md) | L1–L7 实施清单与更新日志 |
| [`docs/engine/vm-l6-implementation.md`](docs/engine/vm-l6-implementation.md) | VM 实现笔记（动 `engine/src/vmgen/` 前必读） |
| [`docs/engine/plan-resemblance-and-security.md`](docs/engine/plan-resemblance-and-security.md) | 安全设计总纲 |
| [`docs/hosting/`](docs/hosting/) | 托管系统、卡密接口、控制台安全审计、邮箱接入 |
| [`docs/redteam/`](docs/redteam/) · [`docs/research/`](docs/research/) | 红队战役与研究存档 |
| [`docs/archive/`](docs/archive/) | 历史流水（**勿当现状读**） |

---

## 约束与诚实边界

- 输出是 **Luau 源码**（v15 产物用 `buffer`/`bit32`/`typeof`/`setfenv`，Lua 5.1 无法运行）。
- 自包含脚本的流密码密钥**必然可恢复**——壁垒是每构建唯一 + 提取成本，
  不是密码学不可破（见 `docs/engine/obfuscation-research.md` §2.2.1）。
- 纯静态拆解已有真实失守先例（V19 立项原因），修复纲领 R1–R6 **未全部落地**。
- 绑定态产物（`--bind-env` / `--bind-executor` / `--env-check`）只在目标运行时可用，
  通用分析沙箱里加载期即失败——这是设计目的，不是缺陷。
- 真机（Roblox / Delta / Synapse）正向验证依赖用户回传；沙箱内只有自编 Luau + 宿主 mock。
