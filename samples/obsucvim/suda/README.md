# 混淆示例：打印 `suda`

最小可运行示例，用来验证「源码 → Obsucvim v15 → 真的能跑」这条链路。

| 文件 | 说明 |
| --- | --- |
| `suda.lua` | 原始源码，两行 |
| `suda.luau` | **服务端默认参数产物**，已在真 Luau 与「Roblox/执行器模拟宿主」上跑通，打印 `suda` |

## 原始源码

```lua
-- 最小示例：打印 suda
print("suda")
```

## 复现命令

用仓库里的 WASI 引擎生成，参数与服务端发布流水线当前默认值一致：

```bash
node -e 'const cfg={wasmPath:"web/tools/obsucvim.wasm",workDir:"/tmp",args:[
  "--preset","v15","--env-check","--no-bind-executor",
  "--seed","20261022","--minify","--vm-seal-strings",
  "/work/input.lua","/work/output.lua"]};
  require("fs").writeFileSync("/tmp/cfg.json",JSON.stringify(cfg));'
node web/tools/wasi-runner.mjs "$(cat /tmp/cfg.json)"
```

`--no-bind-executor` 不能省：引擎的默认值是**绑定执行器**，那一层要求宿主提供
`debug.getconstants` / `getupvalues` / `setupvalue` / `getproto(s)` 等一整套 API，
任何一项缺失都会让产物自我投毒、直接崩掉（见下方 2026-10-23 事故）。

## 验证结果（真实执行，2026-10-23 更新）

用从源码编译的 Luau（`luau-lang/luau` master）直接跑：

```console
$ luau samples/obsucvim/suda/suda.luau
suda

$ cat web/tools/luau-roblox-prelude.luau samples/obsucvim/suda/suda.luau > /tmp/exec.luau
$ luau /tmp/exec.luau        # 模拟 Roblox + Delta 宿主（Vector3/task/typeof/debug…）
suda
```

- 6 个种子（1 / 2 / 3 / 20261022 / 424242 / 987654）全部通过，两种宿主都是 `suda`
- 源码里的字符串 `suda` 不再以明文出现（`--vm-seal-strings` 生效）
- 全文只有 3 行（第 3 行是数百 KB 的单行 VM）

回归入口（需要 Luau 二进制）：

```bash
LUAU_BIN=/path/to/luau python3 web/tools/test-artifact-runtime.py
```

它会对每个种子跑「裸 Luau」与「Roblox/执行器宿主」两遍，并顺带断言
`hardening`（`--bind-executor --bind-env roblox`）**确实会失败**——这条断言就是
2026-10-23 事故的护栏。

## 2026-10-23 事故记录

线上执行器（Delta）里跑任何产物都报 `attempt to call a nil value`，根因是发布流水线
默认带上了 `--bind-executor`（以及 `--bind-env roblox`）。这两层在启动时逐项核验宿主
API，**任何一项对不上不是跳过校验，而是自我投毒**：状态被污染后，脚本在 VM 里随机位置
炸掉，报错看起来和用户代码毫无关系。

修复：默认改为 `--env-check --no-bind-executor`（`env-check` 只比较运行期前后取到的
函数是否同一个，加载前被 hook 不误伤）；两层绑定改成显式 opt-in
（`OBSUCVIM_ENGINE_HARDEN=1` 或发布请求里 `hardening: true`）。

## 体积

| 产物 | 大小 | 相对源码 |
| --- | --- | --- |
| `suda.luau` | 171,707 B | ×3900 |

体积主要来自固定的 VM 解释器与常量池，和源码长度基本无关：**2 行的脚本和 2 千行的脚本，产物都在 170–300 KB 量级**。混淆不是压缩。

## SHA-256

```
91dbd065c12b3766437382f113f4f2c67e8fdd7c14a0f9c6b99f1524f0e07914  suda.lua
ad716ea153a131ec89ff932dacd4246f09bc38403fd2e2e50305bf2889b1c8e1  suda.luau
```

复现时同种子应得到同样大小与内容；不同引擎版本（`obsucvim.wasm`）会不同。


## 执行器证明（2026-10-23 起常驻）

`/files/<id>.lua` 返回的 loader 现在总是以 `tools/attest.luau` 开头，探测执行器与 Roblox 环境，
结论写进 `getgenv().OBS_Attested` / `OBS_Attest`，并随 `validate` 上报。它**永不报错**：
缺什么记什么，脚本照跑。

为什么不在混淆产物里做：引擎对「注入前言」的嵌套函数数量敏感（+2 个还稳、+4 个开始段错误），
详细数据见 `web/docs/HOSTING-V2-PLAN.md` 的「engine preamble cliff」一节。
