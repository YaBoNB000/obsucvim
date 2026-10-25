# DEMO — 产品形态示例脚本（2026-10-19）

## 源码

`DEMO_src.lua`（2,177 字节）：模拟游戏模块——配置表、许可证校验（哈希+分段）、OOP 玩家类（元表/方法/KDR）、匹配循环、斐波那契基准。覆盖：strict 类型标注、字符串格式化、gmatch 模式匹配、setmetatable、复合赋值 `+=`、ipairs、多返回值、浮点格式 `%.2f`。

原始输出：
```
game=pvp v2.1.0 total=2600 kdr=1.00 fib30=832040
license: invalid
```

## 产物

- 文件：`DEMO_target.luau.lua`
- 命令：`obsucvim --preset v15 --minify --vm-seal-strings --env-check --bind-executor --bind-env roblox --seed 68068 artifacts/targets/DEMO_src.lua artifacts/targets/DEMO_target.luau.lua`
- 大小：290,313 字节（膨胀 ×133）
- 首行水印：`-- This file was obfuscated by Obsucvim.`
- `luau-compile` 语法通过；同 seed 重生成逐字节确定。

## 保护层

| 层 | 状态 |
|---|---|
| v15 VM（CPS + 模块表 + 233 超算子） | ✅ |
| VM 字符串封存 | ✅ |
| 环境检查组 tier-1+tier-2 | ✅ |
| executor 认证绑定（EXG 17 闸门） | ✅ |
| 环境绑定（Roblox API 槽） | ✅ |
| 防篡改 (L7) | ✅ 默认开 |
| 反调试环境守卫 | ✅ 默认开 |
| 标识符/字符串/控制流/垃圾代码/数字/全程序加密 | ✅ 默认开 |

## 明文泄露检查（产物中零命中）

`GameConfig` / `License` / `SIGNATURE` / `obsucvim_demo` / `player_` / `runMatch` / `getKDR` — 全部 0 命中。

## 注意

产物带 executor 认证绑定 + Roblox 环境绑定：**只能在完整 Roblox executor 环境内正确运行**。裸 Luau CLI / 通用沙箱会静默死亡（预期行为）。验证语义等价需在 Roblox executor 中运行。
