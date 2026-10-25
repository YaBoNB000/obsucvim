# ⚠️ 历史研究存档

> 本文件属研究战役记录（结论按当时口径），路径对照见 [`../../README.md`](../../../README.md) §3。
> 现状见 [`../../CURRENT-STATE.md`](../../../CURRENT-STATE.md)。

---

# stress_multival 多返回值差异：根因报告

**日期：2026-10-09 定位；2026-10-11 已按路线 2（噪声避让 + 构建期不变量）修复并通过全矩阵（见 docs/implementation-plan.md 更新日志）。本报告保留为根因与消融证据。**

## 一句话根因

v15 管线里，**全展开调用（`c=255`）的返回值在运行期才写进 `base+1..base+nout` 这段"动态结果窗口"，但编译器从未为这段窗口保留寄存器**；H3-4 死寄存器噪声恰好把这段未保留区间当作空闲死寄存器，向其中插入真实写入的 LoadK/Move/NewTab/LoadNil。噪声一旦落在调用与消费点（Return）之间，就覆盖尚未被合并的返回值——输出静默出错，且随 seed 变化。

## 证据链（三步，全部可复现）

1. **复现**：8 个失败 seed 全部重跑，差异都集中在 `fwd1:` 行（本应 `1 2 3`，实际第三/第二个值变成 `2832` / `table` / `nil` / `27` / `11` / `3` 等）。带指令转储的研究构建与正式产物**逐字节一致**（`eight-reproductions.json`）。
2. **指令级对位**：转储显示冲突点完全对上。例如 seed `3525582012654347846` 的转发函数（fn=6）指令序列为 `GetUp → Call(a=0,c=255,d=1) → LoadK(a=3, 常量=2832) → Nop → Return(a=0,b=255)`——噪声 `LoadK` 写的正是逻辑寄存器 3，而该调用的第 3 个返回值运行期就落在逻辑寄存器 3；错误输出值 `2832` 与该噪声常量**同值**。
3. **消融实验（决定性）**：研究构建里**只把"死寄存器目标落在 `c=255` 调用活跃结果窗口内"的噪声指令**中和成自 Move（指令数、随机流、常量池、槽表全部不变）：
   - 8 个失败 seed：**8/8 输出恢复与源码完全一致**（每 seed 3 次运行）；
   - 3 个健康种子（42/64075/65065）：消融为无操作，输出不变；
   - 每个 seed 实际中和 1–4 条同类冲突指令（`*-ablations.txt`），说明同一 seed 里常有多处潜伏冲突。

## 机制详解（代码对位）

1. **窗口未被保留**：`engine/src/vmgen/compiler.rs` 的 `compile_call_into` 只保留 `1 + nargs.max(1)` 个寄存器，而 `call_arg_info` 统计的 `nargs` **不含 Vararg 参数**。`local function f2(...) return f1(...) end` 里 `f1(...)` 的 `nargs = 0` ⇒ 只保留了函数槽 1 个寄存器；`f1` 运行期返回 3 个值，落进未保留的逻辑 `1..3`。
2. **结果窗口运行期才确定**：v15 Call handler（`template.rs` / `handlers/call.rs`）在运行期执行 `for i = 1, wn do V[S[base+i]] = results[i] end`——写多少、写到哪，编译期不可见。
3. **噪声分配器不知道这个窗口**：`compiler.rs` `finish()` 的 H3-4 段把 `dead_base = nregs` 之后的 2–4 个"死寄存器"当作安全写入目标，并以随机位置注入真实执行的 LoadK/Move/LoadNil/NewTab。死寄存器区间 `[user_regs, user_regs+dead)` 与动态结果窗口 `[base+1, base+nout)` **相交且无人检测**。
4. **时序**：噪声插入位置随机；只有落在"Call 之后、消费点（Return）之前"且目标寄存器在窗口内时才毁值——因此是概率性、seed 相关的静默错误。

## 为什么各档表现不同（与早前模式对照一致）

| 模式 | 结果 | 原因 |
|---|---|---|
| v15（默认 / no-minify / no-vm-seal-strings） | 同 seed 10/10 失败 | 三者都保留 H3-4 寄存器写入噪声与 v15 Call 形态 |
| legacy `--preset vm` | 0/10 | legacy 无 H3 寄存器写入噪声（仅 Nop），Call 写回路径不同 |
| `--preset low` | 0/10 | 无 VM |
| 固定矩阵 169/169 | 全绿 | 固定 seed 集；冲突需要"位置 + 死寄存器"同时撞上，属概率事件 |

触发率（本语料）：sweep 1,226 组中 8 次 + 先导 1 次 ≈ **0.7%**。

## 同类隐患（诚实边界）

- 本轮实证的是普通 `Call c=255` + 变参转发这一形态；`CallE/CallM/CallT` 及尾调用链同样存在"动态结果窗口未保留"的结构缺口，**未逐一审计**，风险同类。
- 即使去掉噪声，"未保留的动态窗口"仍是潜伏地雷：任何未来使用 `≥ user_regs` 寄存器的新特性都可能以同样方式撞上。
- 消融构建仅用于证明因果（位于被 gitignore 的 `work/probe/`），**不是修复方案**。

## 修复方向（未实施，需用户批准后按强制工作流执行）

1. **保留结果窗口**：编译器在全展开调用处按策略保留结果寄存器。难点：变参转发的返回数量编译期未知，需定上限或改变安放协议。
2. **噪声避让**：H3-4 注入时检测活跃结果窗口（调用→消费点之间的 `base+1..` 区间），冲突则重选位置/目标寄存器。改动局部，但需覆盖全部调用家族。
3. **结果改走帧槽**：全展开调用结果不再落寄存器堆，改用专用帧槽（`E.rb` 页）。架构级改动，体积/性能需重测。

任一方案落地都必须走强制工作流：全矩阵 + multiseed + 该语料历史失败 seed 回归 + 文档同步 + 提交推送。

## 复现

```bash
# 失败复现（任一失败 seed）
engine/target/release/obsucvim --seed 3525582012654347846 \
  engine/tests/cases/stress_multival.lua /tmp/t.lua
.tools/bin/luau engine/tests/cases/stress_multival.lua   # fwd1: 1 2 3
.tools/bin/luau /tmp/t.lua                                  # fwd1: 1 2 2832
```

证据文件（本目录）：`eight-reproductions.json`、`<seed>-instructions.txt`（逻辑↔wire 指令转储）、`<seed>-ablations.txt`、`ablation-results.json`。研究构建见 `work/probe/`（gitignored）。
