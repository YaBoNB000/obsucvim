# v19-check：V19 真机验收（第三轮 = R076/M1-bb 终态，**已收口 ✓**）

两件同种子（20261025）、同输入（`src-kkksu-mini.lua`，斐波那契打印）：

| 件 | 构建 | 裸 luau（已实测） | 真 Delta（实跑结果） |
|---|---|---|---|
| `kkksu-mini.v19evkey.hard.lua` (176,057B, sha `5b7d1dbd…`) | `--preset v15 --env-check --bind-executor` + `OBSUCVIM_V19_EVKEY=1`（R076 形态：M1 全部 + R5-b 熵坍缩次批：strseal/ConstLcg/ct-csb/oc_boot/fv/APH/TKD/v15-kn 静态可常折面清零） | **干净拒绝**（缺 API 文案，不挂死） | **✅ 通过（2026-10-25，用户回传「正常」）**：真 Delta 正常打印 `kkksu-mini:1,1,...55` ⇒ R5-b 七链真机全线成立，**M1-bb 真机收口** |
| `kkksu-mini.v19ctl.hard.lua` (173,848B, sha `352ea2d2…`) | 同上但旋钮关（EVKEY=0；R076 纹理、旧明文 oracle） | 精确缺 API 拒绝（既有行为） | 上一对同型件两轮已验收 ✓（本轮重出仅作对照，免跑亦可） |
| `diag-evk.lua` (2,407B 明文诊断) | — | 打印各通道存在性与折叠值 | 判据：k2a=1143546169 / k2b=1317214663 / k3c=22 / EVK=4576808（探针公式未变，件保留） |

## 验收历史

- 第一轮（R4 原型 v2，172,358B `f6c1f45e…`）：**通过 ✓**（用户回传「正常了」），
  v1 挂死根因（取值通道未镜像 LKP）已在 v2 修复；对照件（169,171B `80044d24…`）同轮验收 ✓。
- 第二轮（R075/M1 终态，174,679B `3a1f77f5…`）：**通过 ✓**（2026-10-25 用户回传「正常」），
  M1 合并轮（EVKEY 默认 On + R6 + R3 + R5-a）真机全部成立。
- 第三轮（本目录现件，R076/M1-bb 终态）：**通过 ✓**（2026-10-25 用户回传「正常」），
  R5-b 七链在真 Delta 全部成立 ⇒ **M1-bb 真机收口，静态可常折面清零真机定案**；
  `diag-evk.lua` 未动用（无异常），留档备查。

## v2 修复说明（存档）

初代 evkey 件真机挂死定案：探针取 `getfenv(1)` 拷贝表，而 Delta 注入面在
`getgenv()` 侧（exguard LKP 通道）→ EVK=0≠4576808 → blob 失步挂死。
修复：取值通道镜像 LKP（getgenv 优先、getfenv(0) 兜底）+ fail-fast 干净拒绝。

M1-b 追加修复：EVK 探针外置声明（build 骨架 do 块词法断层导致 st2 壳 EVKZ
供桨为 nil）。

M1-bb 追加修复：R5-b 新注入点内一律改用幂二次 `(x-(x%D))/D` 整除——oc_boot /
LoadStr 等注入点作用域并无 FLR 绑定（裸跑 `attempt to call a nil value` 当场
现形）；既有 FLR 站点（mb/APH/TKD）维持原样。

纪律：Dev 档（`OBSUCVIM_V19_EVKEY_DEV`）产物零安全性，严禁交付。
