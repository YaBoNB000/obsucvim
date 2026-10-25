# ⚠️ 战役记录（历史）

> 本文件记录当时一轮红队/加固的完整过程与结论，路径与版本号均为当时口径。
> 路径对照见 [`../README.md`](../README.md) §3；现状见 [`../CURRENT-STATE.md`](../CURRENT-STATE.md)。

---

# R062 目标的离线解混淆记录

> 日期：2026-10-06
>
> 目标：`artifacts/targets/R062_target.luau.lua`（固定构建样本）
>
> 结果：已从目标本身恢复出语义等价的 Luau 源码，见 `artifacts/targets/R062_recovered.lua`。局部变量名与格式是人工重命名/排版，不声称逐字符复原原文件。

## 攻击步骤

1. **入口表截获。** 在 loader-controlled 环境中对 `setmetatable` 加透明 wrapper，先浅拷贝首个模块表，再调用原函数。目标仍运行并返回 `1713`；入口表快照包含 216 个字段、93 个数字键和 187 个函数值。
2. **取回载荷片段。** 从快照枚举候选 staging 函数；对每个函数传入克隆的模块表和独立 C 表，识别它写入的 `C[index]`。按 `index=proto*5+chunk+1` 顺序调用时，得到 15 个正确的 carrier chunk。逆序/独立重置 LCG 会得到乱码；共享状态按索引顺序则 chunk 可拼合。
3. **还原 carrier 与原型 blob。** 从输出的 KeyEmitter 数值表折叠索引，恢复 blob/alphabet 相关参数；从根表取 token 表，解 token escape 与 base-94，再用分段位置流解开 3 个 proto blob。
4. **解析常量与指令。** 对随机 section 顺序做结构搜索，利用 section 长度、原型头、SoA 流长度和常量流完整消费约束唯一确定布局。再解常量 LCG。KeyEmitter 数组中的 OC pair + HW2 alias 可恢复 opcode wire 映射；操作数流置换用 LoadStr 解出的可打印字符串交叉确认。
5. **字符串与语义恢复。** LoadStr 常量通过其指令 c/d seed/step 解出；恢复到 `spinel`、`#`、`脚本`、`suda`、`jade` 等字面量。把三段 proto 的闭包、循环、调用、表访问与返回指令提升成 Luau。

## 验证

- 恢复源码与目标分别包装为 vararg 函数运行，结果均为 `1713`。
- 目标 `luau-compile` 语法通过。
- 此次不是完整通用反编译器；攻击利用了 R062 的固定样本、已识别的编译器格式、明文 KeyEmitter 数值表与入口 hook。它证明的是**一个正常静态分析 + 一处透明启动 hook，已足以还原该样本的源码语义**。

## 突破点

- `setmetatable` 是 chunk 入口处稳定的一次性截获点，guard 对透明 wrapper 未报警。
- KeyEmitter 四张 96 项数字表及纯字面量下标可静态计算；本样本的静态脚本从目标恢复并核对到 48 个 build key manifest 值。
- 拿到 proto 操作数后，固定 LoadStr recipe 可直接恢复封存字面量；对短样本无需先完整解读每条 VM 指令语义。

以上攻击辅助脚本只在临时目录运行，样本恢复结果作为独立文件保存；未写入 build key 值。
