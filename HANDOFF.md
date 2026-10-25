# 交接文档（HANDOFF）

> 给**从未参与过本项目**的新人或 AI。读完本文 + [`docs/CURRENT-STATE.md`](docs/CURRENT-STATE.md)
> 即可开工。设计细节在 `docs/`，本文只讲「是什么、规矩、环境、怎么验收、哪里有坑」。
>
> 最后更新：2026-10-25（仓库整理）。整理前的完整交接文档（77 KB，含逐轮前序流水）
> 逐字保存在 [`docs/archive/HANDOFF-history-2026-10.md`](docs/archive/HANDOFF-history-2026-10.md)。

---

## 1. 项目是什么

**obsucvim**：商业级 **Luau 混淆器**（对标 Luraph）。核心 = 自研私有字节码 VM +
随产物分发的「Lua 写的加密解释器」；本体用 **Rust（std-only，零第三方 crate）** 编写。
外围还有 React 控制台、Node 托管服务（账户/卡密/双产物发布/验卡）与 Discord `/obf`。

- 产品线 **Luau-only**（Lua 5.1 目标已于 2026-09-10 停止支持）
- 当前引擎 **R079**，锚点与体积见 `docs/CURRENT-STATE.md` §2
- **引擎改进已按用户指令收摊（2026-10-25）**：不要再自行开新战场（见 §3）

---

## 2. 用户的硬性要求（必须遵守）

1. **不要擅自扩范围**：用户点名做什么就做什么；引擎层面的新改进已被 §34 收摊令冻结。
2. **每次改混淆模块后的强制工作流**：
   - 跑 **官方矩阵** `engine/tests/run_tests.sh`；改 `engine/src/vmgen/` 必须加
     **多种子回归** `engine/tests/multiseed.sh`（opcode 置换/槽位是每构建随机，
     固定 seed 42 测不出置换面回归）；
   - 改发射面（vmgen）还要跑安全指纹 `engine/tests/security_fingerprint.py` 与
     `engine/tests/defense_audit.sh`（安全项只许逐阶段转绿，不许回红）；
   - 语义等价判据：混淆产物的 stdout + 退出码与源码**逐字节一致**；
   - 改 pass 必须重生成 `engine/examples/`（`engine/tests/gen_examples.sh`）；
   - **同步更新相关 md**（`engine/ENGINE-VERSION.md` 台账、`docs/CURRENT-STATE.md`、
     必要时 `docs/README.md` 索引与 `engine/examples/README.md`）；
   - **commit + push 到当前工作分支**（勿 force push，勿动 `main`）。
   - 任一项不通过 = 该改动未完成，**不许宣称完成**。
3. 混淆器本体 = **Rust**，不要用其他语言重写本体。`artifacts/legacy-lph/`
   里是早期 Lua 参考实现，**仅存档不复用**。
4. **诚实口径是硬要求**：不宣称「不可破解 / 绝对安全」；每条结论给可复现证据
   （命令、体积、sha256、测试计数）；未验证项（真机、部署、CI）必须标明「未验证」。
   这是项目历史上多次纠偏的根因，务必保持。
5. 调研结论写进 `docs/`，用户会查。

---

## 3. 现在不要做的事（2026-10-25 停止令）

| 事项 | 状态 |
| --- | --- |
| 引擎新改进（P2 时钟 / P3 镜像 / P4 懒解层 / P5 相窗三） | **已取消**（§34 收摊） |
| PW-1「径织」载体层 | **已删除**，勿复活（恢复点 git `710005f`/`c2b91ea`/`026469d`） |
| V19 M1（R6+R3+R5）、EVKEY 翻默认 | **待用户拍板**，未拍板不动 |
| rustc 正牌重编 wasm 刷锚 | 通道不可用，已封存（恢复后必须做，见 §4） |

---

## 4. 环境

### 本仓库沙箱（Arena）实际状态 —— 2026-10-25 实测

- **没有** `cargo` / `rustc`；**没有** `.tools/bin`（`lua51`/`luau`/`luau-compile` 全缺）；
  `web/node_modules` 未安装。
- **能跑**：WASI 引擎与锚点自检：
  ```bash
  node web/tools/verify-engine-version.mjs        # → verdict: R079
  node web/tools/wasi-runner.mjs '{"wasmPath":"'"$PWD"'/web/tools/obsucvim.wasm",
    "workDir":"/tmp/work","args":["--seed","42","/work/in.lua","/work/out.lua"]}'
  ```
  （实测：`print("hello")` → 171,430 B 产物；同 seed 两次 sha256 完全一致。）
- 因此**引擎矩阵与控制台套件在本沙箱跑不了**，需要工具链的宿主环境。

### 有工具链的环境（原设计环境）

```bash
# 工具链在仓库内（gitignored）：engine/../.tools/bin
/home/user/obsucvim/.tools/bin/{lua51,luac51,luau,luau-compile,rustc,cargo}
export PATH=/home/user/obsucvim/.tools/bin:$PATH     # 必须先导出：cargo 靠 PATH 找 rustc
cd engine && CARGO_NET_OFFLINE=true cargo build --release
```

重建 `.tools` 的完整步骤见 `docs/archive/HANDOFF-history-2026-10.md` §4
（核心：Rust 工具链来自 npm 的 `@rustbin/*`，因为 `static.rust-lang.org` / `crates.io`
不可达；Lua/Luau 从源码或预编译包取得）。

**网络限制（踩过的坑）**：可达 github/npm/pypi；不可达 `static.rust-lang.org`、
`crates.io`、apt、GitHub release CDN。⇒ **Rust 依赖必须 std-only**，这是设计约束不是缺点。

### WASM 引擎

`web/tools/obsucvim.wasm` 是构建产物（当前 R079，`95f69c99…`）。
重编：`web/tools/build-engine-wasm.sh`（编译 + 锚点校验 + 安装，锚点不符拒装）。
CI 模板在 `web/tools/ci/build-engine.yml`（`.github/workflows/` 无写权限，需手工复制）。

---

## 5. 仓库地图（详见 `docs/README.md`）

| 路径 | 内容 |
| --- | --- |
| `engine/` | Rust 引擎（`src/vmgen/` = L6 私有 VM）、测试矩阵、示例 |
| `web/` | 控制台 + 托管服务 + 引擎 WASM + 验证套件（**部署契约路径，勿改名**） |
| `docs/` | `CURRENT-STATE.md` + `engine/` `hosting/` `redteam/` `research/` `archive/` |
| `artifacts/` | `targets/`（历轮靶标）、`samples/`、`redteam/`、`legacy-lph/` |
| `services/discord_obf/` | Discord `/obf` 适配（未部署） |
| `release-assets/` | Windows 发布包（**部署契约路径，勿移动**） |

---

## 6. 规矩与坑（前人踩过的，仍然有效）

**语义红线（Luau 目标）**
1. Luau CLI 不支持 `-e`：验证一律写临时 `.lua` 文件再跑。
2. 官方 luau CLI 的全局表**只读**（`luaL_sandbox`）：语料里顶层**新建**全局赋值会
   `attempt to modify a readonly table`。
3. 多值与变参必须完整保留：`return ...` / `a,b = ...` / `a,b = f(...)` / `{f()}`
   尾部展开；多余值「求值即弃」（副作用必须保留）。
4. `return f(...)` 尾调用语义不得被扁平化拆掉。
5. `local function f() return f() end`：`f` 在自身体内**不可见**（必须有测试覆盖）。
   Luau 的 `local function` 递归需显式前置声明。
6. 浮点输出用 `%.17g`；Luau 下整数值浮点补 `.0`。
7. 所有随机性走 `--seed` PRNG：同 seed 逐字节一致，异 seed 编码完全不同（验收项）。

**工程纪律**
8. `engine/src/` 的注释已于 2026-10-11 整体剥离（3,155 条）——**改代码时不要惊讶于无注释**；
   设计看 `docs/engine/`。剥离工具 `engine/tools/strip_rs_comments.py`（Rust 感知，
   勿用 sed：字符串里的 `//` 是 Luau 整除运算符）。
9. 绑定态产物（`--bind-env` / `--bind-executor` / `--env-check`）**不进运行等价矩阵**：
   只做语法校验 + 裸 Luau「干净拒绝」验证。矩阵/示例生成一律 `--no-bind-executor`。
10. `--no-key-host` 已在 R070 移除（恒开）；`--key-host` 保留但传与不传等价。
11. 调试 VM 三板斧（完整工具箱见 `docs/engine/vm-l6-implementation.md` §6）：
    `OBSUCVIM_VM_RAW=1` 出未混淆解释器 → 在分派出插入 `io.write` 追踪
    （跨行拼接按整行替换，别用 index 硬拼）→ 用 `--no-flatten/--no-strings/...` 二分。
12. 守卫/探针相关改动必须过「**净化后形态**」检查：净化器会改标识符但**不改字符串字面量**，
    外部对象成员只能 `["字符串键"]` 访问。
13. Shell 陷阱：`/tmp/t_$s_$d.lua` 会把 `$s_` 当变量名 —— 用 `${s}_${d}`。
14. 工作分支以会话说明为准，**别动 `main`**，勿 force push。

---

## 7. 验收标准（「商业级」的落地定义）

1. 全语料 × 全预设 × 目标解释器：stdout + 退出码 100% 一致。
2. 产物通过 `luau-compile` 语法校验 0 错误。
3. 同 `--seed` 两次构建逐字节一致；异 seed 编码完全不同；**多种子回归 ≥5 seed 0 失败**。
4. VM 预设产物：标准反编译器/格式化器无法恢复源码结构（人工抽查）。
5. 反篡改生效：篡改任一密文段 → 触发陷阱。
6. 引擎锚点逐字节可复现（`verify-engine-version.mjs`），换版必须刷新锚点与文档。
7. 托管侧：八套件全绿（数字见 `docs/CURRENT-STATE.md` §7）。

---

## 8. 开工顺序（新任务进来时）

1. 读 `docs/CURRENT-STATE.md` —— 确认要动的东西是否已被停止令覆盖。
2. 若是引擎改动：先 `git log -1` 确认基线，读 `docs/engine/ENGINE-VERSION.md` 末尾几节，
   在 `engine/` 改，按 §2 走完整工作流。
3. 若是托管/控制台改动：读 `docs/hosting/`，在 `web/` 改，跑对应套件。
4. 若只是文档/整理：改完更新 `docs/README.md` 索引，别把状态抄进第二份文档。
