# kkksu 手测包（R067.5，2026-10-24）

`Kkksu.lua`（15 B，`print("kkksu")`）的发布流水线同款产物，
用于在真 Delta 上**手测通过后再去控制台发布**。

与发布流水线完全同构：同 preset（v15）、同选项（minify/mangle/strings/flatten/
junk/numbers/body/vm-seal-strings 全开）、同种子（**safe 与 hard 共用 20261024**，
与流水线「两份产物同一种子」一致）；唯一差别：本地手测版**不含**许可守卫注入
（守卫由服务端在发布时注入，绑定你的站点 origin 与 scriptId，本地无法也不应复刻）。

| 文件 | 大小 | SHA-256（前 12） | 说明 |
| --- | --- | --- | --- |
| `Kkksu.lua` | 15 B | `52df8622c1dc` | 原始源码 |
| `Kkksu.safe.lua` | 191,800 B | `38838223998b` | 兼容版 = `--env-check --no-bind-executor` |
| `Kkksu.hard.lua` | 214,663 B | `3ede3ab1c992` | **加固版 = `--bind-executor --bind-env roblox`（完整保护，一层不少）** |

## 沙箱验证矩阵（官方 Luau release 实测，2026-10-24）

| 场景 | safe | hard |
| --- | --- | --- |
| 裸 Luau | 打印 `kkksu` ✓ | fail-closed 噪音（tier-2 探测先行，R067 起既有，仅影响裸宿主报错美观度） |
| 高保真 Roblox/执行器 mock（prelude） | 打印 `kkksu` ✓ | 精确拒绝：`debug.getconstants,debug.getupvalues,debug.getproto,debug.setupvalue` ✓（与 GateProbe 预测逐字一致） |
| 真 Delta | 应打印 `kkksu` | **应打印 `kkksu`**（同引擎同参数的 `Suda.hard-trace.lua` 已于同日在你的 Delta 上全闸门通过） |

## 真机手测步骤

1. 把 `Kkksu.safe.lua` 粘进 Delta 运行 → 应打印 `kkksu`
2. 把 `Kkksu.hard.lua` 粘进 Delta 运行 → 应打印 `kkksu`（**这次的关键确认**）
3. 两个都过后，去控制台发布（见下）

## 发布（上传）步骤

前置：服务器已升到 R067.5 包（`web/releases/Obsucvim-Web-Windows-x64.zip`，
zip SHA-256 `995c3fe74b8e1c1df107a6db7cbe2518a28de3bf80eb4d186358673a9a5013b4`；
若还没升，用既有的 `Update-Obsucvim.ps1` 流程升）。

1. 浏览器打开你的控制台并登录
2. 新建脚本（名字随意，例如 `kkksu`）或复用既有脚本（loader URL 不变）
3. 源码粘贴 `print("kkksu")`，发布
4. 等后台 job 完成：服务端自动产 **safe + hard 双产物**（同种子），
   hard 失败不拖垮发布（只少一层加固），safe 失败才会令发布失败
5. 发布后 `GET /files/<scriptId>.lua` 即 loader；loader 先跑 attest 证明层，
   按宿主能力选发 hard 或 safe——宿主谎报只会让自己拿到跑不动的 hard

本地产物与线上产物字节不会相同（线上多了许可守卫注入且种子由发布参数决定），
这属预期；要对比请比**保护层配置**而不是字节。
