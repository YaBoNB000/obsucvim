# 环境认证方案 WTF-PORT v1（取代 C6-IMPL）

**日期：2026-10-11。决策：用户拍板废弃 CORE-ATTEST（nonce/本地镜像路线），改用 WTF 式检查组路线。**
**状态更新（同日）：Phase 1（`--env-check`）与 Phase 2（`--env-check --bind-executor` 双开 tier-2）均已实施并通过全矩阵，见 docs/implementation-plan.md 更新日志；误报账目见 §5。**
**C6-IMPL 已归档（文末注明 superseded），其 L1（反录制重放）能力随换轨放弃，如实记录。**
**蓝本：`research/luraph-envcheck-analysis/luraph-envcheck.lua`（真 Luraph 环境校验载荷，1,131 行）。**

---

## 0. 一页结论

以 WTF 检查组为蓝本，**重实现其检查类别**（非逐字拷贝，出处原因），接入我们现有的绑定态守卫管线：检查通过 ⇒ 钥匙不变；任何一项失败 ⇒ 走既有代数 lane 静默投毒（不用 `error()` 固定出口）。分三档实施：纯 Luau 档（CLI 全程可测、可负例测试）→ Roblox/executor 档（绑定态）→ 版本敏感档（默认缓行）。

## 1. 与蓝本的差异（明确声明）

| 项 | WTF 蓝本 | 我们的做法 | 理由 |
|---|---|---|---|
| 代码来源 | 提取自商业产物 | **重实现检查类别** | 出处/授权 |
| 失败出口 | `error("WTF,son",0)` 固定分支 | **代数 lane 静默投毒** | R061：分支可定位可 patch |
| 错误文本 | ~60 条精确匹配 | **normalizeError 容差 + 后缀匹配**，精选 ≤10 条 | 52 号雷区减负 |
| 键序扫描 | 全表位置精确 | 仅键存在性 + 个别长效键 | 版本脆断维护成本 |
| 复检 | 一次性 | 入口捕获/结尾全等 + 周期轻复检（exguard 已有节律） | 已有资产 |

## 2. 检查组规格（按档）

### 档一：纯 Luau（CLI 可测，Phase 1）

| 组 | 检查项 | 失败管线 |
|---|---|---|
| buffer 全族 | 26 函数 C 身份（iscclosure/debug.info）；小端序（writeu16/32 落字节）；读写回环（i8..f64/string/copy/fill/readbits）；**越界必须报错**（status-only） | diff lane |
| 语义核心 | select('#') 精确 arity 含 nil 洞；闭包 per-iteration 捕获；元表 __index/__newindex/__add/__tostring/raw*；(-17%5)==3；floor(-1.25)==-2；bit32 定值 | diff lane |
| 协程 | create/status/resume/yield/close；106 轮 yield 稳定性；running 返回 thread | diff lane |
| 冻结表 | table.freeze 后突变必须报错（status-only） | diff lane |
| 编译 | loadstring 正常/语法错误形态（状态+类型，不验文本） | diff lane |
| 首尾身份 | 入口捕获 ~30 个纯 Luau 引用，结尾一次性全等 | diff lane |

**可测性关键**：此档全部检查在 CLI 能跑 ⇒ 正常产物进官方矩阵自动验证；负例用 mock 环境（砍 buffer 函数/换端序/错 bit32 值/新表代替缓存）必须静默垃圾——`tests/wtf_port_test.sh` 固化。

### 档二：Roblox/executor（绑定态，Phase 2）

| 组 | 检查项 |
|---|---|
| Enum 身份缓存 | FromName/FromValue 多次调用同一 userdata；EnumItem/EnumType typeof/锁定元表 |
| Random 确定性 | 固定种子精确浮点序列 + Clone 分叉 |
| task 语义 | spawn 返回 thread；delay+cancel；wait 参数校验 |
| 成员调用形态 | "Expected ':' not '.'"（≤3 条 + 容差） |
| 对象层 | game/DataModel、GetService、锁定元表、Connect/Disconnect 生命周期、销毁后 Parent 锁定 |
| executor 面 | clonefunction / isfunctionhooked / checkcaller（条件式，已有部分） |
| C 身份广度 | 首尾全等引用集扩到 ~60（含 buffer 家族） |

**测试形态**：绑定态产物只语法校验（现役规矩）+ mock 环境负例（faithful_stub/executor_guard 测试族扩档）。

### 档三：版本敏感（默认缓行，Phase 3 评估）

- 库表键存在性扫描（不做位置精确）；
- utf8 NFC/NFD 单向量字节精确；
- **不做**：Path2D 几何浮点、全量键序、60 条精确文本。

## 3. 接入点

- 档一：独立模块 `vmgen/envcheck.rs`（exguard 同族形态：HQ 加密碎片 + wire 发射 + 净化名），产出"检查差值"折入既有 SLT/MZD lane；默认管线不启用（H4 红线不动），随 `--bind-executor` 或新 `--env-check` 开关。
- 档二：并入 exguard 重档（周期复检节律复用）。
- 体积预算：档一 ~8–15KB，档二 ~10–20KB（估算，超预算砍组不砍容差）。

## 4. 验收（每 Phase 全套）

1. 开关关闭 ⇒ 与 HEAD 产物逐字节一致；
2. 干净 CLI/桩 全矩阵绿（档一可进 run_tests 自动验证）；
3. **mock 环境负例**：每检查组至少一个针对性 mock 必须静默垃圾且无错误特征；
4. cargo / 169 / 370 / multiseed / H 系列 / D 系列 / S1–S5 / 示例重生成；
5. 文档四处同步 + 提交推送。

## 5. 诚实边界（换轨后重申）

- **L1（录制重放）重新归零**：本方案全部为常量挑战，录制件一录全过——CORE-ATTEST 归档后无替代，将来若要补此层需重启该研究（文档保留）；
- 本方案对抗的是**仿真/mock 环境**（L2），与 exguard 现有面互补重叠，价值在广度（buffer/Enum/Random/task）；
- 同进程插桩与全状态观察（R004）不受影响；
- 版本敏感项的维护是持续成本，档三默认不做。

### 5.1 误报账本（Phase 1 实测新增，2026-10-11）

| 项 | 处置 |
|---|---|
| `coroutine.running()` 返回 thread | **移除**：CLI 主线程层返回 nil、Roblox 返回主线程——跨合法变体不恒定（52 类）；单测封禁 |
| 字符串键访问标识符字段（`rawget(t,"v")` 类） | **移除**：sanitize_frag 改名字段但字符串字面量不改，检查代码改名后语义脱节；单测封禁该模式 |

## 6. 阶段与工作量

| Phase | 内容 | 估计 |
|---|---|---|
| 1 | 档一纯 Luau 组 + envcheck.rs + 正负例测试 + 全矩阵 | 2–3 天 |
| 2 | 档二 Roblox/executor 组并入 exguard + mock 负例扩展 | 2–3 天（含真机验证等待） |
| 3 | 档三评估（可选） | 按需 |

## 7. 归档记录

- `C6-IMPLEMENTATION-PLAN.md`（CORE-ATTEST）：**已废弃**，2026-10-11 用户决策，superseded by 本方案。其 L1 能力（nonce 新颖查询 + 本地镜像，ROUND5 实证）未随本次换轨实施；研究证据链（ROUND4/5、c6b/c6c）保留，可随时重启。
