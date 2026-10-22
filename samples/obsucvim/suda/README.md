# 混淆示例：打印 `suda`

最小可运行示例，用来验证「源码 → Obsucvim v15 → 真的能跑」这条链路。

| 文件 | 说明 |
| --- | --- |
| `suda.lua` | 原始源码，两行 |
| `suda.portable.luau` | **已在真 Luau 上跑通**，打印 `suda`（关闭 executor 绑定） |
| `suda.executor.luau` | 服务端默认参数产物，绑定 executor/Roblox 环境，只能在执行器里跑 |

## 原始源码

```lua
-- 最小示例：打印 suda
print("suda")
```

## 复现命令

两个产物都是用仓库里的 WASI 引擎（`web/tools/obsucvim.wasm`）生成的，参数就是服务端发布流水线用的那一套，只是绑定开关不同。

```bash
# 1) 可移植版：可以在任何 Luau 解释器里直接跑（STDOUT 输出 suda）
node -e 'const cfg={wasmPath:"web/tools/obsucvim.wasm",workDir:"/tmp",args:[
  "--preset","v15","--no-bind-executor","--dialect","luau",
  "--seed","20261022","--minify","--vm-seal-strings",
  "/work/input.lua","/work/output.lua"]};
  require("fs").writeFileSync("/tmp/cfg.json",JSON.stringify(cfg));'
node web/tools/wasi-runner.mjs "$(cat /tmp/cfg.json)"

# 2) 执行器版：服务端 /api/publish 默认参数（含 --bind-executor --bind-env roblox --env-check）
node -e 'const cfg={wasmPath:"web/tools/obsucvim.wasm",workDir:"/tmp",args:[
  "--preset","v15","--env-check","--bind-executor","--bind-env","roblox",
  "--seed","20261022","--minify","--vm-seal-strings",
  "/work/input.lua","/work/output.lua"]};
  require("fs").writeFileSync("/tmp/cfg.json",JSON.stringify(cfg));'
node web/tools/wasi-runner.mjs "$(cat /tmp/cfg.json)"
```

## 验证结果（真实执行，2026-10-22）

用从源码编译的 Luau（`luau-lang/luau` master）直接跑可移植版：

```console
$ luau samples/obsucvim/suda/suda.portable.luau
suda
```

- 连跑 3 次输出一致；换 3 个种子（20261022 / 20261023 / 20261024）生成的产物都能正确打印
- 源码里的字符串 `suda` 不再以明文出现（`grep -c suda` = 0，`--vm-seal-strings` 生效）
- 全文只有 2 行（第 3 行是数百 KB 的单行 VM）

**未验证**：`suda.executor.luau` 在真实执行器（Delta / Synapse 等）里的运行。它按设计用到了
`loadstring`、`warn`、`identifyexecutor`、`checkcaller`、`newcclosure` 等执行器 API 与
`Vector3` / `task` 绑定，脱离了执行器环境会直接报错——这正是 `--bind-executor` 的目的。

## 体积

| 产物 | 大小 | 相对源码 |
| --- | --- | --- |
| `suda.portable.luau` | 167,195 B | ×3800 |
| `suda.executor.luau` | 231,031 B | ×5251 |

体积主要来自固定的 VM 解释器与常量池，和源码长度基本无关：**2 行的脚本和 2 千行的脚本，产物都在 150–300 KB 量级**。混淆不是压缩。

## SHA-256

```
91dbd065c12b3766437382f113f4f2c67e8fdd7c14a0f9c6b99f1524f0e07914  suda.lua
9ea282c22b2d338389a65a603ac9ddfa0886a5460d17410ba6e0932ae907c845  suda.portable.luau
ff4bb545fa483315b43a3c361c851b944d80ca7a58f37ea3d82910ffcc4681f7  suda.executor.luau
```

复现时同种子应得到同样大小与内容；不同引擎版本（`obsucvim.wasm`）会不同。
