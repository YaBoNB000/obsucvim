# 强环境绑定（人话版）

> 一句话：**绑了环境的包，只有在目标环境（Roblox 真机）里能跑起来；在你电脑的 `luau` 里跑，出来的就是乱码或直接挂掉。**

## 它是干嘛的

- 你用 `luraph` 加了 `--bind-env roblox` 或 `--bind-executor` 后，打出来的包就跟目标环境“焊死”了。
- 拿到文件的人如果不在真机里跑（比如丢到分析沙箱、改了 `string.char` 想偷常量、文本手术动 visible 层），**看不到源码，也拿不到常量**。

## 两档怎么选

- **`--bind-env roblox`**：轻量，**错钥**。通用沙箱里程序照跑但解出来全是垃圾，难定位是环境问题。
  ```bash
  luraph --preset v15 --bind-env roblox --seed 42 in.lua out.lua
  ```
- **`--bind-executor`**：重量，**毒杀**。非完整 Executor 里直接静默死亡（`IronBrew2` 全套体检：`identifyexecutor`/`getgenv`/`getconstants` 等 6 折 transcript 全过才给真钥匙）。
  ```bash
  luraph --preset v15 --bind-executor --seed 42 in.lua out.lua
  ```
  两档可叠加：`--bind-env roblox --bind-executor`。

## 怎么看出绑成功了

- `out.lua` 用 `luau out.lua` 直接跑：**必挂**（`attempt to index nil with 'new'` 等），不是 `hello world`
- 用半吊子桩（只 mock `print`）跑：**必挂**
- 用**忠实桩**（`Vector3.new`/`Vector2.new` 真实现 + `tostring` 正常）跑：**才能过**（`tests/faithful_stub_test.sh` 4/4 就是干这个的）

## 会不会把我正常包搞坏

- **不会**。不加开关时，包跟以前**逐字节一致**（`R023` 靶标 `cmp` 过）。
- 加了开关后，**真机里完全正常**，只有假环境才错钥/毒杀。
- 想回退：**去掉开关重打一次**就行，不改源码。

## 常见坑

- `string.char` 钩子偷常量？已封：常量拼字符串走 `("")[\"char\"]` 元表 + `..`，钩不到。
- 文本手术改 visible 层想绕过？已封：解码器全在加密碎片 `wire 208` 里，visible 层没有可改的 `CDEC`。
- 全保真桩能过吗？**能**（`R004` 定律）。强绑抬的是“从哑桩到全仿真”的成本，不是绝对防住。

## 一句话决策

- 想防随手分析沙箱 → `--bind-env roblox`
- 想防 `Executor` 转储/半吊子环境 → `--bind-executor`
- 想最强 → 两个一起加
