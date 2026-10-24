# v17-check：V17 Phase-1b `--super-insn`（函数级超指令融合）真 Delta 验收件（2026-10-24，R072）

引擎 R072 构建：`--preset v15 --env-check --bind-executor --super-insn --seed 20261024`
（key-host delta 恒开同 R070 口径；trace 件另置 OBSUCVIM_EXG_TRACE=1）。
与 v16-check 的差异仅在融合层：每函数按其指令流统计自选 top-4 白名单 op 对，
塌缩成 fused 记录，字节段随附 fused 词汇表（第 7 段），运行期经 per-dialect
trampoline 派发；off 态产物与 R070/R071 锚点逐字节恒等（231,571B `6c22c008…`）。

## 用法

1. **`Kkksu.v17fusion.hard.lua`** —— 预期：正常打印 `kkksu`（无报错）。
2. **`Kkksu-mid.v17fusion.hard.lua`** —— 递归 fib + 循环 + table + 拼接，
   **预期精确打印**：`kkksu-mid:1,1,2,3,5,8,13,21,34,55|143`
   任何一位不对/报错都算失败（融合级语义错位的表现）。
3. 两个 `-trace` 件仅失败时用：报错行 `(R-xxxx)` 段照旧可对 GateProbe
   `[XG-KH]` 表；若错误形态是本机缺 API 清单（`lacks APIs the hardened build
   requires: …`），说明跑的不是真 Delta 全链环境，不是融合层故障。

## 沙箱侧已证（非真机）

- 同种子的 `--no-bind-executor` on 态变体 luau 直跑精确打印上述两行；
- 8 输入 × 4 种子 on 态运行矩阵 32/32 逐字节正确（rec fib16/pcall/varargs/
  upvalue 闭环全过）；裸 luau 下 hard 件走 exguard 精确缺失清单、**不挂死**；
- `grep -c kkksu Kkksu.v17fusion.hard.lua` = 0（载荷字符串不进产物字节）。
- 引擎门禁：off 锚点恒等 ✓；单测 57 绿；套件 artifact-runtime/exguard-precheck/
  keyhost-fold/attest 18/loader 24/live-loader 13/engine-retry 9/hosting 78 全绿。

## 边界（诚实声明）

- 正向「真 Delta 全链打印」在沙箱无法自证（hard 链要求真 executor 的
  深内省 API 四件 + game/Instance/Vector3/task），须真机回传；
- 本件验收的是「融合不回退可观察行为」（V3 真机段），不宣称任何「不可破解」。

## 真机验收结果（2026-10-24 回传）

- `Kkksu.v17fusion.hard.lua`：真 Delta 打印 `kkksu`，无报错。**通过**。
- `Kkksu-mid.v17fusion.hard.lua`：真 Delta 精确打印
  `kkksu-mid:1,1,2,3,5,8,13,21,34,55|143`，无报错。**通过**。
- trace 两件未动用（无失败需诊断）。
- 结论：V17 Phase-1b V3 真机段通过，on 态融合在真 Delta 全链下不回退
  可观察行为；Phase-1b 验收正式收口。
