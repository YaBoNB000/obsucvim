# 自研环境记录器 × obsucvim 实测报告

**日期：2026-10-11。对象：R066（`--env-check --seed 66066`，190,869B）。攻击器：`replay_attack.luau` + 诊断变体（序列化按函数队列录制/重放，模拟公开工具 envlog.luau 形态）。**

## 结论（先说清）

| 重放形态 | 结果 | 死因 |
|---|---|---|
| **A. 朴素全录**（一切经 env 的函数都录制重放） | **失败** | ① D9 反挂钩闸检出 loadstring/error 被换成 Lua 闭包 → 毒杀（PD 螺旋）；② AV 折叠的 `math.floor(时钟派生值)` 参数每次运行不同，录制答案对新参数是错值 |
| B. + math 真实 | 失败 | 答案服务不复制**副作用**（setmetatable 等对新表不生效）+ 解码热路径仍录制 |
| C. + 状态函数真实（setmetatable/raw*/debug/newproxy/type…） | 失败 | 解码热路径（bit32 638 次 + string/table）仍录制——**每运行钥匙状态（SALT/epoch 掩码）流经调用参数**，他运行录得的答案对本运行钥匙是错值 |
| **D. + 解码热路径真实**（仅录 buffer 33 次 + unpack 3 次 + 零星） | **成功** | 重放完整跑通，打印 `suda`，队列恰好清零（leftover=0） |

## 分项发现

### 1. 环境检查电池（tier-1）完全可录制重放——L1 空缺实证

ENVFRAG 隔离实验（`envfrag_replay_test.luau`）：电池在纯日志重放下 **Phase A/B 均 EVD=0、队列零剩余**。变体 D 里电池的 33 次 buffer 调用全部由日志供答案，通过。**文档里"L1 空缺（常量挑战不防录制重放）"从声明变成实测事实。**

### 2. D9 反挂钩闸实测有效

逐函数二分：单独包裹 `error` 或 `loadstring`（换成 Lua 闭包）→ 产物静默挂死（PD 形态）；其余 20+ 个顶层函数单独替换不触发。闸的覆盖面正是经典脱壳钩点。

### 3. 解码热路径天然抗答案服务——B-3 设计的意外红利

bit32/string/table 的解码调用参数携带**每运行钥匙派生值**（SALT/epoch 掩码在纯 Lua 里混合后进入 env 调用参数）。录制答案来自他运行的钥匙流——对本运行是错值 → 解码垃圾 → PD。这不是 tier-1 电池的功劳，是既有 B-3 设计（"解码器需要只有活运行才物化的值"）在重放层的自然延伸。

### 4. AV 时钟折叠是朴素录制的第二个杀手

AV 折叠调用 `math.floor(os.clock派生值 × 乘子)`——参数跨运行必变。9189799754.397 这类值的参数错配被我们的诊断器抓到（FIRST-DIVERGENCE call 1160）。副作用级反重放，非设计目的但实测有效。

### 5. 攻击器自身的三个工程坑（对以后写红队工具有用）

- CLI stdout 全缓冲 + 超时杀进程 = 零输出。**靠输出体量强制冲刷管道**（逐调用打印 >4KB 才可见）；`error()` 中止法（正常退出会刷）只适合能退出的路径；
- 诊断输出里 `table.concat(args)` 遇对象参数直接崩——参数窗口要用直接索引；
- `pairs/coroutine/pcall` 必须 passthrough（迭代器/真线程/真执行内层），否则重放阶段交叉污染录制的运行。

## 对威胁模型的对位

- 公开工具形态 = **真 Luau 运行时 + 仿造环境层**：tier-1 电池用的 bit32/buffer/string 在任何真 Luau 里都是真的——**电池对这类工具零成本**（他们的沙箱直接用真库答题）。电池杀的是"语义不忠实的 mock 库"（L1'），不是录制重放（L1）。
- 全产物对朴素录制重放的抵抗来自**既有防线**（D9 闸、AV 折叠、B-3 每运行钥匙），不是新加的电池。
- tier-2（Enum/Random/task）同理：真 Luau 沙箱里这些是 mock 的主战场——那才是电池的对手盘。

## 复现

```bash
# 电池隔离录放对照（EVD 双零）
.tools/bin/luau research/env-recorder-20261011/work/envfrag_rp.luau
# 朴素全录（失败——D9 + AV）
python3 research/env-recorder-20261011/run_attack.py handoff/R066_target.luau.lua
# 变体 D（成功——最小录制集）
.tools/bin/luau research/env-recorder-20261011/work/variant_d.luau
# 分化追踪（FIRST-DIVERGENCE 定位）
.tools/bin/luau research/env-recorder-20261011/work/final_diag.luau
```

攻击器与诊断变体全部入库本目录；`work/` 生成物不入库。

## 诚实边界

- "成功"的变体 D 在 CLI 里 = 库全真 + 33 次 buffer 日志——对 CLI 产物这接近平凡（库本来就在）；**有意义的读法是测绘出"哪些面可以假、哪些必须真"**：电池面可假（L1 空缺实证），解码面必须真（B-3 红利），钩点面必须真（D9）。
- 未测 tier-2 产物（R066 是 tier-1）；未测 R064 大靶；未在真 mock-Roblox 沙箱里跑——结论限于 CLI 形态。
- phase 2 在变体 B/C 的确切毒杀点未 100% 定位到单一语句（值参数全匹配仍投毒，残余疑点在答案服务与每运行钥匙的交互）；已用 A/B/C/D 四级对照把边界钉住。
