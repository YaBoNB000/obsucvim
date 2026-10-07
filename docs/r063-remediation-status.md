# R063 修补阶段自检记录

> 日期：2026-10-06
>
> 已完成入口表观察点和 KeyEmitter 两个阶段。LoadStr codec、编译器名称封存、动态 `char` 风险尚未实施。

## 1. 入口模块表：不再传给全局 `setmetatable`

v15 外层现在把模块表保存在随机命名的词法局部变量中，再以原来的冒号调用方式进入入口处理器。**VM 内部需要的 `setmetatable` 调用仍保留**；只移除了把完整入口表作为参数交给 loader 可替换的全局函数这一步。

回归探针包装 `setmetatable`，保留每个参数表的活引用，并在执行后检查。R063 样本返回 `1713`；探针观察到 11 个内部表参数，最大 14 个字段、0 个函数，没有捕获到 R062 中 216 字段、187 个函数的入口总表。R062 已保存的旧目标文件保持不变；修补作用于之后生成的目标。

**边界：**这关闭的是“完整入口表经 `setmetatable` 一次性暴露”这一观察点，不会阻止有调试权限的执行器检查局部变量或跟踪其他运行时数据。空元表在 scaffold 路径没有被观察到使用，但 `getmetatable` 对象身份语义仍需视为实现差异。

## 2. KeyEmitter：逐使用点绑定 carrier 长度向量

旧的四张明文数值表和对应的表索引表达式已从 KeyEmitter 路径移除。每个 key 使用点现在内联一段独立的模运算 digest：构建端用所有 carrier 的长度及该使用点的可选阶段锚点生成掩码值；运行端对现有 `FN` carrier 字符串表（HBoot 使用已有的 `FN_` 参数）执行同一运算并还原该点的 key。

设计约束：
- 不新增 KeyEmitter 数值表、统一 getter 或解密后的 key 缓存。
- digest 使用实际传给 VM 的 carrier 长度，改变 carrier 会改变解码结果；不是 `state-state` 形式的假依赖。
- KeyEmitter 新路径不调用 `string.byte`，避免额外创建逐字节 API 汇合点。
- 已删除 `OBSUCVIM_KEY_DBG` 构建环境变量直接输出 key 字面量的分支。
- HBoot 原本传递的四个 KeyEmitter 表形参改为四个未使用的 FN 引用，以维持其 25 个随机化形参/实参槽；未增加 key 数据或共享 getter。

**边界：**这是针对“读四张明文表并常量折叠”的加固，不是密码学保密。若攻击者能重建所有 carrier 长度或直接执行/跟踪客户端，仍可复算 key；R063 新格式尚未接受完整通用静态解码攻击。

### 运行时观察自检：已有 `string.byte` 环境 hook

我用 loader 提供一份复制的 `string` 表，只把 `byte` 换成“记录参数后转调原函数”的透明 wrapper；R063 仍正常返回 `1713`，wrapper 观察到了 VM 正常解码时处理的长字符串输入（46 个高频不同字符串，最长 1401 字节）。同样的环境 hook 对历史 R062 目标也能运行。因此这是 VM 原有 carrier 解析路径的条件性动态观察面，不是 KeyEmitter 新增的 `string.byte` 调用点。

但若攻击者从该观察面关联出 FN carriers，就能得到长度向量，再按产物中的 per-site 参数复算 KeyEmitter 值。故这项仅提高纯静态表折叠成本，**不防 loader/executor 级 carrier 跟踪**。本轮不继续宣称或扩展为动态防护。

## 3. 候选自检记录

两种候选未进入最终实现：

1. 每个 key 写三个等价算术分支：简单分支提取器可折叠 52 个表达式并恢复 51 个不同值，故撤回。
2. 对 carrier 字节做逐字节 hash：虽然能绑定内容，但会在 KeyEmitter 新增 `string.byte` 汇合面，可能形成新的单函数 hook 观察点，故撤回。

最终采用长度向量 digest。自动静态形态审计确认 raw VM template 中没有 `KA/KB/KC/KM` 表或索引；`key_literal_check.py` 的 manifest 扫描通过。完整执行仍会经过项目原有字符串/载体处理路径，这属于客户端运行时观察边界，并不表示密文不可解。

## 4. 验证与已知结构回归

- `cargo test`：32 passed、0 failed、1 ignored。
- `run_tests.sh`：169 passed、0 failed。
- `run_presets.sh`：370 passed、0 failed。
- `multiseed.sh`：`FAIL=0`。
- H2、H3、H4 loader/dialect/window/decentralization/poison、H5-P0 micro 和 R061 bypass audits：通过。
- `defense_audit.sh`：D2、D4–D18 通过；D13 debug 构建通过；**D3 结构指纹为 24/32**。旧指纹要求入口表由 `setmetatable` 返回，且若干 v15 形态随 KeyEmitter 改变；未降低或改写其阈值，故整个防御审计仍报告 1 项失败。
- `r063_entry_table_hook_regression.sh`：语法校验和运行通过；`setmetatable` hook 未捕获模块总表；目标返回 `1713`；同 seed 下 `OBSUCVIM_KEY_DBG` 不改变产物；KeyEmitter 数组/索引静态形态检查通过。

D3 是结构相似度/指纹回归，不是已发现的解密漏洞，但会让产物更容易与旧 v15 形态区分。本轮在此停止，不继续改 LoadStr、名称封存或指纹阈值。
