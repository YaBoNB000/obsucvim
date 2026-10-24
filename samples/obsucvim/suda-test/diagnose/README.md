# R066 vs R067 差异定位包

## 0. 先跑这一个：`Obsucvim-Doctor.lua`

**整段粘贴进执行器运行即可**，它不会报错、不改任何东西：

- 逐项探测执行器 API（并区分 API 在 `getgenv()` 还是 `_G`）
- 探测闭包分类 `iscclosure(print)`（exguard 第 9/14 号闸门）
- 探测 `debug.getconstants/getupvalues/setupvalue/getproto` 的**真实调用结果**
- 逐条复现 tier-2 的 Roblox 语义判定（Enum 身份 / `Random` 精确浮点 / `task` / `Instance` / `GetService`）
- 打印报告，**自动复制到剪贴板**，末尾给一行 `RESULT:` 摘要

把 `RESULT:` 那行（或剪贴板全部内容）发给我，就能直接定位到是哪一层、哪一项。

`RESULT` 字段含义：

| 字段 | 含义 |
| --- | --- |
| `tier2_fail=[...]` | tier-2 环境检查里**没通过**的项（空 = 全过）。常见：`enum_metatable`、`random_first`、`random_second` |
| `missing_api=[...]` | 执行器缺的 API（exguard 需要） |
| `missing_debug=[...]` | 缺的 `debug` 深内省（Synapse 级才有） |
| `genv_eq_G` | `getgenv() == _G`？`false` 说明 API 在执行器自己的环境里 |

三个常见结论：

- `tier2_fail` 非空 → R067 的 tier-2 在你这台机器上不成立（`Random` 精确序列、`Enum` 元表最敏感）
- `missing_debug` 非空 → `hard` 版永远跑不了，只能用兼容版
- 两者都空但仍失败 → 把完整报告发我，继续看 exguard 的其余闸门

---


用途：在**你的执行器**上逐层跑，一次定位到底哪一层坏了。
三个文件同源（`print("Suda")`）、同种子（20261023）、同引擎（R067.4），只差防护层。

| 文件 | 大小 | 开启的层 | 期望 |
| --- | --- | --- | --- |
| `Suda.L1-tier1.lua` | 194,326 B | tier-1 环境检查（= **R066** 的层） | 应打印 `Suda` |
| `Suda.L2-tier2.lua` | 198,045 B | tier-1 + **tier-2**（Roblox 语义检查） | 待测 |
| `Suda.L3-full.lua` | 215,171 B | tier-1 + tier-2 + **exguard**（= **R067.4**） | 待真机复测 |

## 怎么用

1. 先跑 `Suda.L1-tier1.lua` —— 确认基线正常（应输出 `Suda`）
2. 再跑 `Suda.L2-tier2.lua`
3. 最后跑 `Suda.L3-full.lua`

三种结果分别对应三种结论：

| L1 | L2 | L3 | 结论 |
| --- | --- | --- | --- |
| ✓ | ✗ | ✗ | **tier-2** 的 Roblox 语义检查没过（Enum/Random/task/Instance 其中之一） |
| ✓ | ✓ | ✗ | **exguard**（执行器认证）没过 → 会打印 `[Obsucvim] this executor lacks APIs ...` |
| ✓ | ✓ | ✓ | 三层都过（那 R067 本应可用，需再查别处） |

## R066 与 R067 到底差什么

| 层 | R066 | R067 | 依赖什么 |
| --- | --- | --- | --- |
| L1 tier-1 环境检查 | ✅ | ✅ | 纯 Luau：`buffer` / `bit32` / `string` / 元表 / 协程 / `freeze` / `loadstring` |
| L2 tier-2 环境检查 | ❌ | ✅ | **Roblox 语义**：`Enum.HumanoidRigType` 的 `FromName/FromValue`、`typeof(Enum)=="Enums"`、`getmetatable(Enum)=="The metatable is locked"`、`Random.new(51018400)` 的**精确浮点**序列、`task.spawn/delay/cancel/wait` 语义、`game:GetService("RunService")`、`Instance.new("Folder")` 的 `GetChildren`/`Destroy` 锁、`clonefunction` |
| L3 exguard 执行器认证 | ❌ | ✅ | **执行器 API**：`identifyexecutor` / `checkcaller` / `iscclosure` / `islclosure` / `newcclosure` / `loadstring` / `typeof` + `debug.getconstants` / `getupvalues` / `setupvalue` / `getproto(s)`，还要求能把 C 闭包与 Lua 闭包区分开 |

实测差异（同一份 14 B 源码、同种子）：

```
R066 等价产物（--env-check --no-executor-guard）      194,326 B
+ tier-2                                           198,045 B  (+3,719)
+ exguard（= R067.4 完整）                          215,171 B  (+16,833)
```

引擎源码里 `tier-2` 的开关条件是 `env_check && executor_bind`，
所以 R066（当年只给 `--env-check`、且当时 `--bind-executor` 还没翻成默认）
只有 tier-1；R067 把 tier-2 与 exguard 一起带上了。

## 为什么这两层会「静默死亡」而不是报错

tier-2 与 exguard 的设计都是 **fail-closed 且不留 oracle**：任何一项对不上就把
状态折叠成毒值（`d` / `TOKEN`），随后 VM 在随机位置崩掉。所以现象是
`attempt to call a nil value`、`attempt to perform concatenation on a nil value` 这类
与真实原因无关的错误——这正是 R066 能跑、R067 跑不动的表现差异来源。

R067.2 已经给 exguard 加了前置自检（会说清缺哪个 API）。tier-2 仍然是静默的，
这个包就是用来把它二分出来的。

## 生成命令（可复现）

```bash
# L1 = R066 的层
--preset v15 --env-check --no-executor-guard --minify --vm-seal-strings --seed 20261023
# L2 = + tier-2（Roblox 环境绑定）
--preset v15 --env-check --no-executor-guard --bind-env roblox --minify ...
# L3 = R067 完整
--preset v15 --env-check --bind-env roblox --minify ...
```

`--no-executor-guard` 是 2026-10-23 新增的诊断开关（默认关闭 = 默认行为不变），
用于把 exguard 从 tier-2 里摘出来单独验证。
