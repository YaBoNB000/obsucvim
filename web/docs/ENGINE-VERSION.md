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
