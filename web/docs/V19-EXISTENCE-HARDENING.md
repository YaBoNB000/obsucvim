# V19「存在性硬化」设计文档（2026-10-24 立项，触发事件：b.txt 纯静态全拆）

> 触发：`samples/obsucvim/v18-check/Kkksu-mid.v18fusion.hard.lua`（182,491B，
> sha256 `b2b41d21…`）被第三方**零执行、零穷举、零 mock** 纯静态拆至三层
> （可读业务稿 / 78 条私有字节码明文 / 常量与字符串池）。证据包：
> main 分支 `ab.analysis.bundle.zip`（复现器 685 行 Python + 逐层 proof JSON）。
> 本文件是失守定案 + 修复纲领。基调纪律照旧：**不宣称不可破解**；目标是兑现
> 「纯静态断钥」并把最低必要攻击抬升为真机插桩动态工程。

## 1. 失守定案（证据 → 根因映射）

攻击者方法与我们的源码层逐项对应：

| # | 层 | 攻击者手法（证据） | 我们的出处 | 失效根因 |
|---|---|---|---|---|
| L0 | 结构锚点 | 正则在格式化视图上直接命中：15 参数 `local function X(...)`（= `_MXF` 工厂）、紧随其后的 8 参 wrapper 族、`\[(=*)\[` 长串 blob、纯数字表、`(function(d) local s=(NUM+(0|#X))%268435456 for…`（= KeyEmitter 文本指纹） | template.rs 工厂/wrapper 形态、lua_dstr 长串容器、KeyEmitter::key_expr 模板 | **文本形态是刚性指纹**：形态固定 ⇒ 解析器一次写好、终身受用 |
| L1 | 外层 base94 传输块 | 数表参数提取 + 104 次状态转移常折 + fold_bytes 掩码重算；块合并校验 `39505×31+95902=1320557` 命中 | 外层 walk/state-machine 发射 | 输入全为产物内常量 ⇒ 可常折；**校验值也在产物内 = 自带 oracle** |
| L2 | KeyEmitter 52 长度哈希 | 49/52 静态重算（载体长度 + 表达式参数） | key_expr 模板 | 同上：密钥表达式 = 全静态输入的确定性函数 |
| L3 | 原型/常量流 | LCG 参数（seed/stride/mul/add）由「源码参数提取 + 常折」 | 段加密参数发射 | 参数明文以混淆数式存放 = 仍是明文 |
| L4 | 微码载体 | 每份 `_src` 后紧跟三个 `local N = <num>` 行（seed/mul/add 三连格式指纹） → stream_decode | 载体加密参数发射 | 同上 + 格式指纹 |
| L5 | 微码记录仿射掩码 | 「写入/读取公式代数相消」——无需真实值 | bss/fragment 读写对 | **成对仿射同驻产物 ⇒ 必相消**（本轮最深病灶） |
| L6 | shard 语义映射 | wrapper 明文参数表给出 (codes 表 / 三元 layout / 用哪个流) → 每 shard 数页 0 成本解 | micro_shard_wrapper | 码表本是多样性载体，但**布局本身在明文参数位** |
| L7 | 操作数置换 | rank 公式 `(18+(id+1)*5)%24` 静态审计即得 | slot_perm 发射 | 静态公式 |
| L8 | 融合描述符 | mask = `(constant_seed*48901+52612)%65536` 线性；172-175/236-239 包装器路线固定；7 条融合记录顺序展开 | fuse.rs pack_fv_entry + trampoline 模板 | 线性掩码 + 固定路由；**融合定位是体积/纹理工具，不是密钥存在性工具**（定位错位已在 §2 纠正口径） |
| L9 | 字符串解密 | 逐字节状态链静态可复算（strings.proof.json 每字节在案） | strpool/strseal | 同 L1 |
| L10 | 校验体系 | **我们的 osum/osum2 被当成他们的分支正确性验证器**（operands 和/加权和全匹配） | compiler.rs operand_sums 断言 | 产物内 oracle 指导攻击路径 |
| L11 | 运行时成分 | **不必 mock tostring 地址/时钟/探针**——运行时量不进不可逆数据路径 | SALT 仅入校验门/相消对 | 「运行时绑定」名存实亡 |
| L12 | key-host delta | 未复现该层也全拆成功 | exguard/keyhost | host 绑定只进执行闸，**没进密钥流** |

跨层共性一句话：**产物中同时携带写入公式与读取公式 ⇒ 一切掩码均可代数相消；
一切密钥材料都是「混淆表达式包装的产物常量」⇒ 常折即出；一切校验都是
静态可算的 ⇒ 攻击者自带过筛 oracle。** 运行时/环境秘密从未真正进入数据路径的
不可逆位置。

波及面：架构级缺陷 ⇒ v15 至 R073 全部存量件同险（与方法/种子无关；
种子变体目前仅加人工配置成本，因为形态指纹不变）。

## 2. 口径纠正（防幻影）

- 对手复现器**不是通用反编译器**：每样本要人工读壳、写 CONFIG、审计视图
  （包内对视图 SHA 钉死）。种子/形态随机化仍然收「每样本人工费」——值得保留。
- 对手恢复的是**业务核心语义**，不是壳等价；他们在自己文档里也声明了边界。
- R073 工厂化的一处副作用要认账：唯一 15 参函数 + 紧随 wrapper 族 =
  更脆的正则锚点（他们的检测条件就是「参数数==15」）。体积仍该省，
  但「结构唯一性」要用形态多态补回。
- 融合的真实收益：9% 记录融合被展开后仅多一步人工——融合继续服务于
  体积/纹理，**密钥存在性由本文件 R 系措施承担**，两条线不互替。

## 3. V19 修复纲领（R1–R6）+ 上限声明

设计不变式（新铁律，所有改动按此验收）：
**密钥流的每一环节点，其恢复的充要条件不得全部存在于产物字节中；
至少一环必须只能由「真实执行于真实环境」产生，且产物内不得存在
可静态复算的校验 oracle 供候选过筛。**

- R1｜运行时秘密进数据、不可逆混：SALT（28bit 全量）/ 每运行派生量以
  丢信息折叠混入段钥流；写入侧编译时、读取侧运行时，二者不得构成
  同驻产物的仿射配对。消灭「相消则常折」这一整类。
- R2｜执行门控解封（原 V18 构并入）：子函数段钥 = 父函数 Closure 真实
  执行点的 dispatch 折叠链输出（对控制流抖动用编译期可证的确定性
  检查点混合）。只跑加载骨架 ⇒ 段钥在信息论上缺失。
  风险备案：原型先行验证自洽性，失败则退化为「函数粒度确定性检查点」。
- R3｜oracle 熄灭：osum/osum2/_mc/MZD 等一切校验的期望值一律混入
  runtime-only 成分后才存储（正确性由运行时自洽锁定，静态无可筛）。
- R4｜host delta 入密钥流本体（不只执行闸）：bind-executor hard 件的
  段钥含真 Delta 内省测量；非真环境 = 解码失败而非仅拒绝执行。
  Delta 环境值可被同宿主旁录 ⇒ R4 只做叠加不做独柱。
- R5｜字节域熵坍缩修复：所有 `%256` 线性掩码替换为 28-bit 全态非线性
  折叠 + 明文级联（前一明文字节参与后一字节钥） ⇒ 单字节 8-bit
  穷举面灭绝，oracle 同时受 R3 约束。
- R6｜指纹熔断（治标但高效，先行抬高现门槛）：KeyEmitter 文本模板
  家族化（多形态随机）、`_MXF` 唯一性打散（参数计数混淆/wrapper 形态
  多态/注入形态等价副本）、长串 blob 容器多形态（轮换编码 alphabet
  结构）、三连 local 参数格拆散。注意 R6 不改变存在性，只抬每样本
  人工解析成本——与 R1–R5 并存不互替。

**诚实上限**：纯离线单文件交付下，「忠实模拟受保护 VM 全语义 + 重放环境
测量 + 过反调试」在原理上永远可达。V19 的可检验目标：
(i) 纯静态路径断钥——无执行则密钥流不可复算（不再有 foldable/oracle 路径）；
(ii) 最低必要攻击 = 真机插桩动态工程，且全程触发计时/md 轮换/反调试咬定；
(iii) 每层失守都有门禁级回归测试（见 §4），失守即红。

## 4. 门禁新增：静态拆包回归

- 移植对手管线为自测器 `web/tools/test-static-unpack.py`：常折器 + 钥链提取 +
  段解密 + 校验复算（不依赖他们的包；按我们的源码知识重写）。
- 判定口径：对每轮验收件，自测器**必须在不引入运行时值的条件下解不出**
  任一段明文（红 = 失守回归）。
- 种子扫描 ≥ 4 种子 × on/off，防「恰好这颗种子脆」。

## 5. 里程碑（估）

- M1（治标拉门槛，R6+R3+R5）：1 轮内完成；重出验收件；预期对手管线全灭
  但属「文本层」，须明说。
- M2（R1+R4）：密钥流换源；行为验收全绿 + 静态回归红转绿。
- M3（R2）：执行门控原型 → 自洽性矩阵 → 入库。
- M4：文档/锚点/验收件/部署件全链刷新。

## 6. 历史责任注记

- R073 体积轮以「纹理由重复→共享」换体积，方向不变；形态唯一性副作用
  由 R6 的工厂形态多态补回。
- 计划文档 §9 系列中「融合提供 descriptor/码位互异结构证据」的口径曾偏向
  「多样性≈强度」——本文件起统一纠正为：多样性只收人工费，存在性只由
  密钥流结构承担。

## 7. 原型一阶段实装与验收（R4/R1 融合通道 EVKEY，2026-10-25）

路线「3」落地：**先原型验证 R1/R2，再定全纲排期**。本轮交付 R4/R1 融合原型
（宿主 EV 进密钥流本体）+ R2 可行性结论。

### 7.1 实装（旋钮 `OBSUCVIM_V19_EVKEY`，默认关）

- 三态：`Off`（产物与 R073 逐字节恒等）/ `Bind`（真宿主测量现算现折）/
  `Dev(n)`（`OBSUCVIM_V19_EVKEY_DEV=<n>` 开发档：常量进产物，仅本地矩阵机制
  验证，零安全性，构建时 stderr 明示"禁止交付"）。
- 密钥函数（exguard.rs `ev_fold`，钉测试）：`EV = ((k2a*31+k2b)%2^28*31+k3c)%2^28`，
  delta 剖面 = **4576808**。
- 编译期：`compiler::finish` blob LCG 基址 `base += ev_term`（两段 g 初值共用
  base，天然覆盖 ksplit 前后）——**at-rest 字节首次依赖产物外秘密**。
- 运行期：构建层早探针（vault 打包前）现算 `EVK`——逐式镜像 exguard 既有
  测量（Vector3.new(1,2,3) 的 tostring/typeof 的 f31 折叠、IsStudio、
  checkcaller、identifyexecutor 名长）——经 **vault 第 9 槽 → HBOOT → BSS 外层
  第 17 参 EVKX → `unmask_pre` 两处 g 初值**。探针任一环节缺失 → EVK=0 ≠ EV
  → blob 链失步。
- knob 开 + `--no-bind-executor` 且无 DEV → 编译期明确报错（无宿主模型可烘）。

### 7.2 验收证据（全绿）

| 检查 | 结果 |
|---|---|
| 旋钮关锚点 | `print("suda")` seed 67067 → 171,881B sha `b67a06e3…` **逐字节恒等 R073** |
| 单测 | 59 绿（57+ev_fold 钉/模式解析 2 新增） |
| 八套件 | hosting 78/78、engine-retry 9/9、artifact-runtime 0 fail（锚断言满足）、exguard-precheck 0 fail、keyhost-fold PASS、attest 18/18、loader 24/24、live-loader 13/13 |
| 行为矩阵 | s1–s5（print/递归斐波那契/pcall-error/pairs/vararg+math）DEV×Off × luau 输出恒等（s3 仅 chunk 名差异，既定口径） |
| EV 烘入字节实证 | 同种子 DEV=7777777 vs 8888888：148KB 件间 143,136 字节差异 |
| 负样本（存在性） | Bind 件裸 luau：探针取空 → EVK=0 → **载荷不执行**（失步挂起，无输出无泄漏；对照件同环境=既有精确缺 API 拒绝信息） |
| 静态审计（TSRC） | EV=4576808 **零字面命中**；探针在案；BSS 外层 17 参、vault 9 槽 |

### 7.3 未定与诚实边界

- **Bind 路径真机未验**：探针镜像公式须真 Delta 回传（验收件
  `samples/obsucvim/v19-check/`，两件同种子对照）。回了才算数。
- 失败形态=**失步挂起**（静默、无投毒文案）；对非真环境这是设计语义
  （解码失败而非仅拒绝执行），对真机 mismatch 场景自认"变砖"风险——
  故 knob 默认关，Bind 先验收件探路。
- EV 观测面可被**同宿主旁录**（在 Delta 上跑探针即可收割三量）⇒ R4 只做
  叠加柱，不独撑；叠加后攻击下限=真机动态工程，符合 §3 上限声明。
- Dev 档产物可被平凡静态拆（常量就在文里）——严禁交付，仅机制验证。
- 熵面：EV 经 LCG 基址的 28-bit 态进每字节 fold_key，叠加在既有 LCG 截断
  输出之上；截断态恢复类攻击面的收口属 R5（28-bit 非线性折叠）后续轮。

### 7.4 R2（执行门控）可行性结论

- **发现**：现架构装载期一次性解析全部函数 blob（phase 逐台 parse），任何
  "执行量"都无法在装载前进数据路径 ⇒ blob 层做不了 R2。
- **可行落点**：stage-2 `bdec` 是懒解（按块在首次 dispatch 时才解），子函数
  段钥=「父闭包直线前缀（编译期可证无数据分支）的 dispatch 折叠 + EV」在
  该层成立；检查点只能设在编译期可证的确定性前缀（计划文档既定备案）。
- 结论：R2 可做但要动 bdec 键源 + 编译器前缀可证性分析，属 M3 轮；本轮
  不动，避免一次开两个战场。

### 7.5 下一步（待拍板）

- 真机回传 v19-check 两件 → Bind 公式确认 → 再拍 M1（R6+R3+R5）是否开工、
  EVKEY 何时翻默认。

## 8. 原型一阶段真机 R1 回传与 v2 修复（2026-10-25 晚）

**回传**：对照件（knob 关）真 Delta 正常 ✓；evkey 件（v1）真机挂死 ✗。

**定案**：探针取值通道错误——v1 用 `getfenv(1)` 拷贝表（构建层既有 `EV`），
而 Delta 注入面（Vector3/typeof/checkcaller/identifyexecutor）在 `getgenv()`
侧；exguard 既有 `LKP` 自始至终是 **getgenv() 优先、getfenv(0) 兜底**。
v1 EVK=0 ≠ 4576808 → blob LCG 失步 → 装载期死循环。
教训留档：**凡镜像 exguard 测量，必须连取值通道一起镜像，不能只镜像算式。**

**v2 修复**（commit 本文档同轮）：
1. 探针改走 LKP 同源：`getgenv()` → `getfenv(0)` 两级查找；
2. fail-fast：探针整体失败（非真环境缺 API）→ 干净拒绝文案（与闸同一 UX），
   不再挂死；值在但不对（hook 环境）→ 仍静默失步（投毒语义保留）。
3. 本地复验：DEV 件 luau 打印正确；Bind 件裸 luau 干净拒绝（非挂起）；
   旋钮关锚点 R073 恒等（171,881B `b67a06e3…`）；单测 59 绿；
   verify/artifact-runtime/exguard-precheck 全绿。

**v2 真机验收（2026-10-25，已收口）**：用户回传「正常了」——按既定口径即
`kkksu-mini.v19evkey.hard.lua`（172,358B sha `f6c1f45e…`）真 Delta 实跑
正常打印（`kkksu-mini:1,1,2,3,5,8,13,21,34,55`）；对照件此前已验收。
⇒ **R4 通道成立**：宿主 EV 真值已进入 blob 密钥流装载链，探针算式与取值
通道（LKP 同源两级查找）与真机一致（k2a/k2b/k3c → EVK=4576808）；
hook 篡改环境走静默失步投毒，缺 API 非真环境走 fail-fast 干净拒绝。
原型一阶段（R1 组件验证）至此收口。

### 8.1 遗留决策点

- **EVKEY 翻默认**：既定方向「key-host delta 直接当底层，永远开启」；但 Bind
  依赖 key-host 链齐备（delta 注入前提 = bind_executor + executor_guard），
  默认语义待拍板（严格 = 缺链直接报错 / 自动 = 缺链回落关）。
- **M1（R6 指纹熔断 + R3 oracle 熄灭 + R5 熵坍缩）**：开工与否待拍板。
- R2（bdec 懒解层 + 编译期可证前缀折叠）按 §7.4 定案排 M3 轮。
- v18-check 4 件真 Delta 复检仍在排队。

## 9. M1-a 实装：EVKEY 默认翻 On + R6 指纹熔断（2026-10-25，路线「3」合并轮）

用户拍板路线 3：EVKEY 翻默认与 M1 合并一轮做，终态只出一轮新锚点 R075、
一次真机验收。本段是第一批（M1-a）落地记录；R3/R5 随后批次，R075 终钉。

### 9.1 EVKEY 默认语义（exguard::resolve_ev_mode，单测钉死）

| 旋钮状态 | 判定 |
|---|---|
| 未设 `OBSUCVIM_V19_EVKEY`（自动档） | v15 + key-host 链齐备 ⇒ **Bind**；v15 缺链 ⇒ Off + stderr 明示「Off 件 R3 oracle 不熄灭，勿作交付件」；非 v15 ⇒ Off（静默） |
| `=0/off/false` | 强制 Off（即便链齐备） |
| `=1/on/true/bind/空串` | 显式 Bind（非 v15 报错；Bind 缺 keyhost 由 resolve_ev_term 报错，同旧版） |
| `OBSUCVIM_V19_EVKEY_DEV=<n>` | 开发档（旋钮显式开或未设均 honored）；零安全，禁交付 |
| 其余取值 | 明确报错（不静默脑补） |

落点兑现既定方向「key-host delta 直接当底层，永远开启」：默认构建命令
（`--preset v15 --env-check --bind-executor`）经 v16-a 自动注入 delta 宿主模型
⇒ 自动档天然 Bind。**口径变更声明**：自本轮起 Off 件不再承诺与 R073 逐字节
恒等（R6/R3/R5 是无条件引擎硬化）；R073 锚点冻结于历史提交，verify 脚本
R073 行仅作历史对照。

### 9.2 R6 指纹熔断落地清单（语义零变，全经门禁矩阵复证）

对应失守表 L0/L2/L4/L6 的文本指纹面（minify 条件下仍然成立的**结构**指纹）：

1. **KeyEmitter 家族化**：`(function(d) local s=(NUM+…)%268435456 for…` 单一
   模板 → 4 方言（for / while / 分解累加 / repeat-guard）× 随机局部名。
   四种方言与 Rust 镜像逐点同态（加法模 2^28 可交换/可结合，repeat 的
   #d==0 边界与 for 零迭代等价），TSRC 实测四方言分布 31/19/16/13。
2. **`_MXF` 唯一性打散**：工厂名随机构（`_FQ{hex}`）、签名尾挂 0–3 个死参
   ⇒ 参数数在 15..=18 抖动（被拆件的检测条件正是「参数数==15 的唯一函数」）；
   伴生 1–2 个**形态等价副本**（13–17 参 decoy）分流正则命中。实测本种子
   高参数带：13/14/18（无 15）。
3. **载体三连格拆散**：`_mb{i} local _ms{i}=… local _mt{i}=…` 同序三连 →
   逐 shard 语句顺序洗牌 + 连接符多态 + 循环变量改名；shard 块发射顺序不动
   （`_mc` 滚动累加的编译/运行两侧顺序严格一致，一动俱动）。

minify 吸收说明：产物默认走 minify，模板层名随机构（`_FQ/_xw/_mb*xx`）会被
minifier 再抹一次——名字层面本就轮不到我们负责；R6 的有效载荷是
**参数计数/语句顺序/表达式方言/decoy 分流**这些 minify 抹不掉的结构面。
（诚实口径：R6 只抬每样本人工解析成本，不改变密钥存在性，§3 原文不废。）

### 9.3 M1-a 门禁证据

- 单测 **60 绿**（新增 resolve_ev_mode_default_auto）；wasm 1,238,826B。
- no-bind 矩阵（`--preset v15 --no-bind-executor`，seed 20261025）：
  自动 Off == 显式 Off **逐字节恒等**；auto/off/dev7/dev8/devauto 五件 luau
  实跑输出与原文恒等；dev7≠dev8（密钥流差异在）；自动回落 stderr 文案在。
- 交付路径（bind 全自动 Bind）：裸 luau fail-fast 干净拒绝（探针文案）；
  同种子双构建逐字节恒等；TSRC：`4576808` 字面量 0 命中、decoy×2、
  工厂 18 参、探针在。
- 六套件全绿 + verify 命中新中间锚点 **R075-preview-a**
  （174,163B `5ebefd82…`，seed 67067 bind 探针）。

### 9.4 M1-b 实装：R3 oracle 熄灭六车道（2026-10-25）

六条校验车道的期望值一律**先扣 ev_term 再入库**，运行期由探针现算的
EVK/EVKX/EVKZ/EVK_ 原位加回——静态常折只能得到错位期望值，oracle 过筛
能力在 EVKEY 激活件上熄灭（默认交付形态=自动 Bind，故默认熄灭；显式
Off 件保留旧明文 oracle，stderr 已明示勿作交付件）：

| 车道 | 存储点（编译/模板期） | 运行期加回点 | EV 名 |
|---|---|---|---|
| `_bp` 校验 | `phase_checksum - ev_term`（mod 2^28） | `BD=(_bc-(_BSSPCX_+EVKX))` | BSS 外层参数 EVKX |
| `_rp` 校验 | `rt_phase_checksum - ev_term` | `PZ4=(_rc-(_RTPCX_+EVKZ))` | st2 壳尾参 EVKZ |
| `_hp` 校验 | `hb_phase_checksum - ev_term` | `_he=(_hc-((…)+EVK_))` | vault 第 9 槽 EVK_ |
| 微码 `MZD` | `micro_chaff_expected - ev_term` | `MZD=(_mc-(…+EVK))` | build 层 EVK |
| QF 折叠 | `qf_checksum - ev_term`（RTQCKX 车道） | `PZ3=(_qk-(_RTQCKX_+EVKZ))` | EVKZ |
| osum/osum2（L10） | 各扣 ev_term（mod 2^32） | `d1/d2=(pz.ck…-(OS+EVKX))%2^32` | EVKX |

**真机前修复（词法断层）**：EVK 探针的 `local EVK` 原在 build 块 `do…end`
内层（HBOOT/vault/微码消费同层无碍），但 st2 壳调用 `RTFRAG(…,EVK)` 在块外
⇒ EVK 出作用域为 nil，Dev 件执行期「number and nil」崩。修复=外层预声明
`local EVK = 0`、探针只做赋值（Dev 同理）。教训留档：**往装载链引新消费点
时先核对该名的词法层（本仓 build 骨架是个大 do 块）。**

门禁：单测 60 绿；no-bind 矩阵 auto/off/dev7/dev8 全 RUN-OK 且 auto==off
逐字节恒等；bind 件裸 luau fail-fast；六套件全绿；verify 命中
**R075-preview-b**（175,190B `d402d182…`，seed 67067 bind 探针）。
R075 终钉仍待 M1-c（R5）。

### 9.6 M1-c 实装 + M1 收口（2026-10-25，R075 终锚）

**R5-a（熵坍缩首批）落地**：`isa::fold_key_nl`（交叉半积 `(x%16384)*(x/16384)`
+ 高位回混 `x*257+x/65536`，双精度精确域内最大中间值 ~2^36）+ 明文级联
（前一明文字节并入 LCG 推进），三条 at-rest 线性链同式改造、双镜面钉死：

1. **stage-1 blob**（主钥流，`unmask_pre` × `compiler::finish`，级联跨 ksplit
   边界连续携带；两处编译侧测试镜像 `unmask_blob`/scatter 测试同步）——
   L1/L3 失守主路径换轨；
2. **微码载体**（shard `_g` 链）——L4 失守面换轨；`_ms/_mt` 运行时对（L5
   病灶）经评估：其对静态拆解无关（静态路径直取 `_src`→`_v`，与 ms/mt 无涉），
   真实防护由本车道承担，`ms/mt` 对留作运行时纹理，不再列为病灶；
3. **boot 名池链**（NBa/NBs + NBr 级联）。

**门禁**：单测 **61 绿**（新增 fold_key_nl_pinned：0→0、1→2、16384→128 手算钉
+ 4096 态抽样与旧线性折叠区分度 >3000）；矩阵全绿（M1-b 修复后 Dev 件词法
断层不复现）；六套件全绿；**R075 终锚 = 175,796B
`a968a510b50a623b538c8a4814724feb39eecc87d79ac0583ad243e6354fabc5`**
（seed 67067 bind 探针，双构建逐字节恒等）。

**M1 收口状态**：R6 + R3 + R5-a 齐。R5-b 余账（不改变 M1 收口结论，逐条明说）：
字符串池 `strseal` 链与常量段 `ConstLcg/CDEC`（L9）、`oc_boot` 16-bit 线性掩码、
fv 融合掩码（`pack_fv_entry` 65536 线性）、`_bp/_rp/_hp` 相位三字节表（3 字节
相位码，**R3 已锁其校验**,本身非数据流）——以上仍为静态可常折面，排 M1-bb
批次或并入 M2；攻击者若追此面只能在 R075 纹理上逐点人工，不再有代数相消快车。

**真机验收件（第二轮，本合并轮唯一验收点）**：
`samples/obsucvim/v19-check/kkksu-mini.v19evkey.hard.lua`（174,679B
`3a1f77f5…`）——正常打印即 M1 全线真机成立；异常跑 `diag-evk.lua` 回传。

### 9.7 M1 真机验收回传（2026-10-25，收口）

**回传**：用户真 Delta 跑 R075 形态 evkey 件（174,679B `3a1f77f5…`）——**正常 ✓**，
打印 `kkksu-mini:1,1,2,3,5,8,13,21,34,55` 与设计一致。按 §8 既定口径（「正常」/「正常了」
即视同真机验收通过），本轮验收点成立，`diag-evk.lua` 未动用。

**由此真机定案的链路**：R4 宿主 EV 取值通道（getgenv 优先/getfenv(0) 兜底镜像）
× R3 六车道期望值（构建期扣 ev_term 入库、运行期 EVK 系加回）× R5-a 三链
（stage-1 blob / 微码载体 / boot 名池，fold_key_nl + 明文级联）在真 Delta 宿主
全部成立；EVKEY 自动档（默认 Bind）的探针公式与 fail-fast 语义真机符合设计。

**M1 合并轮至此真机收口**。R5-b 余账、M2（R1 全量 SALT 入钥流）、R2/M3 排队状态
不变（§9.6 逐条明载），等用户拍板下一批。

### 9.8 M1-bb 实装（R5-b 熵坍缩次批，2026-10-25，R076 终锚）

**R5-b 落地**：§9.6 所列静态可常折余面（除相位三字节表，R3 已锁校验、本身非
数据流）全部换轨为「非线性折叠 + 明文级联」同族构造，逐链双镜面：

1. **字符串池 strseal 链**：65521 域交叉半积 `(st%256)*(st/256)` + 高位回混
   `x*257 + x/256`，`_pv` 明文级联（Rust `strseal` 双函数 × LoadStr handler）。
2. **常量段 ConstLcg/mb 链**：`ConstLcg` 增 `pv` 字段、`fold_key_nl`、
   `mask/unmask` 双口（编译侧 `Const::encode` × parse `mb()` × 测试镜像）。
3. **常量段 ct/csb-CDEC 链**：stj/stp 双 pre-advance 后 NL 混洗成 16-bit 掩码；
   字符串字节流并入 `_pv2` 级联（boot 编码侧 × CDEC 懒解 metamethod 镜面）。
4. **oc_boot 16-bit 线性掩码**：先洗序、后按洗后顺序施掩，NL16 +
   `_ocv` 级联（Rust 装表 × boot 解表循环）。
5. **fv 融合掩码**：`fuse::fv_mask_nl`（NL16，1-based 条目下标演化；
   编译端逐条现算 × fused trampoline 内 `_m` 同式）。
6. **APH 字母表 / TKD 标符节 / v15 分块 kn 折叠**：残留 `fold_key` 线性面
   全数换 `fold_key_nl`（同式 Lua 文本镜面）。

**作用域教训（实测定案）**：oc_boot / LoadStr 注入点内 **FLR 未绑定（nil）**——
裸跑当场 `attempt to call a nil value` 现形。R5-b 全部新注入点整除统一改为
幂二次 `(x - (x % D)) / D`（D 为 2^k，双精度精确除法），不再依赖 FLR 可见性；
既有 FLR 站点（mb/APH/TKD 文本原已存在 FLR）维持不动。

**门禁**：单测 **61 绿**；no-bind 四件矩阵输出恒等、auto==off 逐字节恒等；
Bind 件裸 luau fail-fast；**super-insn 件专项跑通**（fv 掩码路径实测覆盖）；
TSRC `4576808` 零命中；六套件全绿；**R076 终锚 = 177,951B
`20ff221c64e34abafa857c96bbd32d1e85b286923de431a905ed769a5091b728`**
（seed 67067 bind 探针，双构建逐字节恒等）；发布 wasm 1,244,869B 已刷新。

**至此「静态可常折面清零」成立**：攻击者面对 R076 纹理只余逐点人工一途，
不再有代数相消快车。诚实口径照旧：本批次只抬人工解析成本，密钥存在性论
（EVKEY 通道）由 R3/R4 承担，R5-b 不宣称不可破解。

**真机验收件（第三轮）**：`samples/obsucvim/v19-check/` evkey 176,057B
`5b7d1dbd…` + ctl 173,848B `352ea2d2…` 已重出，待真 Delta 回传。

### 9.9 M1-bb 真机验收回传（2026-10-25，收口）

**回传**：用户真 Delta 跑 R076 形态 evkey 件（176,057B `5b7d1dbd…`）——**正常 ✓**，
打印 `kkksu-mini:1,1,2,3,5,8,13,21,34,55` 与设计一致，按既定口径视同真机验收
通过，`diag-evk.lua` 未动用。

**由此真机定案**：R5-b 七链（strseal / ConstLcg-mb / ct-csb-CDEC / oc_boot /
fv / APH-TKD / v15-kn）的非线性折叠 + 明文级联在真 Delta 宿主全线成立；
幂二次整除修正（FLR 作用域缺口）真机有效。**M1-bb 至此真机收口，V19 存在性
硬化之「静态可常折面清零」真机定案**（诚实口径不变：抬人工解析成本，不宣称
不可破解；密钥存在性由 R3/R4 承担）。

后续候选：M2（R1 全量 SALT 入钥流）、R2/M3（bdec 懒解层 + 前缀折叠）、
v18-check 四件真机复检、Phase-1c 余账——等用户拍板。

## 10. M2 首包实装：R1 全量入钥流（2026-10-25，R077 终锚）

**设计落点**（按 §3 R1 原文实施，并对不可行幻想做边界切割）：密钥流每一环
的恢复充要性中必须含运行时捐赠量；本批次把 SALT 从「部分车道线性直加 /
8-bit 截断引用」升格为「28-bit 全量、车道专属丢信息 NL 像」：

1. **SALT 派生强化**（单面，模板内）：三源 tostring（两新表 + 新函数闭包）
   逐字节滚动，每字节并入交叉半积 NL 混洗，终值再过高位回混——替代原
   `*31` 纯线性滚动；CLK 存在时经 PCAL 守卫采样并入低 16 位。
2. **bx/bdec 块字车道**：EP 装载期编码 st 初值与 bdec 懒解 st 初值的
   `+ SALT`/`+ SLT` 直加 → lane 常量 LC_bx 下的 NL 像 `_sv`（两侧同式镜面）。
3. **ct/csb 常量车道**：boot 重编码 cbase 与 CDEC 懒解 base 的 SALT/SLTC
   直加 → LC_ct 下的 NL 像 `_svk`。
4. **km 洗牌索引车道**：EP 段内自洽映射 `(ci*CM2+qf*CQ2+i*CI2+(bz%65521)*CI2)`
   并入 LC_km 的 `_svq%65521`（65521 质域内加常数保持双射性不变）。

NL 像一律幂二次 `(x-(x%D))/D` 整除（不依赖 FLR），LC 常量互不相干并经
obf_num 覆盖（R6 文本随机化延续）；写入侧与读取侧均为运行时——产物内
不驻「编译期写/运行期读」的仿射配对在该三车道就此归零。

**明载不入列**：mu/mnp upvalue 车道——实测定案其镜面本就不对称（EP 编码侧
mst 含 `(bz%65536)*((i+MQ)%65536)` 而 makefn 解码侧无；两侧在既有常量系下
自洽，未有证据前不动）；micro 载体 ms/_mt 属 R6 运行时纹理（静态路径直取
`_src→_v`），与 M1-bb 时结论一致，不入列。oracle 校验车道（osum/QAG/MZD）
由 R3/ev_term 承担，不属于 R1。

**实测插曲（如实留档）**：`--no-minify` 调试通道在 R076 起既有失真（R076
时代 no-minify 件即报 call-nil，本批表现为挂死）；经 R076/R077 双对照
minify 件逐字节确定性（1887f11a 三相构建恒等）确认交付通道（minify）为
唯一有效口径，no-minify 失真登记为调试通道既有缺陷，非本批回归。

**门禁**：单测 61 绿；no-bind 四件（auto/off/dev7777777/dev8888888）输出
恒等且 auto==off 逐字节恒等；super-insn 件跑通；Bind 裸 luau fail-fast；
TSRC `4576808` 零命中；六套件全绿；**R077 终锚 = 178,733B
`fdcd689ea9868cd4ca3310e8f92b66d50ee083286dfc89bfa8e18cbe39d5e5a3`**
（seed 67067 bind 探针，双构建逐字节恒等）；发布 wasm 1,249,048B 已刷新。

**诚实边界**：R1 存在性贡献 = 三车道密钥流必需真机运行时捐赠量（叠加于
EVK 之上），对「同宿主旁录 EV 后离线重放」的攻击面强制其为动态工程；
对忠实模拟 VM 全语义的仿真重放在原理上不构成新墙（§3 上限声明原样适用）。

**真机验收件（第四轮）**：`samples/obsucvim/v19-check/` evkey 178,974B
`c61cdc5c…` + ctl 177,109B `1af735bc…` 已重出，待真 Delta 回传。


## §11 R077 真机故障取证全书与 wasm 手术判别飞行（2026-10-25）

### 11.1 分界实验定案

第四轮验收件在真 Delta 上的回传：

- evkey 件（`c61cdc5c…`）：`qpTaFNJmHnfMMWNVP:3: attempt to call a nil value`，
  LocalScript Line 3 ×7 帧。
- ctl 件（`1af735bc…`，EVKEY=0、同 bind hard 门）：**同错**。

⇒ 嫌疑从「evkey 专属线」切至 **bind 共用链 / R1 共有改造**（EP SALT 三源强派生
与 _sv/_svk/_svq 三车道 lane NL 像——R077 相对 R076 的唯二引擎语义变更）。

### 11.2 取证定案（逐项）

1. **模板 diff 61 行定性**：R076(c8fd8c7)→R077(fb96e01) 的 `vmgen/template.rs`
   全 diff 只有 SALT 一源→三源派生（含时钟门）与三车道 lane NL 像的注入文本
   及其 manifest 三常量；**HBOOT/EXGFRAG/BSSF/exguard 壳代码零变更**。
   此前一度记为「HBOOT 25 参重排/EXGFRAG 变更为主嫌疑」系幻影结论，已证伪弃档。
2. **CLK 守门惰性**：EP build chunk 签名为
   `function(BYTE,CHAR,FLR,SUB,AL,TK,decarrier,r16,CKM,CKC,BKM,BKC,BSEED,BSTEP,TYP,TOSTR{,EVKX})`，
   不含 CLK/PCAL；时钟门内 `CLK`/`PCAL` 为全局裸名读，恒 nil，门不触发。
   运行证据：插桩 luau 的 `[OSCLK]` 计数 R076/R077 四件恒为 2。
   **「R1 称时钟噪声混入」名不副实，登记为 R1 余账修复项。**
3. **镜面一致性**：`_sv/_svk/_svq` 写入侧（EP，SALT 种子）、读取侧
   （bdec，SLT 种子 / CDEC，SLTC=SLT 种子）逐字镜面，幂二次整除浮点算术双
   realized 复核一致。
4. **本地复现与彩票现象**：自建 luau(master) + Delta 形桩下，bind 件呈
   非确定性失败族（arith-nil/length-nil/call-nil 漂移，~1/12 通过率）。
   三大驱动逐一钉死排除：C 级指针串钉死（TOSTR-PIN 常量化）—非地址串驱动；
   os.clock 定值—非时钟驱动；env 覆盖全量（行为日志仅见引擎自检随机名诱饵
   读）—非缺全局。残余嫌疑为桩-真机 Luau 行为差；**本地 bind 复现链对 R077
   根因不具判别力，停用**。
5. **引擎产物结构**：EP build 文本在产物内不可见（加密于载荷区），明文区为
   烧壳+attestation+探针——wasm 数据段是唯一手术场。

### 11.3 wasm 数据段等长手术（应急判别通道）

rustc 不可得（网络白名单），判别修复改走 **wasm 数据段等长字节手术**：

- 模板文本以 rustc `format!` 拼片连续存储：`{}` 洞在数据段**零字节**，
  编译期占位；实测五处 lane piece1 锚与三份 piece2 存储（bx_ep/bx_rt 共用
  `_sv` 版 120B、ct_ep/ct_rt 共用 `_svk` 版 129B、km_ep 独有 `_svq` 版 129B
  ——rustc 字面量合并布局），SALT 块 425B 单点连续。
- 手术一律等长覆盖（不动数据段 (ptr,len) 参照与指针表），新文本尾空格垫齐；
  Lua 侧垫空格经 minify 压缩，无语义影响。
- **S1**：EP SALT 三源+时钟门 → R076 单源派生核（`local _st={} ... end end`
  含 do 闭合）。**S2**：lane piece1 `local _sv=(SALT+` → `local _sv=SALT+(`
  等价翻转、piece2 尾部 NL 链置空格 ⇒ `_sv/_svk/_svq` 恒等 = R076 线性直加
  钥流（LC 常量保留，双镜面随共用存储一并生效）。
- 工具存档 `web/tools/wasm-data-surgery.py`（唯一性/多处出现保险，sha 打印）。

### 11.4 手术件验证链（每件均过）

1. 未动刀 wasm (0f8dc517) 复建：ctl 命中 `1af735bc…`、evkey 命中
   `c61cdc5c…`（构建参数完全可复现）。
2. 手术 wasm 建 no-bind 件：本地 luau **完整打印 fib**（最强本地语义门）。
3. 手术 wasm TSRC dump：luau 直接运行语法静默。
4. 手术 wasm 建 bind 件：裸 luau **既有文案干净拒绝**（不挂死）。

### 11.5 判别飞行件（`samples/obsucvim/v19-check/diff-flight/`）

| 件 | 手术 | sha256 |
| --- | --- | --- |
| kkksu-mini.ws1evk.hard.lua | S1 | `19c9fdb83313616e3e99e7d8cec09417aee7a1b17abdd39993d67c4c0857a303` |
| kkksu-mini.ws2evk.hard.lua | S2 | `6add3d1b3e3d7a40275463764d15036d022cb0c8cd20f664e3b0c647875e0ad1` |
| kkksu-mini.ws3evk.hard.lua | S1+S2 | `2a926c2d518f15a9ffd90b9876b40427986dde722d5474064a8b2b0ab7bdbff5` |

判读表：全挂 ⇒ bind 壳潜伏病（转阶段错误分解诊断）；WS1 独正常 ⇒ SALT
三源/时钟门区有罪；WS2 独正常 ⇒ 三车道 lane 有罪；WS3 正常而 1/2 各挂 ⇒ 共谋。

### 11.6 诚实边界

- 手术件为 wasm 级判别件，**引擎源码未改**；判罪后须源码落同义修复
  （rustc 通道恢复后重出正牌 wasm），手术熵退化（盐/车道回 R076 形）不得
  作为交付态。
- 手术件未过六套件/单测门禁（rustc 不可用），验证链如 §11.4，明载边界。


## §12 判别飞行裁决与 R078 判定性修复（2026-10-25 收口）

### 12.1 真机回传裁决

| 判别件 | 手术 | 结果 |
| --- | --- | --- |
| WS1（S1 盐单回退） | EP SALT 三源+时钟门 → R076 单源 | **仍挂 call-nil** |
| WS2（S2 车道恒等） | 三车道 lane NL 像 → 恒等线性直加 | **正常 ✓** |
| WS3（S1+S2 全回退） | 双区回退 | **正常 ✓** |

**裁决**：R077 病根 = **三车道 lane NL 像**（bx/bdec `_sv`、ct/csb-CDEC `_svk`、
km `_svq`）；盐三源强派生 + CLK 守卫**无罪且保留**（WS3 保留盐区仍正常）。

### 12.2 R078 定案

- 三车道回退 R076「+ SALT/SLT/SLTC 直加」线性注入（`template.rs` 九处手术：
  lane 构造块/ct_ep 并段/EP km `_svq` 项/bx `{svbx}` 前置/CDEC `{sv_ct_rt}`/
  bdec `{svbx}` 及参数行全清，manifest R1LC_* 三键除名）。
- 盐区保留：三源 tostring 派生 + CLK 守卫采样原样（WS3 证其无害）。
- 源码与 R076 模板 diff = 仅 R078 注记 + EP 盐块两处（逐字校验过）。
- **发布 wasm = R077 二进制（0f8dc517）+ S2 数据段等长手术**（sha
  `8413554638764e10…`，1,249,048B）：rustc 通道恢复期的同义产物；源码 R078
  已落盘，rustc 恢复后重编并以新锚冲销手术 wasm（ENGINE-VERSION §30 挂钩）。

### 12.3 新锚与新件

- **R078 终锚** = 178,268B `04572e910c7f7bb18da864bb2a51bf161fccc05e15ef0791dd95d9d4ca2654fe`
  （seed 67067 bind 探针，双构建逐字节恒等）。
- 第五轮验收件：evkey 178,509B `6add3d1b…`（= WS2 判别件逐字节恒等），
  ctl 176,644B `a537cdcf…`——**2026-10-25 双件真机回传均正常 ✓，V19 M2
  首包（R078 态）真机收口**；判别飞行三件撤档归档（commit 58c3895）。

### 12.4 悬案与余账（诚实边界）

1. **NL 像根因未解剖**：定罪到「三车道 NL 像区」粒度，未定案到具体一步。
   嫌疑候选：① 65521 质域内 km `_svq%65521` 项参与的映射键在双镜面
   realized 算术下失步（`_svq` EP 侧经 NL 折叠后参与洗牌表键，读取侧无镜面
   ——§10 明载「无需运行期镜面」的结论有误嫌疑最大）；② NL 链中间值在
   Luau/double 域的边界相位。重做 NL 像前必须先在真机定位到这一步。
2. CLK 守门惰性名不副实（EP chunk 签名不含 CLK/PCAL，门恒不触发，
   `[OSCLK]` 计数恒 2）——R1 余账之一：或接通签名、或移除文案宣称。
3. 本地桩复现链判别力停用结论维持（两版 Luau 下 bind 件皆彩票/挂，
   R076 绿灯件亦挂 ⇒ 桩-真机环境语义差通道未斩断；0.650⇔0.744 版本差
   另致 no-bind 件行为漂移，本地门禁以 0.744 为唯一标准口径）。
4. wasm 手术件未过单测/六套件门禁（rustc 不可用）；验证链 =
   §11.4 四项 + 双构建确定性 + 本裁决真机回传。


## §13 改进台账（P-系命名，2026-10-25 重定义立档）

本公告统一收编此前散落的杂名待办（R1 余账/余案/悬案/队列），代号
`V19-P<n>`，一字名，原名映射如下。状态分：可开工 / 受限（前置）/ 待真机。

| 代号 | 一字名 | 原名/出处 | 内容 | 状态 |
| --- | --- | --- | --- | --- |
| **P1** | 车道解剖 | NL 像根因解剖+重做（§12.4 #1） | 三车道 NL 像定罪到区未到步；F1/F2 半折叠判别件（wasm 数据段手术）真机再判一次定到步，然后重做 NL 像 | 待真机（一次名额） |
| **P2** | 时钟正名 | CLK 守门惰性（§12.4 #2） | EP chunk 签名无 CLK/PCAL，时钟门恒不触发；接签名真启用 或 删宣称 | 受限（rustc+真机） |
| **P3** | 正牌编译 | wasm 手术件冲销 | 发布 wasm 现为 R077+S2 手术件（`84135546`）；rustc 恢复后由源码 R078 重编、六套件复检、新锚冲销 | 受限（rustc） |
| **P4** | 桩医 | 本地 bind 桩判别力 | 本地桩连 R076 绿灯件都挂 bind；嫌疑 debug 深内省/双层 env；修好前 bind 类故障只能真机判 | 可开工 |
| **P5** | 静态自测 | §4 test-static-unpack.py | 静态解包回归自测器未建 | 可开工 |
| **P6** | 镜像悬案 | mu/mnp 上值车道镜面不对称 | EP 编码含 bz 项而 makefn 解码无；M2 明载不入市，待剖析 | 可开工 |
| **P7** | 懒解层 | R2 包 | bdec 懒解层 + 前缀折叠，存在性硬化下一正式包 | 可开工 |
| **P8** | 相窗三 | V17 Phase-1c 窗口 3 + --super-insn 翻默认 | V17 遗留收官 | 可开工 |
| **P9** | 排障通道 | no-minify 失真 / wasi bind 挂死 | 调试体验项，非交付面；no-minify R076 起既有失真（登记非回归） | 降级 |
| **P10** | V18 复检 | v18-check 4 件真 Delta 复检 | 验收侧排队项 | 待真机（用户侧） |

**建议序**：P1 → P2 → P7（真机名额稀缺时 P4/P5/P6 均为纯本地活）。
