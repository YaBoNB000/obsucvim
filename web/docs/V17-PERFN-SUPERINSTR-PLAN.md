# V17 函数级超指令融合计划（数据流驱动 per-function superinstructions）

立项日：2026-10-24（用户拍板，backlog 晋升正式计划）
状态：**Phase-1a 已落地（2026-10-24，R071）**——用户已拍板白名单保守起步与
独立 `--super-insn` 默认关形态；数据流统计/选词/塌缩演练门面入库且 on/off
产物逐字节恒等（§6 实测）。Phase-1b（发射端接线）待下一轮动工。

> 名词对齐：Luraph 语境下「superinstruction」= 按**每个函数**的指令流数据
> 特征生成该函数专属的融合指令集——同一源码 op 在不同函数里落成
> 不同形态/词汇，跨函数的 handler 语义不可迁移。本文以此为验收定义，
> 不以「词面上有 fuse 字样」为满足。

## 0. 现状盘点（代码实证，防幻影：全部以当前真树为准）

### 0.1 已经有的（per-function 地基）

| 机制 | 位置 | 粒度 |
| --- | --- | --- |
| 操作数槽位置换 | `slot_perms: Vec<[u8;4]>`，`compiler.rs:27-28`、`encode_soa_for` 使用点 `compiler.rs:590/722` | **每函数独立**（scatter 开时） |
| opcode 方言（4 套池随机指派） | `s_indices: Vec<u8>`（`compiler.rs:51`），每函数取一个 `rng.int(0,3)`（`compiler.rs:1320/2427`），编码经 `opmap.code_for(op, s_idx)` | **每函数一个方言**（4 选 1） |
| micro-op 编码 + 字段置换 | `MicroShard{codes[M_COUNT], field_perm}`（`template.rs:537-548`），3~5 个 shard（`template.rs:1961` `n_micro_shards = rng.int(3,5)`） | per-shard（宏池级） |
| handler 实现级塌缩 | `M_FUSE_*`（10 形态）+ `MICRO_SUPER_NAMES`（23 形态），handler 体内联 | 全局静态 |

### 0.2 缺的两块（=本计划要补的）

1. **数据流驱动**：`micro_primitive_recipe`（`template.rs:550`）是**写死的全局查表**——
   任何函数里同一个 op 永远展开成同一条 recipe，不读函数内容、不统计、不挑选。
2. **ISA 级相邻融合**：`encode_soa_for`（`isa.rs:312`）按「一条 Instr 一条记录」
   编码，相邻两条指令从不在 ISA 层合并成一条记录。融合止步于 handler 内部实现。

### 0.3 现状里可复用的接缝

- `register_micro_variant`（`template.rs:1056`）：recipe→descriptor 注册已支持
  「同一逻辑、多 shard 方言编码」——per-function 词汇表可沿此路扩；
- dispatch 树 `gen_dispatch_tree`（`template.rs:38`）+ `micro_tree`（954）：
  handler 派发已是生成式，参数化改造有落点；
- per-function 编排先例：`slot_perms[fi]`/`s_indices[fi]` 证明「逐函数差异编码」
  在当前架构成立且有运行矩阵背书（R067 起）。

## 1. 定义与验收（动工前写死）

- **V1（功能定义）**：至少 N=K 对「可安全融合的相邻 op 组合」按**该函数的
  指令流统计**选用；同一源码段在不同函数体中产生的融合词汇**互不相同**；
  字节级证据：同一 suda/mid 输入的各函数 descriptor 段互不相同（结构可证）。
- **V2（回退保险）**：开关 off 时产物与 R070 **逐字节相同**（同 R068 纪律，
  锚点断言行级断言）。
- **V3（行为不回退）**：运行矩阵全绿（含真 Delta hard+keyhost 链；
  融合不得改变任何函数的可观察行为，含错误时机/类型）。
- **明确不做**：不防内存 dump、不替代 v16-a、不承诺「不可破解」。收益口径 =
  跨函数 handler 语义不迁移（逆向摩擦持续抬工时），与 §11 离线约束自洽。

## 2. 数据流统计 pass（Phase-1 第一站，compiler.rs）

对每函数 `Vec<Instr>` 做滑窗统计（窗口 2，预留 3）：

1. 遍历 op 序列，统计相邻对 `(op[i], op[i+1])` 频次；
2. 过滤白名单（§5）后按频次降序取 top-K（K 起步 4，产物体积随线性）；
3. 命中对在发射阶段塌缩成**一条** fused 记录（ISA 记录数减少，产物反而小幅变短）。
4. 判定指标也要 per-function：同名对在不同函数选中与否由该函数自身统计决定。

副作用安全是硬前提：融合对必须满足「合并后与原序逐一执行的可观察行为逐点相等」
（含错误时机、metamethod 触发序、pc 边界）。

## 3. ISA 层 fused 槽设计（Phase-1b 落点，结构已由 Phase-1a 词汇模型钉死）

- **码位来源**：OpMap 每方言区间 `[t*64, t*64+64)`，基础 op 占 43，**空闲 21 码位**
  = fused 词汇落点；fused 码 = `base(t) + 43 + k`（k 为词汇序位，k ≤ 20，
  与 FUSE_TOP_K=4 的起步规模远未顶格）。
- **词汇项结构**（`obsucvim/src/vmgen/fuse.rs` Phase-1a 已入库）：
  `enum FKind { Base(u8), Fused(u8) }`；`struct FRec { kind, a, b, c, d }`——
  fused 记录携带原两条的全部 2+2 逻辑通道，布局 `[X.a, X.b, Y.a, Y.b]`。
- **发射端改造（Phase-1b）**：`encode_soa_for` 消费 `Vec<FRec>` 替代
  `Vec<Instr>`；`FKind::Fused(k)` 写的 opcode 字节 = 本函数方言 base+43+k，
  通道流仍按本函数 slot_perm 走 4 条 u16var 流——**SoA/散射/slot_perm 面零改动**。
- **跳转禁忌处理**：`collapse()` 输入 leaders 集（finish() 已收集返回），
  凡 pair 第二条（旧 1-based `i+2`）∈ leaders 即禁融；分支 b 槽经
  `old_to_new` 映射重写到塌缩后记录索引，`debug_assert!(nt != 0)` 守空索引。

## 4. 模板 handler 参数化（Phase-1b 设计要点）

- fused 码位挂 **HW2 派发链扩展槽**（主循环 `HW2[(oc+AV)%256]`，template.rs:1180 区）；
- fused handler 文本 = **两条原 recipe 顺序拼接**，第二条的 x/y 流偏移 +2
  （读 `[X.a,X.b,Y.a,Y.b]` 后 4 逻辑通道全部就位再跑第一语义、第二语义）；
- 每函数一个 **FV 数据段**（fused vocab 表：k → (原 op 对)），用于反汇编/调试面；
- 复用接缝：`register_micro_variant`（template.rs:1056）→ `{shard, offset}`
  的 per-方言编码路径、dispatch 生成式树皆有先例，不造新基础设施。

## 5. 白名单与 top-K（用户已拍板「1」= 保守起步）

- **白名单 5 对**（全部 2+2 逻辑通道、无副作用、无 metamethod、无 pc 语义）：
  `(LoadK,Move)`、`(Move,Move)`、`(LoadNil,Move)`、`(GetUp,Move)`、`(Move,LoadK)`——
  全篇仅常量/寄存器/上值搬运，可观察行为逐点等相的前提由 op 设计保证，
  不是靠逐案审查。
- `FUSE_TOP_K = 4`（起步值；尾部体积预算见 §8）。
- **确定性**：统计 `BTreeMap`、排序「频次降序、同频 pair key 升序」、全程零 rng
  ——on/off 字节恒等与可复现性的结构根源。

## 6. 测试矩阵（Phase-1a 实跑结果，2026-10-24 / R071）

| 层 | 结果 |
| --- | --- |
| rustc --test（原生） | **53 绿 / 0 败 / 1 ignored**（含新增 fuse_tests 6 项：白名单过滤、确定性选词、tie-break、leaders 禁融、分支重映射、plan 门面幂等） |
| on/off 字节恒等 | 4 配置全过：锚点集（种子 67067）、hard+keyhost（20261022）、minify 集（777）、plain（12345）——**同种子 on≡off 逐字节相同** |
| V2 回退锚点 | off 态产物 = 231,571 B / `6c22c008…` = R069/R070 锚点，**未被 Phase-1a 扰动** |
| 既有全套件 | artifact-runtime 27、keyhost-fold、exguard-precheck、hosting 78、loader 24、attest 18、live-loader 13、engine-retry 9，全绿 |

顺带修复（同一 wasm 之前无法过 --test 的存量问题）：
- compiler.rs 三处 `#[cfg(test)]` 夹具仍在用 5 参 `compile_chunk`（+1 `false` 对齐新签名）；
- envcheck 单测 `tier2_flag_changes_source_and_keeps_tier1_shape` 的
  `zm[\"IsStudio\"](zm)` 针残留旧转义层级（生成器现为无反斜杠形态），
  HEAD 上同样败——已按真生成器输出修针。（两项都是夹具陈旧，引擎语义零改动。）

## 7. 开关与阶段门禁（用户已拍板「1」= 独立旗标、默认关）

- **`--super-insn`**：独立试验旗标，默认关；help 文本明示「Phase-1a 仅演练」。
- **Phase-1a（本轮，R071）**：统计→选词→塌缩演练在 `Ctx::finish()` 里以
  `fuse::plan(code, leaders)` 无名元组形式跑完即弃，**不消费 rng、不触碰发射端**，
  因此 on/off 产物逐字节恒等（§6 已实测断言）——这就是 Phase-1b 动工前的
  稳定基线。
- **Phase-1b 门禁（动工前自查清单）**：①发射端接线后先验 V2（off 态仍≡R070 锚点）；
  ②V1 结构证据（同 op 跨函数词汇互异、descriptor 段互异）；③V3 运行矩阵
  全套件 + 真 Delta 验收件（`samples/obsucvim/v16-check/` 四件标准道）。

## 8. 未排期 / 待拍板

- ~~§5 白名单与 top-K~~（已拍：5 对 + K=4 见 §5；后续扩对需重新逐对论证无副作用）；
- ~~§7 开关形态~~（已拍：独立 `--super-insn` 默认关，见 §7——稳定后是否翻默认
  属下一轮拍板，默认形态未冻结）；
- 尾部产物体积预算（小函数偏多时 fused 表收益是否仍正，Phase-1b 实测后回填）；
- Phase-1c（若立项）：窗口 3 滑窗与超长链塌缩（`collapse` 已预留多词汇顺序消费）。
