# R067 — 绑定态 canary：环境检查组 tier-2 + executor 认证绑定 + 水印（2026-10-11）

## 源码

```lua
print("suda")
```

源文件：`R067_src.lua`（14 字节，与 R066 同源，SHA-256 `514693ad0aafa711936bc69f447af9c1d943f2264b30c2912d543859e3e4eca2`）。

## 产物

- 文件：`R067_target.luau.lua`
- 命令：`obsucvim --env-check --bind-executor --seed 67067 handoff/R067_src.lua handoff/R067_target.luau.lua`
- 大小：229,422 字节，SHA-256 `3775f3b557ca4c95ee16d9d013f290d735b6c022af23ac17f1d0b2339fc689f6`
- 首行水印：`-- This file was obfuscated by Obsucvim.`
- `luau-compile` 通过；同 seed 重生成逐字节确定；无明文 `suda`（0 命中）。
- 与 R066（tier-1，190,869B）对比：+38,553B = tier-2 检查组（Enum/Random/task/成员调用/Instance/clonefunction，身份全等 27 引用）+ executor 认证守卫（exguard）。

## 本样本展示的产品形态

1. **英文水印**：首行品牌注释（所有输出路径统一）。
2. **WTF-PORT tier-1**：buffer/bit32/string/元表/协程/freeze/loadstring 常量挑战 + 17 身份引用。
3. **WTF-PORT tier-2**（双开才发射）：六组 executor/Roblox 语义检查 + 27 身份引用。
4. **executor 认证绑定**（`--bind-executor`，IronBrew2 守卫移植）：产物只在完整 Roblox executor 环境内正确运行。

## 验证矩阵（本产物实测，2026-10-11）

| 检查 | 结果 |
|---|---|
| 确定性（同 seed 重生成） | 逐字节一致 |
| 语法（luau-compile） | 通过 |
| 裸 Luau CLI 运行 | **静默死亡（预期）**：rc=1，`attempt to perform arithmetic (add) on nil and number`，无 `suda`、无 executor API 痕迹——符合 E2 口径（挂起或报错均可，正确输出不可） |
| E3 半吊子桩（getgenv/identifyexecutor/checkcaller 齐全但闭包内省不保真） | 被拒（`STUBDEAD`），无 `suda` |
| E4 可见层零痕迹 | 13 个 executor API 词全部 0 命中 |
| 明文泄露 | `suda` 0 命中 |
| 片段级正向（本构建 ENVFRAG 工厂 × 忠实 Roblox mock） | **EVD=0**（干净差值恰为零） |
| 片段级 9 破坏钩子 | 全部非零：no-game→运行错误；enum-interning→108,759,767；random-value→786,301；random-clone→102,273,715；task-spawn→61,902,279；dotcall→30,476,609；signal→180,362,979；destroy-lock→21,291,811；identity-swap→1 |
| 相关测试组复跑 | executor_guard 6/6；wtf_port_tier2 18/18（无引擎改动，确认工具链状态） |

## 与 R066 的关系

- **R066**（tier-1 canary）：CLI 可正向验证（stdout 同源码），保留为记录器研究样本（`research/env-recorder-20261011/` 的实测对象）。
- **R067**（绑定态 canary）：CLI 静默死亡是**预期行为**——认证要求完整 executor 语义。绑定态产物不进通用运行等价矩阵（只语法校验 + 负面实证 + 片段级 mock），与 `--bind-executor` 的 fail-closed 设计同口径。

## 诚实边界

- **正面运行等价未验证**：产物在真 executor 里打印 `suda` 这一点未实证——闭包内省（getconstants/getupvalues/getproto/setupvalue）在裸 Luau 无法保真构造（E3 桩已实证被拒），只能真执行器验证（Phase 0 真机清单未执行）。
- **L1 反录制重放空缺不变**：tier-2 全常量挑战杀的是语义不忠实的 mock 环境；真环境录制重放照常通过（见 `research/env-recorder-20261011/REPORT.md`——tier-1 电池已实证可录放；tier-2 未测）。
- 片段级 EVD=0 证明的是**本构建的电池在忠实 mock 下差值为零**（接线正确），不等于整产物在 executor 内正向运行。

## 生成与验证

```bash
obsucvim/target/release/obsucvim --env-check --bind-executor --seed 67067 \
  handoff/R067_src.lua handoff/R067_target.luau.lua
.tools/bin/luau-compile handoff/R067_target.luau.lua   # 语法过
.tools/bin/luau handoff/R067_target.luau.lua           # 预期：报错死亡，无 suda
```
