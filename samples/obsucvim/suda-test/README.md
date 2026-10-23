# 测试包：混淆后打印 `Suda`

验证「Obsucvim 的产物在你的执行器里能不能跑」，与许可系统无关（没有卡密、没有网络请求）。

| 文件 | 大小 | 说明 |
| --- | --- | --- |
| `Suda.lua` | 14 B | 原始源码：`print("Suda")` |
| `Suda.attested.lua` | ~180 KB | **推荐先测这个**：执行器证明 + 诊断 + 兼容版产物（等价于线上 loader 在 Delta 上的行为） |
| `Suda.safe.lua` | ~172 KB | 纯兼容版产物（`--env-check --no-bind-executor`） |
| `Suda.hard.lua` | ~212 KB | 纯加固版产物（`--env-check --bind-executor --bind-env roblox`），**原始完整保护** |

## 怎么测

1. 把 `Suda.attested.lua` 整个粘进执行器。预期：

```
[Obsucvim] attested=true score=24/25 executor=Delta
[Obsucvim] missing=executor.debug.depth
[Obsucvim] this host would receive: COMPATIBLE build
Suda
```

最后一行 `Suda` 是产物输出，**必须出现**。前三行告诉你：你的执行器缺什么、
线上会给你哪一版产物。

2. 想验证加固版：把 `Suda.hard.lua` 粘进去跑。
   - 若你的执行器是 Synapse 级别（有 `debug.getconstants`/`getupvalues`），应当输出 `Suda`
   - 若是 Delta 这类，会报错——**这是设计如此**，保护就是「不满足条件不给跑」，
     线上会自动给你 `Suda.safe.lua` 那一版

## 两种产物的区别（实测）

```
Suda.safe.lua  172,293 B   裸 Luau            -> Suda
Suda.hard.lua  212,075 B   裸 Luau            -> attempt to index nil with 'defer'   （保护生效）
                           模拟执行器宿主      -> attempt to call a nil value        （缺 debug 深层 API，被拒）
```

## 复现命令

```bash
printf 'print("Suda")\n' > /tmp/input.luau
# 兼容版
node -e 'const cfg={wasmPath:"web/tools/obsucvim.wasm",workDir:"/tmp",args:[
  "--preset","v15","--env-check","--no-bind-executor",
  "--seed","20261023","--minify","--vm-seal-strings",
  "/work/input.luau","/work/Suda.safe.lua"]};
  require("fs").writeFileSync("/tmp/cfg.json",JSON.stringify(cfg));'
node web/tools/wasi-runner.mjs "$(cat /tmp/cfg.json)"
# 加固版：把 --no-bind-executor 换成 --bind-executor --bind-env roblox
```

## SHA-256

见 `SHA256SUMS.txt`。
