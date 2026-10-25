# v19-check：V19 真机验收（第四轮 = R077/M2-R1，待真机回传）

两件同种子（20261025）、同输入（`src-kkksu-mini.lua`，斐波那契打印）：

| 件 | 构建 | 裸 luau（已实测） | 真 Delta（实跑结果） |
|---|---|---|---|
| `kkksu-mini.v19evkey.hard.lua` (178,974B, sha `c61cdc5c…`) | `--preset v15 --env-check --env-check --bind-executor` + `OBSUCVIM_V19_EVKEY=1`（R077 形态：M1 全部 + R5-b + **M2-R1**：SALT 三源强派生、三车道 lane 专属 NL 像入钥流） | **干净拒绝**（缺 API 文案，不挂死） | **2026-10-25 回传：报错** `qpTaFNJmHnfMMWNVP:3: attempt to call a nil value`（LocalScript Line 3 ×7 帧）。诊断件判据全绿（k2a/k2b/k3c/EVK 4576808 MATCH）⇒ EVKEY 环境底钥层成立，嫌疑切为「bind/exguard 共用链 vs evkey 专属线」 |
| `kkksu-mini.v19ctl.hard.lua` (177,109B, sha `1af735bc…`) | 同上但旋钮关（EVKEY=0；R077 纹理、旧明文 oracle） | 精确缺 API 拒绝（既有行为） | **需跑（调试对照）**：上三轮同型件已验收 ✓，但本轮它升级为分界实验——正常 ⇒ evkey 专属线故障；同错 ⇒ bind 共用链故障 |
| `diag-evk.lua` (2,407B 明文诊断) | — | 打印各通道存在性与折叠值 | **已回传全绿 ✓**：k2a=1143546169 OK / k2b=1317214663 OK / k3c=22 OK / EVK=4576808 MATCH-4576808（执行器名 "Delta"，isStudio=0，checkcaller=1） |

## 验收历史

- 第一轮（R4 原型 v2，172,358B `f6c1f45e…`）：**通过 ✓**（用户回传「正常了」），
  v1 挂死根因（取值通道未镜像 LKP）已在 v2 修复；对照件（169,171B `80044d24…`）同轮验收 ✓。
- 第二轮（R075/M1 终态，174,679B `3a1f77f5…`）：**通过 ✓**（2026-10-25 用户回传「正常」），
  M1 合并轮（EVKEY 默认 On + R6 + R3 + R5-a）真机全部成立。
- 第三轮（R076/M1-bb 终态，176,057B `5b7d1dbd…`）：**通过 ✓**（2026-10-25 用户回传「正常」），
  R5-b 七链在真 Delta 全部成立 ⇒ **M1-bb 真机收口，静态可常折面清零真机定案**。
- 第四轮（本目录现件，R077/M2-R1）：**evkey 件真机报错（assign to call nil @ inner chunk line 3），
  调查进行中**；diag-evk 首次动用，判据全绿（EVK=4576808 与构建期假设逐位吻合）。
  本地交叉取证：commit==源码==wasm==验收件三方恒等（sha 互验）；R076↔R077 模板 diff 仅 61 行
  （三源 SALT 派生 + CLK 守门 + _sv/_svk/_svq 三车道），其中 CLK 守门作用域分析为短路安全
  （TYP(nil)≠"function" 不触发），浮点 SALT 镜面双 realized 算术一致性复核通过；
  R077 no-bind 对照件在本地完整跑通打印 fib（共用 R1 车道机构无恙）⇒ 嫌疑聚焦
  bind/exguard 段（HBOOT 参数重排/EXGFRAG 变更，TSRC diff 的主项）。

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
