# R064 深度自检：第二轮（融合后观察面与 seed 稳定性）

> 日期：2026-10-07
> 分支：`arena/01a038eb-luraph`
> 起始基线：`1c14321`（superoperator 第一层融合）
> 修复提交：本文对应的 `compiler.rs` nested-upvalue scatter descriptor remap
> 范围：7KB 本地语义样本的静态攻击、运行态插桩、API hook 与 seed 差分；不代表真实 Roblox executor 测试。

## 1. 新鲜样本

复用本地 7KB 语义 canary，生成新的 R064 v15 产物：

- 源：`handoff/R064_7kb_src.lua`，7,179 字节。
- 修复后产物：`handoff/R064_7kb_target_r2.luau.lua`，`--preset v15 --dialect luau --seed 64075`，332,466 字节。
- SHA-256：`a04eee0c1aca28444f50988b255d7501268b6d254f98aafe623b02ad298a0450`。
- 源与产物都通过 `luau-compile`；stdout 逐字一致。
- 短名审计 5,940 个局部/参数/循环标识符，均不超过两字母；v15 结构指纹 24/32（结构相似度，不是解密率）。

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

在 `/tmp` 的两份临时生成器中，对**同一解码点**在 `op,x,y` 解码后计数；两边都带本轮 upvalue 修复，一份使用融合前模板，一份使用融合后模板。源文件、seed (`64075`) 和 Luau 环境相同；探针没有进入仓库或正式产物。

| 版本 | shard 函数调用数 | 解码微码记录数 | 结果 |
|---|---:|---:|---|
| 融合前模板 | 412,706 | 929,567 | 样本 stdout 与源一致 |
| 融合后模板 | 412,706 | 701,915 | 样本 stdout 与源一致 |

本样本的实测差值为 **-227,652 条（-24.49%）**；这比单看 10 类配方的 56.5% 静态减少率更接近整体效果。剩下约 70 万条已解码记录仍足以提供大量路径信息。此计数衡量 trace 长度，不等于语义恢复率；本轮没有声称由 trace 自动还原完整源码。

## 4. 条件性动态 hook：`string.byte`

用 Luau CLI 的 `setfenv` harness 给目标加载一份复制的 `string` 表，只把 `byte` 换成透明记录 wrapper，再运行修复后 seed `64075` 产物：

- 程序仍正常输出，过滤 hook 统计行后与源码 stdout 完全一致；
- wrapper 观察到 **2,673,402 次**调用、**397 个**不同字符串参数；最大字符串参数 **15,442 字节**；
- 最大参数为可打印字符串（85 种字节值，Shannon 熵 6.14 bit/byte），且不作为相同子串直接出现于产物文本中。

这是一个真实的运行态观察面：控制加载环境的分析者可以记录目标实际交给 `string.byte` 的数据。证据**不等于**这些字符串都是明文源码，也不证明一个 string hook 就能还原所有语义；但它确认了静态封存之外仍有高带宽的运行态数据入口。实验使用本地 Luau `setfenv` 模拟，不等价于真实 Roblox executor 的能力结论。

## 5. 已修复：seed 相关嵌套闭包 upvalue 错位

### 5.1 发现与根因

第二轮最初在融合前后版本的 32 个指定 seed 上都观察到 17 个 seed 报 `attempt to call a number value`，失败集合一致。动态探针在 seed `64067` 捕获到：R064 的 `make_accumulator` 内层 vararg closure 在 VM `Call`（proto 4、pc 49）试图把数值 `1000003`（源常量 `MOD`）当成函数调用；该位置按源码应调用捕获的 `fold_text`。legacy `--vm`（不做 scatter）相同 seed 正常，指向 v15 per-function upvalue scatter，而非融合逻辑。

根因是时序错位：父函数的子闭包 descriptor 在父 `scatter_operands` 完成之前，按父 upvalue 的编译期原序写入 `CellKind::Up(u)`；之后父函数又随机重排 `upsrc`。部分 seed 下，子闭包持有的旧索引指向另一个父 upvalue（本样本中表现为 `fold_text` 被错映射到 `MOD`）。

### 5.2 修复

`compiler.rs` 现在在开始编译函数体、递归编译子闭包之前，先派生该函数的 `old index -> runtime index` upvalue permutation。子 closure 的 `CellKind::Up` descriptor 经父 permutation 映射；`scatter_operands` 应用同一预计算映射。permutation 使用按 build/function 派生的独立 PRNG；scatter 阶段仍消耗原主 PRNG 的等量 shuffle draws，以避免改变无关布局随机流。新增单测验证 permutation 双射、determinism，并新增 `r064_seed_stability_test.sh` 接入 `run_tests.sh` 与 `multiseed.sh`。

### 5.3 修复验证

- 修复后当前 release 对 42 个定向 seed（含此前失败 seed）扫描：全部生成成功、Luau 编译通过、stdout 与源码逐字一致（FAIL=0）。
- 用户要求继续扩测后，另跑连续 seeds `1..128`：全部生成成功、编译通过、stdout 相同；另以 Python `Random(20261007)` 抽取 128 个高范围 seed 再扫一轮：全部通过（该 128-seed sweep 为人工验证，未加入每次 CI 门禁）。
- 集成回归 `r064_seed_stability_test.sh` 固定覆盖 24 个历史失败 seed 与相邻/官方 seed：PASS。
- 修复前记录的 17 个失败 seed 均在修复后的集成 sweep 中通过；失败集合不再重现。
- 相同 seed 的 v15 产物仍确定；legacy 路径和其他 VM 用例由官方矩阵覆盖。

## 6. 回归总览与剩余边界

- `cargo test`：36 passed、0 failed、1 ignored；`run_tests.sh`：169/169；`run_presets.sh`：370/370；`multiseed.sh`：`FAIL=0`，并包含 R064 专项 24-seed sweep。
- H2/H3、H4 loader、H4-5/H6/H7/H8、H5-P0、R061 bypass audits：PASS。
- `defense_audit.sh`：PASS 20 / FAIL 1；唯一 D3 是结构指纹 23/32（结构相似度口径）；D12 计时守卫接线、触发、干净时钟与误报抽查通过。
- 同进程 `string.byte` hook 与已解码微码 trace 仍可观察运行态数据/路径；fusion 提高成本，不提供客户端密码学保密。
- 高价值密钥、授权、经济与反作弊决策仍应由服务端持有。

真实 Roblox executor 未在本地运行。所有 hook 结论限于这次本地 Luau harness；本轮没有发现远程执行或服务器权限漏洞。