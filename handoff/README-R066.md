# R066 — 英文输出水印（2026-10-08）

- 源码：`R066_src.lua`，`print("Obsucvim watermark test")`。
- 产物：`R066_target.luau.lua`，默认 v15 / Luau，seed `66066`。
- 首行固定为 `-- This file was obfuscated by Obsucvim.`。
- 在压缩/所有 pass 之后统一添加；保持原有正文字节，包括 v15 原有起始换行。不是防删除/防篡改水印，品牌名有意公开；源码行号因此增加一行。
- Luau 源码/产物 stdout 均为 `Obsucvim watermark test`，退出码 0；产物 `luau-compile` 通过；S1–S5 5/5。
- 源码：33 bytes，SHA-256 `7b1d2b92749902ab74fe656590453f6f3aa5734f2acc444b3fa09a8301cab7c6`。
- 产物：188916 bytes，SHA-256 `e152800aff633c54a45d54222b66251c65f239a619fbfd46410ec87f539d37db`。
- 历史 R065 及其哈希未改；相同 seed 新输出减去头部后与原 R065 产物逐字节一致。37 个现役 examples 同样验证正文不变。

生成：

```bash
obsucvim/target/release/obsucvim --seed 66066 handoff/R066_src.lua handoff/R066_target.luau.lua
```

本轮本地测试：cargo 39 单测 + 4 CLI 集成（1 ignored）；主矩阵 169/169；预设 370/370；multiseed FAIL=0；Discord 服务 55/55（含真实引擎、网络模拟）。未部署至 Render/Discord，历史已有工作树改动未自动合并提交推送。
