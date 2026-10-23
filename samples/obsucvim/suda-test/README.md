# 测试包：混淆后打印 `Suda`

用来验证「Obsucvim 的产物在你的执行器里到底能不能跑」，和许可系统无关——
这里没有卡密、没有网络请求，跑不起来就是执行器或引擎的问题。

| 文件 | 大小 | 说明 |
| --- | --- | --- |
| `Suda.lua` | 14 B | 原始源码：`print("Suda")` |
| `Suda.obfuscated.luau` | 172 KB | 纯混淆产物（v15 VM + 名称混淆 + 字符串密封 + 环境自检） |
| `Suda.attested.lua` | 180 KB | **推荐测试这个**：证明层（明文）+ 诊断行 + 上面的产物，等价于线上 loader 的结构 |

## 怎么测

把 `Suda.attested.lua` 整个粘进执行器运行。预期输出两行：

```
[Obsucvim] attested=true score=24/25 executor=Delta missing=executor.debug.depth
Suda
```

- 第一行是执行器证明的结果：`executor` 是你的执行器名字，`missing` 列出它缺哪些能力
  （换行显示为空就是满分）
- 第二行才是混淆产物的输出，**必须出现 `Suda`**

如果只出第一行、没有 `Suda`，说明产物本身没跑起来，请把完整报错发我。
如果两行都没有，说明执行器连明文都没执行到。

## 复现命令

产物用仓库里的 WASI 引擎生成，参数就是服务端发布流水线的默认值：

```bash
printf 'print("Suda")\n' > /tmp/input.luau
node -e 'const cfg={wasmPath:"web/tools/obsucvim.wasm",workDir:"/tmp",args:[
  "--preset","v15","--env-check","--no-bind-executor",
  "--seed","20261023","--minify","--vm-seal-strings",
  "/work/input.luau","/work/out.lua"]};
  require("fs").writeFileSync("/tmp/cfg.json",JSON.stringify(cfg));'
node web/tools/wasi-runner.mjs "$(cat /tmp/cfg.json)"
```

组装测试文件（证明层在前，产物在后，中间夹诊断行）：

```bash
cat web/tools/attest.luau <(诊断行) out.lua > Suda.attested.lua
```

## 已验证（真实执行，2026-10-23）

用从源码编译的 Luau（`luau-lang/luau` master）：

```
$ luau Suda.attested.lua
[Obsucvim] attestation did not run (getgenv unavailable or read-only)   # 裸 Lua 没有 getgenv
Suda

$ cat web/tools/luau-roblox-prelude.luau Suda.attested.lua > /tmp/exec.luau
$ luau /tmp/exec.luau
[Obsucvim] attested=true score=24/25 executor=Delta missing=executor.debug.depth
Suda
```

- 6 个种子（20261023 / 7 / 424242 / 987 / 31 / 555）生成的产物全部成功，无段错误
- 源码里的 `Suda` 在产物中不以明文出现（`grep -c Suda` = 0）

## 已知的宿主要求

产物依赖完整的 Luau 运行时，其中 **`debug` 库必须是真的**（VM 内部会用到
`debug.info`）。如果某个执行器把 `debug` 换成残缺的桩表，产物会报
`attempt to perform arithmetic (div) on nil`。所有主流执行器都带完整 `debug`，
这一条只是记录我们在测试中踩到的差异。

## SHA-256

```
4c21e3779ff8d8cfd6466ca09c2bafa4f4fee1e735b3c49151357509540a6181  Suda.lua
fdc06b0ac71b819f9bc09ced9044367cf5ee596c5d409b599f98fa9b70939781  Suda.obfuscated.luau
e1f62b0619240447c96a814350eab8697bfbba740f0c1c90014745c1e4cd870d  Suda.attested.lua
```
