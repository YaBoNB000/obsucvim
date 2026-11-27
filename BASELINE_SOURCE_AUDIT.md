# BASELINE_SOURCE_AUDIT — obf.lua 技术移植规范 Phase A 源码审计

> 审计对象：`arena/723e28ef-obsucvim` @ `8f6cf9a`（2026-11-26）。
> 方法：只读 Rust 源码与测试，不以仓库 Markdown 的自述为据。行号为审计时点行号。
> 规范来源：`obf_lua_tech_extraction_agent_spec.md`（下称“规范”）。

## A1. 工作区状态

- 分支 `arena/723e28ef-obsucvim`，HEAD `8f6cf9a`，工作区干净（沙箱重置后已按
  临时 index 比对流程从 `origin` 恢复，磁盘与远程逐字节一致后才 `reset --hard`）。
- 工具链：`.tools/bin`（rustc/cargo 1.88、lua51 5.1.5、luau/luau-compile 0.735），
  由 `engine/tools/rebuild_tools.sh` 重建。

## A2. 真实实现基线（逐项证据）

### 序列化 / prototype 结构
- 每函数在 `Ctx::finish`（`engine/src/vmgen/compiler.rs:530`）内组装 sections：
  `(0, head=nregs/nparams/vararg)`、`(1, ckseed)`、`(2, upsrc)`、`(3, consts)`、
  可选 `(4, s_tab scatter slots)`、`(5, code SoA)`、super 时 `(7, fv vocab)`
  （`compiler.rs:914-981`）。section 6 明令禁止发射（`compiler.rs:986` 的
  `assert_ne!(*id, 6)`，且测试 walk 对 id 6 panic）。
- sections 在发射前 `self.rng.shuffle(&mut secs)`（`compiler.rs:982`）——
  **section 顺序已按函数随机**；但 section 的 tag 字节来自全程序共享的
  `program.section_tags: [u8; 8]`（`compiler.rs:42`），在 `compile_chunk`
  由 `(1..=250)` 洗牌取前 8（`compiler.rs:1663-1666`）——**tag 映射是构建级、
  非每函数**。
- blob 加密：stage-1 非线性折叠 `fold_key_nl` + 明文级联 LCG，`base` 含
  `fn_index * blob_step`（`compiler.rs:990-1013`）——每函数密钥流已分离。
- 运行时解析：`template.rs:1707` 的 `walk_head` 按 tag 逐 section 走查；
  code section(5) 走 `else` 兜底分支（无显式 tag 比较）。`NSECT` =
  5（非 v15）/6（v15）/7（v15+super）（`template.rs:1650-1659`）。
  v15 下 parse 两阶段：st=1 走查、st=2 才解码常量段（walk 时仅按 `csl` 跳过
  字节并记录 `cstart`，`template.rs:1793-1808`）——常量段已是“定位后再解码”。
- 审计自检：`compiler.rs` 测试内 `section_walk`（:3133）与
  `sealed_pool_strings`（:3218）以 `program.section_tags.iter().position(...)`
  反查 tag，断言 blob 被恰好消费完。

### opcode 映射与分发
- 语义 opcode：`isa.rs:3-46` 的 `Op`（44 个，`N_OPS=44`）。
- `OpMap`（`isa.rs:141`）：`to_wire: [[u8; N_OPS]; 4]` —— 4 套“方言”映射，
  每套是 base 偏移 + 洗牌置换；每函数经 `s_indices[fn]` 选一套方言
  （`compiler.rs` `slot_perms`/`s_indices`）。即 opcode id 是**构建级 ×4 方言、
  按函数选方言**，不是每函数独立置换。
- 非 v15 分发：`template.rs:16` 的 `gen_dispatch_tree` 已按随机 gap/阈值递归
  生成比较树（depth≥3 退化为洗牌 elseif 链），在 `template.rs:1552` 调用。
- v15 分发：`v15.rs:541` 的 `dispatch_tree` 生成平衡 `<=` 阈值树（状态机 scaffold
  内部使用），与 VM 主分发是两条路径。
- superinstruction：`fuse.rs:20` `FUSE_WHITELIST`（五对）、`collapse`（:94）、
  `encode_soa_fused`（:165）；leaders 在塌缩前计算并重映射（`compiler.rs:713-749`）。

### operand lane / SoA
- `encode_soa_fused` 把指令拆为 opcode 流 + 4 条 operand lane（SA/SB/SC/SD），
  lane 内按 `slot_perms[fn]` 置换（`compiler.rs:146` 的 `operand_perm_from_code`
  给出 24 种置换，按 fn 下标派生）。未用 operand 以分布匹配的噪声填充。

### block-lazy decode
- `template.rs:3205` 的 `bdec(E, bi)`：按基本块解码，缓存 `E.bch[bi]`，
  缓存项含 remask 值 `wm=(E.er*WM+bi*WB+fi)%65536`，`fi` 由
  `((pf.sx-PM-1)%65521)+1` 派生（函数身份进入掩码）。`E` 由 `newE`
  （`template.rs:3216`）**每 activation 新建**——frame 隔离已成立，
  协程挂起时 frame 状态随 `E` 存活。block 列表来自 `block_starts`
  （leaders，编译期记录）。

### 字符串保护
- `strseal.rs`：字面量密封（`encode`/`decode` :21/:36、`seal_literals` :58）、
  P1-2 clear-after-last-use、P1-3 分布式 canary；`strpool.rs`（48 行）运行时池。
  本规范不重复实现字符串层。

### closure / upvalue / call / return
- `handlers/closure.rs`（v15：`makefn(E.k[b+1], V, ups, S)`）、
  `handlers/call.rs`（多返回值窗口、`callcap`、`resolve_call` 自调用偏移）、
  `handlers/ret.rs`（255 = 开放多返回 + vararg/lastcall 拼接）。
- upvalue 置换：`derive_upvalue_permutation`（`compiler.rs:100`，scatter 时
  按 fn 盐洗牌）；upsrc 以 section 2 序列化，open/closed cell 语义由
  template 的 `makefn`/cell 机制承担（HANDOFF 历史轮次已验证，源码为准）。

### API capability
- `caps.rs` 的 `VmPrim`（19 个原语，`caps.rs:91-119`）+ `prim_slots`
  （`template.rs:94`：slot 取 1..=80 洗牌）——数字索引 API 表 + 每构建置换
  **已存在**；`CapabilityManifest::from_program`（`caps.rs:466+`）按程序实际
  用到的 opcode/原语裁剪。

### bootstrap
- 生成产物为多阶段 BSS fragment 装载（`template.rs:3301` 的 shell 函数签名、
  分片 dispatch `_ri` 循环、阶段码 `hb_phase_codes`）——分阶段闭包装载已存在，
  但不是规范 §3.1 的四段显式 `BootstrapEmitter` 结构。

## A3. 差异矩阵（EXISTS / PARTIAL / MISSING）

| 规范项 | 状态 | 证据与缺口 |
|---|---|---|
| 函数级私有字节码 | EXISTS | `compiler.rs` 全链 |
| SoA / operand lane + 置换 | EXISTS | `fuse.rs:165`、`compiler.rs:146` |
| superinstruction 融合 | EXISTS | `fuse.rs:20/94` |
| basic-block leaders | EXISTS | `compiler.rs:713` leaders 计算 |
| block-lazy decode | EXISTS | `template.rs:3205` `bdec`，缓存键含 fi/bi/er |
| 字符串密封 | EXISTS | `strseal.rs`、`strpool.rs` |
| upvalue 置换 | EXISTS | `compiler.rs:100` |
| 数字索引 API 表 + 置换 | EXISTS | `caps.rs:91`、`template.rs:94` |
| opcode 映射 | PARTIAL | 构建级 ×4 方言 + 按函数选方言（`isa.rs:141`）；**非每函数独立置换**；非 v15 已有随机比较树（`template.rs:16`），v15 主分发不经该生成器 |
| section 布局随机化 | PARTIAL | 顺序按函数洗牌（`compiler.rs:982`），**tag 映射构建级共享**（`compiler.rs:1663`）；无每-prototype 字段槽位布局对象、无格式版本字段 |
| 惰性字段解析（lazy field resolver） | PARTIAL | 常量段 walk 时只定位后解码（`template.rs:1793`）；**无** descriptor 目录、无 Unresolved/Resolving/Resolved/Failed 状态、无 nil sentinel 缓存、无子原型惰性物化 |
| Decoder State Machine（独立状态域） | MISSING | parse walk 是模板内硬编码 if/elseif（`template.rs:1707`）；无状态机 IR、无 verifier、无随机状态标签生成器 |
| DispatchTree 独立模块 + 覆盖校验 | PARTIAL | 逻辑在 `template.rs:16` 内联，无独立模块、无“每个有效 opcode 恰达一叶”的生成后校验 |
| Runtime mask per-page context / 完整性状态 | PARTIAL | `bdec` 的 `wm` 已含 fi/bi/er；**无**显式 `DecodeContext{prototype,block,lane,entry_domain,epoch}`、无 per-page integrity record 状态机（完整性靠 CK/CK2 全程序和校验，`template.rs:1786` code_extra 累加） |
| CCPD entry domain | MISSING | 无 EdgeId/ClosureEntryDomain（规范 §7 本为可选；本分支未实现 CCPD） |
| 分阶段 BootstrapEmitter | PARTIAL | BSS 分片装载已有多阶段结构，非规范四段显式发射器 |
| Handler micro-state 拆分 | MISSING | handler 是单块代码串（`handlers/*.rs`），CALL/CLOSURE 等未拆 micro-state |

## A4. 本轮实施范围（按规范 §6 顺序，标注真实深度）

1. **Phase B（完整接入）**：新增 `proto_layout.rs`（`ProtoFieldId` 对齐真实
   section schema、`ProtoFieldLayout` 双射布局、目录编码/校验 fail-closed），
   并把 `section_tags` 改为**每函数独立布局**进入真实序列化/解析链路；
   CLI `--no-proto-layout` 可单独关闭（关闭时全函数共享同一布局 = 旧行为）。
2. **Phase C（IR + 发射器，运行时接入 PARTIAL）**：新增 `lazy_fields.rs`
   （descriptor IR、四状态、nil sentinel、递归/失败语义的 Rust 参考实现 +
   Luau resolver 发射），以 Rust 测试钉住语义；运行时 parse 仍保留现有
   两阶段常量解码，不在本轮把 C 表换成 proxy（原因与风险记入审查报告）。
3. **Phase D（IR + verifier + 发射器，PARTIAL 接入）**：新增
   `state_machine.rs`：IR、图校验（可达/出边/终止/进度）、随机标签、
   Luau 分发树发射、参考解释器差分测试。首批建模对象 = section reader
   （header/varint/section-directory/field-descriptor）。
4. **Phase E（模块化 + 校验）**：新增 `dispatch_tree.rs`，把
   `template.rs:16` 的内联生成器迁入并加覆盖校验（每个语义 opcode 恰达
   一叶、invalid 落失败叶）；template 改调用新模块（行为同构迁移）。
5. **Phase F（接口 + 测试，PARTIAL）**：新增 `runtime_mask.rs` 的
   `DecodeContext` 派生（定宽、wrapping、Rust 参考实现）与 cache key 类型；
   `entry_domain` 显式默认 0（无 CCPD 来源，不伪造 edge id）；不改 `bdec`
   的既有掩码公式（其 fi/bi/er 已提供同类上下文，改动收益/风险不成比例，
   记入审查报告）。
6. **Phase G（复用 + 补缺口）**：API slot 与分阶段装载已 EXISTS（见上），
   不重复实现。补的缺口：`prim_slots` 抽成 `api_slots.rs` 显式模块
   （allowlist 契约、每构建洗牌、唯一性校验 + 单测；调用点/ rng 顺序
   不变，产物字节中立）；G4 运行时矩阵落为
   `engine/tests/phase_g_api_test.py` 并挂入 `run_tests.sh`（entry
   closure 多次调用/父返回后延迟调用/coroutine resume/槽位重映射
   字节变而行为一致/缺失 API 确定性失败）；CCPD 只落
   `runtime_mask.rs` 的显式接口（见上表），不产出变体。

退出条件核对：差异矩阵完成；新增能力均为上表 PARTIAL/MISSING 项；
未重复实现 block-lazy、SoA、string sealing。
