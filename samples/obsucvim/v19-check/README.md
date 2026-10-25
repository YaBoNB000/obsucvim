# v19-check：V19 真机验收（第四轮 = R077/M2-R1，待真机回传）

两件同种子（20261025）、同输入（`src-kkksu-mini.lua`，斐波那契打印）：

| 件 | 构建 | 裸 luau（已实测） | 真 Delta（实跑结果） |
|---|---|---|---|
| `kkksu-mini.v19evkey.hard.lua` (178,509B, sha `6add3d1b…`) | `--preset v15 --env-check --env-check --bind-executor` + `OBSUCVIM_V19_EVKEY=1`（R078 形态：M1 全部 + R5-b + **M2-R1 修复轮**：SALT 三源强派生保留 + CLK 守卫、三车道回退 R076 线性直加注入） | **干净拒绝**（缺 API 文案，不挂死） | **2026-10-25 第五轮（R078）判定性修复件**：真机判别飞行 WS2 同字节件已回传 **正常 ✓**（`6add3d1b…` 与 diff-flight 判别件逐字节恒等） |
| `kkksu-mini.v19ctl.hard.lua` (176,644B, sha `a537cdcf…`) | 同上但旋钮关（EVKEY=0；R078 纹理、旧明文 oracle） | 精确缺 API 拒绝（既有行为） | **2026-10-25 回传：正常 ✓**（R078 判定性修复后首件 ctl；V19 M2 首包双件全收） |
| `diag-evk.lua` (2,407B 明文诊断) | — | 打印各通道存在性与折叠值 | **已回传全绿 ✓**：k2a=1143546169 OK / k2b=1317214663 OK / k3c=22 OK / EVK=4576808 MATCH-4576808（执行器名 "Delta"，isStudio=0，checkcaller=1） |

## 验收历史

- 第一轮（R4 原型 v2，172,358B `f6c1f45e…`）：**通过 ✓**（用户回传「正常了」），
  v1 挂死根因（取值通道未镜像 LKP）已在 v2 修复；对照件（169,171B `80044d24…`）同轮验收 ✓。
- 第二轮（R075/M1 终态，174,679B `3a1f77f5…`）：**通过 ✓**（2026-10-25 用户回传「正常」），
  M1 合并轮（EVKEY 默认 On + R6 + R3 + R5-a）真机全部成立。
- 第三轮（R076/M1-bb 终态，176,057B `5b7d1dbd…`）：**通过 ✓**（2026-10-25 用户回传「正常」），
  R5-b 七链在真 Delta 全部成立 ⇒ **M1-bb 真机收口，静态可常折面清零真机定案**。
- 第四轮（R077/M2-R1 首包）：**evkey/ctl 双件真机同错 call-nil**；判别飞行
  WS1（盐单回退）挂、WS2（车道恒等）**正常 ✓**、WS3（全回退）**正常 ✓**
  ⇒ **三车道 lane NL 像定罪，盐区无罪**。R077 定案「病灶窄判」非全局回退。
- 第五轮（本目录现件，R078/M2-R1 修复轮）：**evkey 件 = WS2 判别件同字节**已真机
  **正常 ✓**；三车道回退 R076 直加，三源 SALT 强派生 + CLK 守卫保留。
  发布引擎为 R077 wasm + S2 数据段手术件（rustc 通道恢复期产物；源码 R078 已
  同义落盘，rustc 恢复后重编刷锚）。NL 像重做登记 R1 余账（V19 §12）。
- 第四轮（原记录留档）：evkey 件真机报错（assign to call nil @ inner chunk line 3）；diag-evk 首次动用，判据全绿（EVK=4576808 与构建期假设逐位吻合）。
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

## diff-flight/（R077 故障判别飞行件，已撤档）

判别飞行一轮已收口（WS1 挂 / WS2 正常 / WS3 正常 ⇒ 三车道 lane NL 像定罪到区，
V19 §12），一轮三件档案存于 git 历史 commit `58c3895`。手术工具
`web/tools/wasm-data-surgery.py` 随树保留（含 f1/f2 模式）。

**二轮（P1 车道解剖：定罪区→定到步）现件**——均作用于 R077 病件（wasm
`0f8dc517`）数据段等长手术，三车道 piece2 同构处理：

| 件 | 手术 | sha256 前16 | 语义 |
| --- | --- | --- | --- |
| `diff-flight/kkksu-mini.F1evk.hard.lua` (178,764B) | F1：只保一级折叠（16384 域），杀二级 | `253ed8ed0cb278ec` | fold₁ 独存 |
| `diff-flight/kkksu-mini.F2evk.hard.lua` (178,729B) | F2：只保二级折叠（65536 域），杀一级 | `5bf8f5d0c39c2d23` | fold₂ 独存 |

判读：F1 正常 F2 挂 ⇒ 病在二级（65536 域）；反之病在一级（16384 域）；双挂 ⇒
共谋（再分车道切）；双正常 ⇒ 病在组合相位。回传格式同前：`1.<报错/正常>`
`2.<报错/正常>`。验证链同 §11.4（双构建恒等、no-bind fib、TSRC 静默、
bind 裸 luau 干净拒绝）。

