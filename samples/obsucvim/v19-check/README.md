# v19-check：V19 EVKEY 原型真机验收（2026-10-25）

两件同种子（20261025）、同输入（`src-kkksu-mini.lua`，斐波那契打印）：

| 件 | 构建 | 裸 luau（预期） | 真 Delta（待回传） |
|---|---|---|---|
| `kkksu-mini.v19evkey.hard.lua` (171,582B, sha `55f0ea78…`) | `--preset v15 --env-check --bind-executor` + `OBSUCVIM_V19_EVKEY=1` | 不执行（失步挂起，无输出） | **=关键验证点**：正常打印 `kkksu-mini:1,1,...55` ⇒ EVK 探针公式与真机一致；拒绝/挂起 ⇒ 公式需修 |
| `kkksu-mini.v19ctl.hard.lua` (169,171B, sha `80044d24…`) | 同上但旋钮关（=R073 形态） | 精确缺 API 拒绝（既有行为） | 正常打印（对照组） |

真机回传口径：各回一句「正常打印 / 拒绝 / 挂起 / 其他（报错原文）」即可。
两件都正常 ⇒ R4 通道成立，EVKEY 可进入 M1/M2 排期讨论；
evkey 件异常 ⇒ 探针与 exguard 测量有异，寄回 trace 件排查。

纪律：Dev 档（`OBSUCVIM_V19_EVKEY_DEV`）产物零安全性，严禁交付。
