# R035 移动端打印注入：保姆级实测教程

> **授权范围**：只用于本仓库自有靶标 `R035_target.luau.lua`，不要拿去测试
> 第三方脚本、游戏或账号。本文测试的是“手机端能否用廉价打印注入拿到内部
> 值”，不是通用执行器教程。

## 0. 你只需要哪个文件

直接使用已经生成好的单文件探针：

```text
handoff/R035_mobile_print_probe.luau.lua
```

它已经把约 150 KB 的 R035 靶标嵌进去了，**不需要**再下载、拼接或修改
`R035_target.luau.lua`。探针支持三种模式，全部由第 3 行控制：

```lua
local MODE = "tail"
```

可填写：

```text
tail
print-hook
loader-hook
```

每轮只改引号内的文字，其他 15 万字节不要动。

## 1. 开跑前必须做的准备

1. 在手机编辑器/测试沙箱中新建一个脚本页。
2. 粘贴 `R035_mobile_print_probe.luau.lua` 的**完整内容**。
3. 确认文件最前面能看到：

   ```lua
   -- Authorized red-team probe generated for: R035_target.luau.lua
   -- Edit ONLY this value: "tail" / "print-hook" / "loader-hook"
   local MODE = "tail"
   ```

4. 确认文件末尾是 `copy_report()`；若不是，说明大文件被手机编辑器截断了。
5. 先找到测试沙箱的 **Stop / Terminate / 停止脚本** 按钮。第三轮会故意
   触发防护，找不到停止按钮就不要运行 `loader-hook`。
6. 每轮运行前清空控制台，避免三轮日志混在一起。

> 探针启动后应打印 `bytes=150285 hash=1818006595`。生成器专门补偿了 Lua
> 长字符串会忽略首个换行的规则，因此这里与磁盘靶标逐字节一致。数字或
> hash 不一致，先不要分析结果，重新复制完整文件。

---

## 2. 第一轮：尾部打印注入（最安全，必须先做）

### 操作

第 3 行保持：

```lua
local MODE = "tail"
```

点击运行，等待完成。

### 正常输出

```text
MOBILE-PROBE> target=R035_target.luau.lua
MOBILE-PROBE> mode=tail bytes=150285 hash=1818006595
MOBILE-PROBE> tail marker armed
MOBILE-TAIL> reached protected chunk end; top_return=    nil
MOBILE-PROBE> finished ok=true err=nil
MOBILE-PROBE> captured_prints=0 load_calls=0
```

最后可能再出现 `report copied`，表示日志已复制到剪贴板；没有剪贴板 API
则会提示手动复制控制台。

### 这轮证明什么

- ✅ 手机端粘贴内容完整；
- ✅ 可以修改靶标最外层文本并执行到末尾；
- ✅ 外壳尾部 `print` 注入成功；
- ❌ **没有拿到内部局部变量 `r`**。

原因：探针只是把靶标最外层 `return ...` 改成局部接收，再在整个保护块执行
结束后打印。R035 原程序没有返回值，所以只能得到 `top_return=nil`；用户程序
内部的 `r` 仍在受保护 VM 的词法作用域中。

如果本轮都没有 `MOBILE-TAIL>`：

1. 检查 mode 拼写；
2. 检查开头的 bytes/hash；
3. 检查控制台是否有 `compile failed`；
4. 若提示内容截断，不要继续后两轮。

---

## 3. 第二轮：替换环境里的 `print`

### 操作

把第 3 行改成：

```lua
local MODE = "print-hook"
```

清空控制台，再次运行。

### 正常输出

```text
MOBILE-PROBE> mode=print-hook bytes=150285 hash=1818006595
MOBILE-PROBE> print hook armed
MOBILE-PROBE> finished ok=true err=nil
MOBILE-PROBE> captured_prints=0 load_calls=0
```

### 如何理解 `captured_prints=0`

这不是探针失效。R035 的六行源程序本来就是“无打印”靶标，计算出的
`z=451`、`r=460` 都没有调用全局 `print`。替换环境中的 `print` 只能拦截
**程序主动调用的 print**，不会自动把所有 local 变量变成输出。

结论：

- ✅ 环境 print 包装没有破坏程序运行；
- ❌ 仅靠全局 print hook 抓不到 `r`；
- ❌ “把 print 换成自己的函数”不等于“能读取 VM 寄存器”。

如果出现 `CAPTURED-PRINT#...>`，请完整保留该行；那代表移动端环境或靶标
出现了与本地基线不同的打印调用。

---

## 4. 第三轮：拦截内部 `loadstring`（高风险，会故意撞防护）

### 运行前再次确认

- 已找到 Stop/Terminate 按钮；
- 能接受当前脚本页卡住 8～10 秒；
- 前两轮都已记录结果。

### 操作

把第 3 行改成：

```lua
local MODE = "loader-hook"
```

运行后最多观察 **10 秒**：

- 若出现 `LOADSTRING#1> len=... hash=...`，立刻截图或复制；
- 若一直没有 `finished`，到 10 秒立即点停止；
- 不要反复连点运行，也不要让它无限挂着。

### 预期现象

本地基线在 12 秒外部超时后退出，未到达 `finished`。移动端可能表现为：

1. 先显示 `loader hook armed`，随后停住；
2. 显示一条或数条 `LOADSTRING#...` 后停住；
3. 因控制台缓冲，什么都没刷新但脚本明显不结束；
4. 沙箱 watchdog 主动报超时。

这些都按“防护被触发”记录。原因是包装后的 `loadstring` 是 Lua 闭包，不再
具备原生函数指纹；R035 会把探针失败折进错误钥匙/静默死亡路径。廉价 loader
hook 因此不能稳定进入“解码后源码 → 注入 print”的阶段。

> **不要把卡住描述成‘破解成功’。** 它只说明 hook 触达了保护面，并被守卫
> 拒绝。只有实际打印出内部 `z/r` 或 VM 寄存器值，才算打印注入取得内容。

---

## 5. 三轮结果表怎么填

复制下面模板，把实际值填好：

```text
设备/系统：
测试沙箱及版本：
R035 bytes/hash：

tail：
- 是否出现 MOBILE-TAIL：
- top_return：
- finished ok/err：

print-hook：
- captured_prints：
- finished ok/err：
- 是否出现 CAPTURED-PRINT：

loader-hook：
- 是否出现 hook armed：
- LOADSTRING 日志条数：
- 10 秒内是否 finished：
- 停止方式/错误原文：
```

手机控制台不便复制时，探针会依次尝试 `setclipboard`、`toclipboard`、
`copyclipboard`、`writeclipboard` 和 `clipboard.set`。`loader-hook` 若卡住，
末尾复制逻辑不会执行，只能截图前面的实时日志。

## 6. 本地复现与重新生成

重新生成同类移动探针：

```bash
python3 obsucvim/tools/mobile_print_probe_gen.py \
  handoff/R035_target.luau.lua \
  handoff/R035_mobile_print_probe.luau.lua
```

本地三轮基线：

```bash
# tail
luau handoff/R035_mobile_print_probe.luau.lua

# print-hook
sed 's/local MODE = "tail"/local MODE = "print-hook"/' \
  handoff/R035_mobile_print_probe.luau.lua > /tmp/r035_print.lua
luau /tmp/r035_print.lua

# loader-hook（必须带外部超时）
sed 's/local MODE = "tail"/local MODE = "loader-hook"/' \
  handoff/R035_mobile_print_probe.luau.lua > /tmp/r035_loader.lua
timeout 12 luau /tmp/r035_loader.lua
# 基线退出码 124：超时，未到 finished
```

## 7. 当前结论与下一步

目前“移动端廉价打印注入”的结论是：

| 方法 | 是否执行 | 是否拿到内部值 |
|---|---:|---:|
| 外壳尾部追加 print | ✅ | ❌，只有 `top_return=nil` |
| 全局 print hook | ✅ | ❌，R035 原程序没有 print |
| loadstring hook | 被守卫卡住 | ❌ |

下一步若继续红队，不应继续堆同类全局 print 包装，而应转向**授权靶标的运行时
帧/handler 定点观测**，并以“能否实际读到 VM 操作数或 `r=460`”作为成功判据。
完整数据流分析仍可能做到这一点，但成本明显高于本页三种移动端廉价尝试。
