# R066 — 当前产品形态 canary：环境检查组 + 水印（2026-10-11）

## 源码

```lua
print("suda")
```

源文件：`R066_src.lua`（14 字节，SHA-256 `514693ad0aafa711936bc69f447af9c1d943f2264b30c2912d543859e3e4eca2`）。

## 产物

- 文件：`R066_target.luau.lua`
- 命令：`obsucvim --env-check --seed 66066 artifacts/targets/R066_src.lua artifacts/targets/R066_target.luau.lua`
- 大小：190974 字节，SHA-256 `e2cb9423d1e02ba702bdc3489540b48a95b71b9b27f229f6e07998aab7c65e13`
- 首行水印：`-- This file was obfuscated by Obsucvim.`
- `luau-compile` 通过；stdout 与源码逐字一致（`suda`）；同 seed 重生成逐字节确定。

## 本样本展示的产品形态

1. **英文水印**：首行品牌注释（所有输出路径统一）。
2. **WTF-PORT 环境检查组（tier-1，`--env-check`）**：buffer 全族（含小端序/越界错误状态）、bit32/string/元表/协程/freeze/loadstring 语义常量挑战 + 17 项首尾身份全等；差值经 SLT/MZD 双 lane **静默投毒**（无 error oracle）。检查组顺序/常数每构建随机。
3. **mock 环境绞杀**（本产物实测）：干净克隆环境照常输出；bit32 错值 mock 静默死亡（无 `suda` 输出、无错误特征）。

## 生成与验证

```bash
engine/target/release/obsucvim --env-check --seed 66066 \
  artifacts/targets/R066_src.lua artifacts/targets/R066_target.luau.lua
.tools/bin/luau-compile artifacts/targets/R066_target.luau.lua   # 语法过
.tools/bin/luau artifacts/targets/R066_target.luau.lua           # 输出同源码
```

## 历史

- 2026-10-08：初版（水印 canary，无 env-check）。
- 2026-10-11（H3-6 修复后）：同 seed 重生成（噪声布局变化）。
- 2026-10-11：加 `--env-check` 重生成为当前产品形态 canary（源 `print("Obsucvim watermark test")`）。
- 2026-10-11（当前）：源码改为 `print("suda")` 重新混淆；产物中无明文 `suda`。

## 诚实边界

- tier-1 全为**常量挑战**：错误语义 mock 环境静默死亡；真环境录制重放照常通过（L1 空缺，见 WTF-PORT-PLAN §5）。
- `--env-check --bind-executor` 双开可叠加 tier-2（Enum/Random/task/Instance 面），但绑定态产物在本 CLI 无法运行验证（设计如此）。
- 本样本不覆盖反 trace/白盒插桩（R004 物理边界）。
