# 强环境绑定实现（AI 详版）

> 面向 AI 代理/自动化审计：本文件为 `luraph --bind-env` / `--bind-executor` 的**可执行规格**，含威胁模型、设计不变量、数学形态、代码锚点、构建/运行时双侧镜像、验证矩阵与排障清单。改动前必读 `docs/vm-l6-implementation.md` §8（9 类坑）与 `HANDOFF.md` §8（规矩与坑）。

## 0. 术语与目标

- **强环境绑定**：产物**仅在目标运行时**（Roblox Luau + 完整 Executor）内正确解密并执行；在通用 `luau` CLI、半吊子桩、`lua5.1`、Studio 裸环境中**静默错钥**（`--bind-env`）或**毒杀**（`--bind-executor`），无明文泄露、无 `while true do end` 等可 `grep` 的 oracle。
- **两档**：
  - `--bind-env roblox`（`㉑`）：目标运行时 API 语义（`Vector3/Vector2/task`）折入钥匙流，通用沙箱**错钥**（非崩溃，难定位）
  - `--bind-executor`（`51` IronBrew2 全量移植）：`identifyexecutor`/`getgenv`/`getconstants`/`getupvalues`/`setupvalue`/`newcclosure` 等 6 折 transcript + 19 原语 `C` 闭包分类 + `transcript` 序列，**毒令牌**错钥（`XTOK`/`XPROBE`），非目标环境**静默死亡**

## 1. 威胁模型与边界

| 攻击向量 | 是否防御 | 原理 | 诚实边界 |
|---|---|---|---|
| 哑桩（`type`/`tostring` 假实现） | ✅ | `CENVPROBE` 每帧闭包重探 `Vector3/Vector2` 分量 + `tostring` 折叠，失真→错钥 | 全保真桩（`R004` 定律）仍能过：完整仿真 Executor 语义者可过 |
| 半吊子桩（`string.char` 钩子） | ✅ | 三处物化 `("")[\"char\"]` 经字符串元表 + `..` 拼接，`string.char` 钩子抓不到载荷（`R021` 已实证封堵） | 最终使用点（`print`）仍可观测（`R004` 同族） |
| 文本手术探针（`R008/Suda` 三处可见手术点） | ✅ | `DDEC`/`CDEC`/`newE` 整体入密 `RT` 碎片（`wire 208`），可见层无解码器函数值 | 内存快照仍可抓已触碰常量（`R004`） |
| 静态钥匙库提取 | ✅ | 钥匙面独立化（块面 `xm1/xm2` vs 常量面 `xm3/xm4`，`53`）+ 每常量独立钥匙流 `stj=(base+j·CKJ)`（`㊼`）+ 块 `SALT` 会话盐（`B-3`） | 文件内钥匙可被构建间差分（`52` 断崖教训：镜像整数精确） |
| 构建期重放 | ✅ | `SALT = fold31(TOSTR({}))` 会话盐 + `AV` 每运行折叠 + `CKJ` 每构建钥匙，静态文件无推导材料 | 收集品一次性，跨运行钥匙全变 |

## 2. 设计不变量（改动必守）

1. **镜像整数精确**：编解码两侧 `PS/PT/KM/KC/PB/CKJ/SLT/SLTC` 全程 `% 268435456`（`2^28`），`%` 为 Lua 非负语义，`v+stj` 等加性掩码在 `2^53` 内精确
2. **零新探针**（`P2`）：`53` 钥匙面独立化**不新增探针**，复用 `51` 的 `XPROBE`/`CENVPROBE` 调用点，`P2` 镜像失配风险归零
3. **失败安全**：探针内 `pcall` 包裹，失真→`cef=99999999`/`xtok2=毒令牌`→错钥，不抛错
4. **开关零影响**：`--bind-env`/`--bind-executor` 关闭时与现役管线**逐字节一致**（`R023` 靶标 `cmp` 实证）
5. **本地可验**：`cargo test` 27 + `169/169` + `370/370` + `multiseed` + `faithful_stub` 4/4 + `charhook` 4/4 全绿

## 3. 数学形态

### 3.1 `--bind-env roblox`（`㉑` + `㊿`）

- **引导期**：`b[env_slots]` 捕获 `Vector3.new`/`Vector2.new`/`task.defer`/`typeof` 四槽（`env_slots: [i64;4]`，高位数段，`HANDOFF` §7 `env_slots`），`env_gate` 在 `bootstrap do` 块计算 `fold31(Vector3(cea)+Vector3(cea+1) + Vector2(cea3,cea4) + tostring(Vector3)折叠)`，`ea/ec` 每构建随机，`fold31` 为 `h= (h*31 + BYTE(t,pi))%2^28`
- **载荷期**：`CENVPROBE` 闭包在 `bootstrap do` 块创建，捕获 `b[env_slots]`/`TSTR`/`BYTE`/`PCAL`，经 `RTFRAG` 的 `CENVPROBE` 形参传入 `cdec_inner`，**每帧** `CDEC` 调用 `cef=CENVPROBE()` 揉进 `base = (PS + q.sd*PT + SLTC + cef + (xtok2*xm3+xm4))%2^28`（`51` 叠加）
- **编码侧**：`BSS` 按期望折叠值 `cenv_exp` 预补偿 `base`，`obf_num(cenv_exp)` 经 `manifest` 烘焙，`SLTC` 与 `SLT` 分离（`H1*CC1+H2*CC2+CCC` vs `H1*CB1+H2*CB2+CBC`）

### 3.2 `--bind-executor`（`51` IronBrew2）

- **EXG 碎片 `wire 209`**：`exguard::build(rng)` 生成 `XGUARD` 闭包（`HW[209]` 工厂），`probe(vmword)` 在 `HBOOT` 经 `(LS(s) or PD)()` 调用 `probe(0,true)` 得 `{XTOK, XPROBE}`，`XTOK` 存 `HW2[256+rand]` 高位槽
- **启动投毒**：`RT` 碎片开头 `XTFB = (XTOK*xm1+xm2)%2^28; SLT = (SLT+XTFB)%2^28`（`xm1/xm2` 每构建随机，`obf_num` 混淆）
- **每帧投毒**：`cdec_inner` 每帧 `xtok2 = XPROBE(q.sd)` 揉进 `base = (PS+q.sd*PT+SLTC+cef+(xtok2*xm3+xm4))%2^28`（`53` 独立面：`xm3/xm4` 与块面 `xm1/xm2` 分离）
- **转储**：`XTOK` 健康→与 `BSS` 预补偿 `XTE=(ATOKEN*xm1+xm2)%2^28` 对齐；`XTOK` 毒化→错钥静默死亡（`PD()` 替代 `while true do end`，`D18` 审计）

## 4. 代码锚点

| 层 | 文件 | 锚点 |
|---|---|---|
| `CLI` | `luraph-rs/src/main.rs:774/779` | `--bind-env`→`env_slots`/`--bind-executor`→`executor_bind`，`VmProgram::new` 传参 |
| `BSS` | `luraph-rs/src/vmgen/template.rs:2280` | `cenv_probe`/`cenv_exp` 生成，`exguard::build` 调用，`XTE`/`XTEC` 烘焙 |
| `RT` | `luraph-rs/src/vmgen/template.rs:2443` | `cdec_inner` 按 `cenv_active`/`exg_active` 四分支拼 `probe_call` 与 `base_env_s` |
| `EXG` | `luraph-rs/src/vmgen/exguard.rs:1` | `build(rng)` 产 `XGUARD` 源码，`KEYS` 掩码 `char` 自举 `f(65)=="A"` 行为发现 |
| `HBOOT` | `luraph-rs/src/vmgen/template.rs:2664` | `HW[209]` 工厂调用 `probe(0,true)`，`HW2` 高位槽持久化 |
| `净化` | `luraph-rs/src/vmgen/template.rs:2544` | `frag_reserved` 保留 `__index` 等元方法名，`sanitize_frag` 跨碎片一致改名 |

## 5. 构建/运行时双侧镜像（逐字对齐）

- **块钥匙**：`st = (PS + i*PT + s0*PB + SALT + XTE)%2^28`（`BSS` 编码） vs `st = (PS + pf.sd*PT + s0*PB + SLT + XTFB)%2^28`（`RT` `bdec` 解码，`XTFB` 为启动投毒）
- **常量钥匙**：`stj = (PS + i*PT + SALT + j*CKJ + cenv_exp + XTEC)%2^28`（`BSS`） vs `base = (PS+q.sd*PT+SLTC+cef+XTEC')%2^28; stj=(base+j*CKJ)%2^28`（`RT` `CDEC`，`XTEC'` 为 `xtok2*xm3+xm4`）
- **探针**：`CENVPROBE` 闭包 `b[es0](cea) + b[es0](cea+1) + b[es3] tostring 折叠 + b[es1](cea3,cea4)`（`cea/cec` 每构建随机，`obf_num` 混淆，`pcall` 包裹）

## 6. 验证矩阵（强制工作流）

```bash
export PATH=/home/user/luraph/.tools/bin:$PATH
cargo test                          # 27
bash tests/run_tests.sh             # 169/169（含 370 交叉，含 guard/executor 6/6）
bash tests/run_presets.sh           # 370/370
bash tests/multiseed.sh             # 5×30 全绿
bash tests/faithful_stub_test.sh    # 5/5（4 忠实桩 + 1 哑桩毒杀）
bash tests/charhook_test.sh         # 4/4（短/长 × MkStr/常量池）
bash tests/v15_fingerprint.py       # 32/32
bash tests/security_fingerprint.py  # 5/5
```

- **开关零回归**：`--bind-env`/`--bind-executor` 关闭时与 `R023` 靶标 `cmp` 逐字节一致
- **负面**：裸 `luau`/`lua5.1` 必挂（`attempt to index nil with 'new'` 等），半吊子桩 `string.char` 钩子截不到载荷（`charhook` 实证）

## 7. 排障清单（真执行器 7 轮血泪）

1. `char` 自举先有鸡：`VK` 解码需 `char`，`char` 名又要经 `VK` → 行为发现 `f(65)=="A"` 零名字锚点
2. `HW[209]` 与碎片同形态存工厂本体，`boot` 需调用一次才得 `{XTOK,XPROBE}`
3. `GFE(0)` 绕过影子环境恒返真 `_G` → `CENVPROBE` 必须用 `bootstrap do` 块闭包捕获 `b[env_slots]`
4. `string.char` 点字段 `.char` 会被净化器改名 → 改 `["char"]` 字符串下标
5. `rawget` 对代理环境恒 `nil` → 探针改索引感知 `b[es0](...)` 而非 `rawget`
6. `game` 服务为 `userdata` → `rawget` 限 `table`，放行 `userdata`
7. `DIAG` 靶字母编号 + 闸门详情串 + 时间戳构建标记防新旧混淆

## 8. 变更最小集（回退即删）

- `main.rs`：`env_slots`/`executor_bind` 透传
- `template.rs`：`cenv_probe`/`exguard::build`/`XTE`/`XTEC`/`MASK16`（`ctA/B` 拆分时）/`hw_expand`
- `compiler.rs`：`s_indices` 前置（`if scatter`）
- `exguard.rs`：`XGUARD` 源码
- 其余管线（`isa` 四表、`handlers` 43 形态）**不为绑定而改**，回退即删上述透传即回 `0d53ace` 基线
