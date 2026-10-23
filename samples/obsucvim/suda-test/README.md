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

## SHA-256

见 `SHA256SUMS.txt`。
