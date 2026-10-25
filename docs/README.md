# 文档总索引（docs/README.md）

> 最后更新：2026-10-25（仓库整理）。**这是文档的唯一入口**。
> 想知道「现在是什么状态」→ [`CURRENT-STATE.md`](CURRENT-STATE.md)；
> 想知道「东西都在哪个目录」→ 本文 §2；想接手干活 → 仓库根 `HANDOFF.md`。

---

## 1. 阅读顺序（15 分钟接手）

| 顺序 | 文件 | 读什么 | 为什么 |
| --- | --- | --- | --- |
| 1 | 仓库根 `README.md` | 产品是什么、构建、用法、测试 | 全局定位 |
| 2 | **`CURRENT-STATE.md`** | 全文（~8 KB） | **唯一权威现状**：引擎版本、锚点、收摊令、V19/MW-1 状态、已知边界 |
| 3 | `engine/ENGINE-VERSION.md` | §33–§36 | 现状的原始台账（逐轮 R0xx 决策与证据） |
| 4 | 仓库根 `HANDOFF.md` | 用户硬性要求、环境、验收口径、坑 | 干活纪律 |
| 5 | `engine/MW1-MISTWEAVE.md` / `engine/V19-EXISTENCE-HARDENING.md` | 当前两条在研/已收口的技术线 | 需要动这两条线时必读 |

再往下才按需：`engine/implementation-plan.md`（179 KB 引擎实施清单）、
`engine/plan-resemblance-and-security.md`（153 KB 安全设计总纲）、
`engine/vm-l6-implementation.md`（动 `engine/src/vmgen/` 前必读）、
`hosting/HOSTING-V2-PLAN.md`（托管系统计划与实施记录）。

---

## 2. 目录布局

```
obsucvim/
├── README.md                产品说明（根，唯一门面）
├── HANDOFF.md               AI/新人交接：规矩、环境、验收、坑
├── render.yaml              Render 部署描述（Discord 服务）
├── engine/                  Rust 引擎本体（CLI + 私有 VM）        ← 原 obsucvim/
│   ├── Cargo.toml            std-only，零第三方 crate
│   ├── src/                  71 个模块（注释已于 2026-10-11 剥离）
│   │   ├── lexer/parser/ast/desugar/mangle/minify/strings/
│   │   ├── flatten/junk/numbers/body/antidbg/guard/printer/rng/symtab
│   │   └── vmgen/            L6：compiler / isa / template / v15 /
│   │                         envcheck / exguard / fuse / strseal /
│   │                         strpool / handlers/*（43 个指令 handler）
│   ├── tests/                官方矩阵、预设矩阵、多种子、各轮审计/红队回归
│   ├── tools/                工具链重建、剥注释脚本、luau CLI 主程序
│   └── examples/             全语料混淆示例（改 pass 必须重生成）
├── web/                     控制台 + 托管服务（React/Vite + Node）  ← 部署契约路径
│   ├── src/                  React 控制台（工作台/脚本/历史/卡密/文档）
│   ├── tools/                引擎 WASM + engine-server.mjs + 全部验证套件
│   ├── deploy/               Windows 一键包脚本（raw URL 路径，勿移动）
│   └── public/ index.html vite.config.ts package.json
├── docs/                    文档（分层，见下）
├── artifacts/               证据与产物（见 §4）
├── services/discord_obf/    Discord HTTP /obf 适配（Flask + Rust 子进程）
└── release-assets/          Windows 发布包（raw URL 路径，勿移动）
```

### docs/ 分层

| 目录 | 性质 | 内容 |
| --- | --- | --- |
| `docs/CURRENT-STATE.md` | **现役** | 唯一权威现状速查 |
| `docs/engine/` | 现役设计 + 分析 | `ENGINE-VERSION.md`（逐轮台账，权威）、`AUDIT-20261025-v19evkey.md`（**静态强度审计：载体层静态可解**）、`MW1-MISTWEAVE.md`、`V16/V17/V19-*`、`implementation-plan.md`、`plan-resemblance-and-security.md`、`vm-l6-implementation.md`、`v15-*`、`luraph15-*`、`obfuscation-research.md`、`performance.md`、`offline-hardening-h4-roadmap.md`、`lura15-adoption-inventory.md`、`strong-env-bind-*` |
| `docs/hosting/` | 现役运维 | `HOSTING-V2-PLAN.md`（计划+实施记录）、`SCRIPT-KEYS.md`（卡密/托管接口全集）、`SECURITY-AUDIT.md`（控制台红队自测）、`EMAIL-SETUP.md`（163 邮箱验证码） |
| `docs/redteam/` | 战役记录 | 真 Luraph v15 三轮突破、R028/R061–R064 各轮整改、H5-P0、plan-53/54 |
| `docs/research/` | 研究存档 | `antidebug-20261009/`、`dynamic-r065-20261010/`、`env-recorder-20261011/`、`luraph-envcheck-analysis/`、`size-compression-20261021/` |
| `docs/archive/` | **历史流水（勿当现状读）** | `PROGRESS-history.md`（151 KB 进度流水）、`HANDOFF-history-2026-10.md`（旧版 77 KB 交接文档）、`CONTEXT-MAP-2026-10-23.md`（旧仓库地图） |

### artifacts/ 分层

| 目录 | 内容 |
| --- | --- |
| `artifacts/targets/` | 历轮交付靶标件 R007–R067 + 对应 `README-R*.md` 复测说明（原 `handoff/`） |
| `artifacts/samples/` | 真 Luraph v15 样本与追踪、`obsucvim/mw1-check/`（MW-1 真机验收件）、`obsucvim/diag-tools/` |
| `artifacts/redteam/` | 红队工具与提取物（`interp18.lua`、`factory73.lua`、`B*_T0.bin`、`STRATEGY.md`） |
| `artifacts/legacy-lph/` | 早期 Lua 参考实现（已被 Rust 取代，仅存档） |

清单与保留策略见 [`../artifacts/README.md`](../artifacts/README.md)。

---

## 3. 2026-10-25 整理：路径映射（读历史文档时对照）

历史文档（`docs/redteam/`、`docs/research/`、`docs/archive/`）里出现的旧路径按此表理解；
`docs/archive/` 内的两份流水文档保持**逐字原文**未改路径。

| 旧路径 | 新路径 |
| --- | --- |
| `obsucvim/`（引擎目录） | `engine/` |
| `obsucvim/src`、`/tests`、`/tools`、`/examples`、`/target` | `engine/…` 同名 |
| `handoff/` | `artifacts/targets/` |
| `samples/` | `artifacts/samples/` |
| `redteam/` | `artifacts/redteam/` |
| `lph/` | `artifacts/legacy-lph/` |
| `research/` | `docs/research/` |
| `docs/*.md`（引擎设计/分析） | `docs/engine/*.md` |
| `docs/redteam-*`、`docs/r06*`、`docs/plan-5*`、`docs/hardening-*`、`docs/h5-*` | `docs/redteam/*.md` |
| `docs/CONTEXT-MAP.md` | 本文 + `archive/CONTEXT-MAP-2026-10-23.md` |
| `PROGRESS.md` | `docs/archive/PROGRESS-history.md` |
| `web/docs/ENGINE-VERSION.md`、`MW1-*`、`V16/V17/V19-*` | `docs/engine/…` |
| `web/docs/HOSTING-V2-PLAN.md`、`web/SCRIPT-KEYS.md`、`web/SECURITY-AUDIT.md`、`web/EMAIL-SETUP.md` | `docs/hosting/…` |
| `web/`（目录名） | **不变**（见 §5） |
| `release-assets/` | **不变**（见 §5） |

---

## 4. 文档写作约定

1. **只写事实与证据**：每条结论给出可复现命令、体积、sha256 前缀或测试计数。
2. **诚实边界必写**：不宣称「不可破解」；已知弱点、未验证项（真机、部署、CI）单独列节。
3. **现状只写一处**：`CURRENT-STATE.md`。其他文档不复制状态，只链接过去，避免多版本口径（这是整理前的主要毛病）。
4. **历史不删**：流水进 `docs/archive/`，战役记录进 `docs/redteam|research/`；只加抬头，不改正文。
5. 新增文档时在本文 §2 表格登记；改引擎必须同步 `engine/ENGINE-VERSION.md` 与 `CURRENT-STATE.md`。

---

## 5. 部署契约路径（禁止移动/改名）

已部署的 Windows 客户端按固定 raw URL 拉取更新
（`web/deploy/Update-Obsucvim.ps1` → `$rawBase`）：

- `web/deploy/self-update.mjs`
- `release-assets/obsucvim-web-win64/Obsucvim-Web-Windows-x64.zip`

因此 **`web/` 与 `release-assets/` 这两个目录名保持不动**。若将来必须改名，
需先在旧路径放置转发文件或迁移全部已部署客户端，否则线上自动升级会 404。
