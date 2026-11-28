# OBF Lua 技术审查报告（obsucvim / arena/723e28ef-obsucvim）

- 日期：2026-11-26
- 范围：按 `obf_lua_tech_extraction_agent_spec.md` §6 的 Phase A–G
  顺序，对本分支实施审计、部分补全与三处真实缺陷修复后的最终审查。
- 标注口径：ACTIVE（产物中生效）/ PARTIAL（接口或参考语义已落，
  运行时未完全接入）/ DORMANT（代码存在但未接线）/ NOT_WIRED。
  **禁止把静态存在写成运行时生效**；本报告严格按此口径。
- 证据基准：`cargo test`、门禁脚本与测试用例的实际运行输出；
  凡未实测者均注明。基线对照提交为分支基点 `8f6cf9a`（Phase 前）。
- 提交：`1f86314`（Phase A–F + 三处缺陷修复 + 审计钩子调和）、
  `9500a26`（Phase G）。

---

## 1. 总览

| Phase | 主题 | 状态 | 产物生效面 |
|---|---|---|---|
| A | 基线审计 | 完成 | `BASELINE_SOURCE_AUDIT.md` |
| B | Proto 布局随机化 | ACTIVE（标量 if/elseif 链形态） | section tag 每原型独立布局 |
| C | Lazy fields | PARTIAL | Rust 参考 resolver + Luau 发射器；parse 运行时仍 eager |
| D | State machine | PARTIAL | IR + verifier + 发射器；未替换模板内硬编码 parse |
| E | Dispatch tree | ACTIVE（非 v15 主分发） | 模块化 + 覆盖校验；v15 主分发不经该树 |
| F | Runtime mask context | PARTIAL | `DecodeContext` 接口 + 测试；`bdec` 公式未变 |
| G | API slots + 分阶段闭包 | ACTIVE（复用为主） | `api_slots.rs` 模块化 + 生成期校验；G4 行为矩阵 |
| §7 CCPD | 条目域隔离（可选） | INTERFACE ONLY | 仅类型/契约；零变体发射 |

**缺陷修复（独立贡献，均为线上潜伏 bug，非本轮新引入）：**

1. **super-instruction 码位碰撞**（`isa.rs`）：`fused_tramp_code`
   旧值 `base+44` 在 `active_count=44`（含 loadstr 时）与
   `nop_alias=base+44` 同槽，别名拷贝覆盖 FUSED(0) 子跳板，
   `FUSED(0) or NOP` 永假 → 对应 fused 指令整体被跳过。受害用例
   13 个（含 strings 类）。修复：tramp 移至 `base+48`、tail
   `base+52`。证据：复现矩阵修后 13 用例 × seeds 1–24 全过；
   基线 `8f6cf9a` 在同矩阵同等失败率，证明潜伏。
2. **计时守卫误报**（`template.rs`）：相对阈值
   `_dt > _minbase*100` 在慢速沙箱下把正常执行误判为调试暂停，
   r064 seed 12345 15 个用例中 5 个中招。修复：加绝对下限
   `_dt > 0.25`，语义（连续两次才触发）不变。修后 15/15。
3. **charhook 篡改静默失效**（`v15.rs` scaffold）：bootstrap 入口
   被 `char(65)→''` 类钩子腐蚀后，trampoline 对非函数 B 静默尾
   调用，产出普通 nil-call 而非守卫签名。修复：guard 构建在入口
   加 `type(B)~="function"` 的 boolean-index fail-closed
   （`main.rs` 传 `opts.do_guard`）。`guard_matrix` 80/0
   （修复前 79/80）。

---

## 2. 分阶段审查

### Phase A — 基线审计（完成）

- 产物：`BASELINE_SOURCE_AUDIT.md`，含 A2 源码盘点、A3
  EXISTS/PARTIAL/MISSING 矩阵、A4 实施范围与深度声明。
- 结论要点：本分支在审计时已是高度完整的混淆器（39k LOC 模板、
  byte-code VM、block-lazy `bdec`、字符串密封、upvalue 置换、
  数字 API 表等均 EXISTS）；规范映射的真实缺口仅在 B 的每原型
  布局对象、C/D 的 IR 化、E 的模块化校验、F 的显式上下文类型。
- 本报告 §3 的状态矩阵与之互为更新（G 补项、CCPD 接口的标注
  已回写审计文档）。

### Phase B — Proto 布局随机化（ACTIVE）

- 落点：`proto_layout.rs`（`ProtoFieldId`、`ProtoFieldLayout`，
  双射布局、目录编码/解码、fail-closed 校验），`compiler.rs`
  的 `proto_layouts` + 域分离 `layout_rng`，
  `template.rs` 的 parse 分发改为标量 if/elseif 链（每臂含
  super 时 `TFV=row[7]`）。
- 深度：真实进入序列化/解析链路 —— section tag 映射由构建级
  共享改为**每原型独立**。`--no-proto-layout` 恢复旧共享布局，
  行为可回退。
- 证据：run_tests 188/0；proto_enc 门禁绿；37 个示例按当前
  二进制重生成后字节稳定。
- 风险/限制：`layout_rng` 派生仅改变 rng 消耗顺序，不是缺陷
  修复手段（历史教训已记入过程文档，勿用它掩盖失败）。

### Phase C — Lazy fields（PARTIAL）

- 落点：`lazy_fields.rs` —— descriptor IR、四状态
  （Unresolved/Resolving/Resolved/Failed）、nil sentinel、
  递归与失败语义的参考实现 + Luau resolver 发射。
- 运行时接入：**未完成**。产物 parse 仍用现有两阶段常量解码
  （walk 时定位、执行时解码），没有把 C 表换成 proxy-based
  惰性物化。原因：运行时替换需重做常量表全链路（SoA remat、
  string sealing 交互），收益/风险不成比例，本轮明确声明
  跳过并在此标注，不冒充 ACTIVE。
- 覆盖：单元测试钉住四状态迁移、递归解析失败、sentinel 语义。

### Phase D — State machine（PARTIAL）

- 落点：`state_machine.rs` —— 状态图 IR、图校验（可达/出边/
  终止/进度）、随机状态标签、Luau 分发树发射、Rust 参考
  解释器与发射产物的差分测试。
- 运行时接入：**未替换**模板内硬编码的 section reader parse；
  建模对象（header/varint/section-directory/field-descriptor）
  与真实 parse 语义对齐，但线上 parse 仍走原路径。
- 覆盖：差分测试证明发射器语义与参考解释器一致；不声称
  产物中的 parse 已状态机化。

### Phase E — Dispatch tree（ACTIVE，非 v15 主分发）

- 落点：`dispatch_tree.rs` —— 原 `template.rs:16` 内联的随机
  比较树生成器迁为独立模块，加生成后覆盖校验（每个有效
  opcode 恰达一叶、无效落失败叶），失败即生成期报错。
- 生效面：非 v15 路径（light/default 等）的 opcode 分发；
  **v15 主分发不经该树**（v15 用其自身的 trampoline +
  FUSED 机制）。模板行为同构迁移，产物字节中立（示例比对
  通过）。
- 价值：把一处「生成器无校验」的风险点变为可回归的模块；
  未夸大 v15 覆盖。

### Phase F — Runtime mask context（PARTIAL）

- 落点：`runtime_mask.rs` —— `DecodeContext{prototype, block,
  lane, entry_domain, epoch}`、定宽 wrapping 掩码派生、
  `DecodeCacheKey`（含 domain/epoch，lane 是掩码输入非页
  身份）、`PageState` 四态（Encoded→Decoding→Verified/Failed，
  Verified 才可执行，Failed 不可复活）。
- 运行时接入：**`bdec` 既有掩码公式未改**。其 `wm` 已含
  fi/bi/er 三维上下文，同类隔离已在线上生效；新模块提供的是
  显式类型与未来替换的契约（含 CCPD entry_domain 位），不声称
  产物已切换到新公式。
- 覆盖：单元测试钉住确定性、上下文隔离、缓存键区分、
  28-bit 状态域、状态机 fail-closed。

### Phase G — API slots / 分阶段闭包（ACTIVE，复用为主）

- G1 落点：`api_slots.rs`（新）—— 原 `template.rs` 私有
  `prim_slots` 抽成显式模块：allowlist = `caps.rs` 的
  `VmPrim`（19 个原语，单一事实源），槽位自池 1..=80 洗牌
  无放回取前 COUNT 个（每构建布局、种子可复现）；
  `validate_layout` 在生成期校验每个启用原语持有唯一、
  非零、≤80 的槽位，违规即生成期报错（防未来编辑把某原语
  悄悄落成 nil 调用）。调用点与 rng 抽取顺序未变 →
  **产物字节中立**（与已提交示例逐字节一致，实测）。
- G2 分阶段闭包：运行时五阶段（env binder、scaffold
  trampoline、lazy block decode、entry closure、deferred
  invocation）均为既有结构，本轮不重复实现；其行为由 G4
  矩阵钉住（见下）。
- G3 禁止项：本轮未新增任何调试器检测、杀软/进程扫描、
  环境规避或 DoS 机制；守卫体系维持既有签名化失败口径。
- G4 落点：`tests/phase_g_api_test.py`（挂入 run_tests）——
  entry closure 多次调用、父函数返回后经保存表延迟调用、
  coroutine resume 续执行、槽位跨种子重映射（字节变、
  行为一致）、宿主缺失 `string` API 时非零退出 + 无载荷
  输出 + 两次失败签名一致（15 项全过）。
- CCPD（§7，可选）：**仅接口**。`runtime_mask.rs` 新增
  `EdgeId`、`EntryTransition`（8 类，稳定 id）、
  `DEFAULT_ENTRY_DOMAIN` 及契约文档；`derive_mask` 与
  `DecodeCacheKey` 已纳入 entry_domain，未来若发射 per-edge
  变体不会与今天的 domain-0 缓存别名。当前**零变体发射**：
  每个闭包/块仍只有一种编码，entry_domain 恒 0。不声称
  具备条目隔离能力。

---

## 3. 当前状态矩阵（最终）

| 能力 | 状态 | 依据 |
|---|---|---|
| 数字索引 API 表 + 每构建置换 | ACTIVE | `caps.rs` VmPrim + `api_slots.rs` |
| API 槽位生成期不变量校验 | ACTIVE | `validate_layout` + 单测 + 生成期 expect |
| Proto 每原型 section 布局 | ACTIVE | Phase B |
| block-lazy decode（bdec） | ACTIVE（既有） | `template.rs` |
| 字符串密封 / upvalue 置换 | ACTIVE（既有） | `strseal.rs` 等 |
| opcode 分发树 + 覆盖校验 | ACTIVE（非 v15 主路径） | Phase E |
| 状态机 IR / verifier / 发射器 | PARTIAL | Phase D，运行时未替换 parse |
| Lazy field 四状态 resolver | PARTIAL | Phase C，运行时仍 eager 两阶段 |
| DecodeContext / 页完整性状态机 | PARTIAL | Phase F 接口；bdec 公式未变 |
| CCPD per-edge 编码变体 | NOT_WIRED | 仅接口（`runtime_mask.rs`） |
| Handler micro-state 拆分 | NOT_WIRED | 未实施，如审计 §A3 原标注 |
| 调试器检测/杀软扫描/环境规避 | 未新增（按 G3 禁止） | — |

## 4. 验证证据（最终 HEAD `9500a26`）

| 门禁 | 结果 |
|---|---|
| `cargo test --release` | 120 passed + 4 doc/other，0 failed |
| `tests/run_tests.sh` | 188 PASS / 0 FAIL（含 phase-g 15 项） |
| `guard_matrix_test.py` | 80 / 0（charhook fail-closed 修复后） |
| `vm_tamper_test.sh` | 60 / 0 |
| `capability_gate_test.py` | 11 / 0 |
| `multiseed_baseline_test.py` | FAIL=0 |
| `bench_presets_eval.sh` | 56 PASS |
| defense / micro-fusion / env_bind / faithful / charhook / canary / string_cache | 全绿 |
| `prod_audit_gate_test.sh` | PASS（SC dump 钩子改为生成时 env opt-in 后与字节同一性门禁调和；此前在基线同点失败，属既有矛盾而非本轮回归） |
| 示例字节同一性 | 37/37（Phase G 重构后复核，逐字节相同） |

## 5. 已知问题（诚实记录）

1. **`strings.lua` seed 1234 输出错误（既有，未修）**：
   最终二进制上 seed 7 OK、seed 42 OK、seed 1234 失败 ——
   `table.concat` 结果 `xyA` 应为 `xyz`，单常量值错位。
   与本轮改动隔离性验证：`--no-super`、`--no-proto-layout`、
   `light` 预设、lua51 方言下均复现，故不在 Phase B–G 的
   改动面内；审计前即已观察到同一现象。本轮**未修**，
   不计入已完成项；建议后续按常量重物化链路单独立项排查。
2. Phase C/D/F 为 PARTIAL 是设计决定而非遗漏：运行时替换的
   收益/风险不成比例，接口与参考语义已落，接入点明确。
3. 种子通过集合不可跨配置引用：每条结果均绑定二进制、
   预设、方言与用例现内容，引用时必须连同注明。

## 6. 不诚实项声明（按规范必须列明）

- 无「写了模块即声称运行时生效」之处：C/D/F 均标 PARTIAL，
  CCPD 标 INTERFACE ONLY。
- `layout_rng` 域分离只改变 rng 消耗，历史上一度被误读为
  修复手段，本报告明确其非修复地位。
- 未运行的对照（如 `ab_compare.sh` 的跨版本 A/B）不声称
  跑过；基线对照以同矩阵失败率与 37 示例字节同一性为据。
- 本报告所有数字均为沙箱内自测，不代表抵御外部独立逆向
  分析的能力。
