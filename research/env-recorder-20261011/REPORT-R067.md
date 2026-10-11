# R067（绑定态 canary）× 环境记录器/智能桩 实测报告

**日期：2026-10-11。对象：R067（`--env-check --bind-executor --seed 67067`，229,422 字节，SHA-256 `3775f3b5…`）。攻击器：`r067_attack.luau` + `run_r067_attack.py`（复用 tier-2 忠实 Roblox mock + 新增执行器面两级桩）。**

## 结论（先说清）

**exguard 没有防住 D 级攻击者。而且比 R066 更进一步：连记录器都不需要——一个探针感知的智能桩把绑定态产物在裸 Luau CLI 里直接跑通，打印 `suda`。**

| 实验 | 结果 | 证据 |
|---|---|---|
| 裸 CLI（既知） | 死 | `arithmetic on nil`（gate 2：真 `_G` 只读，无 getgenv/debug.getconstants 可注入） |
| 半吊子桩（E3 形态）direct | 死 | 毒杀于 **gate 10**：`getconstants(string.byte)` 返回 `{}` 未报错 → C 隔离闸拒；无 `suda` |
| 半吊子桩 replay | record 阶段即死 | 同上 |
| **智能桩 direct（无记录器）** | **活：`suda`，rc=true** | 74 条执行器 API 调用 = 完整 17 闸门序列全过 |
| **智能桩 D 级录放** | **两阶段皆 `suda`，leftover=0** | record 恰 33 条（buffer×30 + unpack×3），replay 队列清零，exguard 两阶段调用序列一致 |
| seed 42 产物 + 同一智能桩 | 活：`suda` | **桩是行为化的，seed 无关**——一次逆向，全部产物沦陷 |

## 攻击构造（四个技术点）

1. **`getfenv(0)` 重定向**：exguard 经 `pcall(getfenv, 0)` 取环境，而 Luau CLI 的 `getfenv(0)` 恒返回真 `_G`（且真 `_G` 与 `debug` 表均只读，无法注入 getgenv/内省 API）。攻击者在**影子环境**里把 `getfenv` 换成包装器：仅 level-0 返回攻击者构造的 FAKE_G（含 `getgenv`→CAP、`debug`→含内省桩的代理表）；`getfenv()`/`getfenv(n>0)` 按层级平移保持真语义（tier-1/tier-2/VM 的环境查找不受影响）。环境层本来就是攻击者完全控制的——这是 envlog 类工具的标准动作。
2. **C/Lua 分类**：`iscclosure`/`islclosure` 用 `debug.info(f,"s")=="[C]"` 实现——对真函数保真；`newcclosure` 返回打标记的包装函数，分类桩对标记特殊处理。
3. **getconstants 靠"调用"**：守卫的探针闭包都是 `return 常数` 形态——桩直接 `pcall(f)` 把返回值包成表。loadstring 探针产物（源码 `return 70636933`，本 seed 的 build 期常数）的常数检查恰好被此法命中。
4. **setupvalue no-op**：gate 13 的探针是"设 changed → 再设回 expected → 验证 expected"——**no-op 桩天然通过**（UPV 从未离开期望值，见"加固建议①"）。真执行器里两次 setupvalue 真实生效，结果相同——该闸对 no-op 不设防。

tier-2 电池（Enum/Random/task/Instance/clonefunction）与 tier-1 电池由忠实 mock + 真 CLI 库通过（fragment 级 EVD=0 早已实证，本次整产物级复现）。

## 完整闸门序列（智能桩 direct，74 调用）

```
getfenv(0)→FAKE_G ×5        环境获取 + CIDENT 复取 + char 自举扫描
getgenv ×3, identifyexecutor ×2, checkcaller     闸 3–8（分离/金丝雀/稳定性/宿主）
iscclosure+islclosure(Cfn) ×20 对 + _lprobe 一对   闸 9 原语分类
getconstants/getupvalues/setupvalue/getproto/getprotos(Cfn)   闸 10 C 隔离（全部报错 ✓）
getconstants(_cprobe) → getupvalues(_uprobe) → setupvalue ×2   闸 11–13 transcript
iscclosure(_actp) + getproto ×2 + getprotos      闸 14 proto 探针
iscclosure/islclosure("return 70636933") + getconstants   闸 15–16 loadstring 探针
newcclosure(_cbody) + 分类包装 + getupvalues(包装)   闸 17 newcclosure
getfenv(0) ×2               周期轻档复检
```

半吊子桩的对照证明闸门是活的：同一 harness、只换桩实现，死在 gate 10。

## 对威胁模型的对位

- **R066**：D 级攻击 = 真库 + 最小录制（buffer×33 + unpack×3）→ 成功。tier-1 电池可录放（L1 空缺）。
- **R067**：D 级攻击 = 真库 + 最小录制（buffer×30 + unpack×3）+ **手工智能桩** → 成功。exguard 的全部增量 = 攻击者必须**逆向守卫的探针形状**（一次性成本；seed 随机化只改变常数如 70636933，而智能桩是行为化的、不需要知道常数——seed 42 实证）。公开 envlog 工具（纯录放）今天不做这件事，它们的 record 阶段仍需真执行器；但任何认真读代码的对手可以把桩写进工具，之后**所有 seed 的产物通杀**。
- E2（裸 Luau 死）与 E3（半吊子桩死）**仍然成立**——但它们只覆盖朴素桩类。E2/E3 的"fail-closed"叙事需要限定：**对探针感知的环境层伪造不成立**。
- 既有防线（D9/AV/B-3）不受影响：A 级全录（含钩点替换）照旧死；本次未重跑 A 级（R066 已钉死，R067 未改该层）。

## 顺带发现的 bug（未修，按研究纪律零产线改动）

- **`OBSUCVIM_EXG_TRACE=1` 诊断构建生成期 parse error**（`--bind-executor` 单开也复现，`obsucvim: parse error line 187/189`）——诊断设施失效，无法用它定位闸门；本次改用桩侧调用序列日志完成定位。修复属引擎改动，待独立增量。

## 诚实边界

- 未在真 executor 上验证（智能桩在真 executor 上的行为无意义——那边本来就是真环境）。
- 智能桩的构造依赖 `exguard.rs` 源码级的探针知识（本仓库内部视角）；外部攻击者需先穿透 v15 混淆读出探针形状——成本真实存在但一次性。
- 录放实验中 mock/CAP 两阶段共享同进程状态（R066 harness 同构）；leftover=0 且输出一致，未见交叉污染。

## 加固建议（研究提案——已按用户决策搁置）

> **2026-10-11 用户决策：防环境记录器强度已经足够。** 下列 6 条提案不实施，存档备查；L1 录制重放空缺为已接受边界（现有防线 = D9/AV/B-3 + exguard 默认开启抬高对手门槛）。

1. **gate 13 反 no-op**：改为"设 changed → 调用验证 changed → 设回 expected → 调用验证 expected"。一行顺序调整，no-op 桩即死。
2. **getconstants 探针去可调用化**：常数检查改用调用不可揭示的形态（如 `if v then return N end return 0`，v 为 nil 上值）——调用式桩只能拿到 0。
3. **探针闭包计数调用**：`_cprobe` 内置调用计数，getconstants 返回后验证计数为 0——直接检测"靠调用的桩"。
4. **identifyexecutor 返回值哈希折入 ATTEST**：当前只查自洽，常量假串可过；折入令牌后假串 → 错钥。
5. **getfenv 纳入 prims 分类**（`iscclosure(getfenv)==true`）：包装器是 Lua 闭包即死——需先核实真执行器形态兼容性。
6. （更大的方向）录放对抗的根子仍是 L1：环境答案服务对"行为效果"类探针（setupvalue 真实变异、调用计数）天然失效——方向 1+3 组合是最低成本的实质增量。

## 复现

```bash
# 智能桩直接运行（无记录器）——打印 suda
python3 research/env-recorder-20261011/run_r067_attack.py handoff/R067_target.luau.lua smart direct
# D 级录放——两阶段皆 suda，leftover=0
python3 research/env-recorder-20261011/run_r067_attack.py handoff/R067_target.luau.lua smart replay
# 半吊子桩对照——死
python3 research/env-recorder-20261011/run_r067_attack.py handoff/R067_target.luau.lua half direct
# seed 无关性
obsucvim/target/release/obsucvim --env-check --bind-executor --seed 42 \
  handoff/R067_src.lua /tmp/R067_seed42.luau
python3 research/env-recorder-20261011/run_r067_attack.py /tmp/R067_seed42.luau smart direct
```

harness 模板 `r067_attack.luau` + 驱动 `run_r067_attack.py` 入库；`work/` 生成物不入库。研究全程零产线改动（git status 仅 research/ 新增两文件）。
