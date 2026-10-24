# v18-check：R073 体积收敛后 `--super-insn` 复检件（2026-10-24）

引擎 R073 构建配方（与 v17-check 相同）：
`--preset v15 --env-check --bind-executor --super-insn --seed 20261024`
（key-host delta 恒开同 R070 口径；trace 件另置 OBSUCVIM_EXG_TRACE=1，trace_tag
按 SystemTime 非确定是设计行为，trace 件字节本来就不会逐次一致）。

与 v17-check 的唯一引擎差异 = R073 体积收敛：n 份 `_mx` 微解释器逐字节同源
→ 工厂化为 1 份共享函数体 `_MXF` + 每 shard 薄包装闭包；HW2 方言别名拷贝
折叠为 pairs 表循环。打印探针 231,571B → 171,881B（8 种子 × on/off 16 件
全落 162–173KB），md 滚动方言 / 全 kind 注册 / key-host 语义全部不变。

## 用法（真 Delta 环境）

1. **`Kkksu.v18fusion.hard.lua`**（163,115B）—— 预期：正常打印 `kkksu`（无报错）。
2. **`Kkksu-mid.v18fusion.hard.lua`**（182,491B）—— 递归 fib + 循环 + table + 拼接，
   **预期精确打印**：`kkksu-mid:1,1,2,3,5,8,13,21,34,55|143`。
3. 两个 `-trace` 件仅失败时用：报错行 `(R-xxxx)` 段照旧可对 GateProbe
   `[XG-KH]` 表；若错误形态是本机缺 API 清单（`lacks APIs the hardened build
   requires: …`），说明跑的不是真 Delta 全链环境，不是引擎故障。

## 沙箱侧已证（非真机）

- 同配方同种子 `--no-bind-executor` 变体 luau 直跑精确打印上述两行；
- 8 输入 × 4 种子 × on/off 共 64 格矩阵全绿（pcall/error 件按既定口径
  `^false.*: boom$` 判定，error 消息 chunk 名被改写是混淆既定行为）；
- bind-executor hard 件裸 luau 下 4 件全走 exguard 精确缺失清单 rc=1、不挂死；
- `grep -c kkksu Kkksu.v18fusion.hard.lua` = 0（载荷字符串不进产物字节）；
- 引擎门禁：R073 新锚点 171,881B `b67a06e3…` 双构建逐字节恒等；
  单测 57 绿；套件 artifact-runtime/exguard-precheck/keyhost-fold/
  attest 18/loader 24/live-loader 13/engine-retry 9/hosting 78 全绿。

## 边界（诚实声明）

- 正向「真 Delta 全链打印」沙箱无法自证（hard 链要求真 executor 深内省 API
  四件 + game/Instance/Vector3/task），须真机回传；
- 本件验收的是「R073 体积收敛后行为不回退」，不宣称任何「不可破解」。
