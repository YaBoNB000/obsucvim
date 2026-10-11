# 网络动态解混淆方法 × R065 实测报告

**日期：2026-10-10。对象：未修改的 `handoff/R065_target.luau.lua`（SHA-256 复核一致）。零产线改动；未运行任何第三方代码。**

## 方法来源（网络检索）

现行公开工具针对真 Luraph v15 的动态解混淆流程可归为五类（代表：[Deobfuscator-Luraph-V15](https://github.com/caomod2077/Deobfuscator-Luraph-V15)、[Lupeh](https://github.com/mariahsophia-jayz/Lupeh)、[PhoenixZeng/LuraphDeobfuscator](https://github.com/PhoenixZeng/LuraphDeobfuscator)、经典 StackOverflow/RE 手法）：

| # | 方法 | 说明 |
|---|---|---|
| M1 | load/loadstring 钩子 | 包裹加载函数截获交付编译的明文 |
| M2 | string.dump → 反编译 | 运行时转储字节码再上 unluac/luadec |
| M3 | AST 钩子/遥测注入 | 解析目标、定位 VM 派发环与闭包构造器，注入非破坏遥测 |
| M4 | 沙箱仿真 + 活体内存解密 | 离线环境模型跑起来，向活动 VM 逐个查询解密惰性常量 |
| M5 | 调试钩子逐事件扫描 | debug.sethook + debug 库读取栈上变量 |

这些工具都**自带改装版 Luau 二进制**——因为原版 Luau 不暴露它们需要的面。按仓库纪律不运行第三方不可信代码，以下在我们自己的 harness 里逐类复现。

## 运行时攻击面探测（stock Luau 0.735 CLI）

`debug` 库仅有 `info, traceback`；**`debug.sethook`、`debug.getlocal`、`string.dump` 均为 nil**。M2/M5 在原版运行时不可用，与 Roblox 同族一致——这就是公开工具捆绑自制运行时的原因。

## 实测结果

对照组 `plain_env`（纯克隆环境、零包裹）正常完成 ⇒ 环境克隆本身透明，以下差异全部可归因。

| 变体 | 结果 | 结论 |
|---|---|---|
| **M1 load 钩子**（loadstring/load 换 Lua 闭包） | **静默挂起（30s 超时）** | 反挂钩闸生效：正常流程里目标**根本不调用内部 loader**（ENVREAD 无 loadstring 项——零装载器架构实证），而把 loadstring 换成闭包触发身份门 → 毒杀挂起。**经典脱壳第一步在 R065 上被杀** |
| **M4 环境记录器/沙箱仿真**（全部库函数包裹记录+扫密） | 正常完成；4,245 次环境调用全记录；读集 = bit32(465 次)/math.floor(3,772)/table.concat(6)；**`bysuda` 出现 0 次** | 秘密不经过环境面：惰性常量解码 + 原生元表捕获把密文留在 VM 内部。环境模型做得再像也拿不到秘密 |
| **字符串表钩子**（复制 string 表包裹 byte/sub/char） | 正常完成；命中 **0/0/0** | R064 的原生元表捕获在 R065 上成立：可替换库表 hook 看不见 VM 内部 |
| **冻结时钟**（os.clock 恒值补丁） | 与正常完全一致 | 更强：目标**根本不从 chunk 环境读时钟**（读集无 os 项——经 `GFE(0)` 真 `_G` 路径），env 层攻击者连时钟面都摸不到 |
| M2 string.dump | 运行时不暴露 | 需改装运行时；且顶层闭包只是 VM 源码（本就在文件里），载荷无原生字节码可 dump |
| M3/M5 遥测+活体扫描 | 运行时不暴露 | 需改装运行时 = R004 物理边界；我们自己的白盒构建对 R065 测得 **5 条微码记录**（README-R065 存档），该边界不变 |

归因补充：envlog 变体里 `table.concat`/`math.floor` 等同样是 Lua 闭包却**不触发**击杀——身份门是**专查 load/loadstring** 的，不是泛查闭包数量。这正是 D9 的设计形态。

## 结论

1. **原版运行时可用的动态方法（M1、env 级 M4、字符串表/时钟补丁）对 R065 全部空手而归**，其中 M1 被反挂钩闸反杀（静默挂起）。
2. **真正剩下的攻击面 = 改装运行时的白盒插桩**（M3/M5/活体解密）——与文档一贯的 R004 边界一致，我们自己的白盒数据（5 微码记录）就是这条线的度量。公开工具走的就是这条路，成本 = 自带运行时 + VM 结构分析。
3. R065 是最小 canary：秘密只有一个字符串且程序无输出。**"秘密不经过环境面"的结论可推广**（架构性质），但"5 条微码记录"级别的白盒成本不能外推到大业务脚本（见 R064 报告）。

## 后续

第二轮（改装运行时白盒）见 [STRONGER.md](STRONGER.md)：字符串驻留钩子在 R065/R064 上各截获 1 次已使用的秘密（R004 边界量化），未执行路径的常量 0 命中（惰性解码有效），计时守卫被运行时级时钟补丁中和。

## 复现

```bash
.tools/bin/luau research/dynamic-r065-20261010/probe.luau
python3 research/dynamic-r065-20261010/build_harness.py   # 5 变体 + 直跑对照
```

产物：`harness-results.json`（含各变体退出码/输出/ENVREAD 全表）、`work/*.luau`（生成式 harness，gitignored）。R065 源/靶哈希在构建时断言一致。
