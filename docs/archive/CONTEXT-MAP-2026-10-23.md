# 仓库 Markdown 地图（上下文恢复用）

> 最后更新：2026-10-23。全仓库 76 个 `.md` 文件的索引与阅读顺序。
> 新会话（或上下文丢失时）**先读本文，再按 §1 的路径读四份核心文档**，即可在 15 分钟内接手。

---

## 1. 必读顺序（15 分钟）

| 顺序 | 文件 | 读什么 | 为什么 |
| --- | --- | --- | --- |
| 1 | `README.md` | 全文（9 KB） | 产品是什么、分层 L1–L7、预设表、水印、`--bind-env` 说明 |
| 2 | `HANDOFF.md` §1–§3、§8、§9 | 项目定义、用户硬性要求、规矩与坑、验收标准 | **用户硬要求全在这里**（改完必须跑矩阵 + 更新 md + push） |
| 3 | `web/docs/HOSTING-V2-PLAN.md` | §4 阶段表 + 末尾「实施记录」 | 托管系统现役能力与每一步的实测证据 |
| 4 | `web/docs/ENGINE-VERSION.md` | 全文（6 KB） | 引擎版本（当前 R067）、锚点、重编配方 |

需要动手改引擎时再加读：`docs/implementation-plan.md`（179 KB，看 §1 状态列与 §4 更新日志）、
`docs/vm-l6-implementation.md`、`docs/plan-resemblance-and-security.md`。

---

## 2. 顶层

| 文件 | 内容 |
| --- | --- |
| `README.md` | 产品说明：双目标方言、L1–L7 分层、预设与体积、Discord `/obf`、构建与用法 |
| `HANDOFF.md` | **AI 交接文档**：项目定义、用户硬性要求、环境（工具链在 `.tools/bin`）、文件地图、双方言差异、VM 设计要点、开工顺序、规矩与坑、验收标准 |
| `PROGRESS.md` | 按里程碑的进度流水（M0–M6、v15 结构同族、各增量），顶部是最近的 2026-10-11 起条目 |

## 3. `docs/` — 引擎设计与战役记录（33 份）

### 3.1 总纲 / 必读

| 文件 | 内容 |
| --- | --- |
| `implementation-plan.md` | 实施清单（按层）、状态列、里程碑、**更新日志（含 R064–R067 各次增量，含 2026-10-11 的三条重大变更）** |
| `plan-resemblance-and-security.md` | 「看着像 + 安全性也像」总计划：指纹口径、P1 常量加密、P2 字节码不规则化、P3 解释器分层保护、防御审计、各轮红队回应（增量 ㊲–㊿） |
| `offline-hardening-h4-roadmap.md` | 纯离线 H4 加固规范：Loader 泄漏闸、Handler 数值 Microcode、BSS/RT loaderless、HBOOT 静态化、Key-Carrying Chaff 等 |
| `obfuscation-research.md` | 混淆技术研究笔记 + 双方言语义实测表 + 语料清单（§6 必覆盖语法） |
| `v15-pipeline-rewrite.md` | v15 管线重写设计 |
| `v15-structural-parity-plan.md` | v15 结构 100% 还原计划（对照 main 样本） |
| `vm-l6-implementation.md` | L6 私有 VM 实现细节 + §6 调试三板斧 |

### 3.2 Luraph v15 分析 / 红队

| 文件 | 内容 |
| --- | --- |
| `luraph15-analysis.md` | 真 v15 样本静态+动态分析（blob 流水线、执行 VM、原语别名表、LCG、opcode 轨迹） |
| `luraph15-defense-analysis.md` | v15 防御手段全解析（威胁模型、A–G 七类防御、诚实弱点评估） |
| `redteam-luraph15-recon.md` / `-deep-recon.md` / `redteam-luraph15-breakout.md` | 三轮红队：侦察 → 深挖 → **完整突破**（提取明文解释器 + 四套操作码表） |
| `lura15-adoption-inventory.md` | 可采纳/改进点盘点（含「不建议采纳」） |
| `strong-env-bind-ai.md` / `strong-env-bind-human.md` | 强环境绑定方案（AI 版 / 人读版） |

### 3.3 各轮加固与自检

| 文件 | 内容 |
| --- | --- |
| `hardening-r028-implementation.md` | R028/R029 三板斧加固实施 |
| `h5-p0-micro-descriptor-response.md` | R057 micro descriptor 静态恢复整改 + R064 动态追踪缓解 |
| `plan-53-stable-antibypass.md` | 增量 53 稳定防绕过（钥匙面独立化）+ 稳定性原则 P1–P5 |
| `plan-54-r025-response.md` | R025 对抗路由（流偏置 + 双向校验和） |
| `r061-autonomous-redteam.md` | R061：OS/OS2 单分支旁路修复 |
| `r062-autonomous-redteam.md` / `r062-deobfuscation-attempt.md` / `r062-string-seal-experiment.md` | R062：字符串封存路径、固定 recipe 可复用问题、AST 封存原型 |
| `r063-remediation-status.md` | R063 修补自检（入口模块表、KeyEmitter 绑定长度向量） |
| `r064-deep-self-audit.md` / `-r2.md` / `r064-vuln-1-2-research.md` / `r064-variable-name-compaction.md` | R064：尾部 nil 变参、micro-shard trace、嵌套 upvalue seed 错位、变量名压缩 |
| `performance.md` | 各预设性能/体积对照 |

## 4. `web/` — 托管系统（控制台 + 卡密 + 发布）

| 文件 | 内容 |
| --- | --- |
| `web/docs/HOSTING-V2-PLAN.md` | **托管 v2 计划与实施记录**：Luarmor 对标结论、能力对照表、Phase 1–6、接口契约、动效规范、事故记录（BAD_REQUEST、执行器崩溃、attestation、双产物）、诚实清单 |
| `web/docs/ENGINE-VERSION.md` | 引擎版本：R067 vs af7fc32 差异表、锚点、npm 工具链重编配方、换版后步骤 |
| `web/SCRIPT-KEYS.md` | 卡密/托管接口全集：项目、签发、验证、发布、运行时变量、心跳、Key Check 库、webhook 保护、外部验卡、双产物 |
| `web/SECURITY-AUDIT.md` | 控制台红队自测（攻击矩阵、修复清单、客户端固有边界、F-01–F-04） |
| `web/EMAIL-SETUP.md` | 163 邮箱验证码接入（Windows Server） |
| `web/deploy/README-Windows.md` | Windows 一键包：启动器菜单/参数、防火墙、SMTP、Turnstile、额度 |
| `web/deploy/README-UPDATE.md` | 远程升级（一条命令）、签名校验、回滚、占用处理 |

## 5. `handoff/` — 靶标与复测记录（12 份）

| 文件 | 内容 |
| --- | --- |
| `README-R007/008/009/010.md` | 早期红队复测靶标说明 |
| `README-R065.md` | 最小字符串 canary（`local a = "bysuda"`，seed 65065） |
| `README-R066.md` | tier-1 环境检查 canary（`print("suda")`，seed 66066） |
| `README-R067.md` | **绑定态 canary**：tier-2 + executor 绑定 + 水印；229,422 B / `3775f3b5…`；含「探针感知桩可绕过」的诚实边界 |
| `README-Suda-r2.md` / `-r3.md` | `print("Suda")` 的 ㉗ 运行时入密 / B-3 按块链式解码复测 |
| `README-DEMO.md` | DEMO 游戏模块（2,177 B）作为大体量样本 |
| `README-R035-mobile-print.md` | 移动端打印相关 |
| `README-WindUI-vx-safe-port.md` | WindUI vx 安全移植说明 |

## 6. `research/` — 研究战役（19 份）

| 目录/文件 | 内容 |
| --- | --- |
| `antidebug-20261009/`（PLAN / INVENTORY / REPORT / ROUND2–5 / STATUS / WTF-PORT-PLAN / C6-IMPLEMENTATION-PLAN / root-cause/*） | H3-4 反调试噪声战役：死寄存器噪声、调用窗口错位根因与修复选项 |
| `dynamic-r065-20261010/`（REPORT / STRONGER） | 动态追踪 R065 样本，记录更强攻击面 |
| `env-recorder-20261011/`（REPORT / REPORT-R067） | **环境记录器战役**：tier-1/tier-2 电池可被录放；R067 产物被探针感知桩绕过（17 闸门全过）；加固提案被用户搁置 |
| `luraph-envcheck-analysis/ANALYSIS.md` | 真 Luraph 环境检查机制分析 |
| `size-compression-20261021/`（REPORT / LZ-RUNTIME-RESEARCH / token_minify.py / baseline-sha256.json） | 体积压缩研究：结论是「不改算法、只在未指定 seed 时 best-of-N 选最短」，并拒绝 gzip/关字符串封存等降保护方案。**`results/`、`experiment/` 已 gitignore** |

> 注：`af7fc32`（2026-10-19「Reduce v15 output size」）就是这一轮研究的落地，但做法与报告结论不同——
> 它直接削了噪声密度（`junk_n 2→1`、`/12→/30`、`obf_num 40→28 bit`），因此 2026-10-23 已回退。

## 7. `redteam/` 与 `services/`

| 文件 | 内容 |
| --- | --- |
| `redteam/STRATEGY.md` | 红队方法论与工具约定（配套 `redteam/` 下的脚本与提取物） |
| `services/discord_obf/README.md` | Discord `/obf` 托管适配：Render 部署、Ed25519 验签、队列与限制（**未部署/未真实联调**） |

## 8. `obsucvim/` 与 `samples/`

| 文件 | 内容 |
| --- | --- |
| `obsucvim/examples/README.md` | 37 个示例的生成方式与已实现 pass 列表（改 pass 必须重生成） |
| `samples/obsucvim/suda/README.md` | `print("suda")` 示例：R067 产物、体积、复现命令、2026-10-23 事故记录 |
| `samples/obsucvim/suda-test/README.md` | **测试包**：`Suda.attested.lua`（证明+诊断+兼容版）、`Suda.safe.lua`、`Suda.hard.lua`、`SHA256SUMS.txt` |

## 9. `release-assets/`

| 文件 | 内容 |
| --- | --- |
| `release-assets/obsucvim-web-win64/README.md` | Windows 包说明（安全/卡密原型、启动、托管、升级） |

---

## 10. 当前系统状态速查（2026-10-23）

**引擎**

- 源码 = **R067**（`--bind-executor` 默认层 + tier-1/tier-2 环境检查 + exguard）
- `web/tools/obsucvim.wasm` = 1,172,743 B，锚点校验 = **R067**
  （`print("suda") --preset v15 --env-check --bind-executor --seed 67067` → 229,422 B / `3775f3b5…`）
- 自检：`node web/tools/verify-engine-version.mjs`

**托管发布流水线**

- 每次发布产出两份：`safe`（compatible，`--no-bind-executor`）与 `hard`（hardened，完整加固）
- loader 先跑 `tools/attest.luau`（25 项执行器/Roblox 证明，永不报错），按能力请求 `hard`，否则 `safe`
- 异步发布作业（`POST /api/publish` → `GET /api/publish/<jobId>`），失败退还额度，崩溃自动重试 3 次

**测试套件**

```
web/tools/test-hosting.mjs        78/78
web/tools/test-engine-retry.mjs    9/9
web/tools/test-loader.py          24/24
web/tools/test-attest.py          18/18
web/tools/test-live-loader.py     13/13   （真 HTTP + 真服务器 + 真 loader）
web/tools/test-artifact-runtime.py 6 seeds × 2 hosts（需 LUAU_BIN）
web/tools/probe-matrix.mjs         引擎注入前言崩溃阈值探测
```

**已知边界（诚实）**

1. 真机（Roblox / Delta / Synapse）正向运行未验证；沙箱里只有自编 Luau + 宿主 mock。
2. 引擎对「注入前言」的函数数量敏感（+2 稳、+4 崩），所以证明层放在明文 loader，不放进混淆产物。
3. 环境记录器/探针感知桩可绕过 executor 绑定（`research/env-recorder-20261011/REPORT-R067.md`），
   加固提案已按用户决定搁置。
4. 存储仍是单 JSON（已原子写），多实例需换数据库；产物落本地磁盘。
5. `.github/workflows/` 无写权限，CI 模板放在 `web/tools/ci/`。
