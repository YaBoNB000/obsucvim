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
三个文件同源（`print("Suda")`）、同种子（20261023）、同引擎（R067.5），只差防护层。

| 文件 | 大小 | 开启的层 | 期望 |
| --- | --- | --- | --- |
| `Suda.L1-tier1.lua` | 195,146 B | tier-1 + **tier-2**（`--bind-executor`，无 Roblox 绑定） | 应打印 `Suda` |
| `Suda.L2-tier2.lua` | 197,033 B | tier-1 + tier-2 + **bind-env**（Roblox 绑定） | 沙箱高保真宿主已正向跑通；真机待复测 |
| `Suda.L3-full.lua` | 217,216 B | tier-1 + tier-2 + bind-env + **exguard**（= **R067.5**） | 待真机复测 |
| `Suda.hard-trace.lua` | 215,458 B | 同 L3，但 exguard 每道闸门炸掉时带**编号** | 真机定位专用 |

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
| L2 tier-2 环境检查 | ❌ | ✅ | **Roblox 语义**：`Enum.HumanoidRigType` 的 `FromName/FromValue`、`typeof(Enum)=="Enums"`、`getmetatable(Enum)=="The metatable is locked"`、`Random.new(51018400)` 的**结构判定**（R067.3 起：值域/整数性/推进/同种子/Clone/种子敏感，不再比精确浮点）、`task.spawn/delay/cancel/wait` 语义、`game:GetService("RunService")`、`Instance.new("Folder")` 的 `GetChildren`/`Destroy` 锁、`clonefunction` |
| L3 exguard 执行器认证 | ❌ | ✅ | **执行器 API**：`identifyexecutor` / `checkcaller` / `iscclosure` / `islclosure` / `newcclosure` / `loadstring` / `typeof` + `debug.getconstants` / `getupvalues` / `setupvalue` / `getproto(s)`，还要求能把 C 闭包与 Lua 闭包区分开 |

实测差异（同一份 14 B 源码、同种子）：

```
R066 等价产物（--no-bind-executor，tier-1）           193,044 B
+ tier-2（--bind-executor，L1）                     195,146 B  (+2,102)
+ Roblox 绑定（--bind-env roblox，L2）              197,033 B  (+1,887)
+ exguard（= R067.5 完整，L3/hard）                 217,216 B  (+20,183)
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

## `Suda.hard-trace.lua`：真机闸门定位版（2026-10-24）

在 Delta 跑它。报错形如：

```
Luau: The debug library is required on Luau platforms. Please open a support ticket. (R-wioh) true/142
```

**伪装文案是故意的**（不让旁观者看出是混淆器的守卫），有用的是括号里的
`字母-四位随机串`（以及冒号后的细节）。把整段发回来即可。字母 → 闸门：

| 字母 | 闸门 | 字母 | 闸门 |
| --- | --- | --- | --- |
| A | 字符解码链 | K | `getconstants` 返回表 |
| B | API 类型总检 | L | `getupvalues` 返回表 |
| C | `getgenv() ≠ getfenv(0)` | M | `setupvalue` 真改 upvalue |
| D | env 金丝雀 | N | `getproto(s)` 子函数枚举 |
| E | `identifyexecutor` | O | `loadstring` 坏源拒载 |
| F | `checkcaller` | P | `loadstring` 常量核对 |
| G | Roblox 全局类型 | Q | `newcclosure` 包装 |
| H | Roblox 深探（带细节） | R | 总开关/转录哈希 |
| I | 原语 C 闭包判定 | T | ATTEST 最终对账 |
| J | 反向 debug 探针 | U | 投毒链路 |

注意：**没打出任何 `(X-…)` 就崩**（例如 `attempt to ... nil`）说明死在闸门之外的
杂讯层，把那一整段也发回来。

## `Obsucvim-GateProbe.lua`：一次拿全闸门风险画像（2026-10-24，R067.5 配套）

trace 版构建一次只报**第一道**炸掉的闸（报完即崩）；GateProbe 把 R067.5 的
SCHALL 各闸判定 **1:1 复刻**成独立的只读探针，**一次运行给出全部闸门的风险画像**。
用法：整段粘贴进执行器运行，把全部 `[XG-*]` 输出（尤其末尾 `[PREDICT]` 行）原样贴回。

- 行号 `[XG-X]` 的字母 == trace 报告 `(字母-xxxx)` 的字母，两者可互验。
- `[PREDICT] risk={...}` 为空集 = hard 产物预期在该宿主直接跑通。
- B 闸打印的 `missing={...}` 应与 hard 产物拒绝时打印的缺失清单**逐字一致**
  （不一致本身就是 bug 证据，请发回）。
- 标 `skip(api-missing)` 的小节不计入 PREDICT：缺 API 时 precheck/前置闸会先拦，
  对应闸实际不可达。
- R/S/T/U 不可测：它们是引擎内部状态机闸（seal 链 / 转录哈希 / ATTEST 对账 /
  投毒链），只能由 trace 产物在真机观测；S 为保留号（产物内不可达，转录不符并入 R）。
- 安全：仅 D 小节做一次“写入后立即还原”的 rawset 金丝雀，其余全程只读 + pcall。

### 逐闸处置手册

| 闸 | 判定内容 | 细节串字段 | 真 Delta 预判（基于 Doctor） | 命中后的处置方向 |
| --- | --- | --- | --- | --- |
| A | 字符解码链（VK 基元来源） | `via=M/E/L` | 已过 precheck ⇒ 不会中 | 中=字符串元表与环境扫描全灭，截图发回 |
| B | 13 项 API 类型总检 | `missing={...}` | 已证明全齐（R067.4 无清单） | 应与 precheck 清单一致 |
| C | `getgenv() ~= getfenv(0)` | `rawequal(ENV,CENV)` | **未实测的高疑点**（Doctor 未覆盖 getfenv） | 中 ⇒ 该宿主环境不分离，C/D 判据需重设计（R068 议题） |
| D | env 金丝雀：分离+持久 | `canok/tr1/tr2/sep/per` | 与 C 同源 | C 过而 D 挂 ⇒ getgenv 代理表；判据放宽为「可写可读回」 |
| E | `identifyexecutor` 双调用一致性 | `i1ok/i2ok/in1/iv1/in2/iv2` | Doctor 已证稳定 ⇒ 应过 | 若中：报四次返回值 |
| F | `checkcaller()==true` | `gate=ok/val nested-pcall coroutine` | 预计过 | 中 = 调用栈身份被稀释；报三种形态值 |
| G | `game/Instance/Vector3/task` 类型 | 四个 type | 真 Roblox 必过 | 中 ⇒ 假 Roblox 全局 |
| H | Roblox 深探 | `hok/hver:g,rp,er,gs,pl,cn,vn,v3,smt,in,tw,tsp,tdf` | 必过（`cn` 必须是 `Players`） | 沙箱假宿主曾在此现形；对照细节串看哪一跳不符 |
| I | 19 项原语 C/Lua 分类 + Lua 反例 | `bad={...}` | 预计过 | 中 ⇒ 列出的原语被换成 Lua 包装 |
| J | 反向 debug（对 `string.byte` 必须全败或空） | 五项 ok 值 | **次高疑点**：若 Delta 对 C 函数返回空表且不报错 | 中 ⇒ J 判据放宽为「ok 且非空才有害」 |
| K | `getconstants`(Lua fn) 返回表 | `ok/type/n/hasConst` | 预计过 | 中 ⇒ 深内省为假 |
| L | `getupvalues`(Lua fn) 返回表 | `ok/type/n` | 预计过 | 同上 |
| M | `setupvalue` 真改 upvalue | `set(999)/set(111)/uprobe(0)` | **高疑点**：Doctor 的 `invalid index` 是对无 upvalue 函数；本闸闭包有真 upvalue | 中 ⇒ 判据核实 Delta setupvalue 索引语义 |
| N | `getproto(fn,1)`/`getprotos(fn)` | 逐项失败描述 | 预计过（探针函数有真子函数） | 中 ⇒ 对版本文案 |
| O | `loadstring` 坏源拒载 | `call/fn/err` | 预计过 | O/P 同中 ⇒ loadstring 形同虚设 |
| P | `loadstring` 好源+常量核对 | `load/call/class/consts` | 疑点：部分实现不暴露动态 chunk 常量 | 中 ⇒ 判据调整或改用静态 chunk |
| Q | `newcclosure` 包装 | `wrap/call/class-not-c/upv-leak` | 预计过 | 判据核实包装产物的分类语义 |
| R | 总开关/转录哈希 | `heavy/tr` | 不可测 | trace 唯一来源 |
| S | （保留号，产物内不可达） | — | — | 转录不符并入 R |
| T | ATTEST 终对账 | ATTEST 值 | 不可测 | 中 ⇒ 状态机被扰 |
| U | 投毒链 / seal 校验 | — | 不可测 | R067.4 的「无编号杂讯崩」本质是 tier-2 投毒（R067.5 已修） |

执行顺序（为什么 trace 只报一道）：precheck → `PROBE(0,true)` → CIDENT →
SCHALL(A…Q) → seal/ATTEST 对账；任何一步失败即终止。GateProbe 则逐闸独立求值，
所以 PREDICT 可能一次给出多个字母，真机首次拒绝只落在顺序最先的那道上。

沙箱自检（真实 Luau 直跑，非产物）：裸 Luau `risk={B,G,H}`；高保真 prelude
`risk={B,Q}`，其中 B 的 missing 清单与 hard 产物在同一 prelude 下的实际拒绝清单
逐字一致（探针保真度的直接证据）；Q 是 prelude 的 newcclosure mock 为 Lua 包装
所致（mock 局限，非探针缺陷）。
