# R064 深度自检：第二轮（融合后观察面与 seed 稳定性）

> 日期：2026-10-07
> 分支：`arena/01a038eb-luraph`
> 起始基线：`1c14321`（superoperator 第一层融合）
> 修复提交：本文对应的 `compiler.rs` nested-upvalue scatter descriptor remap
> 范围：7KB 本地语义样本的静态攻击、运行态插桩、API hook 与 seed 差分；不代表真实 Roblox executor 测试。

## 1. 新鲜样本

复用本地 7KB 语义 canary，生成新的 R064 v15 产物：

- 源：`handoff/R064_7kb_src.lua`，7,179 字节。
- 修复后并应用完整 handler-recipe fusion 的产物：`handoff/R064_7kb_target_r2.luau.lua`，`--preset v15 --dialect luau --seed 64075`，340,895 字节（较 native-primitive-hook 基线 326,848 字节增加约 4.3%）。
- SHA-256：`ba07f782d3579568c9ebf444b5800a9d7922053edc9aff105c0c10d30b288dcb`。
- 源与产物都通过 `luau-compile`；stdout 逐字一致。
- 短名审计 6,326 个局部/参数/循环标识符，均不超过两字母；v15 结构指纹 23/32（结构相似度，不是解密率）。

## 2. 静态攻击检查

对 `R064_7kb_target_r2.luau.lua` 执行 `security_fingerprint.py` 与 `tests/attack/` 静态攻击：

| 检查 | 结果 | 含义/边界 |
|---|---|---|
| S1 常量恢复 / `constants_attack.py` | 抵抗（攻击工具 exit 2） | 静态工具未恢复源常量；不代表运行时不可观察 |
| S2 规整字节码 / `structure_attack.py` | 抵抗（exit 2） | 文档化布局攻击失败 |
| S3/S4 解释器和自描述素材 / `interp_attack.py` | 抵抗（exit 2） | 静态特征审计未找到足够恢复素材 |
| S5 动态加载守卫 / `load_attack.py` | 抵抗（exit 2） | 可见探针及强制结果形态存在 |
| 总安全指纹 | S1–S5 5/5 | 静态门禁通过，不应外推为“不可脱壳” |

## 3. 融合效果：临时白盒计数 A/B

在 `/tmp` 的临时生成器中，对同一 R064 源码、seed `64075`、Luau 环境，在 `op,x,y` 解码后逐条计数；所有版本的临时探针均不进入仓库/正式产物，程序 stdout 都与源码一致。

| 版本 | shard 函数调用数 | 解码微码记录数 | 变化 |
|---|---:|---:|---:|
| 原始多微步 recipe | 412,706 | 929,567 | 基线 |
| 第一层：融合 10 类 safe recipe | 412,706 | 701,915 | 较基线 -24.49% |
| 第二层：所有多步 handler recipe 收进一个 ordered superoperator | 412,706 | 412,706 | 较第一层 -41.20%；较基线 -55.60% |

第二层仍只在**同一个 bytecode handler recipe 内**合并步骤：superoperator 的 body 在一个 decode/dispatch 分支中，按原次序运行该 recipe 的 primitive body，并为每步保留独立 Lua lexical scope。没有跨相邻 VM 指令、用户调用、Call/Return trampoline 或 metamethod callback 合并；错误与回调次序由原 body 保持。对本样本，每次 shard 函数调用现在解出一条记录。此计数衡量 trace 长度，不等于语义恢复率；具备同进程插桩能力者仍可追踪融合后的 handler/参数。

## 4. 条件性动态 hook：replaceable `string` 表

**第一轮 hook 结果（native-capture 修复前）**：Luau `setfenv` harness 用一份复制的 `string` 表替换 `byte/sub/char` 并运行 seed `64075` 产物。程序输出仍与源码一致；`byte` wrapper 观察到 **2,673,402 次**调用、**397 个**不同字符串参数、最大参数 **15,442 字节**。最大字符串可打印（85 种字节值，Shannon 熵 6.14 bit/byte），且不以相同子串直接出现在产物文本中。该结果不表示它是明文或已还原源码，但证明 VM 内部当时经可替换 string table 读取了大量数据。

**本轮缓解**：`PRIM_SRC` 中的 `byte/sub/char` 改为从原生字符串元表取得（`("")["byte"]` 等）；v15 模块表对应 primitive 也改走该路径，staging helper 使用这些捕获槽/原生 `sub`，不再直接调用可替换的 `string` 表。

新增 `tests/string_primitive_hook_test.sh`：在复制 `string` 表并包装 `byte/sub/char` 的环境中，v15 canary 输出逐字一致且三项 hook 命中均为 0。对更新后的 R064 seed `64075` 产物重复同样 harness，亦得到 `byte/sub/char = 0/0/0` 且 stdout 一致。该改动关闭的是这类**环境表替换 hook**；有能力 hook 原生 C 函数、直接 patch 产物/VM 或访问局部运行态的 executor 仍能观察数据。micro-op A/B 记录数是上一轮在同一解码点的测量，本次没有重做融合后的整段 trace 测量。

所有实验仍是本地 Luau `setfenv` harness，不等同于真实 Roblox executor；不以此声称动态 trace 已被彻底阻止。

## 5. 已修复：seed 相关嵌套闭包 upvalue 错位

### 5.1 发现与根因

第二轮最初在融合前后版本的 32 个指定 seed 上都观察到 17 个 seed 报 `attempt to call a number value`，失败集合一致。动态探针在 seed `64067` 捕获到：R064 的 `make_accumulator` 内层 vararg closure 在 VM `Call`（proto 4、pc 49）试图把数值 `1000003`（源常量 `MOD`）当成函数调用；该位置按源码应调用捕获的 `fold_text`。legacy `--vm`（不做 scatter）相同 seed 正常，指向 v15 per-function upvalue scatter，而非融合逻辑。

根因是时序错位：父函数的子闭包 descriptor 在父 `scatter_operands` 完成之前，按父 upvalue 的编译期原序写入 `CellKind::Up(u)`；之后父函数又随机重排 `upsrc`。部分 seed 下，子闭包持有的旧索引指向另一个父 upvalue（本样本中表现为 `fold_text` 被错映射到 `MOD`）。

### 5.2 修复

`compiler.rs` 现在在开始编译函数体、递归编译子闭包之前，先派生该函数的 `old index -> runtime index` upvalue permutation。子 closure 的 `CellKind::Up` descriptor 经父 permutation 映射；`scatter_operands` 应用同一预计算映射。permutation 使用按 build/function 派生的独立 PRNG；scatter 阶段仍消耗原主 PRNG 的等量 shuffle draws，以避免改变无关布局随机流。新增单测验证 permutation 双射、determinism，并新增 `r064_seed_stability_test.sh` 接入 `run_tests.sh` 与 `multiseed.sh`。

### 5.3 修复验证

- 修复后当前 release 对 42 个定向 seed（含此前失败 seed）扫描：全部生成成功、Luau 编译通过、stdout 与源码逐字一致（FAIL=0）。
- 用户要求继续扩测后，另跑连续 seeds `1..128`：全部生成成功、编译通过、stdout 相同；另以 Python `Random(20261007)` 抽取 128 个高范围 seed 再扫一轮：全部通过（该 128-seed sweep 为人工验证，未加入每次 CI 门禁）。这两组扩展 sweep 在后续 native string primitive capture 改动前完成；该改动后重新运行的集成 24-seed 回归及完整 `multiseed.sh` 也通过。
- 集成回归 `r064_seed_stability_test.sh` 固定覆盖 24 个历史失败 seed 与相邻/官方 seed：PASS。
- 修复前记录的 17 个失败 seed 均在修复后的集成 sweep 中通过；失败集合不再重现。
- 相同 seed 的 v15 产物仍确定；legacy 路径和其他 VM 用例由官方矩阵覆盖。

## 6. 回归总览与剩余边界

- `cargo test`：39 passed、0 failed、1 ignored；`run_tests.sh`：169/169；`run_presets.sh`：370/370；`multiseed.sh`：`FAIL=0`，并包含 R064 专项 24-seed sweep。
- H2/H3、H4 loader、H4-5/H6/H7/H8、H5-P0、R061 bypass audits：PASS。
- `defense_audit.sh`：PASS 20 / FAIL 1；唯一 D3 是结构指纹 23/32（结构相似度口径）；D12 计时守卫接线、触发、干净时钟与误报抽查通过。
- copied-string 表 byte/sub/char hook 已由 native primitive capture 缓解；native C hook、debugger 与同进程局部插桩仍能观察运行态。全 recipe fusion 将本样本 generic decoded trace 降到每次 shard invocation 一条，但不提供客户端密码学保密。
- 高价值密钥、授权、经济与反作弊决策仍应由服务端持有。

真实 Roblox executor 未在本地运行。所有 hook 结论限于这次本地 Luau harness；本轮没有发现远程执行或服务器权限漏洞。