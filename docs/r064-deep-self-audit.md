# R064 7KB 示例与深度自检

> 日期：2026-10-07
> 分支：`arena/01a038eb-luraph`
> 范围：R064 示例、protected-metatable 兼容及尾 nil 变参计数修复；不是“不可破解”证明。

## 1. 交付样本

- 源码：`handoff/R064_7kb_src.lua`（7,179 字节，纯本地 Luau/Lua 语义测试，无 Roblox、网络或文件 API）
- 产物：`handoff/R064_7kb_target.luau.lua`（seed `64066`，336,389 字节）
- SHA-256：源 `5f3295d245a4e63d229d9da1022b88e1234335bc177a78256a76597de3fd7a99`；产物 `645c45032f225584335e6818875599d406a35d0739ea952ee4932343a11c2fc3`

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

## 4. 已跑自检

- `cargo test`：32 passed、0 failed、1 ignored。
- `run_tests.sh`：169/169；`run_presets.sh`：370/370；`multiseed.sh`：`FAIL=0`。
- R064-7KB 源与产物均通过 `luau-compile`，标准 Luau 输出逐字一致；短名审计 5,960 个局部/参数/循环标识符，均不超过两个字母。
- `security_fingerprint.py`：S1–S5 **5/5**；KeyEmitter 字面值扫描通过；D5/D6 30 个活动载体跨度篡改点全部拒绝。
- H2/H3、H4 loader（`--enforce-zero`）、H4-5/H5-P0/H6/H7/H8 和 R061 bypass audits 通过。
- `v15_fingerprint.py`：该样本 **23/32**。这是结构相似度/指纹分数，不等同于可解密漏洞。
- `defense_audit.sh`：D3 为 23/32；D12 提取器因硬编码 `args[26]` 抛 `IndexError`，官方该子项未完成。独立合成时钟测试中慢时钟得到 `poison=195, delay=10`，正常时钟为 `0,0`；模板接线存在，审计提取器待更新。

## 5. 仍存在的动态观察边界

临时白盒插桩在 micro 解码出 `op,x,y` 后曾记录 7KB 样本的实际执行路径与状态（多个 shard）。这说明 at-rest descriptor 封存不等于运行态不可观察：具备代码插桩能力的分析者仍可收集已执行路径；它不是对未修改产物的自动脱壳，也不会恢复未执行分支或原始源码。探针未放入正式产物。

真实 Roblox executor 未在本地直接运行；protected-meta 兼容性由模拟保护 sentinel 的 Luau 回归验证。R064 变量名压缩是体积/形态优化，不是密码学安全边界。
