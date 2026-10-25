# artifacts —— 证据与产物存档

> 本目录是**只增不改的证据链**：历轮交付靶标、真机验收件、红队提取物。
> 文件全部保留在 git 跟踪内（用户 2026-10-25 明确要求：证据不落盘外）。
> 目录说明见 [`../docs/README.md`](../docs/README.md) §2。

---

## 1. 内容清单

| 目录 | 文件数 | 体积 | 内容 |
| --- | --- | --- | --- |
| `targets/` | 126 | 13 MB | 历轮交付靶标件 `R007`–`R067`（`.target` 通用件 / `.bound` 绑定态件）+ `README-R*.md` 复测说明；`DEMO_*`、`Suda_*`、`NOL_Wind_*`、`WindUI_*` 大样本 |
| `samples/` | 23 | 1.1 MB | 真 Luraph v15 样本（`luraph15.lua` / `luraph15.txt` / `luraph15_trace.lua`）与分析辅助脚本；`obsucvim/mw1-check/`（MW-1 真机验收双件）、`obsucvim/diag-tools/`（执行器诊断脚本） |
| `redteam/` | 19 | 296 KB | 红队工具与提取物：`interp18.lua`（提取的明文解释器）、`factory73.lua`、`B1/B2/B3_T0.bin`（解码容器）、`STRATEGY.md` 方法论 |
| `legacy-lph/` | 3 | 40 KB | 早期 Lua 参考实现（`lexer.lua` / `parser.lua` / `rng.lua`），已被 Rust 取代，**仅存档不复用** |

## 2. 关键文件锚点（sha256 前 16 位，用于回归对照）

| 文件 | 体积 | sha256 前缀 | 用途 |
| --- | --- | --- | --- |
| `targets/R067_target.luau.lua` | 229,422 B | `3775f3b5…` | R067 锚点件（tier-2 + executor 绑定 + 水印），见 `targets/README-R067.md` |
| `samples/obsucvim/mw1-check/kkksu-mini.mw1evkey.hard.lua` | 206,254 B | `53b3c553d1832882` | MW-1 真机验收（已通过） |
| `samples/obsucvim/mw1-check/kkksu-mini.mw1ctl.hard.lua` | 203,213 B | `d5b1786db81a765c` | MW-1 对照件（已通过） |
| `redteam/interp18.lua` | 32,690 B | `6ccc07239e64912f` | 真 Luraph v15 明文解释器提取物 |

> **注意**：R079 的发布锚点（`ccf5d083…`，178,551 B）是**动态数据段手术**的产物，
> 不在本目录留存文件，只以 `web/tools/obsucvim.wasm` + `verify-engine-version.mjs` 复现
> （见 `../docs/CURRENT-STATE.md` §2）。

## 3. 保留策略

- **不删除、不改名**：文件名（`R0xx_*`）即历史索引，文档按名引用。
- 新增验收件时：目录按线名建（`artifacts/samples/obsucvim/<line>-check/`），
  并在该目录写 `README.md`（体积 / sha256 / 真机判读口径 / 结论）。
- 本目录**不放**可再生的构建产物（如 `engine/target/`、`web/dist/`）与临时日志。
- 体积敏感时优先压缩**历史靶标件**（`.target` 系列），但需用户拍板；默认全部保留。

## 4. 相关文档

- `targets/README-R067.md` —— 绑定态 canary 与「探针感知桩可绕过」的诚实边界
- `samples/obsucvim/mw1-check/README.md` —— MW-1 真机判读口径
- `samples/obsucvim/diag-tools/README.md` —— 真机诊断脚本用法
- `redteam/STRATEGY.md` —— 红队方法论与工具约定
