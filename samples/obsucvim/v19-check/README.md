# v19-check：V19 EVKEY 原型真机验收（2026-10-25）

两件同种子（20261025）、同输入（`src-kkksu-mini.lua`，斐波那契打印）：

| 件 | 构建 | 裸 luau（已实测） | 真 Delta（实跑结果） |
|---|---|---|---|
| `kkksu-mini.v19evkey.hard.lua` (172,358B, sha `f6c1f45e…`) | `--preset v15 --env-check --bind-executor` + `OBSUCVIM_V19_EVKEY=1` | **干净拒绝**（缺 API 文案，不挂死） | **通过 ✓（2026-10-25 用户回传）**：正常打印 ⇒ 探针算式/通道与真机一致，R4 通道成立 |
| `kkksu-mini.v19ctl.hard.lua` (169,171B, sha `80044d24…`) | 同上但旋钮关（=R073 形态） | 精确缺 API 拒绝（既有行为） | 正常打印（对照组，已验收 ✓） |
| `diag-evk.lua` (2,407B 明文诊断) | — | 打印各通道存在性与折叠值 | 判据：k2a=1143546169 / k2b=1317214663 / k3c=22 / EVK=4576808 |

## v2 修复说明（2026-10-25 晚）

初代 evkey 件真机挂死定案：探针取 `getfenv(1)` 拷贝表，而 Vector3/checkcaller/
identifyexecutor 在 Delta 注入在 `getgenv()` 侧（exguard 的 LKP 正是 getgenv
优先）→ EVK=0 ≠ 4576808 → blob 失步挂死。修复：

1. 取值通道镜像 exguard `LKP`——`getgenv()` 优先、`getfenv(0)` 兜底；
2. fail-fast：探针整体缺失（非真环境）→ 干净拒绝文案，不再挂死；
   值在但被改（hook 环境）→ 仍静默失步（投毒语义不变）。

本地复验：DEV 件 luau 打印正确；Bind 件裸 luau 干净拒绝；锚点 R073 恒等；单测 59 绿。

## 真机验收结果（2026-10-25，已收口）

- `kkksu-mini.v19evkey.hard.lua`（v2）：**通过 ✓**——用户回传「正常了」，按既定
  口径 = 真 Delta 正常打印，v2 修复成立，R4 通道（EVKEY 真宿主绑定）真机确认。
- 对照件（旋钮关 = R073 形态）此前已验收 ✓。
- `diag-evk.lua` 留档：后续若探针面再异动，仍按 k2a/k2b/k3c/EVK 四值定位。

纪律：Dev 档（`OBSUCVIM_V19_EVKEY_DEV`）产物零安全性，严禁交付。
