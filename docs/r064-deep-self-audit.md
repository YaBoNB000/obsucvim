# R064 7KB 示例与深度自检

> 日期：2026-10-06
> 分支：`arena/01a038eb-luraph`
> 范围：R064 示例与 v15 protected-metatable 兼容修复；不是“不可破解”证明。

## 1. 交付样本

- 源码：`handoff/R064_7kb_src.lua`（7,179 字节，纯本地 Luau/Lua 语义测试，无 Roblox、网络或文件 API）
- 产物：`handoff/R064_7kb_target.luau.lua`（seed `64066`，339,542 字节）
- SHA-256：源 `5f3295d245a4e63d229d9da1022b88e1234335bc177a78256a76597de3fd7a99`；产物 `fe4dc1231e7d14c8796a692b63ce8aa936bb26b1e946dd9b9ebd01fa8c27daa6`

## 2. 通用字符串方法 / 受保护元表修复

R064-7kb 在 executor 报 `attempt to index a string value` 的根因，是生成解释器手工调用 `getmetatable(o)` 后直接读取 `mt.__index`。当宿主为字符串元表返回非表保护标记（例如字符串）时，旧路径会索引该标记。

生成器通用修复位于 `luraph-rs/src/vmgen/template.rs`：

- `mget` 先确认可见元表确实是表，遇到不透明 sentinel 不再索引它；
- 字符串 GET 走原生 Luau `o[j]`；受保护 table/userdata 的 GET/SET 在宿主隐藏元表时也回退到原生索引/赋值；
- 算术、比较、连接、unary 和 callable 路径在元表不透明时回退到原生 Luau 运算/调用。

源码保留 `row[2]:sub(...)`，没有再以改写调用点回避问题。新增的 `tests/protected_meta_compat_test.sh` 并接入 `run_tests.sh`；在 executor-like 环境中让 `getmetatable(string)` 返回保护 sentinel，并给表设置保护性 `__metatable`，逐项比较源与产物的返回值。覆盖 `:sub/:upper/:rep` 和受保护的 `__index/__newindex/__add/__concat/__lt/__eq/__unm/__len/__call`，回归通过。

## 3. 已跑验证

- `cargo test`：32 passed、0 failed、1 ignored。
- `run_tests.sh`：169/169；`run_presets.sh`：370/370；`multiseed.sh`：`FAIL=0`。其中新增 protected-meta 回归通过。
- R064-7kb 源和产物均通过 `luau-compile`，标准 Luau 输出逐字一致；短名审计 5,894 个变量，均不超过两个字母。
- `security_fingerprint.py`：S1–S5 **5/5**；KeyEmitter 字面值扫描通过；D5/D6 30 个活动载体跨度篡改点全部拒绝。
- H2/H3、H4 loader（`--enforce-zero`）、H4-5/H5-P0/H6/H7/H8 和 R061 bypass audits 通过。
- `v15_fingerprint.py`：该样本 **22/32**。这是结构相似度/指纹分数，不等同于可解密漏洞。
- `defense_audit.sh`：D3 为 23/32；D12 提取器因硬编码 `args[26]` 抛 `IndexError`，官方该子项未完成。独立合成时钟测试中慢时钟得到 `poison=195, delay=10`，正常时钟为 `0,0`；模板接线存在，审计提取器待更新。

## 4. 尚未修复/消除的边界

### A. 变参末尾 nil 的语义不一致（已复现，正确性缺陷，未修复）

```lua
local function count(...) return select("#", ...) end
print(count(nil, "tail", false, 7, nil))
```

源输出 `5`；seed `64066` 的 VM 产物输出 `4`。`makefn_decl` 用 `local all = { ... }` 后通过 `#all - npa` 计算变参数量；表尾 nil/空洞时，`#` 不能可靠代表实参数量。现有矩阵未覆盖显式尾部 nil。建议改用精确 vararg 个数并增加回归用例；本次 protected-meta 修复未改该路径。

### B. Micro-shard 执行期观察面

临时白盒插桩曾在 micro 解码出 `op,x,y` 后记录实际执行路径与状态，覆盖多个 shard。它说明 at-rest descriptor 封存不等于运行态不可观察：具备代码插桩能力的分析者仍可收集已执行路径；这不是对未修改产物的自动脱壳，也不恢复原始源码或未执行分支。探针没有放入正式产物。

## 5. 边界

protected-meta 修复通过 executor-like 环境模拟验证；本地没有真实 Roblox executor 可供测试。R064 变量名压缩是体积/形态优化，不是新的密码学安全边界。
