# R064 7KB 示例与深度自检

> 日期：2026-10-07
> 范围：R064 示例、protected-metatable 兼容、尾 nil 变参计数修复，以及 micro-shard 安全微操作融合；不是“不可破解”证明。

## 1. 交付样本

- 源码：`handoff/R064_7kb_src.lua`（7,179 字节，纯本地 Luau/Lua 语义测试，无 Roblox、网络或文件 API）
- 产物：`handoff/R064_7kb_target.luau.lua`（seed `64066`，338,277 字节；融合前同 seed 产物 336,389 字节，净增 1,888 字节，约 +0.56%）
- SHA-256：源 `5f3295d245a4e63d229d9da1022b88e1234335bc177a78256a76597de3fd7a99`；产物 `6e322c6d405f2e1731149cd7af66eed7dbef0a06d1517044e15349394e98c0d5`
- v15 结构指纹 22/32；仅表示结构相似度，不表示可解密性或运行安全性。

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

每个 shard 的 at-rest/session 描述符仍按 H5-P0 封存。为减少执行时的细粒度轨迹，生成器把 10 种安全的 handler 内部序列融合成一个随机编号的 superoperator：`LoadK`、`Move`、`NewTab`、`Not`、`GetUp`、`SetUp`、`VarArgC`、`Jf`、`Jt`、`VarArgTab`。它们从 2–3 条记录降为 1 条，对这 10 类各执行一次时从 23 条降为 10 条；不把这个比例外推为整个程序的 trace 降幅。融合后不再逐步写入/读取 shard-local `T` scratch。

Call/Return、LoadStr、全局表访问、算术/比较与表 GET/SET 等边界没有融合；其中涉及用户回调、元方法、字符串解密或错误/求值顺序。`LoadK` 可能触发 VM 私有常量池 lazy `CDEC`，但 fused body 先取常量到局部、后写寄存器，不移动用户回调。新增 Rust 单测和 `tests/micro_fusion_test.sh` 检查安全操作配方为一条记录、敏感操作维持旧序列，并用 Luau 比对 v15/legacy 输出。

历史临时白盒探针记录过 7KB 样本 **906,284 条**实际执行记录、覆盖 4 个 shard。该数字是融合前测量；本次没有声称完成融合后同一探针的总 trace 重测。静态配方回归能证明指定 handler 的记录数缩减，但不能证明任意运行中的观察者无法追踪。

## 5. 已跑自检

- `cargo test`：35 passed、0 failed、1 ignored。
- `tests/run_tests.sh`：169/169；`tests/run_presets.sh`：370/370；`tests/multiseed.sh`：`FAIL=0`。
- 新增 `tests/micro_fusion_test.sh`：v15 和 legacy 输出一致；`tests/h5_p0_micro_audit.py`：PASS。
- R064 样本通过 `luau-compile`，源与产物标准 Luau stdout 逐字一致；短名审计 **5,938** 个局部/参数/循环标识符，均不超过两个字母。
- `security_fingerprint.py`：S1–S5 **5/5**；KeyEmitter 字面值扫描通过；D5/D6 30 个活动载体跨度篡改点全部拒绝。
- H2/H3、H4 loader（`--enforce-zero`）、H4-5/H5-P0/H6/H7/H8 和 R061 bypass audits 通过。
- `defense_audit.sh`：D12 计时守卫的合成慢/干净时钟与误报抽查通过；D3 仍报告 23/32 并返回 FAIL，这是结构相似度指标，不是 S1–S5 安全或运行等价失败。

## 6. 边界与限制

Fusion 是增加轨迹整理成本的缓解，不是密码学保护。同进程代码插桩者仍可记录融合后的执行顺序/操作，再配合静态 handler 语义理解已执行路径；它不会自动得到未执行分支、原始变量名或完整源代码。客户端不适合保管服务端级密钥、授权或反作弊决策。

真实 Roblox executor 未在本地直接运行；protected-meta 由模拟保护 sentinel 的 Luau 回归验证。R064 变量名压缩仍是体积/形态优化，不是安全边界。

> **第二轮深度自检**：融合后 trace A/B、`string.byte` 环境 hook 与 32-seed R064 差分结果见 `docs/r064-deep-self-audit-r2.md`。第二轮仍确认同进程观察边界，并发现融合前/后的共同 seed 运行失败集合；本文件保留第一轮修复与验收快照。