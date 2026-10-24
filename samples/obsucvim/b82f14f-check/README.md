# b82f14f 引擎辨认件（2026-10-24 现编）

**用途**：用户辨认用——用 10-11 的 `b82f14f`（R067 直前满噪声版）引擎源码现编 wasm，
再用与当前发布件**完全相同的参数**生成产物，可直接与被破解的 `Kkksu.hard.lua`
（R067.5 产物）逐项对比。

构建：`git archive b82f14f obsucvim` → rustc 1.88.0（wasm32-wasip1，opt-level 3 + LTO）
→ `obsucvim-b82f14f.wasm`（1,172,743 B）→ `wasi-runner` 生成。可复现。

## 产物清单

| 文件 | 大小 | SHA-256（前 12） | 参数 |
| --- | ---: | --- | --- |
| `Kkksu.lua` | 15 B | `52df8622c1dc` | 源码 `print("kkksu")` |
| `Kkksu.b82f14f.hard.lua` | **216,348 B** | `7a9cbbda6150` | `--preset v15 --env-check --bind-executor --bind-env roblox --seed 20261024 --minify --vm-seal-strings` |
| `Kkksu.b82f14f.safe.lua` | 191,800 B | `38838223998b` | 同上但 `--no-bind-executor` |
| `Suda.b82f14f.anchor.lua` | 229,422 B | `3775f3b557ca` | 锚点配置：`print("suda") --env-check --bind-executor --seed 67067` |

## 与当前引擎（R067.5 / `2f5cf52`）的逐项对照

| 对照点 | b82f14f | R067.5（当前） | 结论 |
| --- | --- | --- | --- |
| suda 锚点产物 SHA-256 | `3775f3b5…` | `3775f3b5…`（文档锚点） | **逐字节相同** |
| kkksu safe 产物 | 191,800 B / `38838223…` | 191,800 B / `38838223…` | **逐字节相同** |
| kkksu hard 产物 | 216,348 B / `7a9cbbda…` | 214,663 B / `3ede3ab1…` | 仅差 1,685 B |
| 引擎源码差异 | — | — | 仅 4 文件 76 行：`main.rs +4`（`--no-executor-guard` 诊断开关）、`envcheck.rs`（IsStudio 判定修复）、`exguard.rs`（precheck 判定集） |

**事实结论**：自 b82f14f 起，VM 核心（isa/compiler/strseal/v15 模板、噪声密度、
解码链、操作数置换/别名体系）零改动；R067 第一刀恰恰是把 af7fc32 的
「减尺寸半噪声」撤掉、恢复满噪声密度。b82f14f 与当前引擎在静态混淆强度上**同体**，
差异只在 hard 层的真机可靠性修复（上周让 hard 在 Delta 从「必崩」变「能跑」的那几处）。

沙箱自检：三件套均过 `luau-compile`；safe 件在高保真 prelude 下打印 `kkksu`。
hard 件与 R067.5 一样，只该在真 Delta / 真 Roblox 系宿主上跑。

## 追加：b82f14f + 已定位修复 = 现版（2026-10-24 收尾实锤）

应用户要求把已定位错误点外科移植进 b82f14f 引擎：`exguard.rs`（R067.4 precheck
判定集 / DTBL 走 LKP / CIDENT CENV 容错）与 `envcheck.rs`（R067.3 门槛剔除 +
R067.5 方括号修复）整文件替换为现版内容；`main.rs` / `template.rs` 的
`--no-executor-guard` 开关接线仅服务诊断分层，hard 构建不需要，故不引入。
重编 wasm（1,174,976 B）后同参数重新混淆 `print("kkksu")`（seed 20261024）：

| 件 | 大小 | SHA-256 |
| --- | ---: | --- |
| 修复后 b82f14f hard（`Kkksu.b82f14f-fixed.hard.lua`） | 214,663 B | `3ede3ab1c992…` |
| 现版 R067.5 `Kkksu.hard.lua` | 214,663 B | `3ede3ab1c992…` |

**逐字节相同（`cmp` 实证）。** 即：把定位好的错误点修进旧引擎后，旧引擎产出与
现版完全一致——「R067 前引擎」与「R067.5」的 VM 核心同体再次获得构建级证明。
该件已通过 `luau-compile` 语法检查，并在 prelude（缺深内省宿主）上精确报出
四项缺失清单——与现版行为逐字一致。真 Delta 无需重测：它与今天已通过
SCHALL 全闸门的构建同源同字节。
