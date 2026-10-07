# R064 7KB 示例与深度自检

> 日期：2026-10-07
> 范围：R064 示例、protected-metatable 兼容、尾 nil 变参修复、全 handler-recipe micro superoperator fusion、replaceable string-table hook 缓解，以及嵌套闭包 upvalue scatter seed 错位修复；不是“不可破解”证明。

## 1. 交付样本

- 源码：`handoff/R064_7kb_src.lua`（7,179 字节，纯本地 Luau/Lua 语义测试，无 Roblox、网络或文件 API）
- 产物：`handoff/R064_7kb_target.luau.lua`（seed `64066`，全 recipe fusion + native string primitive capture 后 335,900 字节）
- SHA-256：源 `5f3295d245a4e63d229d9da1022b88e1234335bc177a78256a76597de3fd7a99`；产物 `bc6cbff8f9fa33328d974dece6be3c143d30f8a3613a35ce51388204369c8dac`
- v15 结构指纹 23/32；仅表示结构相似度，不表示可解密性或运行安全性。

## 2. 已修复：尾部 nil 变参数量

此前 `makefn_decl` 先将 `...` 放入 `all`，再用 `#all - npa` 推算变参数量。Lua/Luau 中，表尾 `nil`/表洞使 `#all` 不能可靠表示实参数量；复现输入 `count(nil, "tail", false, 7, nil)` 的源结果是 `5`，旧 VM 结果是 `4`。

修复在 v15 与 legacy 两个 `makefn_decl` 路径都捕获精确参数数目：

```lua
local argc = SEL("#", ...)
local all = { ... }
local vargc = argc - npa -- legacy: argc - pf.nparams
if vargc < 0 then vargc = 0 end
```

v15 `makefn` 位于独立 RTFRAG 闭包，因此从已有随机槽 `P[slots[12]]` 捕获 `SEL`；legacy 使用外层 `SEL` 别名。后续参数表可能含 nil 洞，但 `vargc` 保持独立，不再从表长度反推。

`stress_multival.lua` 新增直接尾 nil、固定参数+变参、转发、`return ...` 尾展开及嵌套闭包用例。seed `64066` 下 v15 和 legacy VM 的最小复现都从 `4` 修正为 `5`，扩展用例源/产物结果一致。

## 3. 已修复：字符串方法 / 受保护元表

v15 micro `M_GET_TABLE` 对字符串使用原生 Luau indexing；`mget` 先验证可见元表类型，遇到非表保护 sentinel 后，GET/SET、metamethod 与 callable 路径回退到原生 Luau 运算。R064 源保留 `row[2]:sub(...)`，在 executor-like `getmetatable(string)` 保护标记测试中通过。

回归脚本 `tests/protected_meta_compat_test.sh` 已集成到 `run_tests.sh`，覆盖字符串方法和受保护的 `__index/__newindex/__add/__concat/__lt/__eq/__unm/__len/__call`。

## 4. 已缓解：micro-shard 动态 trace

每个 shard 的 at-rest/session 描述符仍按 H5-P0 封存。第一层先把 10 种安全内部短序列融合为单记录 superoperator。第二层进一步将**全部 23 种多 primitive handler recipe**打包为每 shard、每构建随机编号的单记录 superoperator：body 在同一 decode/dispatch 分支中按原序执行 primitive Lua body，每步使用单独 `do` scope。它没有跨相邻 VM 指令、用户调用或 Call/Return trampoline 合并；算术、表访问、元方法和 `LoadStr` 都保持原调用/错误顺序，只去掉 recipe 内重复的 record decode/dispatch。新增单测检查 recipe 展开顺序与全部多步 opcode 都有 superoperator；`micro_fusion_test.sh`、全语料与多种子回归确认 v15/legacy 语义。

对相同 R064 seed `64075`、同一解码点临时计数，融合前 / 第一层 / 完整 recipe fusion 分别为 **929,567 / 701,915 / 412,706 条**，shard 函数调用数均为 412,706，stdout 与源一致。完整融合比第一层再减少 41.20%，比原始版本减少 55.60%。该计数探针只在 `/tmp`，没有进入正式构建。

该措施把本样本 generic decoded trace 压到约一条 record/handler invocation，但不保证观察者不能把 superoperator 映射回高层 VM 操作。调用参数 `a,b,c,d`、当前 VM 状态或 primitive body 仍可能被有同进程插桩权的观察者检查；跨 VM instruction fusion 尚未实施，也不能防 native function hook。

另在 8db 的 native primitive capture 中，VM 核心 `byte/sub/char` 改从 string metatable 或 numeric slots 获取，绕过 loader 提供的**可替换 global `string` 表**；`string_primitive_hook_test.sh` 对 copied-table wrapper 命中 0/0/0。它不防 native C hook、debugger 或直接 patch。

## 5. 已修复：嵌套 closure upvalue scatter seed 错位

融合前后的 32-seed 对照发现同一失败集合（17 个 seed：`7, 42, 999, 31337, 4242, 12345, 65535, 123456, 1234567, 222222, 555555, 888888, 10000000, 12345678, 64065, 64076, 64077`），运行报 `attempt to call a number value`。临时 VM 调用探针定位到 `make_accumulator` 的内层 vararg closure：`fold_text` 调用时误取得 `MOD=1000003`。

根因是 child `CellKind::Up(u)` descriptor 在 parent `scatter_operands` 置换 `upsrc` 前按旧序号写入，导致部分 seed 的子闭包索引错位。修复在编译递归闭包之前预计算每函数 upvalue permutation，并把 child descriptor 映射到父帧最终 upvalue 序；scatter 使用同一 permutation，同时消耗原主 RNG 的 shuffle draws 以避免扰动其他布局。

`compiler.rs` 新增 permutation 双射/确定性单测；`tests/r064_seed_stability_test.sh` 覆盖 24 个历史失败与相邻 seed，并接入 `run_tests.sh` 和 `multiseed.sh`。修复后当前 release 的 42-seed sweep 失败 0，集成 24-seed sweep PASS。

## 6. 已跑自检

- `cargo test`：39 passed、0 failed、1 ignored。
- `tests/run_tests.sh`：169/169；`tests/run_presets.sh`：370/370；`tests/multiseed.sh`：`FAIL=0`（含 R064 专项 24-seed 稳定性回归）。
- 新增 `tests/micro_fusion_test.sh`：v15 和 legacy 输出一致；`tests/string_primitive_hook_test.sh`：minimal 与 R064 源在复制 string 表 hook 下输出等价，byte/sub/char 命中均为 0；`tests/h5_p0_micro_audit.py`：PASS。
- R064 样本通过 `luau-compile`，源与产物标准 Luau stdout 逐字一致；短名审计 **6,379** 个局部/参数/循环标识符，均不超过两个字母。
- `security_fingerprint.py`：S1–S5 **5/5**；KeyEmitter 字面值扫描通过；D5/D6 30 个活动载体跨度篡改点全部拒绝。
- H2/H3、H4 loader（`--enforce-zero`）、H4-5/H5-P0/H6/H7/H8 和 R061 bypass audits 通过。
- `defense_audit.sh`：D12 计时守卫的合成慢/干净时钟与误报抽查通过；D3 仍报告 23/32 并返回 FAIL，这是结构相似度指标，不是 S1–S5 安全或运行等价失败。

## 7. 边界与限制

完整 recipe fusion 是增加轨迹整理成本的缓解，不是密码学保护。当前每个多步 handler recipe 只留下一个 per-shard record；同进程代码插桩者仍可记录 superoperator 类型、handler 参数及执行顺序，再配合内部状态追踪语义。它不会自动得到未执行分支或原始变量名，但客户端逻辑不能因此被视为保密；客户端不适合保管服务端级密钥、授权或反作弊决策。

真实 Roblox executor 未在本地直接运行；protected-meta 由模拟保护 sentinel 的 Luau 回归验证。R064 变量名压缩仍是体积/形态优化，不是安全边界。

> **第二轮深度自检与修复**：融合前/第一层/完整 recipe fusion 的同点 A/B trace、`string.byte` 环境 hook、嵌套 upvalue scatter 根因/修复及 seed sweeps 见 `docs/r064-deep-self-audit-r2.md`。当前 full recipe fusion 将该 R064 样本的解码记录从 929,567 降到 412,706；seed 运行失败集合已通过 parent/child upvalue permutation 对齐修复；同进程动态观察边界仍保留。本文件保留 R064 第一轮兼容修复快照。