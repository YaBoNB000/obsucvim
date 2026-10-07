# R064 漏洞 1/2 与 seed 稳定性：修复与自检

> 日期：2026-10-07
> 分支：`arena/01a038eb-luraph`
> 状态：尾 nil 变参计数与 R064 嵌套闭包 upvalue scatter seed 错位已修复；micro-shard 已做第一层融合，但客户端运行态观察边界仍存在。

## 摘要

| 编号 | 问题 | 证据 | 状态 |
|---|---|---|---|
| 1 | 变参表尾 `nil` 被 `#all` 少计 | 旧 seed `64066`：源 `select("#", ...)` 结果 5，VM 结果 4 | v15 与 legacy 两路径已修复，尾 nil/转发/闭包回归通过 |
| 2 | micro-shard 解码后的执行路径可被动态跟踪 | 白盒计数：融合前 929,567 条，融合后 701,915 条；`string.byte` setfenv hook 捕获 2,673,402 次调用 | 第一层融合已落地；仍不能对同进程插桩保证保密 |
| 3 | 子 closure `CellKind::Up` 未跟随父函数 upsrc permutation | R064 seed 64067：`fold_text` 被映射为 `MOD=1000003` 并导致数值被调用；32 个指定 seed 中旧版 17 个失败 | 已修复：预计算 parent upvalue permutation 后生成 child descriptor；R064 专项 24-seed regression 与当前 release 的 42-seed sweep 均通过 |

## 1. 变参尾部 nil：修复与验证

旧 `makefn_decl` 先做 `local all={...}`，再用 `#all - npa`（legacy 为 `#all - pf.nparams`）算变参数量。表尾 nil/表洞下 `#` 不保证等于实参数量，导致转发及 `select("#", ...)` 少计。

**已实施**：v15 与 legacy 路径都在构造 `all` 前，以捕获的 `SEL("#", ...)` 取精确实参数量，再减去固定参数数量；`vargc` 作为独立计数沿调用与 vararg 表传递，不从含 nil 的表推断。v15 的 `makefn` 位于单独 RTFRAG 闭包，故从已有随机槽 `P[slots[12]]` 捕获 `SEL`；legacy 使用外层已捕获的 `SEL`。

`stress_multival.lua` 覆盖直接尾 nil、固定参数+变参、转发、`return ...` 尾展开和嵌套闭包。seed `64066` 下 v15 与 legacy VM 最小复现均返回 5。

## 2. Micro-shard 动态追踪：发现与缓解

H5-P0 已封存静态 `{program,mask,step}` descriptor 表；执行时仍须从 packed/session carrier 解出微码记录，再按 shard 分派。融合前临时白盒探针曾记录 7KB 样本 906,284 条实际执行记录、覆盖 4 个 shard；该证据不代表未修改产物可自动脱壳。

`micro_recipe` 把 10 类不跨用户回调边界的短序列融合成单条、每构建随机编号的 fused micro-op，减少 shard-local `T` 中间读写：

- `LoadK`、`Move`、`NewTab`、`VarArgC`、`Jf`、`Jt`、`VarArgTab`：2 条→1 条；
- `Not`、`GetUp`、`SetUp`：3 条→1 条。

对 10 类各执行一次时静态配方由 23 条降至 10 条（56.5%），不代表整体 trace 降幅。第一轮 R064 artifact 增加了 fused handler 代码；具体当时的体积数据见 `docs/r064-deep-self-audit.md`。

**最新同点 A/B（seed 64075）**：融合前/后的临时生成器都在同一 `op,x,y` 解码点计数，均为 412,706 次 shard invocation；记录数从 **929,567** 降到 **701,915**（-227,652，-24.49%），stdout 一致。探针只存在于 `/tmp`。融合后仍有约 70 万条记录；同进程插桩者仍可观察融合后的路径。

暂不融合 `LoadStr`、全局表访问、算术/比较、表 GET/SET、Call/Return 等涉及字符串解密、元方法、用户回调或求值/错误顺序的操作。`LoadK` 可能触发 VM 私有常量池的 lazy `CDEC`，但 fused body 先取常量到局部再写寄存器；不移动用户回调。H5-P0 packed carrier、每 shard key、session reseal 和 checksum lane 保持不变，未加入正式产物 trace hook。

## 3. 已修复：嵌套 closure 的 upvalue scatter 映射

### 3.1 复现与根因

对 7KB R064 样本比较融合前模板与融合后版本，32 个指定 seed 的失败集合一致；旧版 17 个 seed 报 `attempt to call a number value`。这证明该错误不是 micro-op fusion 新引入的。

动态探针在 seed `64067` 捕获到内层 vararg closure 的 `Call` 指令试图调用数值 `1000003`。对应源码位置应调用捕获的 `fold_text`。编译器 upvalue 调试信息表明该 closure 捕获 `MOD`、`fold_text`、`state`、`calls`。

根因：编译器先按父函数**原始** upvalue 序号写入子 closure 的 `CellKind::Up(u)` descriptor；之后父函数的 `scatter_operands` 才置换 `upsrc`。descriptor 仍指向旧序号，某些 seed 的 permutation 会让子闭包把 `fold_text` cell 读成 `MOD`。

### 3.2 修复

`compiler.rs` 现在在递归编译函数体之前，为每个函数预计算 `old upvalue index → runtime index` permutation。生成子 closure descriptor 时，若其 cell 来源为父帧 upvalue，则按父帧 permutation 重映射；`scatter_operands` 再应用同一预计算映射。per-function permutation 使用基于构建键和函数索引的独立 PRNG；scatter 阶段仍消耗原主 PRNG 的等量 shuffle draws，避免扰动其他布局随机流。

新增 `upvalue_permutation_is_bijective_and_seed_stable` 单测，以及 `tests/r064_seed_stability_test.sh`。脚本检查 24 个历史失败/相邻 seed，已接入 `run_tests.sh` 和 `multiseed.sh`。

### 3.3 修复验证

- 修复后当前 release 的 42-seed R064 扫描：失败 0；用户要求继续扩测后，连续 seeds `1..128` 与 Python `Random(20261007)` 另抽 128 个高范围 seed 均失败 0；
- 集成的 24-seed R064 回归：PASS；此前记录的 17 个失败 seed 均通过；
- `--vm` legacy 不使用 v15 scatter 的同源 seed 样本原本通过；修复保留其 identity upvalue order。

## 4. 第二轮动态 hook 证据与限制

对新样本 `handoff/R064_7kb_target_r2.luau.lua`（seed `64075`）执行静态 S1–S5 攻击均抵抗。另用 Luau `setfenv` harness 给复制的 `string` 表安装透明 `byte` wrapper：目标仍正常输出，wrapper 观察到 **2,673,402 次**调用、**397 个**不同字符串参数、最长 **15,442** 字节。该最大参数含 85 种字节值、Shannon 熵 6.14 bit/byte，且不以相同子串出现在 artifact 中。hook 证明运行态数据可见，不等于这些内容已被解释成源码或明文。

真实 Roblox executor 未在本地直接运行。本轮所有动态观察结论限于本地 Luau `setfenv` harness；客户端混淆仍不能保护服务端密钥、授权或反作弊决策。

## 5. 验证结果

- `cargo test`：36 passed、0 failed、1 ignored；
- `tests/run_tests.sh`：169/169；`tests/run_presets.sh`：370/370；`tests/multiseed.sh`：`FAIL=0`（含 R064 专项 24-seed sweep）；
- H2/H3、H4 loader、H4-5/H6/H7/H8、H5-P0、R061 bypass：PASS；`security_fingerprint.py`：S1–S5 5/5；
- `defense_audit.sh`：D12 慢/干净时钟和误报检查 PASS；总计 PASS 20 / FAIL 1，唯一 FAIL 是 D3 结构指纹 23/32，不是安全指纹或功能等价失败。
- 新 R064 seed `64075` artifact：332,466 字节；SHA-256 `a04eee0c1aca28444f50988b255d7501268b6d254f98aafe623b02ad298a0450`；源/产物 stdout 一致。
