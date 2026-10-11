# 实验进度快照

> 本文件由 `python3 research/antidebug-20261009/summarize.py` 更新；不是实时刷新页面。
> 五小时长跑的较新状态见 `extended-status.json`，逐条记录见 `extended.csv`。

- 长跑状态：**running**。
- 已用单调时钟时间：**1076.99 秒**；预算 18,000 秒，不代表已完成。
- 已记录：**1200** 个源码/seed 组合，**1193** 全链对照通过，**7** 出现差异。
- 这些是未修改引擎的运行 + 外层 adapter 对照，**不是正式 VM 内植入候选后的验证**。
- 首轮：117 组，116 通过 / 1 现役产物差异；独立重现见 `baseline-discrepancy.json`。
- 研究模型：20,000 轮；160,000 次合法调用完成验证；279,535 次合法边推进。完整状态/当前凭证绕过、合法边内改分支、线性伪造均已构造反例。
- Luau 定向语义：9 项正向检查，9 项退步/失效反例复现。
- 调用凭证微基准中位开销比：**1.703×**（相对于同样 pack/pcall 的无凭证 adapter；不是整个 VM 的开销）。
- 现役固定测试：cargo 39+4 passed / 1 ignored；主矩阵 169/169；预设 370/370；loader / H5-P0 通过；防御审计 20 pass / 1 fail，D3 23/32 为已知结构分。
- 不变性复核：353 个原有文件，变化：`[]`。

## 长跑当前已记录差异（最多保存 25 项详细条目）

- `obsucvim/tests/cases/stress_multival.lua` / seed `3525582012654347846`：`production-output-difference:0`。
- `obsucvim/tests/cases/stress_multival.lua` / seed `12552293900259736015`：`production-output-difference:0`。
- `obsucvim/tests/cases/stress_multival.lua` / seed `2087317583667228854`：`production-output-difference:0`。
- `obsucvim/tests/cases/stress_multival.lua` / seed `16157647636138958350`：`production-output-difference:0`。
- `obsucvim/tests/cases/stress_multival.lua` / seed `10407755351155896398`：`production-output-difference:0`。
- `obsucvim/tests/cases/stress_multival.lua` / seed `6026130555315556988`：`production-output-difference:0`。
- `obsucvim/tests/cases/stress_multival.lua` / seed `2791381894518475382`：`production-output-difference:0`。

**只有 state 为 `completed-time-budget` 才表示跑到设定时间预算。`stopped` / 沙箱进程丢失不算完成。**
