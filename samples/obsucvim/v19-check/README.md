# v19-check：V19 真机验收（第六轮 = R079/M2-P1 定案修复：evkey 已验 ✓，ctl 待真机）

两件同种子（20261025）、同输入（`src-kkksu-mini.lua`，斐波那契打印）：

| 件 | 构建 | 裸 luau（已实测） | 真 Delta（实跑结果） |
|---|---|---|---|
| `kkksu-mini.v19evkey.hard.lua` (178,792B, sha `3904533e…`) | `--preset v15 --env-check --bind-executor`（R079 形态：盐三源强派生 + CLK 守卫保留；**P1 车道解剖定案**——bx `_sv` 车道（字节流 st 注入）双折叠独罪、落定恒等线性注入（LC 常量保留）；ct `_svk` / km `_svq` 车道折叠真机无罪、原式复辟） | **干净拒绝**（缺 API 文案，不挂死） | **已真机 ✓**（与 diff-flight 三轮 L1 判别件**逐字节恒等**，2026-10-25 用户回传「1 正常」） |
| `kkksu-mini.v19ctl.hard.lua` (176,927B, sha `6530e016…`) | 同上但旋钮关（EVKEY=0；R079 纹理、旧明文 oracle） | 精确缺 API 拒绝（既有行为） | **待真机**（R079 首件 ctl） |
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
- 第五轮（R078/M2-R1 修复轮）：evkey 件 = WS2 判别件同字节真机 **正常 ✓**；
  三车道回退 R076 直加，三源 SALT 强派生 + CLK 守卫保留；ctl 同轮 **正常 ✓**，
  V19 M2 首包（R078 态）双件全收。
- **第六轮（本目录现件，R079/M2-P1 定案修复，2026-10-25）**：P1 车道解剖
  三轮判别飞行裁决收口 —— F1（一级折叠独存）复现原型 call-nil、F2（二级折叠
  独存）index-nil-'VY'（两级折叠各罪、毒化不同下游）；L1（杀 bx 车道折叠）
  **正常 ✓**、L2（杀 ct 保 bx/km）挂 `argument #1 to 'char' (number expected,
  got nil)`、L3（杀 km 保 bx/ct）挂 call-nil ⇒ **bx `_sv` 字节流车道双折叠
  为唯一病根，ct `_svk` / km `_svq` 折叠真机无罪**。R079 = bx 车道恒等落定、
  ct/km 折叠原式复辟（R077 熵卖点拿回两条车道）。evkey 件 = L1 判别件同字节
  已真机 ✓；ctl 件（`6530e016…`）待真机。发布 wasm = R077 + l1 数据段手术
  （`95f69c99…`，rustc 恢复后重编刷锚）。
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

判读（原案）：F1 正常 F2 挂 ⇒ 病在二级（65536 域）；反之病在一级（16384 域）；
双挂 ⇒ 共谋（再分车道切）；双正常 ⇒ 病在组合相位。验证链同 §11.4
（双构建恒等、no-bind fib、TSRC 静默、bind 裸 luau 干净拒绝）。

**二轮真机回传（2026-10-25）**：F1 **挂**（`attempt to call a nil value`，
与 R077 原型同症状）/ F2 **挂**（`attempt to index nil with 'VY'`，新症状）⇒
**双折叠各级独罪且病状分裂**：一级折叠独存即复现原始 call-nil、二级折叠独存
改在下游表索引处暴雷 —— 两级各毒化不同消费方。按原案进三轮：分车道切。

**三轮（P1 车道解剖：分车道切）现件**——基件同为 R077 病件 wasm
（`0f8dc517`），每件只把一条车道 piece2 的双折叠整段置恒等空格垫，
其余两条车道折叠原样保留：

| 件 | 手术 | sha256 前16 | 语义 |
| --- | --- | --- | --- |
| `diff-flight/kkksu-mini.L1evk.hard.lua` (178,792B) | L1：杀 bx 车道（`_sv`）双折叠 | `3904533ecb3699bd` | bx 恒等，ct/km 仍折叠 |
| `diff-flight/kkksu-mini.L2evk.hard.lua` (178,792B) | L2：杀 ct 车道（`_svk`）双折叠 | `c855bc2848e30b3b` | ct 恒等，bx/km 仍折叠 |
| `diff-flight/kkksu-mini.L3evk.hard.lua` (178,883B) | L3：杀 km 车道（`_svq`）双折叠 | `21d7c1757e8e62a4` | km 恒等，bx/ct 仍折叠 |

判读表：恰一条正常 ⇒ 该车道折叠为唯一病根（修缮 = 该车道恒等、余两车道
保折叠，出 R079）；多条正常 ⇒ 交互共罪（任一被切即断毒化链），修缮取最保守
形态；三件全挂 ⇒ 无单车道可赦，下轮补「只保一车道」组合件或直接定三车道
全罪、NL 像整体换结构重做。回传格式同前：`1.<报错/正常>` `2.<…>` `3.<…>`
（1=L1、2=L2、3=L3）。验证链同 §11.4 四项 + 双构建恒等（本轮全绿：复建门
`c61cdc5c`/`1af735bc` 命中、no-bind fib 全打印、TSRC 静默、bind 裸 luau
干净拒绝、同件双构建逐字节恒等）。

**三轮真机回传（2026-10-25）：L1 正常 ✓ / L2 挂（`argument #1 to 'char'
(number expected, got nil)`）/ L3 挂（nil value）⇒ 恰一条正常分支成立：
bx `_sv` 车道折叠唯一病根**（L1 保 ct/km 折叠仍正常 ⇒ 两车道折叠无罪复辟；
L2/L3 唯一共同保留 bx 折叠且各在 char/index 下游暴雷 ⇒ bx 定罪）。按判读表
出 R079 定案修复并晋升发布件（本目录两件），P1 收口见 V19 §13。

