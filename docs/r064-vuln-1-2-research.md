# R064 漏洞 1/2：复核与修复方案

> 日期：2026-10-07
> 分支：`arena/01a038eb-luraph`
> 状态：研究/方案阶段；本记录不代表两项问题已修复。

## 摘要

| 编号 | 问题 | 证据 | 影响/边界 | 建议优先级 |
|---|---|---|---|---|
| 1 | 变参表尾 `nil` 的参数个数被 `#` 少计 | `select("#", ...)` 的源结果为 5，R064 VM 结果为 4 | 特定含洞/尾 nil 输入会改变程序语义；不是密钥泄漏 | P1 正确性修复 |
| 2 | micro shard 解码后的执行路径可被动态跟踪 | 临时生成器探针记录了 7KB 样本 906,284 条微码记录、覆盖 4 个 shard，正常结果仍出现 | 需要分析者能插桩/修改执行环境；只得到已执行路径，不会自动恢复源代码或未执行分支 | P1 条件性动态观察风险 |

## 1. 变参尾部 nil：原因与修复方案

### 复核

当前 `template.rs` 的 v15 `makefn_decl` 大致采用：

```lua
local all = { ... }
local vargc = #all - npa
```

尾部 `nil` 不会形成稳定的表长度边界；`#all` 不能代表实参总数。此前的最小复现：

```lua
local function count(...) return select("#", ...) end
print(count(nil, "tail", false, 7, nil))
```

源码输出 `5`，seed `64066` 的混淆产物输出 `4`。legacy `makefn_decl` 也用 `#all - pf.nparams`，有同类风险。现有多值语料覆盖了 select、调用展开和转发，但没覆盖显式尾部 nil。

### 建议修复

在构造 `all` 之前从已有捕获原语精确取参数总数，然后继续单独保留 `vargc`：

```lua
local argc = SEL("#", ...)
local all = { ... }
local vargc = argc - npa
if vargc < 0 then vargc = 0 end
```

legacy 路径对应使用 `argc - pf.nparams`。`SEL` 是 VM 已有的捕获 `select` 原语，应优先使用该局部别名，不要再读可替换的全局。`vargc` 必须作为独立计数继续传递；不能从含 nil 的 `all` 或 `vargs` 表重新推断长度。

建议把以下用例加入 `stress_multival.lua` 并纳入全部 seed/profile 回归：直接尾 nil、前导/中间 nil、固定参数加变参、变参转发链、闭包中转发，以及 `return ...`/函数调用尾部展开。通过标准是 `select("#", ...)` 的数量和每个返回槽逐项匹配原程序。

## 2. Micro-shard 动态追踪：发现与方案

### 已有证据

H5-P0 已封存静态 `{program,mask,step}` descriptor 表；其静态提取审计通过。但执行时仍需从 packed/session carrier 逐条解出三字节记录，`micro_shard_source` 随后在局部作用域中取得 `op,x,y` 并调度。该函数还持有 shard offset、`E`、`O`、临时 `T`、VM 寄存器/常量表等上下文。

在临时副本的 `op,x,y` 解码后插入观察点，R064-7KB 样本产生 906,284 条记录、覆盖 4 个 shard，结果仍可观察。探针及输出都在 `/tmp`；没有加入正式产物。它证明的是：拥有源码补丁或足够执行器插桩能力时，可以收集**该次实际执行的微码轨迹**。它不是对未修改样本的静态脱壳，也不能推出未执行分支或原始源变量名。

### 修复/缓解方案

这不是纯客户端可彻底消除的秘密泄漏：运行时必须执行这些操作，拥有同进程代码插桩能力的观察者最终能记录执行结果。方案应分层，不宣称“无法追踪”：

1. **先定义信任边界**：密钥、授权、经济/反作弊等高价值决策移到服务端；客户端混淆只作成本层。
2. **维持 at-rest/session 层**：保留 H5-P0 的每 shard packed carrier、独立 key 与 session reseal，不恢复全量明文 descriptor/cache。
3. **降低逐条映射价值（后续设计项）**：研究安全子集的函数级 superoperator/fusion，把无异常边界的短微码序列融合成每函数/每 shard 方言；首轮排除 Call/Return、metamethod、错误边界和有可观察顺序风险的操作。目标是提高轨迹提升成本，不是阻止记录。
4. **缩小状态可见面（候选）**：评估按操作族传递窄状态视图/专用闭包，避免每个 shard 都能访问完整 `V/C/E`。这会增加体积、复杂度与性能风险，需先做小型 profile 原型和语义矩阵。
5. **独立完整性 lane 是抗篡改，不是防观察**：每 shard checksum/route state 可用于发现修改；它不能阻止透明记录，不能作为“无 trace”验收。
6. **计时/反 trace 仅作辅助信号**：阈值可能误伤低端设备，也容易受稳定、低开销插桩影响；不要把它当主要解法或唯一拒绝路径。

建议的验收不是“探针永远抓不到数据”，而是：在授权的临时红队 harness 中量化单构建/多构建需要观察的 shard 数、实际路径覆盖和语义提升成本；同时确保无完整 at-rest descriptor 泄漏、干净运行等价，并记录未覆盖分支。Superoperator 与状态视图目前是**候选方案，不在本次研究中实施**。

## 后续顺序

1. 先修复尾 nil 参数计数并加入回归（窄范围、可确定验证）。
2. 继续以 H5-P0/H4-H8 审计为基线，设计一个小型、可回退的 superoperator profile；对比 trace 提升成本、输出大小、耗时和语义矩阵。
3. 再评估 state view/按 shard 完整性；不引入新的共享解码 getter、全量明文缓存或固定失败 oracle。
