# V17 函数级超指令融合计划（数据流驱动 per-function superinstructions）

立项日：2026-10-24（用户拍板，backlog 晋升正式计划）
状态：Phase-0 设计定稿（本文档）。**动工前需用户对 §5 融合对门槛与 §7 开关形态拍板。**

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

## 8. 未排期 / 待拍板

- §5 融合对白名单的最终名单与 top-K 值；
- §7 开关形态：独立 `--super-insn`（默认关 → 稳定后翻）还是并入 v15 默认；
- 尾部产物体积预算（小函数偏多时 fused 表收益是否仍正）。

（§3-§7 小节序号预留：§3 ISA 层 fused 槽设计、§4 模板 handler 参数化、
§5 白名单、§6 测试矩阵扩容、§7 开关与阶段门禁——Phase-0 评审后落笔正文。）
