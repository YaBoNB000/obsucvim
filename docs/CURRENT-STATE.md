# 现状速查（CURRENT-STATE）

> **这是唯一权威状态文档。** 其他文档只描述设计或历史，与本文冲突时以本文为准。
> 最后更新：2026-10-25（仓库整理同轮，内容依据 `engine/ENGINE-VERSION.md` §33–§36 与本地实测）。

---

## 1. 一句话

商业级 **Luau 混淆器**：Rust（std-only）引擎把源码编译成**私有字节码**，由随产物分发的
「Lua 写的加密解释器」执行；外围是 React 控制台 + Node 托管服务（卡密 / 双产物发布 / 验卡）。
产品线 **Luau-only**（Lua 5.1 目标自 2026-09-10 起停止支持）。

---

## 2. 引擎版本（发布态）

| 项 | 值 |
| --- | --- |
| 版本 | **R079**（V19 M2 P1 车道解剖定案，2026-10-25） |
| 引擎 wasm | `web/tools/obsucvim.wasm`，1,249,048 B，sha256 `95f69c99f0e0db89a1db537b0a7c7a251b9cd3de3b4b2906e98942b87319f40a` |
| 锚点命令 | `print("suda") --preset v15 --env-check --bind-executor --seed 67067` |
| 锚点结果 | **178,551 B** / `ccf5d0839fef962a8a0c031d745d869dff7a861b6590fe8e267d93f5cf63c045` |
| 自检 | `node web/tools/verify-engine-version.mjs` → `verdict: R079` |

**必须知道的一个事实**：`rustc` 通道在立项环境里不通（`static.rust-lang.org` 不可达、
无 apt 权限、GitHub Actions 无 `workflows` 权限），所以线上 wasm 是
「R077 二进制 + l1 数据段等长手术」的产物；**源码 R079 已同义落盘**
（`engine/src/vmgen/template.rs`），rustc 恢复后重编刷锚即可。这不影响产物字节，
但意味着「改源码 → 重编 wasm」这条链路当前在本机不可执行。

`--no-key-host` 自 R070 起被移除（宿主钥是底层、恒开）；选项表里仍有 `--key-host`
仅为兼容，传与不传等价。

---

## 3. 已生效的停止令（2026-10-25，用户指令）

- **§34 收摊**：停止引擎改进，剩余全部计划取消（P2 时钟接入 / P3 镜像悬案 /
  P4 懒解层 / P5 相窗三）；退役项（含 rustc 正牌重编刷锚）一并封存。
  封存前状态完整可复起（源码、手术工具、验证链都在树里）。
- **§35 PW-1「径织」**：用户定性「外层壳是纸老虎」，**已干净删除**
  （`pathweave.py`、`pathweave-selftest.py`、`PW1-PATHWEAVE.md`、`pw1-check/` 四路全删）。
  恢复点：git `710005f` / `c2b91ea` / `026469d`。**不要主动复活。**
- **§36 MW-1「雾织」**：保留并**真机收口**（详见 §5）。

> 结论：除 MW-1 输入层与 V19 已批的部分外，**不要再自行开引擎新战场**。

---

## 4. 现役能力（产品形态 = 默认 `--preset v15`）

- 零参数可运行：丢进 Luau CLI / Roblox 执行器即可跑，输出与源码逐字节一致。
- 防护构成：结构层（加密解释器 + 去中心化尾链/蹦床 + 每构建全量随机：载体、转义、
  钥匙表、指令编号、槽位、命名）+ 环境绑定 `--bind-env roblox` + 计时守卫 +
  executor 认证绑定（`--bind-executor`，v15 **默认开启**，`--no-bind-executor` 退出）
  + 环境检查 `--env-check`（tier-1 纯 Luau 语义 / tier-2 Roblox+executor 档）+ 宿主钥恒开。
- 每份产物首行固定水印：`-- This file was obfuscated by Obsucvim.`（普通注释，非防篡改）。
- 确定性：同一 `--seed` 输出逐字节一致（本地实测 sha256 相同）；不同 seed 全量重随机。
- 预设：`v15`（默认，Luau/Roblox）、legacy `low`/`medium`/`high`/`vm`/`max`。
  典型体积：`v15` ~150–180 KB 起，legacy `vm` ~130 KB 起。

**改引擎/跑矩阵时一律加 `--no-bind-executor`**：语义回归测的是管线，绑定态由
executor 守卫与 tier2 专测覆盖。

---

## 5. V19「存在性硬化」（2026-10-24 立项，进行中的主线）

- **触发**：第三方对 V18 融合件 `kkksu-mid.v18fusion.hard.lua`（182,491 B）做了
  **零执行、零穷举、零 mock 的纯静态**拆解，拿到业务稿 / 78 条私有字节码明文 / 常量与字符串池。
- **根因（架构级）**：产物内同时携带写入公式与读取公式 ⇒ 掩码代数相消；
  所有钥料都是「混淆表达式包装的产物常量」⇒ 常折即出；所有校验静态可算 ⇒
  攻击者自带过筛 oracle；运行时量从未进入不可逆数据路径。
- **修复纲领 R1–R6**（`engine/V19-EXISTENCE-HARDENING.md` §3）：R1 运行时秘密进数据、
  R2 执行门控解封、R3 oracle 熄灭、R4 host delta 入钥流、R5 字节域熵坍缩、R6 指纹熔断。
- **进度**：
  - R4/R1 融合通道 **EVKEY 原型已实装并真机验收通过**（旋钮 `OBSUCVIM_V19_EVKEY=Bind`，
    **默认关**；`Dev(n)` 档零安全性、禁止交付；EV 剖面 4576808；v1 因探针取值通道走
    `getfenv(1)` 在真 Delta 挂死，v2 改 LKP 同源 `getgenv()` 优先两级查找后正常）。
  - R2 可行性已有定论：blob 层做不了，可行落点在 stage-2 `bdec` 懒解层（属 M3 轮，未开工）。
  - M1（R6+R3+R5）**未开工，待拍板**；在 §34 收摊令下默认不动。
- **诚实边界**：EVKEY=Bind 的失败形态是「静默失步挂起」；EV 观测面可被同宿主旁录，
  R4 只做叠加柱不独撑；纯离线单文件下「忠实模拟 + 重放环境」原理上永远可达。

---

## 6. MW-1「雾织」（第二条自研线，已收口）

- 定位：引擎**输入侧**纯 Lua→Lua 变换器，串接形态 = **源码 → MW-1 → 引擎 R079 VM**。
  引擎本体不动。
- 三模块：**雾锁 Foglock**（整数恒等不透明门族 + 似真假支）、**雾织 Weave**（chunk 顶与
  函数体顶插入，只增不改、唯一新名）、**雾数 Fognum**（整数字面量乘法掩码恒等链）。
- 工具：`web/tools/mistweave.py`（`--seed` / `--num-fog` / `--blocks`）、门禁
  `web/tools/mistweave-selftest.py`（**11/11 绿**，纯净 luau 口径）。
- 真机：验收包 `artifacts/samples/obsucvim/mw1-check/`（evkey 206,254 B / ctl 203,213 B）
  **双件真机全收**（2026-10-25 用户回传「都正常」）。
- 诚实边界：永假门族本身静态可证（SMT 可剥），强度在装配——进 VM 后成为方言随机
  字节码块，剥离前须先完成虚拟化逆向。默认密度下 bind 件 203–206 KB，略超旧预算，
  可用 `--num-fog 0.3 --blocks 3` 压回。

---

## 7. 托管系统（web/）

- 服务端：`web/tools/engine-server.mjs`（单文件，3,954 行）。账户 / 卡密 / 脚本 / 发布 /
  验卡 / webhook / 外部应用 / 审计 / 指标等 60+ 路由；引擎优先原生二进制，缺失时回落
  WASI（`web/tools/obsucvim.wasm` + `wasi-runner.mjs`）。
- 发布双产物：`safe`（`--env-check --no-bind-executor`）与 `hard`（完整加固）；
  明文 loader 先跑 `web/tools/attest.luau`（25 项执行器/Roblox 证明，永不报错），
  按能力请求对应件。发布作业异步（`POST /api/publish` → `GET /api/publish/<jobId>`），
  失败退还额度、崩溃重试 3 次。
- 存储仍是单文档 JSON（已原子写）+ 本地磁盘产物 —— 多实例 / 百万级卡密需换数据库。
- 套件计数（文档记录值，本轮未在本沙箱重跑）：hosting 78/78、engine-retry 9/9、
  loader 24/24、attest 18/18、live-loader 13/13、artifact-runtime 6 seeds × 2 hosts。

---

## 8. 验证矩阵怎么跑

```bash
# 引擎（需要 .tools/bin 下的 lua51 / luau / luau-compile，或 /home/user/tools/bin）
cd engine && cargo build --release
bash tests/run_tests.sh        # 官方矩阵：全语料 × 方言 ×（high + vm + v15）
bash tests/run_presets.sh      # 预设 × 全语料
bash tests/multiseed.sh        # VM/编码改动必跑
bash tests/defense_audit.sh    # 防御审计（D1–D20）

# 托管 / 引擎锚点
node web/tools/verify-engine-version.mjs
node web/tools/test-hosting.mjs
```

**本沙箱现状（2026-10-25 实测）**：无 `cargo/rustc`，无 `.tools/bin`（lua51/luau 全缺），
未 `npm install`。因此引擎矩阵与控制台套件**跑不了**；能跑的是 WASI 引擎与锚点校验：

```bash
node web/tools/verify-engine-version.mjs            # → R079（已验证）
node web/tools/wasi-runner.mjs '{"wasmPath":"<abs>/web/tools/obsucvim.wasm",
  "workDir":"/tmp/work","args":["--seed","42","/work/in.lua","/work/out.lua"]}'
```

---

## 9. 已知边界（诚实清单，别对外夸大）

1. **真机正向验证有限**：本沙箱只有自编 Luau + 宿主 mock；Roblox / Delta 的结果依赖用户
   回传（R067、MW-1、V19 三批已回传通过）。
2. **V19 失守事实已发生**：静态路径可拆（R1–R6 未全部落地前仍可能复现），种子/形态随机化
   只收「每样本人工费」。
3. **环境记录器 / 探针感知桩**可绕过 executor 绑定（`docs/research/env-recorder-20261011/REPORT-R067.md`），
   加固提案已按用户决定搁置。
4. **自包含脚本的流密码密钥必然可恢复** —— 壁垒是「每构建唯一 + 提取成本」，
   不是密码学不可破。
5. **`--bind-env` / `--env-check` / `--bind-executor` 绑定态产物不进运行等价矩阵**，
   只做语法校验 + 裸 Luau 干净拒绝验证（跑不动是设计目的）。
6. **托管侧**：Webhook 保护 / 外部验卡应用的前端界面未做；存储未关系化；产物未上对象存储；
   多节点未做；告警自动升级未做；源码保险箱按产品决策不做。
7. **CI**：`.github/workflows/` 无写权限，模板在 `web/tools/ci/build-engine.yml`，需手工复制。

---

## 10. 下一步（若被要求继续）

按优先级，且**每一项都要先确认是否仍在 §34 收摊令范围内**：

1. **rustc 通道恢复 → 重编 wasm 刷锚**（唯一「技术债」性质的事项，恢复后必须重跑 §2 锚点）。
2. V19 **M1（R6 指纹熔断 + R3 oracle 熄灭 + R5 熵坍缩）** —— 待用户拍板。
3. EVKEY 是否翻默认（严格 = 缺链报错 / 自动 = 缺链回落关）—— 待用户拍板。
4. 托管侧：关系化存储 → 外部验卡前端 → 告警规则 → 多节点。
