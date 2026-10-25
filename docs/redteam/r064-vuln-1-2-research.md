# ⚠️ 战役记录（历史）

> 本文件记录当时一轮红队/加固的完整过程与结论，路径与版本号均为当时口径。
> 路径对照见 [`../README.md`](../README.md) §3；现状见 [`../CURRENT-STATE.md`](../CURRENT-STATE.md)。

---

# R064 动态追踪与 seed 稳定性：修复与自检

> 日期：2026-10-07
> 分支：`arena/01a038eb-luraph`
> 状态：尾 nil 变参数量、嵌套闭包 upvalue scatter 错位已修；micro-shard 做了两层 handler-recipe fusion，并绕过可替换 string 表 hook。**同进程/native hook 仍是客户端观察边界，不宣称 trace 已被彻底阻止。**

## 摘要

| 编号 | 问题 | 证据 | 状态 |
|---|---|---|---|
| 1 | 变参表尾 `nil` 被 `#all` 少计 | 旧 seed `64066`：源 `select("#", ...)` 结果 5，VM 结果 4 | v15 与 legacy 两路径已修复，尾 nil/转发/闭包回归通过 |
| 2 | micro-shard 解码后的执行路径可被动态跟踪 | seed `64075` 同点白盒 A/B：929,567→701,915→412,706 条；复制 string 表 hook 已测为 0/0/0 | 两层 handler 内 fusion + native primitive capture 已实施；原生函数/同进程插桩仍可观察 |
| 3 | 子 closure `CellKind::Up` 未跟随父函数 upsrc permutation | R064 seed 64067 曾把 `fold_text` 映射为 `MOD=1000003`，造成“call number” | 父 upvalue permutation 预计算并映射 child descriptor；24/42/128+128 seed 验证通过 |

## 1. 尾部 nil 变参数量

旧 `makefn_decl` 先将 `...` 放入 `all`，再用 `#all - npa`（legacy 为 `#all - pf.nparams`）推算变参数量。Lua/Luau 对有 nil 洞或末尾 nil 的表，其 `#` 不保证等于实参数量。

已修复：v15 和 legacy 都在构造 `all` 前，以捕获的 `SEL("#", ...)` 获取精确 arity；`vargc` 独立传递，不再从表长度推断。`stress_multival.lua` 覆盖末尾 nil、forward、固定参数+变参、`return ...` 尾展开及嵌套闭包；seed `64066` 的最小复现由 4 修正为 5。

## 2. Micro-shard 动态追踪：两层融合与边界

H5-P0 继续封存 at-rest `{program,mask,step}` descriptor；session reseal 和 checksum lane 不变。运行时仍需要解码并执行，但微码内部记录已进一步压缩：

1. **第一层**：10 类安全内部短序列（LoadK/Move/NewTab/Not/GetUp/SetUp/VarArgC/Jf/Jt/VarArgTab）各自从 2–3 条记录降为 1 条。
2. **第二层（本次）**：`micro_recipe` 把其余 23 种多 primitive handler recipe 也编码为一个 per-shard、per-build 随机 superoperator。运行时在单个 decode/dispatch 分支中按原次序内联 primitive body；每步保留自己的 `do` scope，不重排 lazy decode、元方法、用户回调或错误顺序。没有跨相邻 VM 指令 fusion；Call/PostCall trampoline 和 Return 仍是独立 phase。

对 7KB R064 seed `64075` 样本，临时同点白盒探针（均为 `/tmp` 构建，未提交）计数：

| 配置 | shard invocation | decoded micro record | stdout |
|---|---:|---:|---|
| 原始多微步 recipe | 412,706 | 929,567 | 与源一致 |
| 第一层 safe fusion | 412,706 | 701,915 | 与源一致 |
| 全部多步 handler recipe fusion | 412,706 | 412,706 | 与源一致 |

完整 recipe fusion 相对第一层再减少 289,209 条（-41.20%），相对原始 trace 减少 516,861 条（-55.60%）。这是**trace 长度**的测量，不是“反编译率”或语义恢复率；记录仍约等于实际 handler invocation，具备同进程插桩能力者仍可记录 opcode、VM 参数/状态并做路径分析。

### 可替换 string 表 hook

融合前的 Luau `setfenv` harness 曾用复制的 `string` 表包装 `byte`，观察到 2,673,402 次调用、397 个不同字符串参数、最长 15,442 字节。8db 的 `native primitive capture` 把 VM/v15 staging 的 `byte/sub/char` 获取改为原生字符串元表/数值槽；`tests/string_primitive_hook_test.sh` 在复制表 wrapper 下验证命中 `0/0/0` 且 stdout 不变。

这只绕过**可替换库表** hook；native C function hook、debugger、运行时 patch 和局部状态插桩仍在威胁边界内。R064 新 target r2（seed64075）静态 S1–S5 均抵抗；不以此宣称客户端逻辑无法动态观察。

## 3. Seed 稳定性：嵌套 upvalue scatter 已修复

根因是父函数的 `upsrc` 在 `scatter_operands` 阶段随机置换，但子闭包的 `CellKind::Up(u)` descriptor 先按旧索引编码。特定 seed 下，内层 `make_accumulator` closure 的 `fold_text` descriptor 指向 `MOD=1000003`，后续把数字当函数调用。

`compiler.rs` 现在在编译函数体/子闭包前预计算父函数 `old index -> runtime index` permutation；子 descriptor 经该映射编码，父 `upsrc` 和 GetUp/SetUp operand scatter 使用同一个 mapping。置换由 build/function key 派生独立 PRNG；仍消耗原主 RNG 相同数量的 shuffle draws，避免扰动其他随机布局。

验证：24 个历史失败/相邻 seed 集成回归通过；另测 42 个定向 seed、连续 `1..128` seed，以及 `Random(20261007)` 抽取的 128 个高范围 seed，均生成成功、Luau compile 通过、stdout 与源码一致。扩展 sweeps 属手工验证，未全部加入每次 CI 门禁。

## 4. 当前验收与剩余风险

- `cargo test`：39 passed、0 failed、1 ignored；`run_tests.sh`：169/169；`run_presets.sh`：370/370；`multiseed.sh`：`FAIL=0`，含 R064 专项 24-seed sweep。
- H2/H3、H4 loader、H4-5/H6/H7/H8、H5-P0、R061 bypass：PASS；安全指纹 S1–S5：5/5。
- `defense_audit.sh`：PASS 20 / FAIL 1；唯一 D3 为结构指纹 23/32（结构相似度口径）；D12 计时守卫接线/触发/干净时钟/误报检查通过。
- 最新 R064 seed64075 target r2：340,895 字节，SHA-256 `ba07f782d3579568c9ebf444b5800a9d7922053edc9aff105c0c10d30b288dcb`；Luau compile 与源 stdout 一致；短名审计 6,326 个标识符，≤2 字母；结构指纹 23/32。
- 真实 Roblox executor 未在本地运行。客户端混淆不应承载服务端密钥、授权、经济或反作弊信任决策。

详细 probe、attack 口径与 seed 错位分析见 `docs/r064-deep-self-audit-r2.md`。