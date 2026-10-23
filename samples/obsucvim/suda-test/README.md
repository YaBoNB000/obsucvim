# 测试包：R067 引擎混淆后打印 `Suda`

与许可系统无关（没有卡密、没有网络请求）。产物由 **R067 引擎**生成，
引擎已按锚点逐字节校验：`print("suda") --preset v15 --env-check --bind-executor --seed 67067`
→ 229,422 B / `3775f3b5…`，与 `handoff/README-R067.md` 记录完全一致。

| 文件 | 大小 | 说明 |
| --- | --- | --- |
| `Suda.lua` | 14 B | 原始源码 `print("Suda")` |
| `Suda.attested.lua` | ~196 KB | **推荐先测**：证明层 + 诊断 + 兼容版产物（等价线上 loader 在 Delta 上的行为） |
| `Suda.safe.lua` | 193,044 B | 兼容版（`--env-check --no-bind-executor`） |
| `Suda.hard.lua` | 217,189 B | 加固版（`--env-check --bind-executor --bind-env roblox`），R067.1 完整保护 + 明确报错 |

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
| `Suda.hard.lua` | 217,189 B | **OK** | 被拒：`attempt to index nil with 'defer'`（环境绑定按设计拒绝裸 Luau） |
| `Suda.attested.lua` | 200,990 B | OK | `Suda`（前面多一行证明诊断） |

**语法通过 ≠ 能跑**：`hard` 语法完全合法，但在没有 Roblox/executor API 的宿主上会被
加固层主动拒绝——这是 `--bind-env roblox` 的设计目的，不是语法错误。

同种子重生成逐字节一致（`--seed 20261023`：safe `c8659801…`、hard `22f7eb3b…`）。

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

## SHA-256

见 `SHA256SUMS.txt`。
