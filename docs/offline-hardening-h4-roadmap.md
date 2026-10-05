# 纯离线 H4 加固实施规范（过滤破绽版）

> 日期：2026-10-05
>
> 适用范围：`luraph-rs --preset v15` 纯离线、纯 Luau、无服务器依赖方案
>
> 状态：**H4 P0 已闭环；H4-4A/B 已完成（operand lane + block seed）；内部 loader 0 次**
>
> 原则：只保留能够提高攻击成本、同时不会主动新增稳定明文窗口、统一钩点、
> 兼容性灾难或明显防御 oracle 的方案。

---

## 1. 目标与诚实边界

### 1.1 目标

H4 不再追求“增加几条环境探针”，而是解决当前最重要的结构性问题：

```text
加密源码碎片
    ↓ 运行期解密
明文 Lua 源码字符串
    ↓ loadstring(src)
函数闭包
```

只要这条链存在，攻击环境就可以在 `loadstring` 边界观察 `src`。即使
`debug.info(loadstring, "s") == "[C]"`、栈深度和错误文本全部正常，也不能
证明 loader 没被高保真模拟环境观察。

H4 的核心目标是：

```text
内部明文源码 loader 次数 = 0
运行期只处理加密数字数据 / microcode
任一时刻只暴露当前执行窗口
一次构建的恢复知识尽量不能迁移到下一构建
```

### 1.2 不作虚假承诺

纯离线脚本同时把以下内容交给攻击者：

- 文件本体；
- 解码逻辑；
- 所有内嵌钥匙材料；
- 完整运行权限；
- 调试、修改、重放和快照能力。

因此 H4 不能保证永远不可解。验收目标是：

1. 消灭“一次 loader hook 拿全部明文”的廉价路径；
2. 把攻击提升为每构建恢复数值 VM / Micro-ISA；
3. 把内存快照收益限制到当前基本块或当前函数；
4. 让删除噪声、防御或完整性逻辑会破坏后续真实解码；
5. 保持可靠运行，不能用高误报换取表面强度。

---

## 2. 新威胁模型与实证依据

### 2.1 已实证的攻击能力

对 `KryptIT/luraph-v15-v14.x-deobfuscator` 的隔离测试表明，其模拟环境可以：

- 使用真实 `pcall/xpcall/getfenv` 保持关键栈形态；
- 用包装后的 `loadstring` 观察源码，再调用真实 native loader；
- 伪造 `debug.info`，将 Lua 包装函数报告成 `[C]`；
- 隐藏 harness 栈帧并清理 `debug.traceback`；
- 模拟 Roblox Instance、Signal、方法身份、错误文本和类型关系；
- 用可写 native vector metatable 模拟 Vector3/Vector2 数值语义；
- 运行失败时定位触发 guard 的 proto，并多轮重跑；
- 对大型 `loadstring` 源码块回传完整内容。

### 2.2 对我方靶标的结果

- R035 被其外形检测器识别为 `luraph_v15 0.80`；
- 完整 devirt 在固定 closure-maker 模型中崩溃，未恢复用户程序；
- `--no-hooks --no-devirt` 能完整运行并记录约 54 个 loader 明文入口；
- 旧 `R010_bound` 强环境绑定靶标同样完成运行并暴露 handler/BSS 等源码；
- 它尚未自动恢复 R035 的 `z=451 / r=460`，但 loader 明文泄漏成立。

### 2.3 结论

H1/H2/H3 仍有价值，但它们没有消除动态 loader 的明文参数：

- H1：封闭返回值与 vault 参数面；
- H2：使现有固定 VM mapper 失效；
- H3：提高指令分类与数据流切片成本；
- **H4：必须消除内部源码加载本身。**

---

## 3. 方案筛选规则

候选加固必须同时满足：

1. **不新增明文窗口**：运行时不得重新生成完整 Lua 源码；
2. **不新增固定咽喉点**：不得把所有秘密集中到一个 getter/decoder/dispatcher；
3. **不依赖可伪造单点**：`debug.info`、时间、错误文本不能作为唯一信任根；
4. **不产生显式 oracle**：失败不得稳定表现为固定错误、固定死循环或固定 nil；
5. **不得只增加体积**：随机填充、压缩和加密必须参与真实解码状态；
6. **跨 seed 必须变形**：语义映射、布局、钥匙状态和 microcode 都要变化；
7. **兼容性可验证**：不能依赖少数执行器专有 native API；
8. **性能有上限**：不能强到正式用户被迫关闭保护；
9. **必须有负面实证**：每项都要配对应攻击回归，而不是只做源码 grep。

不满足任一项的方案不进入正式实现清单。

---

# 4. 批准实施项

## H4-0：先建立 Loader 泄漏回归闸（P0）

### 目的

在改架构前先把此次突破路径固化，防止后续“换了一层加密但仍然
`loadstring(明文)`”。

### 实施要求

新增隔离测试环境，具备：

- 包装 `loadstring/load` 并记录全部参数；
- 伪造 `debug.info` 的 `[C]` 返回；
- 保留真实 `pcall/xpcall/getfenv`；
- 统计运行期源码形态字符串；
- 不把测试 hook 写入正式产物。

### 硬门禁

正式 v15 产物运行时必须满足：

```text
内部 loadstring 调用                  = 0
内部 load 调用                        = 0
loader 参数中出现 "return function"   = 0
loader 参数中出现 handler/BSS/RT 特征 = 0
```

外层文件由宿主加载不计入；只统计保护程序开始执行后的内部调用。

### 为什么不会新增破绽

该项仅增加测试代码，不改变正式产物，不引入新的运行时接口。

### 已落地基线（2026-10-05）

脚本：

```bash
python3 luraph-rs/tests/h4_loader_audit.py --expect-open
python3 luraph-rs/tests/h4_loader_audit.py --enforce-zero
```

固定 seed `440041` 的当前架构实测：

```text
clean run                 : YES (H4_RESULT 451 460)
spy run completed         : YES
internal loader calls     : 54
source-like loader args   : 54
largest observed argument : 15368 bytes
payload result survived   : YES
```

`--expect-open` 用于 H4-0 当前漏洞基线并返回成功；`--enforce-zero` 是 H4 P0
最终发布门，当前按预期返回失败。H4-3 收口后必须把正式门禁切到
`--enforce-zero`。

---

## H4-1：Handler 源码碎片改为数值 Microcode（P0）

### 当前问题

43 个 handler 和 Nop alias 最终都会变成明文源码并进入 loader。

### H4-1A 已落地（2026-10-05）

首批 12 个低副作用 handler 已迁移：

```text
LoadK / Move / NewTab / Not
GetGlobal / SetGlobal
GetUp / SetUp
VarArgC
Jmp / Jf / Jt
```

实现特征：

- 每构建生成 `3..5` 个 micro shard；
- 每个 shard 独立 micro-op 编号；
- 每个 shard 独立三字段物理排列；
- equality-left、equality-right、阈值树三种 dispatch 形态；
- handler 仅保留掩码数字 descriptor，不生成 Lua handler 源码；
- descriptor 的 mask/step/数字流进入 boot 期完整性折叠；
- 完整性差异直接偏移 `SALT`，无显式错误 oracle；
- dot-field 通过 fragment 全局改名表同步到 RT 状态字段，不新增公开字段协议。

Loader 审计从：

```text
54 calls / 54 source-like
```

下降到：

```text
42 calls / 42 source-like
```

固定增量门：

```bash
python3 luraph-rs/tests/h4_loader_audit.py --expect-open --max-calls 42
```

此项是 H4-1 的第一批，不代表 Handler Microcode 已全部完成。

### H4-1B 已落地（2026-10-05）

第二批新增迁移 21 个 handler：

```text
Add / Sub / Mul / Div / Mod / Pow / Idiv / Unm / Len / Concat
Lt / Le / Gt / Ge / Eq / Ne
LoadNil / TabN / VarArgTab / VarArgTabN / Closure
```

累计 `33/43` 个 handler 已 loaderless。为完整保持 Lua 语义，算术、比较、
拼接、长度和相等判断的 micro-op 仍包含 metamethod fallback，并通过 `MS`
数字字符串池取得类型名、元方法名及错误文本，不新增明文短字符串。

实现收口：

- 每个 shard 只发射实际被分配使用的 micro-op，不再复制整套语义；
- descriptor 按 shard 分表保存，避免 33 个同作用域 local 触发 Luau 200
  register 上限；
- 额外顺序表只保存 descriptor 引用，用于保持完整性折叠顺序；
- shard 分表与顺序表均为 boot 局部，不是全局语义 closure 表；
- 对 `HashSet` 先排序再 seed 洗牌，修复跨进程同 seed 非确定性。

Loader 审计进一步下降：

```text
54 → 42 → 21 calls
```

HQ 源码碎片：

```text
53 → 41 → 20
```

当前剩余 10 个源码 handler：

```text
GetTab / SetTab
CallT / Call / CallE / CallM / Return
Nop / NopA
MkStr
```

固定增量门更新为：

```bash
python3 luraph-rs/tests/h4_loader_audit.py --expect-open --max-calls 21
```

### H4-1C 已落地（2026-10-05）

最后 10 个 handler 全部完成：

```text
GetTab / SetTab
CallT / Call / CallE / CallM / Return
Nop / NopA
MkStr
```

关键处理：

- GetTab/SetTab 保留 `__index/__newindex`、rawget/rawset、函数/表 fallback
  以及原错误语义；
- Nop/NopA 继续使用 H3 四形态，但形态代码进入随机 shard，不再作为源码碎片；
- MkStr 保持基于字符串元表 `char` 的去钩化物化路径；
- Return 直接写 `E.out/E.total/E.done` 后解链，不新增返回适配器；
- Call 四家族的 chain phase 进入 microcode；H4-2B 又将结果安放阶段编码为
  独立 post-call descriptor，仅由四个最小浅层 executor 跨越真实用户调用，
  保持 Luau 深递归帧预算；
- 每个 shard 只发射实际使用 micro-op，descriptor 分 shard 局部表；
- descriptor 全局顺序仅保存引用，用于完整性折叠，不保存语义 closure；
- 修复 `..name` 被 dot-field remapper 误判及 HashSet 非确定性。

最终 Handler 迁移结果：

```text
43/43 handler loaderless
Loader 调用：54 → 42 → 21 → 11
HQ 源码碎片：53 → 41 → 20 → 10
```

剩余 11 次 loader 来自：

```text
HBOOT 1
continuation 4
Call executor 4
BSS 1
RT 1
```

H4-1 最终增量门：

```bash
python3 luraph-rs/tests/h4_loader_audit.py --expect-open --max-calls 11
```

### 新结构

```text
Rust 编译器
  ├─ 生成真实 VM 指令
  ├─ 将每类 handler 编译成数字 micro-program
  └─ 加密 micro-program

运行时
  ├─ 按块解密少量 micro-op
  ├─ 由构建期生成的 micro-executor 执行
  └─ 覆盖当前数字窗口
```

### Microcode 允许表达的最小能力

- 读取/写入 VM 寄存器；
- 读取常量、upvalue 和环境；
- 算术、比较、索引、调用；
- 条件跳转与返回；
- 更新保护状态和 block key；
- 调用少量经过编号的原语。

### 防止 Micro-VM 变成新固定咽喉

不得实现为一份永久固定的：

```lua
while true do
    local op = code[pc]
    handlers[op](...)
end
```

必须同时具备：

- 每构建 micro-op 编号随机；
- 每构建 operand 字段排列随机；
- 每构建字段宽度/编码族随机；
- `3..7` 个执行 shard，而非单一 `step()`；
- shard 使用不同 dispatch 形态；
- 函数到 shard 的分配每构建变化；
- 无全局 `micro_handlers[op]` 闭包表；
- 不存在一个返回全部 micro-op 语义对象的 getter。

### 验收

- handler 运行时源码字符串为零；
- H3/H2 功能不回退；
- loader spy 看不到任何 handler 文本；
- 同源码双 seed 的 micro-op map 和数字流显著不同；
- 单 shard 被观测不能直接推出其他 shard 的全部语义。

---

## H4-2：BSS 与 RT Loaderless 化（P0）

### 当前问题

即使 handler 已改 microcode，BSS/RT 仍是大型明文源码块，足以暴露：

- parser；
- 常量池结构；
- block key 公式；
- runtime frame 布局；
- trampoline 与 closure 约定。

### 实施方式

- BSS parser 改为构建期生成的静态 boot 状态机或 boot-microcode；
- RT 的 frame/newE/CDEC/bdec 改为静态生成逻辑 + 数字描述数据；
- 运行时不得拼接 RT/BSS Lua 文本；
- parser 输出直接进入加密态 prototype，不产生全量明文 proto 表；
- 数值描述在 boot 完成后按阶段置 nil。

### 防止新破绽

- BSS 与 RT 不共用一个可单独调用的万能 decoder；
- 不返回“完整 parser 状态 + 完整 prototype”的多值对象；
- 不创建长期可遍历的全局 descriptor table；
- 不保留构建期语义名称或字段名；
- 不为兼容旧架构保留隐藏 fallback loader。

### 验收

```text
BSS loadstring 次数 = 0
RT loadstring 次数  = 0
完整解码 proto 常驻 = 0
```

### H4-2A 已落地（2026-10-05）

- 4 个 continuation 改为静态、构建随机 closure；
- continuation 高位 wire 与 `HW[...]` 装配路径退役；
- Call executor、BSS、RT 暂保留加密碎片，避免改变多返回值与 proto 协议。

结果：

```text
Loader 调用：54 → 42 → 21 → 11 → 7
HQ 源码碎片：53 → 41 → 20 → 10 → 6
```

当前剩余 loader：

```text
HBOOT 1 + Call executor 4 + BSS 1 + RT 1 = 7
```

H4-2A 增量门：

```bash
python3 luraph-rs/tests/h4_loader_audit.py --expect-open --max-calls 7
```

### H4-2B 已落地（2026-10-05）

4 个 Call executor 不再以源码碎片进入 loader。Call 协议保持两相，但两相都
改为数值描述：

1. `M_CALL/M_CALLE/M_CALLM/M_CALLT` 在 handler shard 中解析函数、收集参数，
   将调用状态暂存到帧并解链回蹦床；
2. 四个最小静态 executor 在蹦床浅层直接执行真实用户调用，以 `table.pack`
   保留尾部 `nil` 与准确返回数量；
3. 用户调用返回后才进入 `M_POST_CALL/M_POST_CALLE/M_POST_CALLM/
   M_POST_CALLT` descriptor，分别镜像结果安放协议，再续接随机 chain epilogue。

关键约束：

- micro dispatcher 不跨越用户函数调用存活，不增加深递归持久帧；
- 4 个 post-call descriptor 轮转分配到 `3..5` 个 shard，保证每个构建至少
  跨多个独立编号/字段排列/dispatch 方言；
- post-call descriptor 与 handler descriptor 一起进入 `MICRO_CK` 完整性折叠；
- `Call/CallM` 保留 `lastbase/lastn` 与固定/全返回槽清理；
- `CallE` 保留 spill table 与返回数量槽；
- `CallT` 在调用前快照目标表和旧计数，防止被调用函数改写帧后改变语义；
- 静态 Call 状态字段先进入碎片随机改名表，避免某些 seed 下与 RT 字段随机名
  碰撞。该闸实测拦截并修复过 `E.oa` 与 `E.bdec -> oa` 的碰撞回归。

结果：

```text
Loader 调用：54 → 42 → 21 → 11 → 7 → 3
HQ 源码碎片：53 → 41 → 20 → 10 → 6 → 2
```

当前仅剩：

```text
HBOOT 1 + BSS 1 + RT 1 = 3
```

H4-2B 增量门：

```bash
python3 luraph-rs/tests/h4_loader_audit.py --expect-open --max-calls 3
```

### 已验证并回退的候选

曾试验将 Call executor、BSS、RT 直接静态化。虽然 loader 可降至 1，但
`edge.lua` 的多返回值/块状态协议出现 `attempt to index number with number`，
说明该方案会用表面上的 loaderless 换取新的语义破绽，因此未保留。H4-2B
最终采用上面的数值 descriptor/协议镜像完成 Call executor 迁移；BSS/RT 也
不得直接搬源码，必须沿用相同的“先建立协议镜像、再移除 loader”原则。

### H4-2C1 已落地（2026-10-05）

BSS 不再作为 HQ 源码碎片重建并交给 loader，而是构建期生成的一次性静态 boot
状态机。它不直接按固定顺序串行执行原流程，而是使用三阶段数值协议：

```text
carrier parse + 双校验 → Nop alias rewrite → encoded prototype rebuild
```

实现约束：

- 三个 phase code 每构建从独立编号池重抽；
- phase descriptor 使用随机 mask、奇数 stride 与随机 branch 顺序；
- descriptor 完整性折叠不匹配时偏移 `PB`，让编码侧与 RT 块解码侧静默错钥；
- BSS 与 RT/Handler 不共享 dispatcher，不形成新的永久万能 decoder；
- BSS closure 经 H1 `vault[nonce]` 一次性交给 HBOOT，不进入公开形参表；
- HBOOT 消费完成后外层 `BSS = nil`，不保留全局或长期可遍历 parser 对象；
- BSS 先进入与 RT 相同的构建随机字段/标识符方言，保持 prototype 协议一致；
- `W_BSS` 与 BSS HQ wire 正式退役。

Loader 审计：

```text
Loader 调用：54 → 42 → 21 → 11 → 7 → 3 → 2
HQ 源码碎片：53 → 41 → 20 → 10 → 6 → 2 → 1
剩余：HBOOT 1 + RT 1
```

固定增量门：

```bash
python3 luraph-rs/tests/h4_loader_audit.py --expect-open --max-calls 2
```

R041 同一最小输入/同 seed 的体积由 `158568B` 降至 `152138B`，没有用体积
膨胀换取 loader 指标。

### H4-2C2 已落地（2026-10-05）

RT 不再作为最后一个 HQ 源码碎片经 loader 恢复。工厂拆为五个离散、静态但
构建随机的 closure phase：

```text
CDEC → bdec → makefn → newE → run/trampoline
```

实现约束：

- 五个 phase code、descriptor mask/stride 和 dispatch 分支顺序每构建重抽；
- 所有 phase closure 先预声明再按数字协议赋值，不依赖源码拼接或 fallback；
- descriptor 完整性差异同时污染 `SLT/SLTC`，分别破坏指令块与常量池钥匙；
- checksum/poison 继续通过 KeyEmitter 外层值装配，不出现大钥匙裸字面量；
- RT 与 BSS 共享构建随机字段方言，但不共享 phase dispatcher；
- 初始化 dispatcher 只在工厂装配期存在，返回 `run` 后 descriptor 置 nil；
- `RTFRAG` 工厂调用完成立即置 nil，运行期不保留万能 RT 构造入口；
- `W_RT` 与 RT HQ wire 退役；默认构建 `LURAPH_FRAG_SAN` 零源码碎片；
- HQ 数为零时禁用 `#hqi == 0` 锚定配方，避免锚点退化导致钥匙表裸值；
- 防御审计 D5/D6 改为篡改活跃 FN carrier，D12 从静态 TSRC 抽取计时守卫，
  不保留仅为旧碎片架构服务的测试后门。

结果：

```text
Loader 调用：54 → 42 → 21 → 11 → 7 → 3 → 2 → 1
HQ 源码碎片：53 → 41 → 20 → 10 → 6 → 2 → 1 → 0
剩余：HBOOT 1
```

固定增量门：

```bash
python3 luraph-rs/tests/h4_loader_audit.py --expect-open --max-calls 1
```

R041 同输入/同 seed 从 H4-2C1 的 `152138B` 变为 `152712B`（`+0.38%`），
仍比 H4-2B 的 `158568B` 小 `3.69%`，未用显著体积膨胀换取清零 HQ。
下一提交边界为 H4-3 HBOOT 静态化；完成后切换正式门为 `--enforce-zero`。

---

## H4-3：HBOOT 静态化并移除最后内部 Loader（P0）

### 设计

HBOOT 不再作为“元源码字符串”动态加载，而是由 Rust 直接生成外层 boot
状态机。外层源代码本来就必须交给宿主，因此不再做“先隐藏、后明文加载”的
循环。

### 必须保留

- 源码/数据链式完整性；
- vault 阅后即焚；
- build seed 差异化；
- 字段和状态机布局随机；
- boot 结束后的 key material 清除；
- 错钥进入语义投毒，不给明确错误。

### 禁止

- 静态 HBOOT 中出现固定 `decode_all()`；
- 一次性解出全部 microcode；
- 为调试保留正式环境可开启的明文 dump；
- 出错后回退旧 `loadstring` 路径。

### 完成定义

v15 产物内部不再调用任何源码 loader。

### H4-3 已落地（2026-10-05）

- HBOOT 改为构建随机的三阶段静态数值协议：BSS factory、prototype encode、
  seal；phase 编号、mask/stride 与 branch 顺序每构建重抽；
- descriptor 差异与原生 loader/环境探针差异统一折入 `HBD`，同时污染 BSS
  块编码和 RT 的双钥匙约束，不产生固定错误 oracle；
- H1 `vault[nonce]` 扩展为一次性交付 BSS closure 与 `HBD`，读取后即焚；
- 原 HBOOT 源码字节数组、MetaVM 解掩程序、HQ/HQI 链和最后一次
  `loadstring(HBOOT)` 全部从正式产物移除；
- 可选 EXG 也改为静态一次性 factory，调用后清除，不保留条件式 HQ fallback；
- 静态守卫的 API/元方法词通过 HBOOT `CHAR` 参数按字节构造，继续满足
  可见层蜜罐词与保护词零痕迹；
- HBOOT、BSS、RT、EXG factory 均在装配后置 nil，只保留运行必需闭包；
- H4 发布门正式从增量上限切换为零调用强制门。

最终结果：

```text
内部 Loader：54 → 42 → 21 → 11 → 7 → 3 → 2 → 1 → 0
source-like loader 参数：0
HQ 源码碎片：0
payload result survived：YES
```

正式门禁：

```bash
python3 luraph-rs/tests/h4_loader_audit.py --enforce-zero
```

该门禁完全在本地构建/测试环境执行，不引入服务器、联网密钥或运行期授权。

R041 同输入/同 seed 体积从 H4-2C2 的 `152712B` 降至 `116444B`
（`-23.75%`），相对 H4-2B 的 `158568B` 共下降 `26.57%`；原先用户看到的
HBOOT 掩码源码密集数字区随 MetaVM 一并退役。

---

## H4-4：Key-Carrying Chaff（P1）

### 当前问题

H3 活噪声会真实执行和写死寄存器，但完整数据流分析仍可能证明这些值不流向
用户结果。

### 新设计

噪声值必须参与后续保护状态：

```text
chaff value
   ↓
block state / operand mask / next-key / alias state
   ↓
后续真实 microcode 解码
```

允许的耦合位置：

- 下一基本块 seed；
- operand lane mask；
- opcode alias 选择；
- 常量索引偏移；
- microcode 校验状态；
- block cache 淘汰序列。

### 安全约束

- chaff 必须是确定性的构建数据函数，不依赖时间、地址或设备浮动值；
- 不能使用显式 `if checksum ~= expected then crash`；
- 不能有统一 `mix_chaff()` 咽喉函数；
- 不能用最终恒零的代数式伪装依赖；
- 删除任意一类 chaff 应在后续若干块产生错钥，而非立即固定报错；
- Rust 编码镜像与运行时消费必须由同一生成描述产生，避免手写公式漂移。

### 验收

- 删除 NOP/chaff 后无法继续正确解后续真实块；
- 静态 DCE 不能只靠“用户结果未读取”删除 chaff；
- 无明显固定投毒常量和单点比较。

### H4-4A 已落地（2026-10-05）

先按路线要求只试点一个低风险 operand lane：

- 编译器对每函数所有 Nop 的四个**物理流操作数**及位置做非零模 `65521`
  折叠，得到每函数 `qf`；
- fold 乘数、四 lane 系数、位置系数和函数系数由已有构建钥匙派生的独立
  PRNG 生成，不额外推进主编译 RNG，避免扰动既有布局空间；
- 每构建从 `SA/SB/SC/SD` 中随机选一个真实 operand lane；BSS 使用从载体
  实际重算的 `qf` 加密该 lane，RT 则使用编译器期望 `QF[pf.sd]` 解密；
- 删除、归零或重排 Nop chaff 后，两侧 qf 不一致，后续真实指令操作数静默
  错位，不再只是“删掉无用户输出的死工作”；
- QF 表自身进入独立 `CHAFF_CK` 折叠，差异同时污染 `SLT/SLTC`，不存在
  明文 `if qf ~= expected then crash`；
- BSS 实际 qf 不写入 prototype，不新增长期可遍历状态或统一 `mix_chaff()`；
- H3 审计新增第 7 项，验证 qf 非零、跨 seed 变化并接入真实 lane 镜像。

试点结果：

```text
内部 Loader：0（--enforce-zero PASS）
R041：116444B → 117460B（+0.87%）
169/169；370/370；multiseed FAIL=0；defense 21/21
```

### H4-4B 已落地（2026-10-05）

第二个独立提交只扩展 block seed，不同时触碰常量和 microcode：

- 从 H4-4A 已有 fold 系数族确定性派生非零 `CHAFF_BLOCK`，不新增独立原始
  随机系数；
- BSS 每块钥匙种子新增 `qf * CHAFF_BLOCK`，RT 镜像使用
  `QF[pf.sd] * CHAFF_BLOCK`；
- qf 不一致时不再只破坏单 operand lane，而是从块种子开始破坏 opcode 与
  五路指令流；
- 没有路径依赖：同一 prototype 的 qf 固定，任意控制流入口均可正确解块；
- 沿用 `CHAFF_CK → SLT/SLTC` 完整性约束，不增加显式比较或崩溃分支；
- H3 审计新增第 8 项，确认 BSS/RT 两侧 block seed 镜像均实际消费 chaff。

结果：

```text
内部 Loader：0（--enforce-zero PASS）
R041：117460B → 118108B（+0.55%）
169/169；H3 chaff 8/8
```

H4-4 尚未全部完成；下一小阶段为 constant key，再后才是 microcode checksum，
不得一次性混入以免无法定位错钥。

---

## H4-5：每函数独立方言（P1）

### 批准范围

仅在同一安全执行框架内，为每个函数独立生成：

- micro-op permutation；
- operand lane permutation；
- 字宽/varint 编码族；
- block key family；
- constant tag family；
- register slot domain；
- shard 选择；
- rekey 周期；
- NOP/chaff 比例。

### 为什么不直接上多种完全不同 VM

寄存器 VM、栈 VM、CPS VM 混用确实能增强差异，但会新增标准化调用 adapter、
扩大活状态、显著增加语义 bug 和回退路径。当前阶段先做“同框架不同方言”，
不引入跨架构桥接咽喉。

### 验收

- 一个函数的方言描述不能直接解其他函数；
- 不存在全程序统一 opcode map；
- 子函数 closure metadata 也必须随方言加密；
- 方言切换不通过可识别的固定 adapter table。

---

## H4-6：单块数值窗口、重播种与主动销毁（P1）

### 设计

- 每帧默认只保留当前基本块；
- microcode 窗口是数字数组，不是源码；
- 执行完立即覆盖并解除引用；
- cache 上限按构建取 `0..2`，高价值模式强制 `0`；
- 同一块再次进入时使用新的会话轮次状态；
- 常量仍按索引惰性解密；
- closure 创建只解其必要 metadata。

### 禁止

- 启动时批量解全部函数；
- 为性能保留永久完整块缓存；
- 提供可枚举的全局 proto registry；
- 销毁前复制到日志、诊断表或 fallback 表。

### 验收

- 任意时刻内存快照不含完整程序数值流；
- 重复执行同块时静态密文不直接对应相同运行窗口；
- 深递归和 coroutine 回归无泄漏、无栈回退。

---

## H4-7：常量、closure 与调用约定继续去中心化（P2）

### 实施项

- 每常量独立状态保留，但 tag 编码改为每函数方言；
- closure proto id、upvalue descriptor 与参数数量进入独立加密字段；
- Call 家族不共享一份固定参数打包描述；
- Return/Vararg 协议按函数方言生成；
- 字符串只在最终使用点物化，物化后不进入共享缓存；
- 数值常量与字符串常量不共享顺序钥匙流。

### 防破绽约束

- 不新增统一 closure adapter；
- 不让所有调用都经过一个可替换的 Lua wrapper；
- 不把完整常量池放进 metatable 的 `[0]` 或固定隐藏槽；
- 不用“加密表 + 单 getter”替换现有分散结构。

---

## H4-8：完整性失败只做分散语义投毒（P2）

### 批准行为

- 改变后续 micro-op map；
- 改变 constant tag；
- 偏移下一 block seed；
- 交换少量 register slot；
- 延迟数十到数百 micro-op 后产生错误结果；
- 对不同构建选择不同投毒面。

### 禁止行为

- `while true do end`；
- 固定 `error("tamper")`；
- 固定返回 nil；
- 一个全局 `tampered` 布尔；
- 一个明显的 expected checksum 字面量；
- 所有失败都走同一个 poison 函数。

### 验收

篡改测试只能确认“结果不正确”，不能稳定定位第一触发点或得到统一异常文本。

---

# 5. 明确排除的加固点

以下方案可能在某些场景增加难度，但会新增更稳定的破绽、兼容性问题或错误
安全感，因此本轮明确不实施。

| 排除项 | 看似收益 | 排除原因 |
|---|---|---|
| 再套一层/多层源码加密后 `loadstring` | 增加解码层数 | 最终仍把完整明文交给同一个 loader；观察点不变 |
| 把 loader 改名、间接索引或拆字符串 | 隐藏调用位置 | 动态环境按函数行为拦截，不依赖名字 |
| 继续堆 `debug.info == "[C]"` | 检查 native | `debug.info` 本身可被高保真环境伪造 |
| 大量新增 Roblox API 探针 | 提高仿真成本 | 工具已有 API/错误/Vector/Signal 模型；还会增加误报与兼容洞 |
| 时间差、FPS、`os.clock` 作为钥匙根 | 反单步 | 设备/GC/移动端差异导致误伤；攻击者可虚拟化时钟 |
| 固定死循环或固定异常 | 阻止运行 | 形成稳定 oracle，工具可定位并跳过触发 proto |
| 一个中央 micro-VM + 固定 `step()` | 无源码 loader | 形成比现有 handler 更便宜的单一钩点 |
| 一个全局 micro-op closure 表 | 方便实现 | dump 表即可得到完整语义映射 |
| 动态字符串拼接生成函数源码 | 减少完整源码常驻 | 拼接结果仍会进入 loader，且增加可观察阶段 |
| 同时上线栈/寄存器/CPS 三类 VM | 架构差异大 | 标准 adapter 成为新咽喉，测试面和语义 bug 急剧扩大 |
| 执行器专有 bytecode/native API | 避开源码 | 兼容性差、形成执行器指纹和 fallback 漏洞 |
| 仅增加死代码/大随机数 | 扰乱静态分析 | 数据流可证明无效；体积与性能先受损 |
| 最终恒零的“假依赖” | 看似参与钥匙 | 符号化后可直接删除，重演 H2/H3 旧问题 |
| 启动时一次解全部再销毁 | 简化运行 | 启动快照即可拿全程序，销毁没有意义 |
| 全局完整 proto 缓存 | 提升速度 | 一次内存遍历拿全程序 |
| 针对 KryptIT 的字符串/文件名黑名单 | 暂时挡特定工具 | 极易改名绕过，增加错误安全感 |
| 只做水印并称为防破解 | 可追踪泄漏 | 水印不阻止恢复；只能作为独立运营措施 |
| 内嵌一个“秘密主密钥” | 类似离线授权 | 文件内的秘密对攻击者不是秘密，只能白盒分散 |
| 服务端 nonce/远程认证 | 可提供真秘密 | 用户要求纯离线，不进入本方案 |

---

# 6. 实施顺序与提交边界

每阶段单独提交，不允许把架构重写与多项高风险变更混在一次提交中。

## 阶段 A：H4-0 Loader 审计闸 ✅

已交付：

- loader-spy 假环境；
- `[C]` 伪造测试；
- `getfenv(0)` 线程环境克隆；
- 当前架构 54 次明文 loader 基线；
- 报告模式与 `--enforce-zero` 发布门。

阶段 A 已完成，下一提交边界为 H4-1 Handler Microcode。

## 阶段 B：H4-1 Handler Microcode ✅（43/43）

全部 handler 已迁移。HQ 源码碎片 `53 → 41 → 20 → 10`，loader 调用
`54 → 42 → 21 → 11`。剩余 loader 均属于 continuation、Call executor、
BSS、RT 与 HBOOT，进入 H4-2/H4-3 范围。

阶段验收：

```text
handler loader 源码 = 0
169/169
370/370
multiseed = 0 fail
H1/H2/H3 回归不退化
```

## 阶段 C：H4-2 Loaderless Runtime ✅

H4-2A continuation：`11 → 7`；H4-2B Call executor 数值 post-call：`7 → 3`；
H4-2C1 BSS 一次性数值 boot 状态机：`3 → 2`；H4-2C2 RT 五阶段数值初始化
协议：`2 → 1`。Handler/Continuation/Call/BSS/RT 的 HQ 源码碎片现为零，
唯一剩余 loader 属于 HBOOT，转入 H4-3。

## 阶段 D：H4-3 HBOOT Loaderless ✅

HBOOT 三阶段静态数值协议与静态 EXG 已完成，内部 loader `1 → 0`；
`--enforce-zero` 正式门通过，H4 P0 闭环。

## 阶段 E：H4-4 Key-Carrying Chaff（实施中）

H4-4A operand lane 与 H4-4B block seed 已完成；下一步按独立提交扩展到
constant key，再后才接 microcode checksum，不与每函数方言改造混合。

## 阶段 F：H4-5/H4-6 每函数方言与单块窗口

先方言隔离，再缩小运行窗口；不得反序同时进行，避免无法定位错钥。

## 阶段 G：H4-7/H4-8 收口

调用/closure/常量去中心化与分散语义投毒。

---

# 7. 固定验证矩阵

每个阶段至少运行：

```bash
cargo build --release --manifest-path luraph-rs/Cargo.toml
cargo test --manifest-path luraph-rs/Cargo.toml
bash luraph-rs/tests/run_tests.sh
bash luraph-rs/tests/run_presets.sh
bash luraph-rs/tests/multiseed.sh
python3 luraph-rs/tests/redteam_probe_regression.py
python3 luraph-rs/tests/h2_diff_audit.py
python3 luraph-rs/tests/h3_noise_audit.py
python3 luraph-rs/tests/h4_loader_audit.py --enforce-zero
bash luraph-rs/tests/defense_audit.sh
```

H4 新增指标：

| 指标 | P0 完成要求 |
|---|---:|
| 内部 `loadstring/load` 调用 | 0 |
| handler/HBOOT/BSS/RT 明文 loader 参数 | 0 |
| 运行期完整 proto 表 | 0 |
| 同 seed 确定性 | 100% |
| 双 seed 运行等价 | 100% |
| loader-spy 获取用户常量/源码 | 0 |
| 单一全局 micro-op 表 | 不存在 |
| 单一全局 `step/decode_all` 钩点 | 不存在 |
| 当前块退出后的数字窗口残留 | 0 |
| 篡改后固定错误/固定死循环 | 0 |

性能预算按阶段记录，不得静默退化：

- 每个阶段记录 8 个真实语料的构建时间、文件大小和执行时间；
- 单阶段执行时间增长超过 25% 必须解释或回退；
- H4 P0 总体积增长目标不超过当前 v15 的 35%；
- 移动端长循环、深递归、coroutine 和错误语义必须继续全绿。

---

# 8. H4 完成定义

只有同时满足以下条件才能称为 H4 完成：

1. handler、BSS、RT、HBOOT 全部不再生成并加载 Lua 源码；
2. fake `debug.info` 环境无法在 loader 边界获取任何内部明文；
3. microcode map、布局和钥匙状态至少按构建变化；
4. 高价值函数具备每函数独立方言；
5. chaff 实际参与后续真实解码，不是可删除死代码；
6. 运行期最多保留当前小窗口；
7. 修改/删除防御只导致分散语义投毒，无稳定 oracle；
8. 全部现有测试、红队测试、多种子和防御审计通过；
9. 文档明确保留纯离线可被完整动态跟踪的理论边界，不宣传“不可破解”。

---

# 9. 最终决策摘要

本轮不再投入主要精力到“更多环境探针、更多源码加密层、更多死循环”。这些
方案确实可能拖慢低水平攻击，但已经被高保真环境证明存在统一绕路，并且会给
正式用户带来兼容性和误报风险。

批准路线只有一条主轴：

```text
消灭内部明文源码 loader
    → 数值 Microcode
    → 每函数方言
    → Key-Carrying Chaff
    → 单块窗口与主动销毁
    → 分散语义投毒
```

它仍不能违反纯离线客户端的物理上限，但可以把当前“观察 loader 参数”的廉价
突破，提升为“逐构建恢复多 shard Micro-ISA + 完整数值数据流”的长期工程。
