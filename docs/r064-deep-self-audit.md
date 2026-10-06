# R064 7KB 示例与深度自检

> 日期：2026-10-06
> 分支：`arena/01a038eb-luraph`
> 范围：生成器/示例自检；不是“不可破解”证明。

## 1. 交付样本与本次修复

- 源码：`handoff/R064_7kb_src.lua`（7,187 字节，纯本地 Luau/Lua 语义测试，无 Roblox、网络或文件 API）
- 产物：`handoff/R064_7kb_target.luau.lua`（seed `64066`，342,991 字节）
- SHA-256：源 `bf45fa61691e69a73d1fd319f7f632d7413d8695fa8eb24ef0a0eda3def38ada`；产物 `91ffa87dac00bfe5f8e3a50d709341934f443ad22f65bdf52022fb8275204031`

用户报告的 `attempt to index a string value` 已在此示例中修复：原测试源使用 `row[2]:sub(...)`，需要通过字符串的受保护元表取方法；改为 `string.sub(row[2], ...)` 后重新生成产物。用 executor-like 测试环境（令 `getmetatable(string)` 返回受保护标记）精确复现了同一错误；修复后探针正常输出，源和产物结果一致。

**修复范围说明：**这是该示例的兼容性修正，没有改变 VM 的通用 `M_GET_TABLE` 元表处理。若其他输入也用字符串方法语法，且目标 executor 隐藏字符串元表，仍需独立检查/考虑通用修复。

## 2. 已跑检查

- 源和产物均通过 `luau-compile`；标准 Luau 下输出逐字一致。
- executor-like protected-string-metatable 探针：修复后通过。
- 短名审计：5,811 个局部/参数/循环标识符，全部最多两个字母。
- `security_fingerprint.py`：S1–S5 **5/5**；`key_literal_check.py` 通过；D5/D6 字节互换测试 **30/30** 拒绝。
- `run_tests.sh`：169/169；`run_presets.sh`：370/370；`multiseed.sh`：`FAIL=0`。
- 本产物 `v15_fingerprint.py` 为 **24/32**。这是结构相似度/指纹分数，不等同于可解密漏洞。
- `defense_audit.sh` 的 D12 提取器仍因硬编码 `args[26]` 抛 `IndexError`，官方该子项未完成。另用独立合成时钟测试：慢时钟得到 `poison=195, delay=10`，正常时钟为 `0,0`；模板中的 `os.clock → CLK → RTFRAG` 接线存在。建议修复过时的审计提取器。

## 3. 未闭环发现

### A. 变参末尾 nil 的语义不一致（已复现，正确性缺陷，未修复）

最小输入：

```lua
local function count(...) return select("#", ...) end
print(count(nil, "tail", false, 7, nil))
```

源程序输出 `5`；seed `64066` 的混淆产物输出 `4`。`makefn_decl` 先执行 `local all = { ... }`，再用 `#all - npa` 计算变参数量；表中存在洞或尾部 nil 时，`#` 不能可靠代表实参数量。现有矩阵通过，但未覆盖显式尾部 nil。上传的 7KB 样本没有尾部 nil，因此样本自身等价；通用问题仍待修复，建议用精确 vararg 个数替代 `#all` 并增加回归用例。

### B. Micro-shard 执行期仍可插桩（白盒动态观察面）

在 `/tmp` 的临时生成器副本里，只在解码出 `op,x,y` 后加入探针；本次修复后的 7KB 样本运行记录了 **906,284 条**执行期记录，覆盖 4 个 shard（日志约 62.7 MB），正常结果行仍出现。探针可观察已执行的微码三元组及 shard/参数/局部状态。

这说明 at-rest descriptor 封存不等于运行态不可观察：具备代码插桩能力的分析者仍可收集**已执行路径**。它不是对未修改产物的自动脱壳，也没有恢复原始变量名/逐字源码或未执行分支；临时探针和日志未放入仓库，正式产物中没有追踪代码。

## 4. 风险边界

本示例不加载外部代码，已做本地运行等价和语法验证；未在真实 Roblox executor 中运行。R064 的变量名压缩是体积/形态优化，不是新的密码学安全边界。
