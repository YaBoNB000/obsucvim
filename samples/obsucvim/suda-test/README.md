# 测试包：R067 引擎混淆后打印 `Suda`

与许可系统无关（没有卡密、没有网络请求）。产物由 **R067 引擎**生成，
引擎已按锚点逐字节校验：`print("suda") --preset v15 --env-check --bind-executor --seed 67067`
→ 229,422 B / `3775f3b5…`，与 `handoff/README-R067.md` 记录完全一致。

| 文件 | 大小 | 说明 |
| --- | --- | --- |
| `Suda.lua` | 14 B | 原始源码 `print("Suda")` |
| `Suda.attested.lua` | ~196 KB | **推荐先测**：证明层 + 诊断 + 兼容版产物（等价线上 loader 在 Delta 上的行为） |
| `Suda.safe.lua` | 193,044 B | 兼容版（`--env-check --no-bind-executor`） |
| `Suda.hard.lua` | 213,679 B | 加固版（`--env-check --bind-executor --bind-env roblox`），R067 完整保护 |

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
| `Suda.hard.lua` | 213,679 B | **OK** | 被拒：`attempt to index nil with 'defer'`（加固层按设计拒绝裸 Luau） |
| `Suda.attested.lua` | 200,990 B | OK | `Suda`（前面多一行证明诊断） |

**语法通过 ≠ 能跑**：`hard` 语法完全合法，但在没有 Roblox/executor API 的宿主上会被
加固层主动拒绝——这是 `--bind-env roblox` 的设计目的，不是语法错误。

同种子重生成逐字节一致（`--seed 20261023`：safe `c8659801…`、hard `22f7eb3b…`）。

## SHA-256

见 `SHA256SUMS.txt`。
