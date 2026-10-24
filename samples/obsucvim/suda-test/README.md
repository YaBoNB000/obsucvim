# 测试包：R067 引擎混淆后打印 `Suda`

与许可系统无关（没有卡密、没有网络请求）。产物由 **R067 引擎**生成，
引擎已按锚点逐字节校验：`print("suda") --preset v15 --env-check --bind-executor --seed 67067`
→ 229,422 B / `3775f3b5…`，与 `handoff/README-R067.md` 记录完全一致。

| 文件 | 大小 | 说明 |
| --- | --- | --- |
| `Suda.lua` | 14 B | 原始源码 `print("Suda")` |
| `Suda.attested.lua` | ~196 KB | **推荐先测**：证明层 + 诊断 + 兼容版产物（等价线上 loader 在 Delta 上的行为） |
| `Suda.safe.lua` | 193,044 B | 兼容版（`--env-check --no-bind-executor`） |
| `Suda.hard.lua` | 217,216 B | 加固版（`--env-check --bind-executor --bind-env roblox`），R067.5 完整保护 + 精确报错 |

## 预期输出

`Suda.attested.lua`：

```
[Obsucvim] attested=true score=24/25 executor=Delta
[Obsucvim] missing=executor.debug.depth
[Obsucvim] this host would receive: COMPATIBLE build
Suda
```

最后一行 `Suda` 必须出现。

`Suda.hard.lua` 单独跑：
- Synapse 级（有 `debug.getconstants`/`getupvalues`）→ 输出 `Suda`
- Delta 这类 → 报错拒绝，**这是保护生效**，线上会自动发兼容版

## 实测（沙箱，真 Luau 解释器）

```
$ luau Suda.safe.lua                       -> Suda
$ cat luau-roblox-prelude.luau Suda.safe.lua > x.luau && luau x.luau -> Suda
$ luau Suda.hard.lua                       -> attempt to index nil with 'defer'（保护）
grep -c Suda Suda.safe.lua Suda.hard.lua   -> 0 0（明文已封存）
```

## 复现

```bash
printf 'print("Suda")\n' > /tmp/input.lua
# 兼容版
node -e 'const cfg={wasmPath:"web/tools/obsucvim.wasm",workDir:"/tmp",args:[
  "--preset","v15","--env-check","--no-bind-executor","--minify","--vm-seal-strings",
  "--seed","20261023","/work/input.lua","/work/Suda.safe.lua"]};
  require("fs").writeFileSync("/tmp/cfg.json",JSON.stringify(cfg));'
node web/tools/wasi-runner.mjs "$(cat /tmp/cfg.json)"
# 加固版：把 --no-bind-executor 换成 --bind-executor --bind-env roblox
```

## 语法校验（Luau 真编译器）

```bash
LUAU_COMPILE=/path/to/luau-compile ./web/tools/check-artifact-syntax.sh
RUN=1 ... ./web/tools/check-artifact-syntax.sh      # 顺便在 CLI 里跑一遍
```

2026-10-23 实测（Luau master 编译出的 `luau-compile`）：

| 文件 | 大小 | 语法 | CLI 运行 |
| --- | --- | --- | --- |
| `Suda.lua` | 14 B | OK | `Suda` |
| `Suda.safe.lua` | 193,044 B | OK | `Suda` |
| `Suda.hard.lua` | 217,216 B | **OK** | 被拒：`attempt to index nil with 'defer'`（环境绑定按设计拒绝裸 Luau） |
| `Suda.attested.lua` | 200,990 B | OK | `Suda`（前面多一行证明诊断） |

**语法通过 ≠ 能跑**：`hard` 语法完全合法，但在没有 Roblox/executor API 的宿主上会被
加固层主动拒绝——这是 `--bind-env roblox` 的设计目的，不是语法错误。

同种子重生成逐字节一致（`--seed 20261023`：safe `c8659801…`、hard `0369d02a…`）。

## 2026-10-23 · hard 版报错定位与修复

线上现象：Delta 里跑 `Suda.hard.lua` 报
`attempt to perform concatenation on a nil value`（`function qO`，第 3 行）。

**定位**：这条错误不是语法问题，也不在守卫的报错路径上——产物里根本没有 `XF`/`XFM`
（`grep -c XFM` = 0）。它是 **exguard 判定失败后 `TRIP()` 污染状态**，随后 VM 在随机
位置执行到「把 nil 当字符串拼接」才炸出来的**次生错误**，与真实原因（缺哪个 API）无关。

**修复（R067.1）**：在 exguard 进入任何投毒逻辑之前插入一段只读自检，把缺失项直接说出来：

```lua
[Obsucvim] hardened build needs a Synapse-class executor; missing: debug.getconstants,
use the compatible build (--no-bind-executor) if your executor lacks these
```

- 不改变放行策略：缺 API 仍然拒绝运行（保护不减），只是**不再静默投毒**
- 只影响 `--bind-executor` 产物：同一引擎下 `safe` 版**逐字节不变**
  （193,044 B / `c8659801…`，改动前后对比过）
- 环境绑定（`--bind-env roblox`）仍是最外层，裸 Luau 上先在 `task.defer` 处就失败

**仍未闭环**：R067 的 `--bind-executor` 从未在真实执行器上被正面验证过
（仓库文档原本就写着「正面运行等价未验证」）。沙箱里用 C 级宿主（`iscclosure` 为真）
也只能得到拒判或挂起，所以目前只能保证「拒绝时给出可读原因」，不能保证「Delta 上能通过」。
要让 hard 版在 Delta 上真正跑通，需要拿它输出的 `missing:` 清单逐项对齐。

## 2026-10-23 第二轮 · 报错清单为什么全亮了

第一版前置自检上线后，Delta 上得到：

```
missing: identifyexecutor,checkcaller,iscclosure,islclosure,newcclosure,
loadstring,typeof,debug.getconstants,debug.getupvalues,debug.setupvalue
```

`typeof` 也被报缺失——这在任何 Roblox 客户端都不可能。说明**探针本身读错了环境**：

- 自检查的是 `_G[name]`
- 但执行器把 API 注入到 `getgenv()` 返回的**执行器环境**；Roblox 的 `_G` 是另一张共享表
- 所以 `_G.identifyexecutor` 本来就是 nil，全部被判缺失

**R067.2 修复**：自检先取 `getgenv()`，取不到才回退 `_G`，查表也同样二级回退；
`debug` 也一样。用三种宿主布局做了单测（`web/tools/test-exguard-precheck.py`）：

| 宿主 | 期望 | 结果 |
| --- | --- | --- |
| API 全在 `getgenv()`、`_G` 空白（真实执行器） | 通过 | PASS |
| 只有 `debug.getconstants`（Delta 类） | 只报 `debug.getupvalues,debug.setupvalue` | PASS |
| 空环境 | 报全部，且不误报 `_G` 里存在的 `typeof` | PASS |

回归脚本：`LUAU_BIN=/path/to/luau python3 web/tools/test-exguard-precheck.py`。

## 逐层定位包

`diagnose/` 下有同源同种子的三层产物（L1 = R066 的层 / L2 = +tier-2 / L3 = R067 完整），
用来在你的执行器上一次定位是哪一层不过。说明见 `diagnose/README.md`。

## 2026-10-23 第三轮 · tier-2 的三处实现敏感判定已改为结构判定

Delta 的 Doctor 报告显示：执行器 API 与 `debug` 深内省**全部齐全**，Enum 全过，
唯一失败的是 `Random` —— 因为 Delta 用自己的 PRNG（`0.19639633043786484` vs 官方
`0.9598971928030796`）。tier-2 原来把这个差异当成「环境被篡改」，直接投毒。

R067.3 把三处改成版本无关判定（详见 `web/docs/ENGINE-VERSION.md` §9）：

| 位置 | 旧 | 新 |
| --- | --- | --- |
| `Random` | 精确浮点相等 | 值域 / 整数性 / 推进 / 同种子可复现 / Clone / 种子敏感（6 项） |
| 方法调用 | 错误文案含 `"Expected ':' not '.'"` | 点调用必须报错 + **冒号调用必须成功**（更严的一方反而更稳） |
| 销毁锁 | 错误文案含 `"locked"` | 重新挂父必须被拒绝 |

## SHA-256

见 `SHA256SUMS.txt`。

## 2026-10-24 第四轮 · precheck 不再裸写 `getgenv`（R067.4）

第三轮的产物在真 Delta 上依然误报「整张清单全缺」。字节级实锤：产物里
`type(getgenv)=="function"` 被模板级改名器 `sanitize_frag` 改成了 `type(Idu)`（恒假）——
执行器 API 全在 `getgenv()` 侧、不在 `_G`，于是误报。R067.4 的 fix：

- precheck 不再做任何查表，改为复核守卫自己解析好的绑定（与 SCHALL 第一道闸门
  逐条对应），整个文件里**不再出现裸 `getgenv`**（回归测试里有专门断言）；
- `debug` 库绑定改走 LKP（`getgenv()` 侧和脚本 env 侧都查），CIDENT 对来源容错。

现在预期：在 Delta 上 precheck **应当通过**（Doctor 已证明 API 全齐），随后 SCHALL
深闸门是否全部通过**仍待真机实测**——特别是闸门 14 用 `debug.getproto(fn,1)`
探子函数，Delta 对无子函数报 `out of proto bounds`（正常），对带子函数的
探针是否返回真 proto，只有真机知道。若仍失败，把新的控制台输出整段发回。

## 2026-10-24 第五轮 · 真凶是 R067.3 自己引入的（R067.5）

真机分叉实测：safe 正常打印 → VM 核心无辜；L2（tier-2，无 exguard）崩 → 与守卫无关。
R067.3 把 member 检查改成「冒号调用必须成功」时用了**冒号方法语法** `svc:IsStudio()`，
而注入片段会过 `sanitize_frag`：非全局对象的方法名一律被改名（产物里变成
`svc:tk()`）→ 冒号调用永败 → tier-2 折叠值与编译期模型不符 → **任何宿主必投毒**。
即：tier-2 自从 R067.3 起在真 Roblox/真 Delta 上就没有通过的可能——与执行器无关，
是我自己的低级错误。R067.4 的 precheck 修复（exguard 侧）本身没有问题。

修法：冒号调用改回方括号字符串形式 `svc["IsStudio"](svc)`。同时发现 bind-env 还需要
`Vector2.new`（沙箱 prelude 缺失时也炸同一位置，已记入宿主对照清单）。

**分层尺寸（R067.5，同种子 20261023）**：safe 193,044 B（safe 仍逐字节不变）/
L1（tier-2）195,146 B / L2（+bind-env）197,033 B / hard（+exguard）217,216 B。
引擎锚点（`--env-check --bind-executor`）：231,237 B / `f11a6cea…`（R067.5）。

**沙箱首次正向验证（真实 Luau + 高保真 Roblox 宿主 mock）**：
L2 打印 `Suda`（tier-2/bind-env 史上第一次正向跑通）；hard 在缺深内省的宿主上
精确报出四项缺失，无误报。exguard 的 SCHALL 深闸门能否在真 Delta 上全过，
仍需真机：先跑 L2 应打印 `Suda`，再跑 `diagnose/Suda.hard-trace.lua`
（带闸门编号的定位版），把输出发回。

**同场加映（引擎零变更）**：`diagnose/Obsucvim-GateProbe.lua` —— 把 R067.5 SCHALL
闸门判定 1:1 复刻成独立只读探针，一次运行给出全部闸门风险预测（`[PREDICT]` 行），
字母编号与 trace 产物一一对应。沙箱自检：prelude 下 `risk={B,Q}`，其中 B 的
missing 清单与 hard 产物实际拒绝清单逐字一致。判定手册见 `diagnose/README.md`。
