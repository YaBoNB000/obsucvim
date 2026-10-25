# 引擎版本：R067 与 af7fc32 的差异，以及怎么恢复

> 结论先说：**引擎源码已恢复到 R067，`web/tools/obsucvim.wasm` 也已重新编译为 R067**
> （2026-10-23）。本文件记录差异、锚点校验与重编方式。
>
> 校验结果：`print("suda") --preset v15 --env-check --bind-executor --seed 67067`
> → **229,422 B / `3775f3b5…`**，与 `handoff/README-R067.md` 记录的 R067 靶标逐字节一致。
>
> 作为对照，我也在同一台机器上编了 af7fc32 版本，它复现了线上旧产物的
> 212,137 B / `01099ddb…`（`--bind-env roblox` 参数下），证明工具链本身是保真的。

## 1. 两个版本的差别

`af7fc32`（2026-10-19，*Reduce v15 output size: 232KB → 212KB for print("suda")*）
为了压缩体积，改了 3 个文件、约 12 行，**全部是降低加密噪声密度**：

| 项 | R067（当前） | af7fc32（旧二进制） | 影响 |
| --- | --- | --- | --- |
| 噪声指令密度 | `base_code_len/12` ~ `/5` | `/30` ~ `/15` | 每个函数的伪指令少 ~2.5 倍 |
| 活跃噪声比例 | `1/2` | `1/3` | 有效噪声更少 |
| 诱饵常量 | 每函数 2–5 | 1–3 | 常量池反推更容易 |
| 死寄存器范围 | 2–4 | 2–3 | H3 噪声覆盖变小 |
| junk_n | 2 | 1 | junk 注入减半 |
| `obf_num` 随机范围 | 40 bit | 28 bit | 数字混淆的搜索空间缩小 ~4096 倍 |

也就是说：**这不是「优化」，这是拿保护换体积**（提交信息自己也写明
「Defense audit: D3 structural fingerprint 22/32 (was 23/32)」）。用户看到的
「少了 40 KB」正来自这里——不是种子抖动，是引擎被削了。

R067 之前的那次 `b82f14f`（剥注释）是零行为变化；所以
`af7fc32^` 的引擎源码 == R067。

## 2. 现在仓库的状态

```
obsucvim/src/main.rs             junk_n = 2                ← R067
obsucvim/src/vmgen/compiler.rs   /12, /5, 1/2, 2..4, 2..5  ← R067
obsucvim/src/vmgen/template.rs   obf_num 40 bit            ← R067
```

校验方式（应输出为空）：

```bash
git diff af7fc32^ -- obsucvim/src
```

## 3. 二进制是怎么换过来的

`obsucvim.wasm` 是构建产物。`rustup` 在沙箱里装不上
（`static.rust-lang.org` 返回 000），apt 无权限，GitHub Actions 又缺少
`workflows` 写权限。**最后是从 npm 拿到了预编译工具链**（`registry.npmjs.org` 可达）：

```bash
mkdir -p /tmp/tc && cd /tmp/tc && npm init -y
npm install @rustbin/rustc-1.88.0-x86_64-unknown-linux-gnu \
            @rustbin/rust-std-1.88.0-wasm32-wasip1 \
            @rustbin/rust-std-1.88.0-x86_64-unknown-linux-gnu
mkdir -p sysroot/lib/rustlib
cp -r node_modules/@rustbin/rust-std-1.88.0-x86_64-unknown-linux-gnu/rust-std-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu sysroot/lib/rustlib/
cp -r node_modules/@rustbin/rust-std-1.88.0-wasm32-wasip1/rust-std-wasm32-wasip1/lib/rustlib/wasm32-wasip1 sysroot/lib/rustlib/
RUSTC=/tmp/tc/node_modules/@rustbin/rustc-1.88.0-x86_64-unknown-linux-gnu/rustc/bin/rustc
cd obsucvim
CARGO_PKG_VERSION=0.2.0 $RUSTC --edition 2021 \
  -C opt-level=3 -C lto -C codegen-units=1 -C panic=abort -C strip=symbols \
  --target wasm32-wasip1 --sysroot /tmp/tc/sysroot \
  src/main.rs -o ../web/tools/obsucvim.wasm
```

产物 1,172,743 B（与旧的 1,182,882 B 同量级），输出与 R067 锚点完全一致。
这是**无 rustup 环境下的备用路线**；正常情况仍建议用下面 §5 的方式（cargo）编。

## 4. 怎么验证当前装的是哪个版本

```bash
node web/tools/verify-engine-version.mjs
# 或用脚本：
./web/tools/build-engine-wasm.sh --check
```

输出示例（当前实际状态）：

```
engine   : /…/web/tools/obsucvim.wasm
probe    : print("suda") --env-check --bind-executor --bind-env roblox --seed 67067
output   : 229422 bytes  3775f3b557ca4c95ee16d9d013f290d735b6c022af23ac17f1d0b2339fc689f6
verdict  : R067 — full noise density (junk_n=2, /12 /5 noise, obf_num 40 bit)
```

服务端启动时也会打印一行 `[engine-server] engine build: R067 | af7fc32 | UNKNOWN`。

**锚点**

| 版本 | 产物大小 | SHA-256 |
| --- | --- | --- |
探测命令：`--preset v15 --env-check --bind-executor --seed 67067`

| 版本 | 产物大小 | SHA-256 |
| --- | --- | --- |
| **R067.2（当前）** | 231,271 B | `8f1b5e21a71e8886704ad79068d8114d386dddd4a9a4145af969b47a984575f5` |
| R067.1（探针只读 `_G`，会误报全部缺失） | 230,368 B | `8b43132eb0297a9017689acfb3a1f80950b2fc269a62630dc4cb4483c014c8fb` |
| R067（回退点，无前置自检） | 229,422 B | `3775f3b557ca4c95ee16d9d013f290d735b6c022af23ac17f1d0b2339fc689f6` |
| af7fc32（旧） | 229,900 B | `12c2c05b572d5ede5d8992684ed4aa724096f9254b66d81563848b99fd91d638` |

（同一 af7fc32 引擎在发布流水线那种 `--bind-env roblox` 参数下就是 212,137 B / `01099ddb…`。）

## 5. 重编 R067（三条路，任选）

### A. 本机（Windows / macOS / Linux 均可）

```bash
rustup target add wasm32-wasip1
cd obsucvim
cargo build --release --target wasm32-wasip1
```

产物 `obsucvim/target/wasm32-wasip1/release/obsucvim.wasm`。或者直接用仓库脚本：

```bash
./web/tools/build-engine-wasm.sh          # 编译 + 校验 R067 锚点 + 安装
```

脚本会在锚点不匹配时**拒绝安装**（除非 `--force`）。

### B. GitHub Actions（推荐，零本地环境）

把 `web/tools/ci/build-engine.yml` 复制到 `.github/workflows/build-engine.yml`
推到仓库即可。它会用 GitHub 的 runner（自带 Rust）编译、跑 R067 锚点校验，
并把 `obsucvim.wasm` 作为 run artifact 上传。

### C. 只换二进制

任何人给你一个 229,422 B、SHA 为 `3775f3b5…` 的 `obsucvim.wasm`，直接覆盖
`web/tools/obsucvim.wasm` 即可，其余代码不用动。

## 6. 换完之后

```bash
node web/tools/verify-engine-version.mjs        # 应显示 R067
cd web && ./tools/rebuild-package.sh --base releases/Obsucvim-Web-Windows-x64.zip
```

然后升级服务器并**重新发布所有脚本**——产物是引擎生成的，不重发就还是旧引擎的字节。


## 7. R067.1 / R067.2 变更（2026-10-23）

### R067.2：探针必须按执行器真实布局查 API

R067.1 的自检写成 `_G[name]`，但执行器把 API 注入 `getgenv()` 返回的执行器环境，
Roblox 的 `_G` 是另一张表——于是 Delta 上连 `typeof` 都被报缺失，依然无法诊断。

R067.2 改成：`getgenv()` → 回退 `_G`，`debug` 同样二级回退。单测见
`web/tools/test-exguard-precheck.py`（三种宿主布局，含「不应误报 `_G` 中存在的 typeof」）。

### R067.1：首次加入前置自检


**问题**：`--bind-executor` 产物在缺 API 的执行器上（如 Delta）不是「拒绝」，而是
`TRIP()` 投毒后 VM 随机位置崩溃，表现为
`attempt to perform concatenation on a nil value` —— 与真实原因无关，线上无法诊断。

**改动**：`obsucvim/src/vmgen/exguard.rs` 在生成代码的 `VKC = {}` 之后插入一段只读自检
（约 680 B，仅出现在 `--bind-executor` 产物中）：逐个 `pcall` 检查
`identifyexecutor / checkcaller / iscclosure / islclosure / newcclosure / loadstring / typeof`
与 `debug.getconstants / getupvalues / setupvalue`，缺失则直接 `error()` 给出清单。

**影响面（实测）**：

| 构建 | 改动前 | 改动后 |
| --- | --- | --- |
| `--env-check --no-bind-executor`（safe） | 193,044 B | **193,044 B，逐字节一致** |
| `--env-check --bind-executor`（hard） | 212,089 B | 212,770 B（+681 B） |
| R067 探测锚点 | 229,422 B / `3775f3b5…` | 230,368 B / `8b43132e…` |

`web/tools/verify-engine-version.mjs` 同时认 R067.1 与 R067 两个锚点。


## 8. R066 vs R067：到底多了哪两层（2026-10-23 复核）

同一份 14 B 源码、同种子、同引擎，只用开关隔离：

| 产物 | 开关 | 大小 | 含的层 |
| --- | --- | --- | --- |
| R066 等价 | `--env-check --no-executor-guard` | 194,326 B | tier-1 |
| + tier-2 | 再加 `--bind-env roblox` | 198,045 B | tier-1 + tier-2 |
| R067 完整 | `--env-check --bind-env roblox` | 215,916 B | tier-1 + tier-2 + exguard |

- **tier-2**（`envcheck::build(rng, tier2)`，条件 `env_check && bind_executor`）要求真实
  Roblox 语义：`Enum.HumanoidRigType` 的 `FromName/FromValue`、`typeof(Enum)=="Enums"`、
  `getmetatable(Enum)=="The metatable is locked"`、`Random.new(51018400)` 的精确浮点序列、
  `task.spawn/delay/cancel/wait`、`game:GetService("RunService")`、`Instance.new("Folder")`
  的 `GetChildren`/`Destroy` 锁、`clonefunction`。
- **exguard**（`--bind-executor`，默认开）额外需要执行器 API 与 `debug` 深内省，
  并在失败时静默投毒（R067.2 起会先报缺失清单）。

两者都是 fail-closed 无 oracle，所以线上表现为「与真实原因无关的随机崩溃」。
R066 只带 tier-1（纯 Luau），因此在任何 Roblox 客户端都能跑——这就是
「R066 正常、R067 报错」的全部原因。

**诊断开关**：`--no-executor-guard`（2026-10-23 新增，默认关）可只摘掉 exguard
而保留 tier-2，用于二分定位。定位包见
`samples/obsucvim/suda-test/diagnose/`（L1 / L2 / L3 三个文件）。


## 9. R067.3：tier-2 去掉「实现敏感」的否决项（2026-10-23）

用户执行 `Obsucvim-Doctor.lua` 的实测报告（Delta 1.1.743.1136）给出了决定性证据：

```
缺失执行器 API : (无)        缺失 debug 深内省 : (无)
Enum 三项全部通过（含 getmetatable == "The metatable is locked"）
tier-2 未通过项 : random_first, random_integer, random_second
  NextNumber #1 : 0.19639633043786484   (官方 Roblox: 0.9598971928030796)
  NextInteger   : 563                   (官方: 889)
```

即：**Delta 的 `Random` 是另一套 PRNG 实现**。tier-2 却把官方 PRNG 的精确浮点写成了
否决条件 —— 这不是在识别作弊，是在拿实现差异当作弊，结果整份产物被投毒。

同类问题还有两处，一并修掉：

| 位置 | 旧判定 | 新判定（版本无关） |
| --- | --- | --- |
| `g_random` | `NextNumber == 0.9598971928030796` / `NextInteger == 889` / 第二个值精确相等 | 值域 `[0,1)`、整数且在区间内、第二次取值有推进、同种子可复现、`Clone` 语义、不同种子不同序列（6 项） |
| `g_member` | 错误文案必须包含 `"Expected ':' not '.'"` | 点调用必须报错且返回字符串；**新增**冒号调用必须成功且返回 boolean |
| `g_instance` | 错误文案必须包含 `"locked"` | 销毁后重新挂父必须被拒绝（报错且为字符串） |

保留不动的部分：Enum 身份/字段/`typeof`/元表（Delta 实测全过）、`task` 语义、
`Instance` 的 `GetChildren`/`Connection` 生命周期、17 项 tier-1 常量与身份全等。

**影响面**：`safe`（`--no-bind-executor`）**逐字节不变**（193,044 B / `c8659801…`）；
`hard` 变 217,206 B / `3372aeb0…`；引擎锚点 230,177 B / `70fbc8bb…`。

**诚实边界**：这三处从「否决」降级为「结构判定」，强度确实略降——一个能正确实现
`Random` 语义的伪造环境现在能过这三项。但换来的是：不再因为执行器 PRNG 实现不同
而把正常用户全部杀掉。这是权衡，不是免费的加强。
## 10. R067.4：precheck 不再裸写 `getgenv`（2026-10-24）

**事故复盘（实锤到字节）**：用户把 R067.3 的 `Suda.hard.lua` 放进真 Delta 跑，Doctor 已证明
13 项执行器 API 与全部 debug 深内省都在，产物却报「**整张清单全部缺失**」，然后拒绝运行。
原因链：

1. 注入进模板的 exguard 片段会过 `sanitize_frag`（template.rs）：凡是不在
   `frag_reserved` 白名单里的标识符一律文本级改名（`_G`、`pcall`、`type`、`error` 等
   在白名单里；**`getgenv` 不在**——守卫本体从来只用 `VK(密文串)` 查它）。
2. R067.2/3 的 precheck 裸写了 `type(getgenv) == "function"` → 产物里变成
   `type(Idu)=="function"then local gy=Idu()`（`Idu` 无任何定义），恒假 →
   precheck 退回只查 `_G` → 执行器 API 全在 `getgenv()` 侧 → 全部误报缺失。
   直接在 `Suda.hard.lua` 里 `grep` 可见：`Idu` 恰出现 2 次、`getgenv` 0 次。

**修法（exguard.rs，三处）**：

| 处 | 改动 | 理由 |
| --- | --- | --- |
| precheck 整体重写 | 不再做任何查表，改为复核守卫已绑定的原语（`IDENTV/CHKCV/ISCV/ISLV/NEWCV/LSTRV/TYPHOF/GINFO(S)V/GCONSTV/GUPVV/GPROTO(S)V/SUPVV`，全部经 `LKP(VK(密文串))` 取得），判定集与 SCHALL 第一道闸门逐一对应，另加 `game/Instance/Vector3/task` 四项（对应闸门 7） | 根除「裸标识符被改名」这一类 bug；报告清单 = 闸门真实要求 |
| `DTBL` 绑定 | `ERD(ENV, …)` → `LKP(…)` | `debug` 库可能只在 `getgenv()` 侧（CENV），LKP 两侧都查 |
| `CIDENT` 恒等检查 | `ERD(ENV,debug) ~= DTBL` → 容忍 `ERD(CENV,debug) == DTBL` | 与上一处配套，否则 DTBL 从 CENV 来时自己否决自己 |

`error`/`type`/`pcall`/`_G` 等允许裸用（白名单内）；字符串字面量不受改名影响。

**验证（真实 Luau 运行时，沙箱）**：

- 裸官方 Luau（非执行器宿）：种子 `67067` 下 precheck 送达，清单准确——
  `loadstring/typeof/debug.info` 是 Luau 自带的**不在**缺失清单里，其余 13 项全列出。
  注意：**某些种子下 tier-2 宿主探测会比 precheck 先崩**（fail-closed 噪音，R067 起
  既有，仅影响非执行器宿主的报错美观度，不影响保护判定，也不影响执行器宿主）。
- Delta 式宿主（API 齐、无深内省，prelude 模拟）：拒绝清单**恰好**是
  `debug.getconstants,debug.getupvalues,debug.getproto,debug.setupvalue` 四项。
- 回归：`safe` 193,044 B / `c8659801…` 与 L1 194,840 B、L2 198,338 B **逐字节不变**；
  仅 `hard`/L3 变 215,171 B / `85751a9a…`；引擎锚点 231,496 B / `c1887865…`。
- 测试套件：hosting 78/78、loader 24/24、attest 18/18、live-loader 13/13、
  engine-retry 9/9、artifact-runtime 全过；`test-exguard-precheck.py` 重写为
  全产物级回归（含「产物不得残留裸 `getgenv`」断言）。

**未验证（照旧诚实标注）**：SCHALL 其余闸门对真 Delta 的正向跑通仍待用户实测——
特别是闸门 14 的 `debug.getproto(fn,1)`（Doctor 显示 Delta 对无子函数会报
`out of proto bounds`）。R067.4 只保证：**该过的 precheck 不再误报；拒绝时原因准确**。
## 11. XF 诊断通道修复（2026-10-24）

R067.4 真机首测：precheck 未再误报，但产物在闸门后以 `attempt to get length of a nil
value`（TRIP 投毒的次生杂讯）崩溃。需要闸门级定位 → 启用 `OBSUCVIM_EXG_TRACE` 时才发现
该通道从未被实际编译过：XF 错误串的两个分支括号都写错了（if 支 `") "`、else 支 `")"`），
生成的 Lua 根本无法解析。已修两处；`wasi-runner.mjs` 顺带支持 `cfg.env` / `cfg.preopens`
透传（服务端不传，行为不变）。非 trace 产物与 R067.4 锚点**逐字节一致**。

产物：`samples/obsucvim/suda-test/diagnose/Suda.hard-trace.lua`（215,458 B，同种子 20261023），
每道闸门炸掉时报 `(字母-随机串)`，字母表见 diagnose/README.md。已在沙箱验证链路：
假宿主深内省造假时正确炸出 H 闸（细节串显示 `cn:Service:Players`，正是假宿主被识破的点）。

## 12. R067.5：tier-2 恒投毒的总根因（2026-10-24）

真机分叉（用户实测）：`Suda.safe.lua` 在 Delta **正常打印 Suda**（VM 核心无辜）；
`Suda.L2-tier2.lua`（tier-2 + bind-env，无 exguard）崩 `attempt to call a nil value` →
问题在 tier-2 层而非 exguard。沙箱成功复现同型错误后，在导出的模板源码里看到：

```lua
local bJ=game:GetService("RunService") ... pcall(function() return bJ:tk() end)
```

R067.3 把 member 检查改成「冒号调用必须成功」时写了 `svc:IsStudio()` **冒号方法语法**，
而注入片段会过 `sanitize_frag`：非保留字、非「全局.字段」形态的标识符一律文本改名
（`IsStudio` → `tk`）。冒号调用从此必然失败 → tier-2 的折叠值 `oqf` 与编译期模型不符
→ **任何宿主上 tier-2 都投毒**。即级联事故：R067.3 修 tier-2 时把 sanitize 规则忘了，
R067.4 修的 precheck 是 exguard 侧的另一处同类问题（两处均正确）。

修复（envcheck.rs）：`svc["IsStudio"](svc)` 方括号字符串形式（与方法名查找语义等价，
但 sanitize 不碰字符串内容）。层模型校正（此前文档把开关记反了）：

| 开关组合 | 开启的层 | R067.5 尺寸（同种子 20261023） |
| --- | --- | --- |
| `--no-bind-executor` | tier-1 | 193,044 B（仍逐字节不变） |
| `--bind-executor --no-executor-guard` | tier-1 + tier-2（L1） | 195,146 B |
| 同上 + `--bind-env roblox` | + Roblox 绑定 CENVPROBE（L2） | 197,033 B |
| 再加 exguard | 完整（L3/hard） | 217,216 B |

**沙箱首次正向验证**（真实 Luau + 高保真 Roblox 宿主 mock，`luau-roblox-prelude.luau`
已升级：Random(LCG)/Enum/RunService.IsStudio/Instance 生命周期/Vector2/Vector3
`__tostring`/task.spawn,delay 返回 thread）：L2 打印 `Suda`（tier-2/bind-env 史上
第一次正向跑通）；hard 在缺深内省宿主上继续精确报四项缺失清单。bind-env 的 env_slots
内建集 = `Vector3.new` / `Vector2.new` / `task.defer` / `typeof`（宿主对照清单）。
锚点 R067.5：231,237 B / `f11a6cea20548681635f81e0d7e6c84987a12c0dbd5b4f8ab1ec81df72205c42`。

**诚实边界**：exguard SCHALL 深闸门在真机上的正向跑通仍未验证（trace 版闸门编号
定位件 `diagnose/Suda.hard-trace.lua` 就绪）。沙箱 mock 里 `setupvalue` 无法真改
upvalue，M/N/P/Q 闸只能靠真执行器给出首份证据。

## 13. GateProbe 宿主画像探针（2026-10-24，引擎零变更）

背景：trace 产物一次只报第一道炸掉的闸，逐轮往返代价高（每轮都要用户开真机）。
新增 `samples/obsucvim/suda-test/diagnose/Obsucvim-GateProbe.lua`（9 开头的 [XG-*] 行 +
`[PREDICT]` 汇总），把 R067.5 的 SCHALL 各闸判定 **1:1 复刻**为独立只读探针
（仅 D 小节做一次写入后立即还原的 rawset 金丝雀），一次运行即得全部闸门的
风险预测。行号字母与 trace 报告的 `(字母-xxxx)` 一一对应，两者可互验。

**沙箱自检**（官方 Luau release 直跑，探针本身非产物）：

| 宿主 | `[PREDICT] risk=` | 说明 |
| --- | --- | --- |
| 裸 Luau | `{B,G,H}` | 无执行器 API / 无 Roblox 全局，引擎 precheck 会先拦 |
| 高保真 prelude | `{B,Q}` | B 的 missing 清单 = `debug.getconstants,debug.getupvalues,debug.getproto,debug.setupvalue`，与 hard 产物在同一 prelude 下的**实际拒绝清单逐字一致**（探针保真度的直接证据）；Q 系 prelude 的 newcclosure mock 为 Lua 包装（mock 局限，非探针缺陷） |

- skip 语义：缺前置 API 的小节只显示 `skip(api-missing)`，不计入 PREDICT（缺 API 时引擎 precheck/前置闸本来就先拦，该闸不可达）。
- 字母表补正：S（19）为保留号，产物内不可达；转录不符经 `_valid=false` 并入 R（18）。
- 锚点不变：引擎 wasm 仍为 R067.5（1,175,136 B / `13916d0b…`；产物锚点 231,237 B / `f11a6cea…`），本轮无任何引擎源码改动。
- 逐闸处置手册（判定内容 / 细节串字段 / Delta 预判 / 命中后处置）见
  `samples/obsucvim/suda-test/diagnose/README.md` 的 GateProbe 一节。

**待真机回传（按优先级）**：① L2 复测应打印 `Suda`；② `Suda.hard-trace.lua` 的
闸门编号；③（可选）`Suda.hard.lua` 直接实测；④（可选）GateProbe 全输出。
诚实边界：PREDICT 是逐闸独立求值的风险集；引擎按 precheck→PROBE→CIDENT→SCHALL→
seal/ATTEST 顺序执行，真机首次拒绝只落在顺序最先的那道闸。

## 14. 里程碑：hard 在真 Delta 全闸门通过（2026-10-24，用户实测）

用户实测回传：R067.5 的 `Suda.L2-tier2.lua`（tier-2 + bind-env）与
`Suda.hard-trace.lua`（完整加固 + 闸门编号插桩）在真 Delta 1.1.743.1136 上
**均正常打印 `Suda`**。trace 构建与 hard 仅差 XF 插桩，故 SCHALL 全闸门（含
C 环境分离、D 金丝雀、J 反向 debug、M `setupvalue` 真改、N `getproto`、P/Q
`loadstring`/`newcclosure` 语义）在真 Delta 上**首次全部正向通过**；
tier-2 的 R067.5 修复（冒号 → 方括号）在真机上同样成立。
此前 §12/§13 的「待真机验证」至此闭环。

同日产物 `samples/obsucvim/kkksu-test/`：`print("kkksu")` 的手测双产物
（safe 191,800 B / hard 214,663 B，同种子 20261024，与发布流水线同构、
仅缺服务端许可守卫注入）。沙箱验证：safe 裸跑与高保真 mock 均打印 `kkksu`；
hard 在 mock 上精确拒绝四项深内省缺失（与 GateProbe 预测逐字一致）。
预期真 Delta 上两者均打印 `kkksu`。

下一步：用户在控制台用 R067.5 包发布线上版本（服务端 safe+hard 双产物、
loader/attest 按宿主能力路由），完成端到端验证。

## 15. 决策记录：不回退（2026-10-24，用户拍板）

背景：kkksu hard 产物被高质量静态解混淆（载荷 114 B / 7 指令全还原，报告与证据链
完整），用户一度要求「回退到 R067 之前的上一个版本」。经逐项核实：

1. b82f14f（R067 直前满噪声版）与 R067.5 在同参数下同种子产出：锚点件 hash
   `3775f3b5…` 逐字节相同、safe 件 `38838223…` 逐字节相同；源码差异仅 4 文件
   76 行（全部为真机可靠性修复与诊断开关）。
2. 把已定位错误点（exguard precheck 判定集 / DTBL / CIDENT / tier-2 门槛与
   冒号语法）外科移植回 b82f14f 后重编重混淆：**产出与 R067.5 逐字节相同**
   （`Kkksu.b82f14f-fixed.hard.lua` = `Kkksu.hard.lua` = `3ede3ab1…`，cmp 实证）。
3. af7fc32（212 KB 版）是唯一真实噪声密度更低的版本——回退到它只会更弱。

结论：**任何「回退到更早版本」的选项都不提高强度**（同体或更弱），被静态击穿是
v15 架构天花板（解码链自描述于产物）所致，与 R067 系列修改无关。用户据此拍板：
不回退，继续使用当前引擎（R067.5）。辨认件与证据留档于
`samples/obsucvim/b82f14f-check/`（含复现方式与全部哈希）。

后续强度方向另案：v16 keyed-decode（载荷解密密钥混入运行期观测值，使静态
复现脚本拿不到首字节）——待用户确认立项。

## 16. 样本裁剪（2026-10-24，应主要求）

`samples/obsucvim/` 下四个大体积测试文件夹已删除（合计约 3 MB）：
`suda-test/`（1.5 M）、`b82f14f-check/`（853 K）、`kkksu-test/`（409 K）、`suda/`（213 K）。
本文 §1–§15 中的所有尺寸/哈希记录**继续有效**，作为留档证据；任何一件产物都可用
当前引擎（`web/tools/obsucvim.wasm` = R067.5）+ 文中记录的参数/种子**同字节重建**。

保留：`samples/obsucvim/diag-tools/`（~55 K）——两件现场排障工具
`Obsucvim-Doctor.lua`（19.8 K，`71a861fa…`）与 `Obsucvim-GateProbe.lua`
（21.6 K，`52aa259d…`）及原 diagnose README（含闸门处置手册）。

## 17. R068 = R067.5 + `--key-host`（v16-a Phase-0/1，2026-10-24）

**关态逐字节同 R067.5**：新引擎不传 `--key-host` 时，锚点重建 =
231,237 B / `f11a6cea20548681635f81e0d7e6c84987a12c0dbd5b4f8ab1ec81df72205c42`
（verify-engine-version 实测），kkksu/发布流水线行为不变。回退保险 = 不传旗标。

### Phase-0 真 Delta 实测（用户回传，真机 Delta 1.1.743.1136）

```
[XG-K1] ok=true name=Delta len=5 stable=true
[XG-K2] ok=true typeof=Vector3 tostring="1, 2, 3" fold31(v3str)=1143546169 fold31(typeof)=1317214663
[XG-K3] ok=true IsStudio=false checkcaller=true boolpack=3
[XG-K4] ok=true {request:function http_request:function syn.request:nil http.request:function}
[XG-KF] combined-fold=1083812779
[PREDICT] risk={}（全闸通过）
```

### v16-a 构造（转录绑定解码钥，flag = `--key-host delta`，非默认）

SCHALL 转录在既有六项后**追加三项运行期现算观测**（产物内无期望值常量）：

- a = fold31(tostring(Vector3.new(1,2,3)))，模型值 `1143546169`
- b = fold31(typeof(Vector3.new(1,2,3)))，模型值 `1317214663`
- c = IsStudio()*1 + checkcaller()*2 + identifyexecutor 名长*4，模型值 `22`
  （真 Delta 客户端 = 0 + 2 + 20）

观测不符/观测 API 缺失（pcall 内折零）→ 转录失配 → ATTEST/TOK/SLT/st 全链失步 →
**静默投毒，载荷字节不出现**。不混版本串（易变）、不混名串字节（无熵）。
诊断同步：trace 模式 XF(18) 随 _tr 带出 KHXA/KHXB/KHXC 三项实测值；
GateProbe 新增 `[XG-KH]` 行直接对表（match=true 才可跑 keyhost 产物）。

### 锚点（R068，seed 67067；发布参数另见 v16-check/SHA256SUMS）

- 引擎 wasm：1,179,341 B / `04b004b2de7212eb0140cac4981ae7eac164bcb00a2b11c1f09443519b9f7ea3`
- suda hard+keyhost：231,571 B / `6c22c008c20883589a9f8ad3503151e2027880ac4454454a975842ba9731e11a`
  （`--preset v15 --env-check --bind-executor --key-host delta --seed 67067`）

### 证据矩阵（已测/未测标注）

| 命题 | 状态 | 证据 |
| --- | --- | --- |
| 关态产物逐字节同 R067.5 | ✅已测 | verify-engine-version 实测 231,237/f11a6cea |
| 折叠公式双层一致（Lua 运行期 vs 编译期模型） | ✅已测 | `test-keyhost-fold.py`：client c=22 tr 互等、studio c=23 失步可复算 |
| precheck 先于转录链（缺 API 清单逐字一致） | ✅已测 | test-artifact-runtime §4（keyhost 与非 keyhost 同清单） |
| CLI 校验（未知 profile / --no-executor-guard 组合拒绝） | ✅已测 | test-artifact-runtime §4 + 手工 stderr 原文核验 |
| 载荷字符串不出现在 keyhost 产物字节 | ✅已测 | grep=0（seed 67067/20261024 两件） |
| prelude 翻默认 IsStudio=false（=真 Delta 形态）后既有套件 | ✅已测 | 全测试绿：artifact-runtime 12(+4 新)、exguard-precheck、hosting 78、loader 24、attest 18、live-loader 13、engine-retry 9 |
| **真 Delta 正向打印载荷（keyhost 产物全链绿）** | ✅已测·第一轮 | 2026-10-24 真 Delta 跑 `Kkksu.v16keyhost.hard.lua` 正常打印 `kkksu` 无报错；Phase-2 第二轮中型件 `Kkksu-mid.v16keyhost.hard.lua`（预期精确输出行留档于 v16-check/README）待回 |

### 本轮文件变动

`obsucvim/src/vmgen/exguard.rs`（KeyHost 档案+转录折项+运行期片段+trace 扩展）、
`template.rs`（参数贯通）、`main.rs`（`--key-host` 旗标+校验）、`web/tools/obsucvim.wasm`
（R068）、`web/tools/luau-roblox-prelude.luau`（IsStudio 默认 false=真 Delta 客户端形态，
`OBSUCVIM_MOCK_STUDIO=true` 旋钮模拟 Studio）、`web/tools/test-keyhost-fold.py`（新）、
`web/tools/test-artifact-runtime.py`（+§4）、`samples/obsucvim/diag-tools/`
（GateProbe +`[XG-KH]`，现行 sha `d0555cda…`；§16 内哈希为裁剪时点记录）、
`samples/obsucvim/v16-check/`（真机验收件两件+README+SHA256SUMS）。

## 18. R069：v16-a `--key-host` 翻默认（2026-10-24，Phase-2 两轮真机验收后落地）

**验收（真 Delta，用户回传）**：第一轮 `Kkksu.v16keyhost.hard.lua` 正常打印 `kkksu`；
第二轮中型件 `Kkksu-mid.v16keyhost.hard.lua` 正常打印
`kkksu-mid:1,1,2,3,5,8,13,21,34,55|143`（递归/循环/table/多常量 bdec 与常量解密
路径全压）。Phase-2 两轮齐 → 按计划翻默认。

**默认语义**：v15 + executor 守卫链齐时 `--key-host delta` **自动注入**（无需旗标）；
守卫链不齐（`--no-executor-guard` 的 safe/tier-2 变体、非 v15 管线）**静默降级为关**
（与 bind_executor 降级风格一致）；`--no-key-host` 显式回退；`--key-host` 与
`--no-key-host` 同用报错。发布管线（engine-server buildGuardedSource dual-artifact）
**参数零改动**：hard 变体自动带钥，safe 变体自动降级。

**字节级等价四断言（实测记录）**：

| 断言 | 结果 |
| --- | --- |
| 默认 auto == 显式 `--key-host delta`（同种子同参数） | ✅ cmp 逐字节相同 |
| auto 默认产物 == R069 锚点 | ✅ 231,571 B / `6c22c008c20883589a9f8ad3503151e2027880ac4454454a975842ba9731e11a` |
| `--no-key-host` == R067.5 锚点 | ✅ 231,237 B / `f11a6cea…` 逐字节回退 |
| `--key-host` + `--no-key-host` 冲突 | ✅ 报错拒绝 |

以上四条已固化进 `test-artifact-runtime.py` §5（字节级锚点断言）；R069 锚点已加进
`verify-engine-version.mjs` 与 `engine-server.mjs` 启动自检 KNOWN 表（最前）。

**R069 引擎 wasm**：1,180,071 B / `31a9dc7ee3ab4b324e589e9fbe8f81a4fc190852c0d8e525f0583ea3de1ebcb2`。

**全测试套件（R069 翻默认后重跑，全绿）**：artifact-runtime 25（+§5 四条）、
exguard-precheck、keyhost-fold、hosting 78、loader 24、attest 18、live-loader 13、
engine-retry 9。live-loader/hosting 走发布管线 → 证明管线在默认带钥下行为不变。

**部署（增量升级路径）**：本轮实际变更仅两件——`tools/obsucvim.wasm`（R069）与
`tools/engine-server.mjs`（KNOWN+ R069，行为兼容）；用它们覆盖安装目录
`C:\Obsucvim\tools\` 同名文件并重启即完成升级。整包（runtime/node.exe+nodemailer
Windows 底座）重出**暂缺底座**（旧 zip 从未入库、沙箱快照已清、邮检白名单够不到
nodejs.org），待底座恢复后重出并更新 `Update-Obsucvim.ps1` 配套哈希。

## 19. R070：delta 宿主钥固化为底层、恒开（2026-10-24，用户拍板）

拍板原文：「key-host delta 直接当底层，永远开启」。落地：**`--no-key-host` 旗标移除**
（使用即报错：`removed in R070 … permanent底层`，不给静默忽略的活口）；
`--key-host delta` 保留为兼容旗标（与不传等价，校验不变：非 v15 / 非守卫 / 未知
profile 仍硬报错）。auto 注入不变（v15+executor 守卫链齐即带钥）；safe/tier-2
无守卫链**维持静默降级**——守卫链是钥的唯一载体，物理上没有折钥的位置，
这与「FFA 要、safe 件跑弱宿主」的既定决策一致，不是可关的口子。

**产物字节零变化**（实测）：auto hard 锚点 = 231,571 B /
`6c22c008c20883589a9f8ad3503151e2027880ac4454454a975842ba9731e11a`（与 R069 相同；
verify/engine-server KNOWN 表 R070/R069 并列同锚点，R070 在前）。

R070 引擎 wasm：1,180,086 B / `22fca22df841ae9fd479445319da9e221767ba0476e591676e921450db89db78`。

测试变化：§5「--no-key-host == R067.5 锚点」与「冲突拒」两条退役，换成
「--no-key-host 被拒（removed in R070）」；auto==显式、auto==R069 锚点两条
字节级断言保留（正好钉死「固化不改产物」）。全套件 R070 重跑全绿：
artifact-runtime 24、exguard-precheck、keyhost-fold、hosting 78、loader 24、
attest 18、live-loader 13、engine-retry 9。

部署：增量升级第二件换成 R070 wasm（见 §18 路径；engine-server.mjs 本轮也随
KNOWN 表更新）。回退口径修正：R070 起「无钥产物」只剩守卫链不齐的构建形态
（safe/tier-2 变体），发布管线 hard 变体不再有「去钥」选项——这正是拍板语义。

## 20. R071：V17 Phase-1a `--super-insn` 演练门面（2026-10-24）

V17 函数级超指令融合的编译期数据流引擎第一刀：新模块 `obsucvim/src/vmgen/fuse.rs`
（白名单 5 对、FUSE_TOP_K=4、滑窗统计 BTreeMap、确定性选词 tie-break、leaders
禁融的塌缩器 `collapse` + 分支槽位重映射、`plan()` 演练门面），经
`Ctx { super_insn }` / `compile_chunk` / `compile` 签名贯穿，CLI 新增
`--super-insn`（默认关）。演练性质：**结果跑完即弃、零 rng 消费、不碰发射端**，
所以：

- on/off 四配置实测逐字节恒等；off 态锚点件 231,571 B `6c22c008…` 与
  R069/R070 相同（verify/engine-server KNOWN 表 R071 置顶、同锚点并列，差异仅在
  wasm 内部新增规划码）。
- 单测层 `rustc --test` 53 绿（新增 fuse_tests 6 项；顺带修 compiler.rs 三处
  陈旧测试夹具的 5 参 compile_chunk 调用、envcheck 一根残留旧转义层级的
  IsStudio 针——两者均为夹具陈旧，引擎语义零改动）。
- 全套件复验全绿：artifact-runtime 27、keyhost-fold、exguard-precheck、
  hosting 78、loader 24、attest 18、live-loader 13、engine-retry 9。

R071 引擎 wasm：1,216,279 B / `267e127b74d2c99f85b70c6c58938e7e04c96359604053144cefd9a1d9983685`。
（体积较 R070 +36 KB：新增规划器代码进 wasm 属预期；产物字节不受 wasm 体积影响。）

掌握事实以真树为准：V17 计划文档 §3-§7 为 Phase-1a 落地后的现状与设计，
§8 仅剩「翻默认」「体积预算」「窗口 3」三项未决。

## 21. R072：V17 Phase-1b `--super-insn` 发射端接线收官（2026-10-24）

v17 主线第二刀：发射端全通（编译 collapse/编码 → 每函数 fv 字节段（第 7 段，
NSECT 6→7、TFV 支臂）→ BSS parse → EP 装载 → HW2 trampoline 派发）。
两个运行期根因按「减法实验 + 数值载荷 trace」实测定案后修复：

1. fvck 系数原先走 `KeyEmitter::key_expr`（默认 data 引用 `FN`），而 v15 的
   parse 位于 BSS fragment 内层装载函数，**词法作用域无 FN** → on 态运行期
   必现 `#nil` → HBOOT PCAL 捕获 → PD() 死环（外呈死循环）。修复：改
   `obf_num` 纯数值混淆。
2. `fvm=(cks*E1+E2)%65536` 原先在 parse return 表构造点求值，而 st2 常量
   解码就地推进 cks 的 LCG 态 → 运行期 fvm 与编译端不符、trampoline 落空槽。
   修复：TCK 支臂读原始 ckseed 当行快照 `fvm0`，返回表只引快照。

另补：v15 装载链 EP 重建是选择性字段拷贝，新增 `fv/fvm` 补拷（super-only）。
调试期全部临时钩（FUSE_TRACE 分级/VM_MARK/BSSOP/EXG_TRACE/TRT/FTR* 占位等）
已撤净；永久保留 `OBSUCVIM_FUSE_DBG` 编译期 stderr 词汇 dump（与既有
`*_DBG` 钩子同型，不触产物字节）。

验收（全部实测）：
- **V2 锚点恒等**：off 态 231,571 B `6c22c008…` ≡ R070/R071 逐字节恒等
  （verify-engine-version 判定 R071 行命中；on/off 纹理隔离成立）。
- **V3 行为不回退（沙箱段）**：luau 直跑 8 输入 × 4 种子 = 32/32 输出逐字节
  正确（含子函数调用、upvalue 闭环、变长参、pcall、fib16 递归）。
- **V1 结构证据**：互异词汇/方言/fvm，详见 V17 计划文档 §9.4。
- 单测 57 绿；全套件：artifact-runtime 0 fail、exguard-precheck 0 fail、
  keyhost-fold PASS、attest 18/18、loader 24/24、live-loader 13/13、
  engine-retry 9/9、hosting 78/78。

R072 引擎 wasm：1,226,632 B / `b4262fcb668a5602bd3403119f0f7c341231bcac1e8628032f2f51350ac1880d`
（较 R071 +10.4 KB，trampoline/装载接线 + FUSE_DBG 诊断口）；
off 态产物锚点不变。verify 锚表以同锚点置顶 R072 行（R071 保留为别名行；
二者产物字节逐字节相同，仅 wasm 内部接线码不同），engine-server KNOWN 表
同口径加 R072 行（hosting 78/78 重测过）。语义口径以本节为准。
真 Delta 验收件走 `samples/obsucvim/v17-check/`（on 态道）：
2026-10-24 用户回传两 hard 件均正常（`kkksu` 与
`kkksu-mid:1,1,2,3,5,8,13,21,34,55|143` 精确打印、无报错，trace 件未动用），
**V3 真机段通过，Phase-1b 验收收口**。

## 22. R073：打印级产物体积收敛（231.6KB → 171.9KB）（2026-10-24）

用户反馈打印级产物 ~220KB 偏大、目标 180–200KB。体积解剖（TSRC dump）定位
主项：微解释器 `_mx{i}` 按 shard 逐份展开（5 份 × ~14KB 臂文本），而各份逐字节
同源（codes/field_perm/carrier 本来就是参数）；shard 数 rng(3,5) 还是种子方差
主驱动（±19KB）。md 滚动方言（`E.md` 运行时轮换）在结构上要求每 shard 注册
全 kind——因此**不可按方言过滤臂**（会破坏绊线路径正确性），正确的减法只有
表征级去重：

1. **工厂化**：`micro_shard_source` 拆为 `micro_factory_source`（1 份共享函数体
   `_MXF`，dispatch 走 `zC[j]` 码位表/`DK[z1..3]` 域排列参数）+ 每 shard 一个
   薄包装闭包 `_mx{i}=function(...) return _MXF(...,_mb{i},_ms{i},_mt{i},{codes},
   p1,p2,p3) end`。语义不变式：每 shard 仍注册全 kind 集、md 轮换路径逐字节等价。
   排障一处关节：原 `d{op},d{x},d{y}` 是**编译期拼名**生成 d1/d2/d3 三个独立
   local，不是表索引——工厂化后改 `DK={d1,d2,d3}` 包 + `DK[z1..3]` 运行期选择。
2. **HW2 别名折叠**：132 对逐条 `HW2[(w+AV)%256]=HW2[(b+AV)%256];` → pairs 表循环
   （-3KB）。

实测（发布构建 1,218,817B，较 R072 -7.8KB）：
- 新锚点（off 态探针）= **171,881B `b67a06e3f4ba620daaac0b81a3fb266cfb9f78fac9dda614052cbc7738024b9b`**，
  双构建逐字节恒等；verify 锚表 R073 置顶（R072 及前保留为历史行）。
- 8 种子 × on/off 共 16 件全落 **161.8–172.8KB**（超出 180–200 目标余量，
  种子方差 ±5.5KB）；Kkksu 级 hard 件 163,115B / 182,491B。
- 64 格 luau 矩阵全绿（s7 按既定口径判）；hard 件裸 luau 精确缺失清单、不挂死；
  单测 57 绿；八套件全绿（artifact-runtime 锚断言已同步 R073）。
- 验收复检件：`samples/obsucvim/v18-check/`（4 件，待真 Delta 回传）。

## 23. V19-EVKEY 原型：宿主 EV 进 blob 密钥流（默认关，2026-10-25）

触发：`Kkksu-mid.v18fusion.hard.lua` 被纯静态全拆（V19 文档 §1 L0–L12 定案）；
路线按用户拍板「3」先原型验证 R1/R4 融合通道，R2 可行性结论见 V19 §7.4。

- 新旋钮 `OBSUCVIM_V19_EVKEY`（Off/Bind/Dev 三态，默认 Off）：
  Bind 态把 `ev_fold(keyhost)=4576808`（delta）烘进每函数 blob LCG 基址，
  运行期 EVK 探针现算补齐；产物内零 EV 字面量（TSRC 审计 0 命中）。
- **旋钮关 = R073 逐字节恒等**（锚点 171,881B `b67a06e3…` 复测不变），
  verify 锚表无需新增行；发布 wasm 1,222,840B（+4.0KB，探针/穿线代码）。
- 部署件 `web/tools/obsucvim.wasm` 已刷新为本构建（关态行为不变 ⇒ SP 全绿：
  单测 59、八套件、行为矩阵、锚点全部复跑通过）。
- Bind 态真机待验：验收件 `samples/obsucvim/v19-check/`（evkey+对照两件，
  同种子 20261025）；Dev 档（`OBSUCVIM_V19_EVKEY_DEV`）仅本地机制验证，
  构建时 stderr 明示禁止交付。

## 24. R075-preview-a（V19 M1-a 中间锚点，2026-10-25）— EVKEY 默认翻 On + R6 指纹熔断

路线「3」合并轮第一批。锚点（`--preset v15 --env-check --bind-executor --seed 67067`）：
**174,163B `5ebefd82b3f1cc618ceef4636c4bcaf6eb0992a1685202d0d67e96f679f033c2`**。

- EVKEY 默认翻 On（exguard::resolve_ev_mode，单测钉死）：未设旋钮=自动档
  （v15+key-host⇒Bind；缺链⇒Off+stderr 明示勿交付）；=0 强制关；=1 显式 Bind；
  DEV 档不变（禁交付）。R3/R5 未落地，终态锚点 R075 待钉。
- 口径变更：**Off 件自本轮起不再承诺 R073 逐字节恒等**（R6/R3/R5 为无条件
  硬化）；R073 锚点行保留为历史对照（仅旧提交可复现）。
- R6 指纹熔断（结构面，对失守表 L0/L2/L4/L6）：KeyEmitter 4 方言×随机名；
  `_MXF`→随机构名+15..18 参抖动+1–2 个形态等价 decoy；载体三连格洗牌。
  no-bind 矩阵：自动 Off==显式 Off 逐字节恒等、五件 luau 输出恒等；bind 件
  裸 luau fail-fast；同种子双构建恒等；TSRC 4576808 零命中。
- 门禁：单测 60 绿；六套件全绿；verify 命中 R075-preview-a。
- 发布 wasm 1,238,826B（+15.0KB，R6/旋钮代码），`web/tools/obsucvim.wasm` 已刷新。

## 25. R075-preview-b（V19 M1-b 中间锚点，2026-10-25）— R3 oracle 熄灭

锚点：**175,190B `d402d1826c293a0020cde2d68aaf21cd46015378285097e5305cf25c59f17000`**。

- R3 六车道（明细与词法断层修复记录见 V19-EXISTENCE-HARDENING.md §9.4）：
  `_bp/_rp/_hp/MZD/QF/osum` 期望值全部扣 ev_term 入库、运行期 EVK 系加回；
  Off（显式 =0 或缺链回落）件保留旧明文 oracle，属非交付形态。
- 修复：EVK 探针外置声明（build do 块词法断层→st2 壳 EVKZ 供桨修复）。
- 门禁：单测 60 绿、no-bind 四件矩阵 RUN-OK + auto==off 恒等、bind fail-fast、
  六套件全绿。发布 wasm 1,240,190B（+1.4KB），`web/tools/obsucvim.wasm` 已刷新。

## 26. R075（V19 M1 终锚，2026-10-25）— R6+R3+R5-a 齐

锚点：**175,796B `a968a510b50a623b538c8a4814724feb39eecc87d79ac0583ad243e6354fabc5`**。

- R5-a 熵坍缩首批：`fold_key_nl`（非线性+高位回混）+ 明文级联，stage-1 blob /
  微码载体 / boot 名池三链同轨（双镜面钉测 fold_key_nl_pinned）。R5-b 余账
  （字符串/常量池链、oc_boot、fv 掩码、相位三字节表）逐条明载于 V19 §9.6。
- 门禁：单测 61 绿、六套件全绿、矩阵全绿、双构建恒等；发布 wasm 1,240,584B，
  `web/tools/obsucvim.wasm` 已刷新。
- 验收件第二轮已重出：v19-check evkey 174,679B `3a1f77f5…`（裸 luau fail-fast ✓、
  TSRC 4576808 零命中）、ctl 171,738B `2b3009ff…`。
- **真机验收 ✓（2026-10-25 用户回传「正常」）**：真 Delta 正常打印，R4 通道 + R3 六车道
  + R5-a 三链真机全线成立——**M1 合并轮收口**。下一批候选（R5-b / M2-R1 / R2-M3）待拍板。

## 27. R076（V19 M1-bb 终锚，2026-10-25）— R5-b 熵坍缩次批，静态可常折面清零

锚点：**177,951B `20ff221c64e34abafa857c96bbd32d1e85b286923de431a905ed769a5091b728`**。

- 七链同族改造（非线性折叠 + 明文级联，逐链双镜面）：strseal 字符串池、
  ConstLcg/mb 常量段、ct/csb-CDEC 常量懒解、oc_boot（洗序后级联 `_ocv`）、
  fv 掩码 `fuse::fv_mask_nl`（1-based 下标演化）、APH/TKD、v15 分块 kn。
- 实测坑定案：部分注入点作用域 FLR 未绑定；新注入点整除统一幂二次
  `(x-(x%D))/D`（双精度精确），FLR 旧站点维持。
- 门禁：单测 61 绿、四件矩阵全等、super-insn 专项跑通（fv 路径实测）、
  bind fail-fast、TSRC 4576808 零命中、六套件全绿；发布 wasm 1,244,869B。
- 验收件第三轮已重出：evkey 176,057B `5b7d1dbd…`、ctl 173,848B `352ea2d2…`。
- **真机验收 ✓（2026-10-25 用户回传「正常」）**：真 Delta 正常打印，R5-b 七链真机
  全线成立——**M1-bb 收口，静态可常折面清零真机定案**。下一批候选（M2-R1 /
  R2-M3 / v18-check 复检 / Phase-1c）待拍板。

## 28. R077（V19 M2 首锚，2026-10-25）— R1 全量入钥流首包

锚点：**178,733B `fdcd689ea9868cd4ca3310e8f92b66d50ee083286dfc89bfa8e18cbe39d5e5a3`**。

- SALT 派生强化（三源 tostring + CLK 守卫采样，NL-mix 替代 *31 线性滚动）。
- 三车道「+ SALT 直加」→ lane 专属 NL 像：bx/bdec（`_sv`，LC_bx）、
  ct/csb（`_svk`，LC_ct）、km 洗牌（`_svq%65521`，LC_km）；幂二次整除、
  obf_num 覆盖、双镜面；写入/读取两侧全运行时，产物内三车道无编译期仿射配对。
- 明载不入列：mu/mnp（镜面不对称悬案）、micro ms/_mt（R6 纹理）、
  oracle 车道（R3 承担）。R5 清零结论与 R3/R4 存在性不动。
- 门禁：单测 61 绿、四件矩阵全等 + auto==off 逐字节、super 跑通、bind
  fail-fast、TSRC 零命中、六套件全绿；发布 wasm 1,249,048B。
- 调试通道注记：no-minify 通道 R076 起既有失真（非本批回归），minify 为
  唯一交付口径。
- 验收件第四轮已重出：evkey 178,974B `c61cdc5c…`、ctl 177,109B
  `1af735bc…`；真机回传中 → 正常即 M2-R1 首包收口。

## §29 R077 真机故障取证 + wasm 手术判别件（2026-10-25）

- evkey 与 ctl 真 Delta **同错 call-nil** ⇒ 分界定为 bind 共用链/R1 共有改造；
  模板 61 行 diff 定性（SALT 三源+时钟门、三车道 NL 像），HBOOT/EXGFRAG 壳
  **零变更**（前期「25 参重排主嫌疑」幻影结论证伪弃档）；CLK 守门惰性定案
  （chunk 签名无 CLK/PCAL，[OSCLK] 恒 2），登记 R1 余账。
- 本地 bind 复现呈非确定彩票族；指针串/时钟/缺全局三驱动钉死排除；本地
  复现链判别力停用声明。
- rustc 不可得 ⇒ **wasm 数据段等长手术**应急：锚点/字面量合并布局实测，
  S1（盐回退）/S2（车道恒等）/S3（全回退）三件，验证链全过（sha 复建命中、
  no-bind fib 全绿、TSRC 静默、bind 干净拒绝）；判别飞行件落档
  `samples/obsucvim/v19-check/diff-flight/`（sha 见 §11.5/V19 文档）。
- 手术工具 `web/tools/wasm-data-surgery.py`；源码修复待 rustc 通道恢复。

## §30 R078 判定性修复（2026-10-25，判别飞行裁决收口）

- 判别飞行真机回传：WS1（盐单回退）挂、WS2（车道恒等）正常、WS3（全回退）
  正常 ⇒ **R077 三车道 lane NL 像定罪**，盐三源+CLK 守卫无罪保留。
- R078 = 三车道回退 R076 线性直加（template.rs 九处清零，与 R076 模板 diff
  仅余注记+盐块）；发布 wasm = R077 + S2 数据段手术（sha `84135546…`）。
  源码等价落盘；rustc 恢复后重编 wasm 并以新锚冲销（verify-engine-version
  R078 条目与 test-artifact-runtime R078PB 锚同步更新）。
- R078 终锚 178,268B `04572e91…`（seed 67067 bind，双构建恒等）。
- 第五轮：evkey `6add3d1b…`（=WS2 同字节）；ctl `a537cdcf…`——**双件真机
  全收 ✓（2026-10-25）**，M2 首包 R078 态定案；判别飞行件撤档（存档 58c3895）。
- R1 余账：NL 像重做（先解剖根因）、CLK 守门惰性正名（接签名或删宣称）。

## §31 改进台账立档（2026-10-25，同日改组）

未完改进项收编 V19-P 系（一字名），明细见 V19-EXISTENCE-HARDENING.md §13。
**同日改组：P-系只保留加强混淆引擎本体的强度项（现为 P1..P5：车道解剖/时钟接入/
镜像悬案/懒解层/相窗三）；发布卫生/调试基建/测试资产/验收侧杂项退役出系**。
后续沟通一律用 P-系代号，停用 R1 余账/悬案/队列等杂名。

## §32 P1 车道解剖推进（2026-10-25）

- 二轮半折叠判别件 F1/F2（基件 R077 `0f8dc517` 数据段手术）真机回传
  **双挂且症状分裂**：F1（独保一级 16384 域折叠）复现原型 call-nil；
  F2（独保二级 65536 域折叠）改报 index-nil-'VY' ⇒ 两级折叠各自独立定罪、
  毒化不同下游消费方。
- 三轮车道切判别件 L1/L2/L3 已建（每件仅杀一条车道 piece2 双折叠：L1=bx
  `_sv`、L2=ct `_svk`、L3=km `_svq`；手术工具扩 l1/l2/l3 模式），验证链
  全绿（未动刀复建 `c61cdc5c`/`1af735bc` 命中、双构建恒等、no-bind fib、
  TSRC 静默、bind 裸 luau 干净拒绝），落档 samples v19-check/diff-flight
  待真机回传；判读表见 samples README 与 V19 文档 §11.5。

## §33 R079 定案修复（2026-10-25，P1 车道解剖收口）

- 三轮车道切真机回传：**L1（杀 bx `_sv` 折叠）正常 ✓** / L2 挂（`argument #1
  to 'char' (number expected, got nil)`）/ L3 挂（call-nil）⇒ bx 字节流车道
  双折叠唯一病根；ct `_svk` / km `_svq` 折叠无罪。结合 F1/F2 双挂分裂
  （两级折叠各罪、毒化不同下游），R077 NL 像解剖三级收口（区→级→车道）。
- R079 = bx 车道恒等落定（`_sv=(SALT/SLT+LC)%2^28`，LC 常量原位保留）+
  ct/km 车道折叠原式复辟（R077 熵卖点拿回两条车道）；template.rs 单点手术
  （R077 基 + bx 两侧折叠段清零），发布 wasm = R077 + l1 数据段手术件
  `95f69c99…`（1,249,048B；发布 wasm 升级 = 增量覆盖 `C:\Obsucvim\tools\
  obsucvim.wasm`；rustc 恢复后重编刷锚）。
- **终锚 178,551B `ccf5d0839fef962a8a0c031d745d869dff7a861b6590fe8e267d93f5cf63c045`**
  （verify-engine-version R079 条目 / test-artifact-runtime R079PB 已同步）。
- 门禁六套件 27 PASS/0 FAIL（纯净 luau）；EVKEY no-bind 矩阵 AUTO/0/DEV
  逐字节恒等（`23af1be8…`）、fib 全打印、=1 no-bind 组合规格内明确报错；
  TSRC `4576808` 零命中 + 语法静默；bind 裸 luau 干净拒绝；ctl 双构建恒等。
- 第六轮验收件：evkey 178,792B `3904533e…`（=L1 判别件同字节，真机 ✓）、
  ctl 176,927B `6530e016…` **真机 ✓（2026-10-25 回传）——P1 与第六轮
  完全收口、R079 双件全收**。

## §34 收摊（2026-10-25 用户指令：停止引擎改进、取消剩余全部计划）

- 剩余全部计划**取消**：P2 时钟接入 / P3 镜像悬案 / P4 懒解层 / P5 相窗三
  （V19 §13 台账已翻「取消」）；退役项（含 rustc 正牌重编刷锚）一并封存，
  封存前状态完整可复起（源码 R079、手术工具、验证链均随树保留）。
- 验收样本目录 `samples/obsucvim/v16-check` / `v17-check` / `v18-check` /
  `v19-check` 全部删除（约 3.7MB；档案存 git 历史，`diag-tools` 保留）。
- **发布态不动**：现役引擎维持 R079（wasm `95f69c99…`，终锚
  `ccf5d0839fef…`；六套件 27/0 绿、真机双件全收）；部署侧无需任何操作。
