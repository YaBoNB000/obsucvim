# R064 漏洞 1/2：复核与缓解状态

> 日期：2026-10-07
> 分支：`arena/01a038eb-luraph`
> 状态：漏洞 1 已修复；漏洞 2 已落地第一层安全微操作融合，但同进程动态观察仍不能从纯客户端彻底消除。

## 摘要

| 编号 | 问题 | 证据 | 状态 |
|---|---|---|---|
| 1 | 变参表尾 `nil` 被 `#all` 少计 | 旧 seed `64066`：源 `select("#", ...)` 结果 5，VM 结果 4 | v15 与 legacy 两路径已修复，尾 nil/转发/闭包回归通过 |
| 2 | micro-shard 解码后的执行路径可被动态跟踪 | 旧版临时白盒探针记录过 7KB 样本 906,284 条微码记录、覆盖 4 个 shard | 已实施保守的 intra-handler superoperator 融合；记录量降低但不能防止有同进程插桩能力的观察者记录融合后的执行路径 |

## 1. 变参尾部 nil：修复与验证

旧 `makefn_decl` 先做 `local all={...}`，再用 `#all - npa`（legacy 为 `#all - pf.nparams`）算变参数量。表尾 nil/表洞下 `#` 不保证等于实参数量，导致转发及 `select("#", ...)` 少计。

**已实施**：v15 与 legacy 路径都在构造 `all` 前，以捕获的 `SEL("#", ...)` 取精确实参数量，再减去固定参数数量；`vargc` 作为独立计数沿调用与 vararg 表传递，不从含 nil 的表推断。v15 的 `makefn` 位于单独 RTFRAG 闭包，故从已有随机槽 `P[slots[12]]` 捕获 `SEL`；legacy 使用外层已捕获的 `SEL`。未新增全局查找、共享 getter 或密钥/描述符表。

`stress_multival.lua` 已加入直接尾 nil、固定参数+变参、转发、`return ...` 尾展开和嵌套闭包回归。seed `64066` 下 v15 与 legacy VM 的最小复现均返回 5；完整矩阵及多种子回归通过。

## 2. Micro-shard 动态追踪：发现与缓解

H5-P0 已封存静态 `{program,mask,step}` descriptor 表；执行时仍须从 packed/session carrier 解出微码记录，再按 shard 分派。此前在临时生成器副本的 `op,x,y` 解码点加入白盒探针，7KB 样本产生 **906,284 条**实际执行记录、覆盖 4 个 shard，且正常结果仍出现。那是改过生成器的白盒插桩证据，不是未修改产物自动脱壳，也不覆盖未执行分支或恢复原变量名。

**本次实施：安全微操作融合。** `micro_recipe` 将以下 10 类不跨用户回调边界的短序列改为一个专用、每构建随机编号的 fused micro-op，直接访问当前 handler 的 `O` 和必要 VM 状态，不再逐条暴露 shard 临时表 `T` 中间读写：

- `LoadK`、`Move`、`NewTab`、`VarArgC`、`Jf`、`Jt`、`VarArgTab`：每次执行从 2 条记录降为 1 条；
- `Not`、`GetUp`、`SetUp`：每次执行从 3 条记录降为 1 条。

因此，对这 10 类操作各执行一次时，内部记录由 23 条降为 10 条（少 13 条，约 56.5%）；这只是这组操作本身的静态配方差值，不代表整段程序或 R064 样本总 trace 下降 56.5%。样本输出从融合前 336,389 字节增至 338,277 字节（+1,888，约 +0.56%），是增加 fused handler 代码的体积成本。`tests/micro_fusion_test.sh` 用 v15 与 legacy 两条路径验证运行结果一致，Rust 单测检查 fused recipe 确为单条记录且不再依赖 `T` scratch。

为控制语义风险，暂不融合 `LoadStr`、全局表访问、算术/比较、表 GET/SET、Call/Return 等涉及字符串解密、元方法、用户回调或错误顺序敏感边界的操作。`LoadK` 仍可能触发 VM 私有常量池的 lazy `CDEC`，但 fused body 先读出常量到局部，再写目标寄存器，未跨越或移动用户回调；`LoadStr` 不做此融合。H5-P0 的 packed carrier、每 shard key、session reseal 和 checksum lane 保持不变；未加入正式产物可调用的 trace hook、日志或调试接口。

## 3. 安全边界与剩余风险

这是一项**追踪成本缓解**，不是“阻止 trace”的保证。能修改目标源码/生成器，或在同进程运行时插桩的观察者，仍能记录融合后的 opcode、执行顺序，并结合 handler 语义继续分析已执行路径；不同输入也可能触发更多路径。融合主要减少临时读写轨迹和记录量，不会让客户端逻辑成为服务端秘密，也不能保护客户端持有的授权/经济/反作弊决策。

建议继续把高价值信任决策放服务端；保留 at-rest/session 防护；后续如继续融合，逐项证明求值顺序和回调边界，再做双解释器、受保护元表、多种子和性能回归。独立 checksum lane 只防篡改，不防观察；计时/反 trace 只能作为辅助信号。真实 Roblox executor 未在本地直接验证，因此不宣称同进程插桩在特定 executor 中必然可用或不可用。

## 4. 验证结果

- `cargo test`：35 passed、0 failed、1 ignored；
- `tests/run_tests.sh`：169/169，含新增 v15/legacy fused-handler 语义回归；
- `tests/run_presets.sh`：370/370；`tests/multiseed.sh`：`FAIL=0`；
- `tests/h5_p0_micro_audit.py`：PASS；`security_fingerprint.py`：S1–S5 5/5；
- R064 7KB 样本 seed `64066`：源 7,179 字节、目标 338,277 字节，Luau 编译通过且标准输出相同；目标 SHA-256 `6e322c6d405f2e1731149cd7af66eed7dbef0a06d1517044e15349394e98c0d5`；
- R064 样本结构指纹为 22/32（结构相似度，不是解密成功率）；`defense_audit.sh` 的 D12 合成计时触发/干净时钟检查已恢复通过。该脚本仍将 D3 结构分数 23/32 报为 FAIL；它不是 S1–S5 安全指纹或功能等价失败。

## 5. R064 深度自检第二轮（2026-10-07）

新产物 `handoff/R064_7kb_target_r2.luau.lua` 使用 seed `64075`，7,179 字节源与产物 stdout 一致；静态攻击工具 S1–S5 均退出 2（抵抗）。对相同 source/seed，以融合前 `2edb768` 与融合后 `1c14321` 临时白盒生成器在同一解码点计数：两边都是 412,706 次 shard invocation；记录数从 929,567 降至 701,915（-227,652，-24.49%），两边语义输出仍相同。计数探针只在 `/tmp`，未进入正式代码。

补充的 executor-like `setfenv` string library wrapper 捕获 2,673,372 次 `string.byte` 调用、397 个不同参数、最长 15,442 字节；目标仍输出正确。最大参数为运行态可打印编码字符串（85 种字节值，6.14 bit/byte），不在 artifact 中原样出现；hook 观察到数据不等于已解出源码，但确认该运行面可见。

第二轮对 32 个定向 seed 做 pre/post-fusion R064 样本差分，两个版本同样在 17 个 seed 报 `attempt to call a number value`，失败集合一致。具体 seed 和复现结果见 `docs/r064-deep-self-audit-r2.md`。这是本轮新确认的**既存 seed 稳定性缺陷**，未在本轮修改 VM；官方 multiseed corpus 没有包含该 handoff 7KB 样本。