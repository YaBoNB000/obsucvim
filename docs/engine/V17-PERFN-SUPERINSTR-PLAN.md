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
- **词汇项结构**（`engine/src/vmgen/fuse.rs` Phase-1a 已入库）：
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
  全套件 + 真 Delta 验收件（`artifacts/samples/obsucvim/v16-check/` 四件标准道）。

## 8. 未排期 / 待拍板

- ~~§5 白名单与 top-K~~（已拍：5 对 + K=4 见 §5；后续扩对需重新逐对论证无副作用）；
- ~~§7 开关形态~~（已拍：独立 `--super-insn` 默认关，见 §7——稳定后是否翻默认
  属下一轮拍板，默认形态未冻结）；
- 尾部产物体积预算（小函数偏多时 fused 表收益是否仍正，Phase-1b 实测后回填）；
- Phase-1c（若立项）：窗口 3 滑窗与超长链塌缩（`collapse` 已预留多词汇顺序消费）。

---

# Phase-1b（发射端接线）收官记录（R072，2026-10-24）

## 9. Phase-1b 实填（接线落地 + 两个实测定案的根因修复）

**定案范围**：`--super-insn` on 态端到端打通——编译期 collapse/编码、
字节段（每函数第 7 段 fv 词汇表）、模板装载链、运行时 trampoline 派发
全部上线且语义正确；off 态产物与 R070 锚点**逐字节恒等**。

### 9.1 字节段与装载链（最终形态）

- 每函数第 7 段（fv 词汇段）：`u16 n + n × u16 词条`，词条 =
  `(tailCodeA + tailCodeB*256 + fvm) % 65536`（`pack_fv_entry`），随 6 段一起
  shuffle 进函数字节流；walk 状态机 NSECT 6→7、TFV 支臂解析进 `fvt`。
- parse 返回表新增 `fv = fvt, fvm = fvm0`；**`fvm0` 在 TCK 支臂读到原始
  ckseed 的当行立即快照**（`fvm0 = (v * E1 + E2) % 65536`）。
- v15 装载链的 `EP[i]` 重建是**选择性字段拷贝**，本次补拷
  `fv = pf.fv, fvm = pf.fvm`（super-only 文本，off 态纹理不变）。

### 9.2 实测定案的两个根因（防幻影备忘）

R072 调试期 on 态曾双层失守，均按「减法实验 + 数值载荷 trace」定位，非臆测：

1. **fvck 系数的作用域失守**（原罪）： fv_ret_fields 最初用
   `ke.key_expr(...)`（默认 `data_expr="FN"`），但 v15 的 parse 函数被嵌进
   BSS fragment 的内层装载函数，该词法作用域**没有 FN 载体**（只有 `FN_`
   参数）→ 运行期 `(function(d)...#d...)(FN)` 对 nil 取长度 →
   `#nil` → HBOOT 的 PCAL 捕获 → PD() 死环（外症状 = 死循环 rc=124）。
   证据链：产物全文 `Pzr` 仅出现 2 处且**无任何定义点**；EXG_TRACE 抓到
   `attempt to get length of a nil value`。
   修复：fvck 两系数改 `obf_num` 纯数值混淆。代价：fvm 不再绑定载体长度
   （该绑定本非设计性质，是 key_expr 默认行为的副产品）。
2. **fvm 求值时点错位**：`fvm = (cks * E1 + E2) % 65536` 写在 parse 的
   return 表构造点，而 st2 的常量解码（`mb()/lw()`）会**就地推进 cks 的
   LCG 态**——return 时 cks 已是演化终值。实测同构建对照：编译端
   ckseed=59509/fvm=17139，运行端数值载荷抓回 `fvm=38257`
   （对解方程 `Uh ≡ 18139 (mod 65536)`，唯一解 = LCG 演化后的 cks），
   而 `fv[1]=62372` 与编译端 packed 完全一致 → 唯 fvm 错 → trampoline
   解码 `(fv[k]-fvm)%65536` 落到未注册码位（wa=51/187 等）。
   修复：TCK 支臂快照 `fvm0`（见 §9.1）。修复后同一表达式数学自证：
   `cks=59509 → fvm=17139`、`cks=1810 → 2756` 与编译端逐项一致。

调试期约束顺带实测：micro_fill 片段过 ast.rs 迷你解析器，
`error('x'..expr..']',0)` 里的 `..` 拼接不受支持（报 `expected Punct ')'`），
定位信息须走静态文本或 `error(数值,0)` 数值载荷/`print(数值)` 通道。

### 9.3 发射端与运行时派发（最终形态）

- 码位：tramp_k = `t*64+44+k`（k<FUSE_TOP_K=4）；tail-less_j =
  `t*64+48+j`（j∈FUSE_TAIL_OPS=[LoadK,Move,LoadNil,GetUp]）；
  每方言空闲区 [base+44, base+64) 20 码位，on 态按 used_t 集合注册。
- trampoline 形态：`local fv=E.bt.fv local fe=(fv[k+1]-E.bt.fvm)%65536
  local wa=fe%256 HW2[(wa+AV)%256](E,a,b,c,d) fe=((fe-wa)/256)%256
  HW2[(fe+AV)%256](E,c,d,c,d); <chain_epilogue>`（E.bt = 当前函数 pf）。
- tail-less 四 op 语义全直译、无 return/pc/metamethod，尾衔由 trampoline
  内嵌完成后接 chain_epilogue，链预算口径不变。

### 9.4 验收矩阵（本论全绿）

| 门禁 | 结果 |
|---|---|
| off 态锚点（`verify-engine-version.mjs`） | **231,571B / 6c22c008… ≡ R070/R071 逐字节恒等** ✓ |
| 单测 `rustc --test` | 57 绿（含 fuse 塌缩/映射/编码、section_walk、vocab_consistency、id7 臂） |
| on 态运行矩阵（luau 直跑） | 8 输入（print/子函数/循环/globals+upvalue 闭环/字符串/table/varargs/pcall/递归 fib16）× 4 种子 = **32/32 输出逐字节正确** |
| on 态 bind-executor 产物的 exguard 拒绝路 | 裸 luau 下精确缺失清单、**不再挂死**（原罪修复的直接回归） |
| V1 结构证据 | 同形态双子函数 fa/fb + main：词汇=[(4,5),(5,4)]/[(5,4),(5,5)]/[(5,5)] 互异、方言 s_idx=[1,1,2]、fvm=[10946,28959,22615] 互异 ⇒ descriptor 段与 handler 码位双互异 ✓ |
| 仓库套件 | artifact-runtime **0 fail**、exguard-precheck **0 fail**、keyhost-fold **PASS**、attest **18/18**、loader **24/24**、live-loader **13/13**、engine-retry **9/9**、hosting **78/78** |

### 9.5 已知观察项（不阻塞，移交 Phase-1c 回填）

- handler 计数差观察：on 态 HW2 注册 59 = 43 基础（本构建序号 0..42，nop_alias
  独立 1 槽未计入基础）+ 16 fused（2 方言 × 8）；off 态 44 = 43+1 nop_alias。
  口径自洽（on 态 fused 走 HW2 扩展槽、nop_alias 注册位不变），非丢注册。
- 体积：探针 on 态 198KB vs off 态 231KB 同种子同配置（塌缩减 ISA 记录数，
  与计划 §2.3 预期一致）；Kkksu 级件的体积预算回填移交 Phase-1c。
- `--seed` 流预算：super on 态每函数多耗 `s_idx∈{0..3}` + fv_m1/fv_mc + 段
  shuffle 的 rng 抽取已在 Phase-1a 钉死（off 态零消耗证据 = 锚点恒等）。

### 9.6 真机验收收口（2026-10-24）

`artifacts/samples/obsucvim/v17-check/` 四件由用户在真 Delta 环境回传：

| 件 | 结果 |
|---|---|
| `Kkksu.v17fusion.hard.lua` | 打印 `kkksu`，无报错 ✓ |
| `Kkksu-mid.v17fusion.hard.lua` | 精确打印 `kkksu-mid:1,1,2,3,5,8,13,21,34,55|143` ✓ |
| 两个 `-trace` 件 | 未动用（无失败需诊断） |

**V3 真机段通过，Phase-1b 验收正式收口。** 剩余待拍事项移交 §8：
窗口 3、`--super-insn` 翻默认、Kkksu 级体积预算回填。

### 9.7 R073 体积收敛（2026-10-24，§8 体积预算项销项）

用户验收 Phase-1b 通过后直接出题：打印级 ~220KB 偏大 → 目标 180–200KB。
解剖与定案（全部实测，细节见 ENGINE-VERSION §22）：

- 主项 = `_mx{i}` 微解释器 5 份逐字节同源展开（~70KB）+ 44 个派发 stub +
  132 对别名拷贝；shard 数 rng(3,5) 是种子方差主驱动。
- **否决项留档**：按「方言用谁注册谁」过滤臂会破坏 md 滚动方言
  （`E.md=(E.md+…)%E.mx` 运行时轮换，pg≠0 后 md 遍历全部 shard）——
  滚动是保护性质（慢环境降级仍须正确），保护不丢 ⇒ 只许表征级去重。
- 实装：工厂化共享函数体 + 薄包装 + 别名 pairs 循环；`d{op}` 编译期拼名
  关节改运行期 `DK` 包选择。
- 结果：off 锚点 231,571→**171,881B**（R073 新锚点）；16 件种子扫描全落
  161.8–172.8KB；64 格矩阵 + 八套件全绿；Kkksu 级 163KB/182KB。
- 体积预算口径就此锚定：打印探针 ≤173KB、Kkksu-mid ≤183KB；
  留 §8 剩余两项：窗口 3、`--super-insn` 翻默认。
