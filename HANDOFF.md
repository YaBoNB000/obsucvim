# 交接文档（HANDOFF）

> **仓库现在只有混淆引擎。** 2026-11-15 第二次规整：除本文件外不再保留任何 markdown；
> 控制台（`web/`）、托管服务（`services/`）、研究数据（`docs/`）、证据链（`artifacts/`）、
> 发布包（`release-assets/`）与 Render/Docker 部署文件已从工作树删除。测试仍需的 4 个靶标
> 已移入 `engine/tests/fixtures/`。旧文件恢复记录见 §10；当前 shallow checkout 不含旧基线 `68b9187` 对象。
>
> 最后更新：2026-11-22。§2 和 §12 保留 2026-11-15 的历史基线；Delta 诊断见 §13，本轮移除 executor binding/key-host/EVKEY 的现状与验证见 §14。
> 当前工作分支与 PR 状态以 Arena 会话说明为准；此 checkout 是 **shallow clone**，分支基线为 `aeca245`，本轮工作起点为 `44696d5`；不要依赖更早历史。

---

## 1. 项目是什么

**obsucvim**：Luau / Roblox 混淆引擎。Rust 把输入脚本编译成私有字节码，再由随产物分发的
「Lua 写的加密解释器」执行；可选 v15 结构克隆、通用 Luau 环境检查、反调试 guard 与 `--bind-env` 目标 API 绑定。
executor 认证绑定、key-host 转录链和 EVKEY 路径已移除（详见 §14）。

- **Luau-only**：Lua 5.1 目标自 2026-09-10 停止支持，`--dialect 5.1` 直接报错。
- **引擎本体 Rust，std-only，零第三方 crate**（`engine/Cargo.toml` 无依赖）。沙箱到不了
  crates.io，这是硬约束，不是疏忽。
- 仓库内**没有** WASM 发布件、Web 控制台、托管服务或部署配置；只剩 CLI 引擎、语料、示例、
  测试矩阵与工具链重建脚本。

---

## 2. 当前实测状态（2026-11-15）

| 项 | 值 |
| --- | --- |
| 原生二进制 | `engine/target/release/obsucvim`，**1,998,688 B** / `b1988fc669c8f52cc45ef899e8493b7d0b4c0efa665c69aa92352f470e082c03`（2026-11-22 重建） |
| 构建 | Rust 1.88.0，`cargo build --release` 离线成功（约 30 s 增量） |
| 单元/集成测试 | `cargo test --release`：**65 passed / 0 failed / 1 ignored**（61+4） |
| 官方矩阵 | `bash engine/tests/run_tests.sh` → **PASS 169 / FAIL 0**（2026-11-22，约 110 s；含新增 h9 密封闸） |
| VM 专项 | 37 语料 × `--vm`（字符串密封默认开）× run + `luau-compile` → **74 / 74 PASS** |
| 根样例锚 | `print(6)` + `--preset v15 --dialect luau --seed 42` → `example.txt` **149,118 B** / `9ceb3202cf050587ed3074398cb871fa45369d74464f47de6b0e9af40aa4c783` |
| 水印 | 每个产物第一行固定 `-- This file was obfuscated by Obsucvim.`（普通注释） |
| 加固复核 | 12 项外部改进方案逐条实测：9 项早已落地、1 项设计不适用、1 项（W3 假密封）本轮修复（见 §15） |

**确定性**：同一 `--seed` 连跑逐字节一致；不同 seed 全量重随机。

**v15 产物不带字符串密封时与旧锚点逐字节一致**；本轮只改了 legacy VM 的 LoadStr 解码
handler，v15 发射路径未动，所以上述两个锚点与整理前相同。

### 2.1 本轮修掉的缺陷：legacy VM 字符串密封解码失配

现象：`--vm`（默认 `--vm-seal-strings`）产物在 Luau 下把字符串读成垃圾字节（`basics`
第 6 行的 `"x"` 变成单字节乱码），37 个 VM 语料全红；加 `--no-vm-seal-strings` 则 74/74 全绿。

根因：`engine/src/vmgen/strseal.rs::encode` 在 V19 R5-b 已改为
「NL-65521 非线性折叠 + 明文字节级联」，v15 模板（`template.rs` 的 `M_LOAD_STR`）也同步镜像；
但 legacy VM 使用的 `engine/src/vmgen/handlers/loadstr.rs` 仍是旧式 `st*257+d+i` + 低 8 位掩码，
既缺 `+1`，也缺前一字节日志 `_pv`，还不是 NL 折叠。单字节字符串也会立刻错。

修复：把 handler 的 Lua 解码文本改成与 `encode`/v15 镜像同式（含 `_pv`、`+1`、两次 NL 折叠）。
修复后 `--vm` 全语料通过，且 v15 输出字节不变。**以后改 `strseal` 必须同时改这三处镜像：
Rust encode/decode、v15 micro-op、handlers/loadstr.rs。**

---

## 3. 仓库布局（规整后）

```
engine/                 Rust 引擎（CLI + 私有 VM）+ 语料 + 示例 + 测试
  Cargo.toml            零依赖；release: opt-level=3 + lto
  src/                  73 个 .rs，约 2.05 万行（注释 2026-10-11 整体剥离）
  src/vmgen/            compiler / template / v15 / isa / envcheck /
                        fuse / strseal / strpool / handlers（43 个指令 handler）
  tests/                37 个语料 + 矩阵/审计/多种子/红队探针
    fixtures/targets/   测试用 4 个历史靶标（R035/R062/R064），从原 artifacts 移入
    cases/              37 个语义语料
    run_tests.sh        官方矩阵入口
  examples/             37 个 v15 混淆示例（改 pass 必须重生成）
  tools/                rebuild_tools.sh + luau CLI 主程序源码（main_luau*.cpp）
HANDOFF.md              本文件（唯一文档）
.gitignore              忽略 .tools/ 与 engine/target/
```

没有 `web/`、`services/`、`docs/`、`artifacts/`、`release-assets/`、`render.yaml`、
`.dockerignore`。所有路径均相对仓库根，不再依赖 `/home/user/obsucvim` 之外的目录。

---

## 4. 构建与运行

### 4.1 工具链（沙箱重置后先跑这个）

```bash
bash engine/tools/rebuild_tools.sh          # 默认装到 .tools/（gitignored）
# 或指定目录：
OBSUCVIM_TOOLS=/tmp/obsucvim-tools bash engine/tools/rebuild_tools.sh
```

脚本做三件事，全部可离线缓存后复现：

1. Rust 1.88：`npm pack @rustbin/{rustc,cargo,rust-std}-1.88.0-x86_64-unknown-linux-gnu`，
   解包后 `install.sh --prefix=.tools` 并合并 std。
2. Lua 5.1.5：GitHub 镜像源码，`make generic` → `lua51` / `luac51`。
3. Luau 0.735：codeload 源码 + 仓库内 `main_luau.cpp` / `main_luau_compile.cpp`，
   `g++ -O2 -std=c++17 -DLUA_USE_LONGJMP=1 '-DLUA_API=extern "C"'` 直编 7 个库。

依赖：`npm`、`curl`、`make`、`g++`、`tar`。约 2 分钟。结束后打印
`rustc 1.88.0`、`Lua 5.1.5` 与 `toolchain ready`。

> `main_luau.cpp` **默认不加** `luaL_sandbox`；只有设 `LUAU_READONLY_GLOBALS=1` 才启用只读全局。
> 官方 CLI 的 sandbox 会让所有写全局的产物报 `attempt to modify a readonly table`，不要默认开。

### 4.2 引擎

```bash
export PATH="$PWD/.tools/bin:$PATH"
cd engine
CARGO_NET_OFFLINE=true cargo build --release    # → engine/target/release/obsucvim
cargo test --release
```

`engine/target/` 与 `.tools/` 都在 `.gitignore` 里，**不会随工作区快照保留**；重置后必须重跑
§4.1 与本节。测试脚本优先找 `/home/user/tools/bin`，不存在则回落仓库 `.tools/bin`。

### 4.3 WASM

**本仓库不再包含 wasm 发布件，也没有安装 `wasm32-wasip1` std**，所以
`cargo build --target wasm32-wasip1` 在当前沙箱不可执行（未验证）。不要参考旧文档里的
`web/tools/*.mjs` 路径——那些文件已删除。

---

## 5. CLI 用法

零参数即产品形态（v15）：

```bash
./target/release/obsucvim --seed 42 in.lua out.lua
# 等价于
./target/release/obsucvim --preset v15 --seed 42 in.lua out.lua
```

| 预设 | 内容 | 说明 |
| --- | --- | --- |
| 默认 = `v15` | v15 全防护 | 产品形态 |
| `low` | L1 + L2 | 名称/字符串不可读 |
| `medium` | low + L3 | 控制流打散 |
| `high` | medium + L4/L5/L7 | 整段密文 + 反篡改 |
| `vm` | high + L6 | legacy 私有 VM |
| `max` | 当前 = `vm` | legacy 最强档 |

常用旗标：`--env-check`（通用 Luau tier-1）、`--bind-env roblox`、`--guard` / `--no-guard`、
`--super-insn`（默认关）、`--vm-seal-strings` / `--no-vm-seal-strings`，以及
`--no-mangle/-minify/-strings/-flatten/-junk/-numbers/-body/-antidbg`。

- `--env-check` 仅包含通用 Luau 语义挑战；Roblox/executor 专用 tier-2 已随绑定路径移除。
- `--bind-env` 是独立的目标 API 绑定功能，产物只做语法校验与目标环境绑定验证；普通 v15 与 `--env-check`
  产物可在 stock Luau 运行。默认矩阵/示例不再传 executor-binding 开关。
- executor binding、key-host 和 EVKEY 相关 CLI/运行时代码已移除，不再提供旧开关或兼容路径（详见 §14）。
- Luau 输入支持 `//`、`continue`、复合赋值、反引号插值、类型注解（解析期剥离）。
- 多值/变参、`return f(...)` 尾调用、`local function` 递归可见性都有语料覆盖。

---

## 6. 验证矩阵（改引擎前后必看）

```bash
cd engine
cargo build --release
bash tests/run_tests.sh        # 官方矩阵：37 语料 ×（默认 v15 + --vm）+ 专项套件
bash tests/run_presets.sh      # 预设 × 全语料
bash tests/multiseed.sh        # 改 vmgen/ 必跑：1 7 4242 31337 999999
bash tests/defense_audit.sh    # 防御审计 D1–D20
bash tests/gen_examples.sh     # 改 pass 必跑，重生成 examples/
python3 tests/security_fingerprint.py
python3 tests/v15_fingerprint.py
```

语义判据：混淆产物的 **stdout + 退出码**与源码逐字节一致（Luau 下）；绑定态产物按
「干净拒绝」判定；超时（124）一律 FAIL。任何改动必须：矩阵 + 多种子 + 指纹/审计 +
重生成示例 + 刷新锚点 + 更新本文件。

**2026-11-15 实跑结果**

- `cargo test --release`：61 + 4 全绿，0 failed，1 ignored。
- `tests/run_tests.sh`：**PASS 169 / FAIL 2**，两个红项见 §7。
- VM 专项（37 语料 × `--vm`，密封默认开）：run 37/37 + `luau-compile` 37/37。
- 标准语料 + 示例一致性包含在矩阵与 `wtf_port_test.sh` 中；改 v15 发射后必须重生成示例，
  否则 `flag-off parity` 会红。

**已知红项（不是本轮整理造成的回归）**

1. `exguard/E4`「可见层零痕迹」：绑定态产物命中 10 个痕迹词。
   EVKEY 探针（`template.rs` 的 `EvKeyMode::Bind` 文本）和 executor 前置自检的 `_miss`
   诊断串仍以明文列出 `identifyexecutor/getgenv/checkcaller/iscclosure/…`。
   这是 R075 引入 EVKEY 后的真实硬化缺口，不是清理能修的；修它要改默认 v15 发射字节并重钉
   所有锚点/示例，本轮未做。**不要对外宣称「可见层零痕迹」。**
2. `wtf-port/tier2` 的 `destroy-lock` 负向探针恒红（`difference stayed 0`）：mock 已实现
   `_destroyed` 开关，但 ENVFRAG 里没有对应的 destroy-lock 检查组，属于测试夹具未接通；
   其余 19 项绿。补这个组等于新增守卫检查，超出「整理 + 可用性验证」范围，未做。

---

## 7. 已知问题与限制

1. **E4 明文痕迹**（见 §6），真实缺口，优先级高于任何外观问题。
2. **tier2 destroy-lock 未接通**（见 §6），测试覆盖缺口。
3. **WASM 路径不可验证**：仓库已无 wasm 与 wasm32 std；旧发布锚点
   `a019dc83…` / R081 只能从 git 历史取回，不能在本沙箱重建。
4. **真实 executor 验收**：Agent 未独立在 Roblox/Delta 中正向运行绑定产物。用户现已报告根目录
   `example.txt` 在 Delta build `1.0.747.1486` 直接执行时第 3 行 nil-call；详见 §13.4。此前“未反馈
   即视为通过”的约定不再适用于该样例，根因仍未确认。EVKEY 失败形态也可能是「静默失步挂起」，可能被同宿主旁录。
5. **`tests/run_tests.sh` 的输出比较用命令替换**，遇到二进制 stdout 会打印
   `ignored null byte in input` 警告；这是比较方式的噪声，不是失败（需要严格比较时改用文件 diff）。
6. 部分测试脚本仍硬编码 `/home/user/tools/bin` 作为首选、仓库 `.tools/bin` 作为回落；
   在只有 `.tools` 的环境里会自动走回落，但不要删除回落分支。

---

## 8. 诚实边界（必须保留这些口径）

1. **静态可解性未解决**：载体层密钥流是产物内常量的纯函数，纯静态审计可解；
   V-01/V-03 未修，R080 只修了 V-06/07/08。**不要宣称「静态不可拆」。**
2. **自包含脚本的流密码密钥必然可恢复**——壁垒是「每构建唯一 + 提取成本」，
   不是密码学不可破。
3. **绑定态产物在通用沙箱加载即失败**是设计目的，不是缺陷。
4. 水印是可删除的普通注释，不是防篡改。
5. 未验证项必须标注：真实 executor、真机 Roblox、WASM 构建、任何部署环境。
6. 仓库只剩引擎；旧的控制台/托管/部署结论不再适用于本树，不要再从记忆里补写。

---

## 9. 工作规则（用户硬性要求）

1. **不擅自扩范围**；引擎新功能已被 §34 收摊令冻结，只做修复与验证。
2. 改混淆模块的强制工作流：官方矩阵 +（动 `vmgen/`）多种子 + 安全指纹/防御审计 +
   重生成 `engine/examples/` + 刷新锚点 + 更新本文件；任一项不过 = 未完成。
3. 混淆器本体只写 Rust；不要引入 crates.io 依赖。
4. 每条结论给可复现证据（命令、体积、sha256、测试计数）；未验证就写「未验证」。
5. 只动当前工作分支：commit + push 到当前分支，**不动 `main`，不 force push**。
6. 新生成或重生成、且需纳入仓库的 example（特别是根目录 `example.txt`），完成验证后直接提交并推送到当前分支，不只留在本地；提交前检查工作区，只包含预期改动并保留既有用户文件。
7. 代码里没有注释（历史剥离）；设计看 git 历史，不要惊讶。
8. Shell 小坑：`/tmp/t_$s_$d.lua` 会把 `$s_` 当变量名，用 `${s}_${d}`。
9. 守卫/探针改动要过「净化后形态」：标识符会被改名，字符串字面量不会；
   外部对象成员只能 `["字符串键"]` 访问。
10. `strseal` 三处镜像（Rust encode/decode、v15 micro-op、handlers/loadstr.rs）必须同步改。

---

## 10. 2026-11-15 仓库整理记录

**本次（引擎-only）**

- 删除 `web/`、`services/`、`docs/`、`release-assets/`、`artifacts/`、`render.yaml`、
  `.dockerignore`；`.gitignore` 同步去掉 web/artifacts 规则。
- 测试仍需要的 4 个靶标移入 `engine/tests/fixtures/targets/`，5 个脚本的路径已改。
- 修复 legacy VM 字符串密封解码（§2.1）。
- E6b 的「翻转前默认 SHA」已不可达（R080 改变了关闭态字节），重钉为当前输出
  `11e2bfa4…`，并改名为「当前形态锚」；E6 的确定性检查保留。
- 重写本文件。

**上一次（只删 md，提交 `cbd21a5`）**

- 删除除本文件外的 88 个 markdown；保留全部代码/测试/产物。

**恢复旧文件**

```bash
git show 68b9187:README.md > /tmp/README.md
git show 68b9187:web/tools/obsucvim.wasm > /tmp/obsucvim.wasm
git show --stat 68b9187 | head
```

上述 `git show 68b9187:...` 是整理时的历史恢复示例；**当前 checkout 无法解析该 commit**（浅克隆对象不存在）。需要这些文件时从远端仓库的完整历史或完整克隆恢复。

---

## 11. 新任务开工顺序

1. 读本文件 §2（实测态）、§6（红项）、§7–§8（限制与边界）。
2. 确认任务在允许范围内（引擎修复/验证，不开新功能）。
3. 重建工具链（§4.1）→ 构建（§4.2）→ 跑官方矩阵（§6）建立基线。
4. 改动后按 §9-2 全工作流复跑；动 `vmgen/` 必跑 `multiseed.sh`。
5. 收尾：`git status` 干净、套件有记录、结论可复现；提交并 push 到当前分支。

---

## 12. 引擎评估摘要（2026-11-15，未改代码）

> 结论先行：**原生 CLI 能跑、语料矩阵基本绿，但默认 v15 路径还不是可交付的商业级实现。**
> 下面每条都有本轮实测证据；修复前不要把 §2 的“可用”理解为“可上线”。

### P0（阻断上线）

1. **默认 v15 只匹配一个 executor 档案。** `exguard.rs::keyhost_profile` 只有 `delta`
   一项（`k3c=22`，即 `identifyexecutor` 名长 5、`checkcaller=true`）；`main.rs` 在
   v15+bind+guard 时自动注入该档案。换名长/宿主值就静默失步：
   把 `identifyexecutor` 改成 `"Synapse"`、其余提供 Delta 形状的假环境，产物 40 s 内
   没有任何输出（挂起），不是可诊断错误。帮助文本里的“Synapse-class”与代码不符。
2. **默认绑定产物没有正向端到端验证。** 测试只有 E2/E3 负面拒绝、语法校验和
   `--bind-env` 的 faithful stub；没有任何测试让默认 `--bind-executor` 产物在仿真
   executor 里成功跑完。默认路径的“能用”目前只是历史真机结论，不是本轮证据。
3. **数字字符串算术语义丢失（v15/vm；历史基线问题，已修）。** 2026-11-15 基线中
   `print(("5")+0)` 等输入会报 `attempt to perform arithmetic on a string value`；
   已由提交 `44696d5` 修复并纳入 `engine/tests/vm_semantics_regression.sh`，本轮该专项通过。
4. **括号调用截断语义丢失（所有 preset；历史基线问题，已修）。** 基线中
   `print((f()))` 会保留多返回值；提交 `44696d5` 增加 `Expr::Paren` 并贯通解析、打印与 VM
   编译，本轮 `vm_semantics_regression.sh` 通过（详见 §13）。
5. **运行时字符串钩子可恢复明文。** 不需要改产物，只需 `loadstring(artifact)` 前把
   真实 `string.char` 换成记录版，执行后 `table.concat` 截获 1,850 个字符，
   其中直接含 `UNIQUE_MARKER_9f3a`（源里的秘密字符串）。`tests/charhook_test.sh`
   替换的是 `setfenv` 环境里的 **string 副本**，所以报 0 命中；真实全局 `string.char`
   被替换时命中。D17/S5 的“char 钩子封堵”结论不成立。
6. **输入规模硬顶且以 panic 结束。**
   - 3,493 B、100 个顶层 `local function` → `panicked: st table exhaustion`（rc=101）；
   - 20,709 B、3,000 条顶层赋值 → `H3 sparse register pool exhausted`（rc=101）；
   - 521 B、20 层嵌套闭包 → `upvalue descriptor miss`（rc=101）；
   - 20,000 层嵌套括号/表 → Rust 栈溢出 SIGABRT。
   没有输入大小/递归深度限制，也没有把这些情形变成可读错误；对服务端是 DoS 面。
7. **性能不适合实时脚本。** 200,000 次循环：源码 0.0065 s，v15 产物 18.50 s
   （约 2,800×）；50,000 次：3.67 ms → 4,892 ms（约 1,330×）。legacy `vm` 约
   1,000×，`high` 约 10×。v15 的 Lua-in-Lua 解释器是数量级瓶颈，Roblox 逐帧逻辑
   基本不可用。
8. **体积放大。** 空脚本产物 146,546 B；713 B 的 basics → 164,085 B（230×）；
   7,179 B 的 R064 → 341,539 B（47.6×）；13.8 KB/2,000 行 → 613,269 B。
   每个产物都自带完整解释器，分发/移动端/Executor 限制都要考虑。

### P1（重要）

9. **解析器覆盖不足。** 常见 Luau 写法直接拒绝：
   - `local x = 5 :: number` → `expected ')' got ':'`
   - `local function f<T>(x:T):T` → `expected '(' got '<'`
   - `y: string?` / `...: number` → 解析失败
   - UTF-8 BOM、shebang、`\u{...}` 转义不支持。
   简单类型注解、泛型类型别名、广义迭代、`continue`、复合赋值可通过。
10. **错误语义不兼容。** VM 的错误串更泛化（`attempt to perform arithmetic on a
    number value` 丢失 `(add)` 和操作数类型），行号整体后移；依赖 `pcall` 错误文本/
    traceback 的脚本会行为改变，也容易被反作弊指纹识别。
11. **静态可解性是设计约束，不是已解决。** KeyEmitter/载体字母表/解码器全部随产物
    分发；V-01/V-03 仍在。仓库的 S1–S5 攻击脚本只覆盖“找不到 LCG 生成器”这类
    浅层特征，5/5 绿不能证明抗手工逆向。EVKEY 只是宿主形状闸，不是密码学认证：
    宿主值可伪造/旁录，Delta 上可直接 dump EVKX；`--no-bind-executor` 与
    `OBSUCVIM_*` 调试口在原生源码里仍然存在。
12. **测试覆盖仍可能给出错误安全感。** `string_primitive_hook_test.sh`/D17 只换副本表；
    §12.3/12.4 已由 `44696d5` 增加专项语义回归，但 `run_tests.sh` 不是完整的反篡改或
    executor 正向证明；`v15_fingerprint.py` 对 basics 仅 21/32，`defense_audit.sh` 基线为
    18 PASS / 3 FAIL（D3 指纹、D18 E4；D13 是当时 PATH 下 debug 构建未编成）。现有
    169/2 只说明已有语料结果，不代表产品完备性。

### P2（工程/运维）

13. **RNG 的历史问题（已部分修正，见 §13）。** 2026-11-15 基线使用 31 位 Park–Miller，
    `next_raw()` 直接暴露完整状态；本轮已换为 ChaCha20、默认系统熵与私有输出接口。显式固定
    `--seed` 仍可预测，且产物内的随机化参数不是秘密；不能把此 PRNG 当成产物保密或安全边界。
14. **调试环境变量是生产脚枪**：`OBSUCVIM_KEY_MANIFEST` 可把密钥写文件，
    `OBSUCVIM_VM_RAW`/`OBSUCVIM_VM_TSRC` 可导出中间形态；发布构建应编译期禁用。
15. **重建脚本从 GitHub `master` 拉 Lua 5.1.5 源码**（非固定 commit/无校验和），
    供应链与可复现性弱；Rust/Luau 版本已钉。
16. **无 CI、无 fuzz、无 benchmark、无默认产物正向测试**；20,534 行 Rust 中
    输入可达的 `panic!/assert!/unwrap` 不少，建议加入结构化限制和 fuzz 门禁。

### 仍然成立的部分

- 2026-11-15 基线的 37 语料 × v15/vm 矩阵为 169/2、`cargo test` 为 65 绿；本轮更新后的测试结果见 §13；
- v15 同一显式 seed 逐字节确定；随机化增加分析成本，但不构成对已交付产物的保密保证；
- VM 字符串密封的 legacy 解码 bug 已修，`--vm` 74/74；
- 协程、元表、多值、upvalue、变参等语料语义通过（但未覆盖上述两个核心缺失）。

**最低限度的下一步**：继续补证 §12 中尚未解决的输入规模、动态钩子与性能问题；决定“单 executor
绑定”是产品策略还是缺陷（至少评估多档案/通用模式），并建立性能预算；最后重写 S1–S5/D17
的攻击口径，避免继续用副本表测试宣称安全。本次绑定样例的正向验收按用户手测约定记录，见 §13.3。
历史问题 §12.3/12.4 已有代码修复与专项回归，但仍应在完整发布验证中保留。

---

## 13. 2026-11-21 评估报告跟进（本轮工作）

> 附件报告称其自建 Luau stub 从样本运行中观测到 `print(6)`，并通过钩子取得一段 216 字节
> 明文；作者没有反汇编原始字节码，也没有在真实 Roblox/executor、本地引擎构建或性能基准中
> 验证这些结论。因此它证明的是该 stub 与该样本下的行为/读取路径，不是原始源码还原或通用
> executor 结论。报告提出的环境指纹失败静默挂起行为按用户要求保留。当前 `example.txt` 是按
> 本轮修改重新生成的样例，体积和 SHA-256 见下文 §13.1。

### 13.1 本轮实现

1. **按需解密与解析。** `vmgen/template.rs` 移除了先把整个明文 blob 拼进 `um`、再
   `table.concat(um)` 的路径。现在 `rb()` 解出单字节、`rv()` 按需读 varint，解析时直接消费
   当前 section；常量 section 首轮只记录偏移并跳过，随后重置 reader、从密文流回放到该偏移，
   由 `mb()` 直接解码常量，不再缓存整个明文常量段。运行时仍保留 VM 必需的解析结构
   （如 `W`、操作数流和常量表），且载荷密文仍在内存；这不是“运行时绝无明文”，也不是防止
   调试器/钩子读取已解析数据的密码学边界。
2. **增加反篡改行为断言与可见层清理。** `exguard.rs::SCHALL` 除原有类型/闭包分类外，还通过保护调用
   检查字符串、表、数学、`select`/`pcall`、类型与转换、raw access、迭代、元表、unpack，以及
   debug source/info 的实际结果；不符合预期时现有校验链拒绝该运行。debug info 的 source 字段
   改用字面键 `"source"`。守卫使用的 executor API 名在生成 Lua 中由相邻字面片段拼接，静态 12 词
   扫描为零命中；这只是减少直接字面痕迹，不是加密，也不妨碍动态恢复名称。探针只覆盖列出的行为，
   不证明可抵御能完整仿真同样输出的钩子。真实 executor 手测口径见 §13.3。
3. **移除报告中可识别的 unread section padding。** `compiler.rs` 不再生成 0–2 个随机 section-id-6
   字节段；发射端加不变量断言，模板移除了 `TDC` 读取分支，Rust section walker 遇到 id 6 会失败。
   section tag 数组仍保留该索引作保留槽位，但不会发射该段。本改动针对报告指出的两块未读取区，
   不等于证明引擎里所有其他散布/诱饵数据都被执行读取。
4. **替换 RNG。** `rng.rs` 改为 ChaCha20，默认 `--seed`（值为 0）从系统熵初始化：Unix
   `/dev/urandom`、Windows BCrypt、WASI `random_get`；显式非零 seed 仍确定性（SplitMix64 扩展
   到 key），整数区间用拒绝采样，子流通过 `fork` 域分离。`next_raw()` 已变为私有，散布噪声
   使用分离子流且不再把它的原始样本写入 audit 行。此改动改善构建随机性，不隐藏产物里已经
   编码的随机参数；固定 seed 和从产物可推导的子流仍可复现，不应当作产物保密机制。新随机序列
   触发了 H3 稀疏寄存器上界问题，已将高槽上限夹到 4095 并由布局测试覆盖。
5. **样例和固定层。** 根目录 `example.txt` 已用绑定开启的 `print(6)` 重生成：
   `--preset v15 --bind-executor --dialect luau --seed 42 print6.lua example.txt`。
   当前为 **172,186 B**，SHA-256 `98798595917262d7ff59671a7754b5eb1f8046203941bb04a73e6781d8932c27`；
   首行 watermark 仍是 `-- This file was obfuscated by Obsucvim.`。固定 watermark 常量与
   `--bind-executor` 开关/默认策略未改，样例明确使用 `--bind-executor`，没有用 `--no-bind-executor`
   生成验收样例。`engine/tests/gen_examples.sh` 也按 seed 42 重生成了全部 **37** 个
   `engine/examples/*.v15.luau.lua`（这些测试参考样例显式关闭 binding）。既有未跟踪 `print6.lua`、
   `print6_obf.lua` 和评估报告未覆盖。
6. **刷新验证基线。** 37 个 examples、executor/no-bind SHA 锚及 tier-2 形态锚因 ChaCha20 与按需
   reader 更新而重钉。`v15_fingerprint.py` 改为解析当前 root-table/VM 调用壳，阈值按 7 个固定 seed
   校准；`r063` 回归按新的 KeyEmitter 语法族检查并显式关闭 binding（它在 stock Luau 中执行）。
   tier-2 mock 的 `destroy-lock` 负例修正为只破坏锁、保留子表，避免被 fixture 自身的清理行为误判。
   `envcheck.rs` 未改；这些改动是测试口径/夹具更新，不改环境指纹的 fail-closed 行为。

### 13.2 本轮验证

- `cargo test --manifest-path engine/Cargo.toml --quiet`：**69 passed / 0 failed / 1 ignored**，另有
  **4 个集成测试通过**；`cargo check --manifest-path engine/Cargo.toml --all-targets --quiet` 通过，
  仍有仓库既有 unused/dead-code 警告。
- `engine/tests/vm_semantics_regression.sh`：通过；v15 样例保持 binding 开启，stock Luau 语法编译成功，
  运行时按预期拒绝绑定产物。
- 官方 `bash engine/tests/run_tests.sh`：**169 checks PASS / 0 FAIL**。其中 executor guard **11/11**、
  env-check **16/16**、tier-2 env battery **20/20**；R063、protected-meta、micro-fusion、字符串钩子、
  call-window 与 WTF-PORT 回归均通过。
- `bash engine/tests/multiseed.sh`（默认 seed 1、7、4242、31337、999999）：`FAIL=0`；另含 R064
  nested-upvalue **24 seeds** 与 call-window 噪声 **13/13** 回归通过。
- `bash engine/tests/defense_audit.sh`：**21 PASS / 0 FAIL**（退役的 D7 `--bind-key` 按设计跳过）；D3
  结构指纹 **32/32**，D4 安全指纹 **5/5**，D5/D6 真跨度篡改 **30/30 拒绝**，D13 debug 构建通过，
  D15 朴素环境桩静默毒药通过。另将同一结构指纹在 7 个固定 seed 上各跑一次，均为 **32/32**。
- `bash engine/tests/gen_examples.sh` 重生成 37 个 seed-42、unbound 测试参考样例；绑定开启的 v15
  `--super-insn` 产物及根 `example.txt` 均通过 `luau-compile`。样例大小与 hash 见 §13.1。
- 固定 seed 的绑定 v15 两次生成逐字节一致；省略 seed 的两次生成不同，验证默认 OS 熵路径。
- `git diff --check` 通过。环境指纹 fail-closed 的模拟反例见上述 env-check/tier-2/D15 结果；`envcheck.rs`
  未改。

### 13.3 明确边界与用户验收约定

- Agent 未独立在真实 Roblox/executor 中正向运行绑定产物。用户最初约定「未反馈问题即视为通过」，但
  后续明确报告 `example.txt` 在 Delta 上失败；此前对该样例的“手测通过”记录已被此反馈取代，不能再用于
  声称该 Delta build 已验收。该报告不证明其他 executor 档案或宿主版本的兼容性。
- 没有反汇编报告中的 216 字节，也没有重新构造报告中的 hook/读取追踪；不能据此声称已阻止所有
  明文钩子。VM 必须持有可执行的常量和解码后的操作数，仍可能被动态观察。
- 没有测本轮 reader 的峰值内存、性能开销或真实脚本运行时间；常量读取需回放 blob 前缀，可能增加
  工作量。报告中约 2,800× 的数字未复测。
- 当时的 env-check/tier-2 结果仅代表当时基线；随后按 §14 用户决策移除 Roblox/executor 专用 tier-2，
  保留通用 Luau tier-1。当前测试结果见 §14。
- 其余 §12 的已知问题、静态可解性、调试环境变量和输入规模限制仍需独立评估；ChaCha20 不改变这些边界。

### 13.4 Delta 运行时兼容性复测（仍未解决）

用户确认截图对应根目录 `example.txt`，由 Delta executor 直接执行（不是 LocalScript `loadstring`）；
`identifyexecutor()` 报告 build `1.0.747.1486`。故障为第 3 行 `attempt to call a nil value`，
调用栈重复第 3 行后回到第 1 行。原样例 SHA-256 见 §13.1。

用户在 Delta 执行 API 类型诊断：列出的 executor、debug、Vector3/Instance/game/task、bit32、table、
string、math 和 clipboard 成员均为 function；`Vector3.new(1,2,3)` 成功（`type=vector`），
`game:GetService("Players")` 成功（`type=userdata`）。这排除了这些具名成员整体缺失，但不验证
debug introspection 的具体语义，也没有测试零参数 `Vector3.new()`。

诊断版 `delta_trace_probe.txt` 与原始样例使用相同 `print6.lua`、v15、Luau、`--bind-executor`、seed 42
参数；`OBSUCVIM_EXG_TRACE=1` 使部分守卫早退变成带标签的错误。用户在 Delta 运行后反馈
`R-ixhj) true/0/-1/-1/-1`。`R` 是最外层 guard 的汇总失败，`true/0` 表示 `CIDENT` 通过但
`SCHALL()` 返回失败及 transcript 0，三个 `-1` 表示失败发生在 key-host 观测前；它没有指出
SCHALL 中哪项行为探针失败。初版文件 171,905 bytes，SHA-256
`7fd113231144869ec701e87a35210304e902c0923bf99d864e1d0c665b91019a`，`luau-compile` 通过；已在当前
分支提交 `728a5ea` 并推送。

随后用户在 v2 中得到 `R-btze) true/SCHALL_TRACE_01/-1/-1/-1`，确认失败落在基础库行为探针块，
而非 `loadstring`/`newcclosure` 分类分支。它仍未指出该块中的具体断言。v2 文件 171,950 bytes，
SHA-256 `ba4a471073a323ba14f8d5326453a2285691d3545a9febbb6e529b28dc75d9d7`，`luau-compile` 通过；
提交 `2dba601` 已推送。

已生成 `delta_trace_probe_v3.txt`，把基础库块中的 20 个断言分别标为 `LIB_CHECK_01`–`LIB_CHECK_20`，
并在汇总错误中带出 pcall 结果或具体断言号；另外保留 `SCHALL_TRACE_02`（loadstring chunk/closure）与
`SCHALL_TRACE_03`（newcclosure/closure）分支标记。LIB 检查顺序为：byte、char、sub、concat、insert、
ldexp、select(#)、select(values)、pcall、type、tostring、tonumber、rawget、rawset/metatable、rawequal、
next、metatable、unpack、debug.info source、debug.getinfo source。文件 172,123 bytes，SHA-256
`d1115cfbbdf7b04a69ce7cbff9740a900532093c7f03e2d8b1affc5b8ef7040`，`luau-compile` 通过；提交
`b1045b3` 已推送。用户随后明确要求不再运行 trace，而直接测试移除 executor binding 后更新的
根目录 `example.txt`（151,440 bytes，SHA-256 `c90a244ed2ab0afa370bb2aaa5507c07e0ee20b04fbd56f7cadbdbd32a46e10b`）。
2026-11-22 截图给出 `attempt to compare nil <= nil`，堆栈为 `Line 3 - function fm`、多次重复
`Line 3`，最后回到 `Line 1`。这与旧绑定样例的 `attempt to call a nil value` 不同，不能假设同根因。

当前引擎重生的默认 guard 样例与交付文件 hash 一致；`.tools/bin/luau-compile` 通过，stock
Luau 0.735 执行 `example.txt` 输出 `6`。当前 `cargo test` 为 65 passed / 1 ignored、4 个集成测试通过；
`vm_semantics_regression.sh` 通过。为做不带 trace 的 A/B，另生成 `example_no_guard.txt`：同一输入、
seed 42、v15，显式 `--no-guard`；143,643 bytes，SHA-256
`e82bc2ee1f0ac069a24256f75ed93fad7016af702fcf584c5188e91bf2748a12`，编译通过且 stock Luau 输出 `6`。
用户已在同一 Delta build 直接运行 `example_no_guard.txt` 并反馈不报错，问题收敛到 guard-enabled 路径。
用户选择默认 guard 保持开启。随后临时省略 `ANTI_DEBUG`、保留其余四个 stage 生成的
`example_guard_no_debug.txt`（148,520 bytes，SHA-256
`73e480bc054d35346e6c5e43a9f5dea79384984f09532f8aa02b41c6d328c991`）也在 Delta 正常，确认问题在
debug stage。

为保留 debug 来源检查并避开不兼容的栈行号/xpcall 错误文本解析，当前工作树将 `ANTI_DEBUG` 改为受保护的
`debug.info(..., "s")` 来源探测；仅在 `debug.info(pcall)` 返回 `[C]` 时校验 loader 来源，缺少元数据则跳过。
默认 guard 仍开启。防御审计 D9/D10 仍通过。候选 `example_guard_source_probe.txt` 用该改动、seed 42 生成，
148,769 bytes，SHA-256 `00e7ee9136cc3e5d73deff03cbee31509884734b45ea0184c2851fcadb6d6ecd`；Luau 编译通过，
stock Luau 输出 `6`。`cargo test` 65 passed / 1 ignored、4 个集成测试通过；`run_tests.sh` 169/0，
`defense_audit.sh` 20/0，全部 shell 语法与 `git diff --check` 通过。测试样例已在当前 PR 分支提交
`9e9b340` 并推送。用户已在 Delta 直接运行 `example_guard_source_probe.txt` 并反馈正常；这确认简化的
debug source 检查可用且其余默认 guard 保持开启。根目录 `example.txt` 已由当前引擎重生并与该候选逐字节一致，
148,769 bytes，SHA-256 `00e7ee9136cc3e5d73deff03cbee31509884734b45ea0184c2851fcadb6d6ecd`；Luau 编译通过，
stock Luau 输出 `6`。未运行 trace；现可提交并推送 guard 修正及本轮相关清理。

---

## 14. executor binding / key-host / EVKEY 移除（2026-11-21）

本节覆盖并取代 §1、§3、§5、§6、§7、§12 中关于当前 executor-binding/key-host/EVKEY 实现的旧描述；§13 的 Delta 诊断过程保留为历史上下文。

- 从 CLI、VM compiler/template 链中移除 `--bind-executor`、`--no-bind-executor`、`--no-executor-guard`、
  `--key-host`、EVKEY/EVKX、XTOK/XPROBE 与 EXG 运行路径；删除 `vmgen::exguard` 及对应专项测试。
- 用户选择 `--env-check` 只保留通用 Luau tier-1：Roblox/executor 专用 tier-2 检查和 mock 回归已删除。
  `--guard` / `--no-guard` 显式保留；通用 guard 默认开启，两个开关均受支持。`--bind-env` 是独立功能，未移除。
- `engine/examples/` 已用当前 `gen_examples.sh` 重生 37 个默认 v15 样例。
- 验证：`cargo test` 为 **65 passed / 0 failed / 1 ignored**，水印集成测试 **4 passed**；
  `bash engine/tests/run_tests.sh` 为 **169 PASS / 0 FAIL**；
  `bash engine/tests/defense_audit.sh` 为 **20 PASS / 0 FAIL**；
  `wtf_port_test.sh` 为 **23 PASS / 0 FAIL**；VM semantics、`env_bind_test.sh`（6 PASS）、
  `faithful_stub_test.sh`（5 PASS）均通过；`git diff --check` 与全部 shell `bash -n` 通过。
- 根目录 `example.txt` 已按当前引擎从 `print6.lua` 以 `--preset v15 --dialect luau --seed 42` 重生，
  151,440 bytes，SHA-256 `c90a244ed2ab0afa370bb2aaa5507c07e0ee20b04fbd56f7cadbdbd32a46e10b`；
  `.tools/bin/luau-compile` 通过。Delta 截图报告 `attempt to compare nil <= nil`，多次重复 Line 3
  （function fm）后回到 Line 1。Stock Luau 输出 `6`、VM semantics 回归通过；为 A/B 新增
  `example_no_guard.txt`（显式 `--no-guard`），hash 与验证见 §13.4。用户在 Delta 上运行该文件反馈不报错；
  默认 guard 样例曾报 `attempt to compare nil <= nil`。用户选择默认 guard 保持开启并不跑 trace；
  `example_guard_no_debug.txt` 与随后简化 debug source 检查的 `example_guard_source_probe.txt` 均在 Delta 正常。
  当前根目录 `example.txt` 已重生为后者的字节副本，具体 hash 与验证见 §13.4。

---

## 15. 加固方案复核与「真密封」落地（2026-11-22）

> 外部提交了一份 12 项改进方案（出口收敛 / 构建差异化 / 动态密钥 / 自密封 / MBA /
> 链式状态 / 去中心化 VM / JIT 解密 / 短暂常量 / 多态上下文 / 噪声随机化），声称当前
> 综合评分 4.56/10、7 大弱点全中。本轮**逐条用产物本体复核**，结论是该评分模型不适用
> 于本仓库现状：12 项里 9 项早已落地并有审计闸，1 项属静态可解性设计约束，只有
> **1 项（W3 假密封）确实仍成立**并已修复。

### 15.1 逐条复核结论（证据均为本沙箱实跑）

| 方案项 | 声称 | 实测结论 | 证据 |
|---|---|---|---|
| W1 出口收敛 | 六路 `return` 泄漏 6 个内部对象 | **已闭合** | 产物末尾只有 `return <ft>:(...)`；正则无六路返回；`h4_loader_audit` = CLOSED（loader 参数 0 字节、内部调用 0） |
| W2 构建差异化 | 53/54 段跨构建相同 | **已闭合** | `h2_diff_audit`：同位置相同字节 2.35%、骨架 token 4.94%、24-gram Jaccard 0.33%、长串段相同率 0% |
| W5 噪声指纹 | 噪声跨构建固定 | **已闭合** | `h3_noise_audit` 11 项全过（真/噪声高低域双向重叠 29.1% vs 27.0%） |
| W9 别名表固定 | 43×3 固定别名 | **已闭合** | 别名/分发表跨构建重掷（`h2`、`h7`） |
| W10 指令字布局 | 5×u16 固定 | **已闭合** | 每函数 slot 置换 + SoA 变体（`h2`[F]、`h7`[3]） |
| W6/W7 静态密钥 | 密钥可离线算 | **已闭合** | `OBSUCVIM_KEY_MANIFEST` 全部键跨构建重掷（`h7`[1]、`h8`[1][6]） |
| W8 反 AI 设计 | 中央分发器 + 无 MBA | **已闭合（部分口径不同）** | 无 `OC.`/`if oc <=` 中央表；~236 状态 continuation + trampoline；Poison 六面分散（`h8`）。**未采用 MBA 与链式状态**——见 15.3 的取舍说明 |
| W4 壳自密封 | 外层壳无自源码哈希 | **设计上不适用** | 产物是单条 `return` 表达式，无「壳」可改；源代码已无明文，`string.dump`/`debug.getinfo` 自哈希在 Luau 下既易伪造又破坏语义（`debug` 可被 hook）。密文完整性已由 D5/D6 真跨度篡改 30/30 拒绝覆盖 |
| **W3 假密封** | 完整性检查为恒等式（≡0） | **成立，已修复** | 见 15.2 |

### 15.2 W3：把「恒假无后果」换成「可证伪 + 有后果」

修复前：`v15.rs` 的 V2 stage 是
`if C[verifyslot]~=C[sumslot] then C[sumslot]=C[sumslot]+7919 end` —— 两个槽从未被
任何一方初始化，nil 恒等于 nil，分支永不进入；即使进入也只是「加常数后自愈」，
对攻击者是零成本、对执行者是零作用。

修复后（`engine/src/vmgen/v15.rs`）：

1. 入口装配初始化 `C[verifyslot] = C[sumslot]`，使比较在正确路径上是**活的**而非 nil 跳过；
2. 失配不再自愈，而是把 `poison`（每构建随机、经 `st_pair_expr` 双表拆分）注入 `sumslot`，
   并置 fail 槽；
3. continuation（`ctl`）在抵达 `sdone` 时检查 fail 槽，命中则把 poison 当返回值交给解释器，
   解释器随即 **静默毒药**退出（rc=0、无正确输出、无报错信号），与仓库既有 poison 口径一致。

可证伪性实证（`engine/tests/h9_seal_audit.py`，已并入 `run_tests.sh` 官方矩阵）：

- 正确构建：语义等价（`6 9 sealprobe`），不误报；
- 篡改解码载荷字节 → 触发失败；
- 篡改存放的校验值 → 触发失败；
- **负对照**：把 `v15.rs` 回退到旧式 `+7919` 后同一审计 1/3/4 项全红、退出码 1。
  这证明审计测的是密封本身，而不是恒真断言。

### 15.3 未采纳项及理由（诚实边界）

- **自源码哈希（W4）**：需要在产物内读自身源码。Luau 无 `string.dump`；`debug.info`/`getinfo`
  在 executor 下可被 hook 伪造，且 §13.4 已证明 Delta 上 debug 路径脆弱。加它只会新增
  误伤面而不增加真实攻击成本——攻击者本来就能改任意字节，哈希比对逻辑同样可改。
- **MBA 语义纠缠（W8）**：`(x⊕y)·(x∨y)` 在 Lua 里按 double 运算，须显式 `%2^32` 与位库，
  会把 VM 热路径再放慢一个量级（当前已是 ~2800×），换来的是可被常量折叠/代数化简消掉的
  复杂度。仓库现有分散投毒 + 每构建重掷的收益/成本比更高。
- **链式状态 / JIT 单指令解密 / 短暂常量**：与 15.1 的按需 reader（`rb()/rv()`）和
  分散 poison 部分重叠；真正做到「内存中只有一条明文」需要重写解释器主循环，属新功能，
  受 §9-1 收摊令冻结，本轮不动。

### 15.4 本轮验证（全部本沙箱实跑）

- `cargo test --release`：**65 passed / 0 failed / 1 ignored**，集成 **4 passed**。
- `bash tests/run_tests.sh`：**169 PASS / 0 FAIL**（已含新增 h9 闸）；`wtf-port env-check` 23/23。
- `bash tests/multiseed.sh`：**FAIL=0**；`bash tests/defense_audit.sh`：**20 PASS / 0 FAIL**。
- H2/H3/H4/H5/H6/H7/H8/H9 八个加固审计：**全部 PASS**。其中 `h3_noise_audit.py` 本轮
  修复了脚本自身的两处缺陷（引用早已不存在的 `seed` 字段、缺 `.tools/bin` 回落），
  修复前它直接 `KeyError` 崩溃——即该审计此前**从未真正跑过**。
- 重生成 `engine/examples/` 37 个样例；根 `example.txt` 重生为
  **149,118 B**、SHA-256 `9ceb3202cf050587ed3074398cb871fa45369d74464f47de6b0e9af40aa4c783`，
  `luau-compile` 通过、stock Luau 输出 `6`。
- 性能：200k 循环 17.96s（改前记录 ~18.50s），无可测回归。
- 已知既有问题（**非本轮引入**）：`v15_fingerprint.py` 在 seed 888 上 F9 偶发不过；
  回退旧代码为 30/32，本轮为 31/32，属既存形态抖动，未在本轮处理。
