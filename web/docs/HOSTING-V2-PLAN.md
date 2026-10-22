# 脚本托管 v2：研究与实施计划

> 状态：Phase 1 已实施（见文末「实施记录」），Phase 2–6 待做。
> 目标：把 Obsucvim 的脚本托管从「能用的卡密系统」推进到「可日常运营的商业级托管」，并把控制台交互从「静态表单」升级为「有反馈、有节奏、有状态的界面」。

---

## 1. 研究方法

| 来源 | 用途 |
| --- | --- |
| `https://luarmor.mintlify.app/quickstart` | 项目 / 脚本 / 卡密的运营模型、上传选项、loader 分发方式 |
| `/scripting/runtime-vars` | 脚本内可读的运行时变量及其「可被 hook」的边界 |
| `/scripting/key-check-library` | 13 种 `check_key` 状态码语义、`purge_cache` |
| `/features/webhook-protection` | 服务端渲染的 webhook 模板、`%VAR%` 服务端变量、限流与体积上限 |
| `/features/source-locker` | 源码加密备份（默认不存源码） |
| `/api-reference/endpoint/script-management/get-update-progress` | **后台上传 + `track_id` 轮询**的进度模型 |
| `docs.luarmor.net`（v3 API） | 用户/卡密 CRUD、`unban_token`、API Key 详情/统计 |
| 本项目代码审计 | `web/tools/engine-server.mjs`（2.6k 行）、`web/src/pages/Scripts.tsx`（1.25k 行） |

**核心结论**：Luarmor 体验上的「顺滑」不来自功能数量，而来自三件事——

1. **一次点击后有明确进度**（上传 → 解析 → 混淆 → 完成，可轮询）；
2. **同一份 loader URL 永远不变**（我们已具备）；
3. **每个开关都有可验证的行为**（FFA / silent / heartbeat / lightning 都不是装饰）。

我们的差距集中在第 1 与第 3 项，以及前端**缺少状态反馈与动效**。

---

## 2. 能力对照表

图例：✅ 已有并验证 · 🟡 部分 / 有边界 · ❌ 缺失 · ⛔ 明确不做

| 能力 | Luarmor | Obsucvim 现状 | 差距与处理 |
| --- | --- | --- | --- |
| 项目文件夹（一键多脚本） | ✅ | ✅ 文件夹 + 卡密归属，跨文件夹 403 | 已对齐 |
| 稳定 loader URL | ✅ | ✅ `/files/<scriptId>.lua` | 已对齐 |
| 后台上传 + 进度轮询 | ✅ `track_id` | ❌ 同步阻塞，前端只转圈 | **Phase 1 实施** |
| FFA（免卡密） | ✅ | ✅ 服务端判定，`OBS_IsPremium` 区分 | 已对齐 |
| 运行时变量 | ✅ 7 个 | 🟡 5 个（premium/note/scriptName/secondsLeft/executions） | Phase 3 补 `version`、`discordId`、`expiresAt` |
| Key Check 库（13 状态码） | ✅ | 🟡 只有 `/api/key/status` + 9 个错误码 | Phase 4 对齐状态码命名 |
| HWID 自助重置 + 冷却 | ✅ | ✅ 项目级开关 + 冷却 | 已对齐 |
| 封禁（临时/永久/设备级/token 解封） | ✅ | ✅ 且含设备黑名单 | 已对齐 |
| 日志 / 告警 Webhook | ✅ | 🟡 仅 https + Discord 域白名单 | Phase 4 补签名与重试 |
| Webhook 保护（服务端渲染 + 脱敏） | ✅ | ❌ | Phase 5（需服务端模板引擎） |
| silent / heartbeat / lightning | ✅ | ❌ 仅存字段，无行为 | **Phase 1：heartbeat 落地；silent 说明为无输出可静默；lightning 评估后暂不做** |
| 卡密 JSON 导入导出（迁移） | ✅ | 🟡 仅 CSV 导出（无明文 Key） | **Phase 1：JSON 导出 + 导入** |
| 源码保险箱 | ✅ | ⛔ 有意不做（默认不存原文） | 已写进产品边界 |
| Discord 机器人 / 广告奖励 | ✅ | ⛔ 用户已取消 | 已标注 |
| 多节点 / 外部验卡 API | ✅ | ❌ 单机 | Phase 6 评估 |
| 数据库 | ✅ 分布式 | 单 JSON 文件 | Phase 6（见风险） |

---

## 3. 设计原则

1. **诚实边界**：任何开关必须能说清「它到底做了什么」，做不到的不放进 UI。
2. **失败可恢复**：上传失败不扣额度、不留下半成品；进度状态可查询、可重试。
3. **不引入明文密钥**：导出永远不含卡密原文，导入只允许生成新卡密。
4. **动效服务于状态**：动画只用来表达「正在发生什么 / 刚刚发生了什么」，不做无意义装饰；全站尊重 `prefers-reduced-motion`。
5. **零新增依赖**：服务端继续单文件 Node，前端不引入动画库（CSS + 原生实现）。

---

## 4. 实施阶段

### Phase 1（本轮）：可观察的上传 + 有意义的开关 + 基础动效

**后端**

- `POST /api/publish` 改为**异步作业**：
  - 校验 → 预留额度 → 返回 `202 { jobId, stage: "queued" }`
  - 后台推进 `injecting → obfuscating → storing → completed`
  - 失败：释放额度、状态 `failed` + 错误码，不写任何产物
  - 并发上限 2/账号；作业 10 分钟过期；`GET /api/auth/publish/:jobId` 轮询
- `POST /api/auth/scripts/update` 新增 `heartbeat`（布尔）
- loader 模板注入 `__HEARTBEAT__`；开启时在后台每 ~90s 重新校验，失效则更新 `OBS_License.valid = false`、调用 `obsucvim_on_heartbeat_fail` 钩子并 `warn`
- `GET /api/auth/licenses/export.json`（仅元数据）与 `POST /api/auth/licenses/import`（生成新卡密，返回明文一次）

**前端**

- 发布流程改为「提交 → 轮询 → 进度步进器」，四段动画；失败就地展示错误并允许重试
- 新增 `Toast`（成功/失败，滑入 + 自动消失）、`Skeleton`（骨架微光）、`AnimatedNumber`（统计数字滚动）
- 卡密表 / 项目卡列表错峰入场；复制按钮成功态形变；弹窗进出场
- 全站 `prefers-reduced-motion` 降级

**验收**：详见 §7。

### Phase 2：多脚本项目视图与批量运营

- 文件夹内脚本矩阵（一次看到所有脚本的 FFA/版本/最近发布）
- 批量发布：同一份源码推到文件夹内多个脚本（每脚本独立产物，独立扣额度）
- 卡密批量操作：批量封禁/解封/重置设备/改到期（异步作业 + 进度条，沿用 Phase 1 的 job 模型）
- 上传历史：谁在什么时候发布了什么版本（审计日志，含 IP 哈希与耗时）

### Phase 3：运行时变量与脚本内集成

- 变量补齐：`OBS_ScriptVersion`、`OBS_ExpiresAt`、`OBS_DiscordId`（可选绑定）、`OBS_FolderName`
- 提供 `obsucvim` 标准库片段：`check_key` / `load_script` / `purge_cache` 三个方法，语义对齐 Luarmor
- 补齐 13 种状态码：`KEY_INVALID`、`SCRIPT_ID_INVALID`、`SCRIPT_ID_INCORRECT`、`INVALID_EXECUTOR`、`SECURITY_ERROR`、`TIME_ERROR`、`UNKNOWN_ERROR`
- 状态码文档页 + 前端 `#key` 页展示对应人话

### Phase 4：通知与可观测性

- Webhook：HMAC 签名、失败重试（指数退避，最多 3 次）、投递记录页
- 指标：执行数/验卡数/失败率/独立设备数，按日聚合（存 `state.metrics`）
- 告警规则：短时间内大量 `KEY_INCORRECT`、同一 IP 多账号、异常 HWID 复用
- 控制台「今日概览」卡片组

### Phase 5：Webhook 保护（脚本内发送）

- 脚本内宏 → 服务端渲染模板 → 服务端持有真实 webhook URL
- `%CLIENT_IP%` `%COUNTRY_CODE%` `%USER_KEY_HASH%` 等服务端变量
- 脱敏：正则白名单（客户端传入值必须匹配开发者的正则）
- 限制：30 req/min/IP、3 embeds、7 KB payload（与 Luarmor 对齐）

### Phase 6：规模化与部署

- 存储抽象：`state.json` → SQLite/Postgres（接口不变，先做 repository 层）
- 产物存储：本地目录 → S3 兼容对象存储（可选）
- 多实例：共享状态 + 粘性会话 / 无状态 JWT
- 外部验卡 API（SHA1 签名 + 时间同步 + 多节点），供第三方应用使用

---

## 5. 接口契约（Phase 1）

### 5.1 异步发布

```
POST /api/publish
{ "scriptId": "scr_...", "version": "v3", "source": "...", "options": {...} }
→ 202 { "ok": true, "jobId": "pub_...", "stage": "queued", "quota": {...} }
→ 409 { "ok": false, "error": "PUBLISH_BUSY" }        # 同账号并发超限
→ 429 { "ok": false, "error": "QUOTA_EXCEEDED" }
```

```
GET /api/auth/publish/:jobId
→ 200 {
    "ok": true,
    "status": "processing" | "completed" | "failed",
    "stage": "queued" | "injecting" | "obfuscating" | "storing" | "completed",
    "progress": 0..100,
    "elapsedMs": 1234,
    "result": { "scriptId", "version", "sha256", "size" },
    "error": "ENGINE_TIMEOUT" | ...
  }
```

**阶段与进度**：`queued 5` → `injecting 20` → `obfuscating 65`（停留到完成）→ `storing 90` → `completed 100`。

### 5.2 卡密导出 / 导入

```
GET /api/auth/licenses/export.json?folderId=fld_...
→ { "ok": true, "folderId", "exportedAt", "count", "licenses": [
     { "licenseId", "label", "createdAt", "expiresAt", "keyDays",
       "revokedAt", "bound": true, "lastUsedAt", "executions", "ban": {...} } ] }
   # 永不包含卡密原文
```

```
POST /api/auth/licenses/import
{ "folderId": "fld_...", "entries": [ { "label", "keyDays"|"expiresAt", "note" } ] }
→ 201 { "ok": true, "count": n, "licenses": [ { "licenseKey": "osk_...", "license": {...} } ] }
```

### 5.3 heartbeat

`POST /api/auth/scripts/update { scriptId, heartbeat: true }` → 落盘；`/files/<id>.lua` 下次请求即输出带心跳的 loader。

心跳行为（loader 内）：

- 首次校验通过后，若 `HEARTBEAT` 为真，尝试 `task.spawn`（无则 `coroutine`）起每 90s 的复核；
- 复核失败：把 `OBS_License.valid` 置 false、写 `OBS_License.reason`，调用 `obsucvim_on_heartbeat_fail(reason)`，并 `warn`；
- **明确限制**：客户端无法可靠地杀死已启动的线程/协程，心跳只能「通知 + 让脚本自行退出」，不能冒充服务端强制断线。UI 与文档都这么写。

---

## 6. 动效系统

统一放在 `web/src/index.css`，全部 CSS keyframes，无第三方库。

| 名称 | 触发 | 时长/曲线 | 用途 |
| --- | --- | --- | --- |
| `enter-page` | 路由切换 | 420ms `cubic-bezier(.22,1,.36,1)` | 页面淡入 + 上移 8px |
| `stagger-item` | 列表挂载（`--i` 变量控制延迟） | 380ms，每项 +45ms | 卡密行 / 项目卡 / 版本列表错峰 |
| `shimmer` | 骨架屏 | 1.4s 无限 | 加载占位 |
| `toast-in` / `toast-out` | 通知 | 260ms / 180ms | 右下角滑入 + 淡出 |
| `step-pulse` | 进行中的步骤 | 1.6s 无限 | 进度步进器当前节点呼吸 |
| `step-fill` | 步骤完成 | 320ms | 进度条填充 |
| `pop-check` | 复制成功 | 220ms | 图标缩放 + 对勾描边 |
| `modal-in` | 弹窗 | 240ms | 背景模糊 + 面板上浮 |
| `count-up` | 统计数字 | 由 JS 逐帧插值 | 数字滚动（`AnimatedNumber`） |
| `hover-lift` | 卡片悬停 | 180ms | `translateY(-1px)` + 边框提亮 |

`@media (prefers-reduced-motion: reduce)` 下全部 `animation: none !important`，数字直接跳到终值。

---

## 7. 测试计划

**后端（可在沙箱真实执行）**

1. 异步发布会话：注册 → 建项目 → `POST /api/publish` → 轮询直到 `completed`，`/api/releases/fetch` 能取到产物且 SHA-256 与 job 结果一致。
2. 失败路径：超长源码 → `failed` 且额度未扣；引擎超时 → `failed` 且无产物文件。
3. 并发：同账号第二个作业 → `409 PUBLISH_BUSY`（上限 2）。
4. 越权：他人 jobId → 404；未登录 → 401。
5. 作业过期：伪造 10 分钟前启动 → 轮询返回 `failed / JOB_EXPIRED`。
6. 导出：CSV 与 JSON 均不含 `osk_` 明文；导入生成的卡密能通过 `/api/licenses/validate`。
7. heartbeat 字段：`scripts/update` 写入后 `/files/<id>.lua` 含心跳代码；关闭后不含。
8. loader/guard 回归：`python3 web/tools/test-loader.py`（9 项）全绿，新增心跳用例。

**前端**

- `tsc --noEmit`、`vite build`
- 静态校验：骨架/Toast/步进器在构建产物中出现；`prefers-reduced-motion` 规则仍在
- **未验证**：真实浏览器点击流（沙箱无浏览器），只做构建与静态断言

**打包**

- `web/tools/rebuild-package.sh` 重建 → 解压 `sha256sum -c SHA256SUMS.txt` 全通过 → 推送 GitHub

---

## 8. 风险与边界

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| 单 JSON 状态文件 | 高并发写丢失、无法多实例 | Phase 6 迁移数据库；当前 `saveState()` 为同步全量写，作业状态只在内存 |
| 混淆耗时 ~1–3s/次 | 同步请求易超时 | Phase 1 异步化后彻底规避 |
| 心跳无法强杀线程 | 用户可屏蔽 | 文档明示；关键逻辑用「心跳后再次校验才继续」的保守写法 |
| 明文卡密只在签发时出现 | 用户没保存就找不回 | 已有提示 + 可重置设备 + 可作废重签 |
| 动画过多影响低端机 | 卡顿 | 只做 transform/opacity，尊重 reduced-motion |

---

## 9. 实施记录

### 2026-10-22 · Phase 1（已完成并推送）

- ✅ 异步发布作业：`POST /api/publish` → `202 { jobId }`，`GET /api/publish/:jobId` 轮询
  - 路线 `/api/publish/<jobId>`（认证在路由内做；他人作业与不存在作业同为 404）
  - 额度提交时预留、失败退还；同账号并发上限 2；作业 10 分钟过期
- ✅ `heartbeat` 开关：落盘 → loader `local HEARTBEAT = true` → 每 90s 复核；失败写 `OBS_License.valid=false` 并调用钩子
- ✅ 卡密 JSON 导出（无明文）与导入（整批校验、只生成新卡密）
- ✅ 前端：发布步进器（5 段 + 进度条 + 用时）、Toast、骨架屏、数字滚动、列表错峰、卡片悬停、弹窗进出场、复制成功形变
- ✅ `prefers-reduced-motion` 全覆盖（两段媒体查询 + 组件内数字直接跳终值）
- ⏸ `silent`：我们的 loader 不打印任何内容，**没有可静默的输出**；字段保留、UI 明说是「无输出可静默」，不做假开关
- ⏸ `lightning`：会削弱校验强度，评估后**不做**（收益不足以抵消风险）

**实测证据（真实执行，非推断）**

```
web/tools/test-hosting.mjs   27/27 passed
web/tools/test-loader.py     13/13 passed
```

`test-hosting.mjs` 覆盖：作业创建与完成、阶段上报、100% 进度、产物 SHA-256 与 release 一致、
并发上限（2 接受 1 拒绝）、坏源码 `ENGINE_FAIL` 且额度不变、失败不落产物、未知作业 404、
未登录 401、JSON 导出不含 `osk_`、导入的卡密可校验、整批校验、heartbeat 开关进出 loader。

`test-loader.py` 覆盖：userdata/table 两种 `game`、缺 game 报错、封禁原因、缺 key 提示、
`obsucvim_on_missing_key` 钩子、guard 正反用例、FFA 无 token、心跳复检/失效标记/钩子调用/关闭时不复检。

**未验证**：真实浏览器里的点击与动画（沙箱无浏览器），仅做构建 + 静态断言（CSP 哈希、类名存在）。

### 后续

- Phase 2–6 按 §4 顺序推进；每阶段结束同步 `SCRIPT-KEYS.md` 与本文档。
