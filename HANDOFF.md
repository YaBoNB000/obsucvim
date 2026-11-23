# 交接文档（HANDOFF）

> **仓库现在只有混淆引擎。** 2026-11-15 第二次规整：除本文件外不再保留任何 markdown；
> 控制台（`web/`）、托管服务（`services/`）、研究数据（`docs/`）、证据链（`artifacts/`）、
> 发布包（`release-assets/`）与 Render/Docker 部署文件已从工作树删除。测试仍需的 4 个靶标
> 已移入 `engine/tests/fixtures/`。旧文件恢复记录见 §10；当前 shallow checkout 不含旧基线 `68b9187` 对象。
>
> 最后更新：2026-11-21。§2 和 §12 保留 2026-11-15 的历史基线；Delta 诊断见 §13，本轮移除 executor binding/key-host/EVKEY 的现状与验证见 §14。
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
| 原生二进制 | `engine/target/release/obsucvim`，**2,117,872 B** / `ff2bc1a88aab9da1e6ba22096e5b1c9ca862ee3233543191d804f6674ca8d4b0` |
| 构建 | Rust 1.88.0，`cargo build --release` 离线成功（约 30 s 增量） |
| 单元/集成测试 | `cargo test --release`：**65 passed / 0 failed / 1 ignored**（61+4） |
| 官方矩阵 | `bash engine/tests/run_tests.sh` → **PASS 169 / FAIL 2**（约 155 s） |
| VM 专项 | 37 语料 × `--vm`（字符串密封默认开）× run + `luau-compile` → **74 / 74 PASS** |
| v15 锚点 | `print("suda") --preset v15 --env-check --bind-executor --seed 67067` → **179,643 B** / `1b1056221c386f5a7232153e5d5f6f63a2e68cc4c51f5e6e7165a46b8b5c7135` |
| 关闭绑定锚 | `--preset v15 --no-bind-executor --seed 4242`（executor 守卫语料）→ **159,951 B** / `11e2bfa48e2eca198688110bdab29d76cfb247e005959f51522e563a00dfb52f` |
| 水印 | 每个产物第一行固定 `-- This file was obfuscated by Obsucvim.`（普通注释） |

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

## 15. P0–P5 能力最小化、状态不变量、常量分块物化、守卫硬化与发布门禁（2026-11-22）

本节记录在 §14 通用 Luau 基线之上完成的 P0–P5 架构收敛与可验证指标；不恢复任何 §14 已退役的 executor-binding / key-host / EVKEY / tier-2 探针机制。P0–P5 对照剖析以历史 `example.txt`（148,769 B，SHA-256 `00e7ee9136cc3e5d73deff03cbee31509884734b45ea0184c2851fcadb6d6ecd`）为改前基线；验收后按用户指令将根目录 `example.txt` 更新为 `a = 999` 的默认 `v15` 混淆产物（`--preset v15 --dialect luau --seed 42`，101,834 B，SHA-256 `b2fbad2231f5a6d37aae0f6f0897c8e2e4ed67bc5f2a7b250cb72a1fdd3675e1`）。

### 15.1 关键改动摘要

- **P0 产物诊断剖析器（`engine/tests/artifact_profile.py`）**：基于自包含 Luau 词法/AST 分析器输出字节数、token/AST 节点、全局宿主 API 引用、短/长字符串常量、可证恒真/恒假谓词、外层 staging 与内层 `HW2` 包装簇集中度，以及构建/运行耗时与子进程 Peak RSS，支持单产物剖析与 7 语料 × 6 seed 回归对比。
- **P1 按需能力清单与隐式全局清零（`engine/src/vmgen/caps.rs`, `v15.rs`, `template.rs`）**：将外层模块表原语与内层 `P`/辅助函数拆分为 `InterpreterBase`（引导与常量/帧基础设施必需）和 `ProgramDriven`（由 `VmProgram` 活跃指令、浮点常量及 `--guard`/`--env-check`/`--bind-env`/`--super-insn` 标志确定性推导）；裁剪未使用 `(op, dialect)` 槽与未命中 `--super-insn` 融合尾槽，保留动态全局经运行时环境表 `G` 解析；补齐 `template.rs` 局部声明，消除旧产物中的 `DK`、`Qe`、`qA` 隐式全局读写；清理 `v15=true` 分支下不可达旧模板拼接死路径。
- **P2 真实状态不变量与分派/包装多样化（`v15.rs`, `template.rs`）**：移除代数/奇偶恒真 `opaque_wrap` 与 `if false` 死分支，改用绑定初始化与运行期真实状态槽的 `state_guard_wrap`（AST 可证代数恒真分支与线性恒等分支均降为 `0`）；外层 `gen_filler_staging` 扩展为 `10` 类状态转移骨架 × `5..=9` 参数签名（最高单簇占比由 `27.4%–29.8%` 降至 `3.5%–4.4%`）；内层 Micro-VM 引入 `zC` 稀疏秩映射表、`12` 种工厂形态（4 种秩分派树 × 3 种循环骨架）与 `5..=8` 参 `HW2` 包装；`--super-insn` 维持显式 opt-in。
- **P3 分块字符串物化、特殊数值常量与明文边界（`template.rs`, `handlers/loadstr.rs`, `compiler.rs`）**：将 `CDEC`、`LoadStr` 与 `M_LOAD_STR` 的逐字符 `..` 累加改为 `64` 字节分块 `CHAR(UNP(..., 1, k))` + `TCAT` 的 $O(L)$ 装配；`CDEC` 支持 IEEE-754 `±Inf` 与 `NaN`；将 `SALT` 时钟折叠改为 `FLR(_ckv*1000000)%65536` 整数域计算以避免浮点取模漂移。静态常量串沿用「每函数首次读取解码并写回该函数常量代理缓存」设计以保证热循环性能与 table key 相等性；首次读取后明文串在函数生命周期内驻留运行时内存，动态 hook 或内存转储仍可观测已求值常量，静态混淆对此不作过度承诺。
- **P4 通用守卫阶段分类与沙箱兼容性硬化（`anti/mod.rs`, `guard.rs`, `template.rs`）**：新增 `STAGE_MANIFEST` 五阶段元数据表（`core`/`env`/`debug`/`canary`/`misc`），区分必选核心原语（`type`/`pcall`/`error`/`rawget`/`rawset`/`getmetatable`/`setmetatable`）与可选宿主 API（`xpcall`/`debug`/`debug.info`/`os`/`os.clock`/`newproxy`/`warn`/`unpack`/`loadstring`/`load`）：缺失可选 API 放行，非预期类型、伪造 `[C]` 来源、Lua 包装核心原语/加载器或破坏元表锁则静默 fail-closed；修正 `stage_env` 为 `pcall(function() return gfe(1) end)` 以正确读取 `setfenv` 闭包环境并兼容受保护字符串/表元表沙箱；`stage_debug` 在局部环境隐藏 `debug` 时回退检查 `getfenv(0)` 宿主 `debug.info`。新增超时隔离的 `engine/tests/guard_matrix_test.py`（31 场景）。
- **P5 编译期审计钩子门禁、Luau 基准与样例更新（`audit_gate.rs`, `Cargo.toml`, `bench_presets.sh`, `examples/`）**：新增默认关闭的 `audit-hooks` feature，生产 `cargo build --release` 在编译期禁用 `OBSUCVIM_KEY_MANIFEST`、`OBSUCVIM_VM_RAW`、`OBSUCVIM_VM_TSRC`、`OBSUCVIM_NOISE_AUDIT`、`OBSUCVIM_PROFILE_AUDIT`、`OBSUCVIM_*_DBG` 与 `OBSUCVIM_TIMING`，且同 seed 产物与 `--features audit-hooks` 构建逐字节一致（`prod_audit_gate_test.sh`）；将 `bench_presets.sh` 升级为 Luau `0.735` 8 类语料 × 7 模式基准；通过 `gen_examples.sh` 重生 `engine/examples/*.v15.luau.lua` 全部 37 个样例。

### 15.2 验证状态与明确边界

- `cargo check --all-targets`、`cargo test --release`（**72 passed / 0 failed / 1 ignored** + 水印集成测试 **4 passed**）通过；本地 `.tools/bin` 最小工具链不含 `rustfmt` 组件（`cargo fmt` 不可用）。
- `run_tests.sh`（**169 PASS / 0 FAIL**）、`run_presets.sh`（**370 PASS / 0 FAIL**）、`multiseed.sh`（**FAIL=0**，含 R064 **24 seeds**）、`vm_semantics_regression.sh`、`guard_matrix_test.py`（**31/31 PASS**）、`capability_manifest_test.py`（**6/6 PASS**，含 `--super-insn` 与字符串/`±Inf`/`NaN` 多 seed 等价）、`prod_audit_gate_test.sh`、`defense_audit.sh`（**20 PASS / 0 FAIL**）、`v15_fingerprint.py`（**32/32 PASS**）、`security_fingerprint.py`（**5/5 PASS**）、`h2`..`h8`、`r061_bypass_audit.py`、`redteam_probe_regression.py` 及 `bench_presets.sh`（56/56 组合等价）全部通过。
- **真实执行器边界**：上述兼容性测试均在本地 stock Luau `0.735`、`LUAU_READONLY_GLOBALS=1` 及受控模拟环境矩阵中完成；未在真实第三方执行器（如 Delta 等）实机运行本轮新产物前，不声称特定执行器版本已通过实机验收。

---

## 16. P0–P2 原语契约收口、守卫清单双向对齐、强制审计门禁与同输入/同 seed Worktree A/B 基准（2026-11-22）

本节记录针对 §15 代码复审四项遗留工程问题的收口改动、回归期间捕获并修复的两处真实缺陷，以及基于隔离 `git worktree` 对改动前基线提交 `f583c2c3c9c66770c38169dc6547d10c85025555` 的同输入、同 seed、同主机实测 A/B 数据。

### 16.1 输入语义口径澄清（`print(6)` vs `a = 999`）

为避免跨轮对比混淆不同输入脚本，本节严格区分两类微基准产物：
- **根目录 `example.txt`（提交产物）**：输入源码为 `a = 999\n`（7 B），以 `--preset v15 --dialect luau --seed 42` 生成，当前大小 **102,394 B**，SHA-256 `b79e79c3b534d54b1d1a5db999e7281a463621719eda78f582ac16de4b1b45bf`；通过 `.tools/bin/luau-compile`、stock Luau 运行验证（`a == 999`）与 `v15_fingerprint.py`（**32/32 PASS**）。
- **同输入微基准 `01_micro_print6`（A/B 对照口径）**：输入源码固定为 `print(6)\n`（9 B），用于与 `f583c2c3` 基线在完全相同输入与相同 6 个 seed（`1, 7, 42, 4242, 31337, 999999`）下对比：默认 `v15` 6-seed 中位体积由 **`147,400 B` 降至 `106,343 B`（`-27.9%`）**，`v15 --no-guard` 由 **`141,954 B` 降至 `100,709 B`（`-29.1%`）**。下述 §16.3 所有 A/B 对比表均严格遵循「同一输入文件 + 同一 seed + 同一命令标志」，不再将 `a = 999` 与历史 `print(6)` 直接相减。

### 16.2 关键工程改动摘要

- **P0 原语绑定契约统一与运行期降级收口（`engine/src/vmgen/caps.rs`, `template.rs`, `engine/tests/capability_manifest_test.py`）**：
  - 删除 `template.rs` 中与 `caps.rs` 平行的旧硬编码数组 `PRIM_SRC`、`PRIM_NAME`、`PRIM_CAP` 及硬编码下标 `slots[12]`，改为统一由 `VmPrim::ALL`（`0..19`）单一数据源提供槽位名、驱动分类、模板启用谓词 `prim_enabled_in_template(v15, guard)` 与 Lua 绑定表达式 `lua_bind_expr()`（`Select` 统一取 `slots[VmPrim::Select as usize]`）。
  - 统一 `v15` 与 `--preset vm` 的三项能力降级语义：`Getfenv` 在宿主缺失 `getfenv` 时回退至 `function() return type(_G)=="table" and _G or {} end`（宿主存在原生 `getfenv` 时维持 `local G = GFE(0)` 以保持 R064 `setfenv` 字符串表反挂钩契约 `0/0/0`）；`Unpack` 按 `unpack -> table.unpack -> 纯 Lua 切片降级` 三级回退；`TablePack`（仅 `v15`）按 `table.pack -> { n = select("#", ...), ... }` 回退，保证含中间/末尾 `nil` 的多返回值计数与解包语义一致；`callcap` 仅在 `v15 && luau_target` 下发射。
  - 扩展 `capability_manifest_test.py`（**11/11 PASS**）与 `caps.rs` 单元测试，覆盖 `getfenv == nil`、`unpack == nil`、`table.pack == nil` 及三者同时缺失在 6 个 seed × `--guard`/`--no-guard` + `--preset vm --no-body` 下的运行期等价性。
- **P1-A 守卫阶段元数据双向对齐与矩阵扩展（`engine/src/anti/mod.rs`, `guard.rs`, `template.rs`, `engine/tests/guard_matrix_test.py`）**：
  - 将 `next` 显式纳入 `STAGE_MANIFEST`（`stage_misc` 可选能力）：`next == nil` 放行，`type(next) ~= "function"` 或空表/单键表迭代语义失真则静默 fail-closed；`v15`（经 `HBOOT` 第 30 参传入 `NXT`）与非 `v15` `guard_prelude` 同步接线。
  - 将 `STAGE_MANIFEST` 的 `test_ids` 与 `guard_matrix_test.py` 建立**双向精确对齐**（Rust 单元测试解析 Python 用例表校验每个 `test_id` 真实存在且无孤儿项；Python 运行时反向解析 `anti/mod.rs` 校验双向集合相等），并将守卫矩阵由 31 项扩展至 **46/46 PASS**（新增 `print`+`warn` 同时缺失、`print` 重定向、`setfenv` `__index` 代理表、`unpack`/`table.unpack` 互备与全缺、`os`/`os.clock` 缺失与非函数篡改、`next` 缺失/篡改，以及 `--guard` vs `--no-guard` 同篡改差分对照）。
- **P1-B 生产二进制零审计泄漏强制门禁（`engine/src/audit_gate.rs`, `engine/tests/prod_audit_gate_test.sh`）**：
  - 将原先「未编译 `audit-hooks` 二进制时跳过对照」的软检查改为**硬门禁**：若 `engine/target/audit/release/obsucvim` 缺失或早于源码/Cargo 配置则自动执行 `cargo build --release --features audit-hooks --target-dir engine/target/audit`（失败即非零退出）。
  - 在全部 8 个内部审计环境变量同时注入下，验证生产二进制 `target/release/obsucvim` 零审计文件落盘、零 `stderr` 内部标记，且在 `v15`、`v15 --no-guard`、`v15 --super-insn`、`vm` 四种模式下与干净环境的 `audit-hooks` 构建逐字节一致；同时对 `audit-hooks` 二进制做正面对照，并内置 3 项反向自检（模拟文件泄漏、`stderr` 泄漏、跨构建字节漂移均确认拦截失败）。
- **全量回归期间捕获并修复的两处真实缺陷（`engine/src/vmgen/v15.rs`, `engine/src/strings.rs`, `body.rs`）**：
  1. `v15.rs::idle_local` 局部遮蔽缺陷：`multiseed.sh` 在 `stress_multival` `seed=31337` 捕获运行期 `attempt to perform arithmetic (add) on number and nil`——当外层名称池 `nm.take_avoid(...)` 抽中 `"_t"` 且 `idle_local` 命中第 4 分支 `local {name}; do local _t={probe}; {name}=(_t==nil) and ... end` 时，内层 `local _t` 遮蔽外层 `_t` 导致外层保持 `nil`；现改为从 `["_t", "_u", "_v"]` 选取与 `name`/`probe` 均不冲突的临时名并补充单元测试。
  2. `strings.rs::build_loader` 重复逐字节解密开销：旧版 legacy 预设（`low`/`medium`/`high`/`vm`/`max`，不含 `v15`）字符串解密器每次调用均重新分配 `{...}` 并逐字节 `string.char` 解密，与 `junk.rs` 注入的 `#dec(...)` 叠加后导致 `--preset vm`/`max` 在 `stress_bigtable` 热路径出现重度放大；现为 `apply_strings` 解密器增加局部表 `cache[s]` 记忆化（`body.rs` 整包一次性加载维持原非缓存路径，`v15` 完全不经过 `strings.rs`），使 `stress_bigtable` 在 `--preset vm` 6 个 seed 下耗时由 `4.4s–30s+` 降至 `0.51s–1.36s`。

### 16.3 Worktree 同输入 / 同 seed 实测 A/B 对比（`f583c2c3` vs 当前分支）

复现入口：`bash engine/tests/ab_compare.sh`（驱动 `engine/tests/ab_compare.py`，自动以 `git worktree add --detach` 检出并编译基线提交 `f583c2c3c9c66770c38169dc6547d10c85025555`，机器可读原始数据与摘要表写入 `engine/tests/results/ab_baseline_f583c2c3.{json,md}`）。
- **测试环境**：`Linux 6.1.158+ (x86_64)`，`Intel(R) Xeon(R) @ 2.60GHz`（2 核，`3939.9 MB` RAM），`rustc 1.88.0`，Luau `0.735`，6 seeds `[1, 7, 42, 4242, 31337, 999999]`，每点顺序执行 `3` 轮 Linux `wait4` 计时/RSS 采样（共 **480/480** 构建点语义等价 + `luau-compile` 语法通过，同 seed 确定性 **PASS**，异 seed 6/6 互异）。

#### 表 A：默认 `v15`（`--guard` 开启，6-seed 中位数 / p90）

| Corpus | In (B) | Base Out (B) | Cand Out (B) | Δ Bytes | Base→Cand Tokens | Base→Cand AST | APIs (B→C) | ConstP / Par+Lin (B→C) | StgTop% (B→C) | Build ms Med/p90 (B→C) | Run ms Med/p90 (B→C) | Peak RSS KB (B→C) | Eq / Det / Rnd |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `01_micro_print6` | 9 | 147,400 | 106,343 | -27.9% | 68,060→48,066 (-29.4%) | 41,190→28,976 (-29.6%) | 71→28 | 87.5(52.5)→10.0(0.0) | 27.4%→4.1% | 110.1/126.5→88.8/132.8 | 25.7/32.9→24.2/32.0 | 22000→22000 | 6/6/Y/6 |
| `02_basics` | 713 | 162,678 | 146,370 | -10.0% | 74,013→66,912 (-9.6%) | 45,190→40,774 (-9.8%) | 71→28 | 87.5(51.0)→10.0(0.0) | 27.4%→5.1% | 107.5/138.5→110.9/148.7 | 31.5/41.1→34.6/44.3 | 22000→22000 | 6/6/Y/6 |
| `03_strings` | 1737 | 167,599 | 144,753 | -13.6% | 76,118→64,553 (-15.2%) | 46,716→39,766 (-14.9%) | 71→28 | 87.5(50.5)→10.0(0.0) | 27.9%→3.8% | 106.3/131.3→94.0/141.0 | 43.0/59.6→35.1/46.7 | 22000→22000 | 6/6/Y/6 |
| `04_luau_escapes` | 301 | 144,112 | 111,419 | -22.7% | 67,394→50,257 (-25.4%) | 40,727→30,362 (-25.4%) | 71→28 | 86.5(52.5)→10.0(0.0) | 26.0%→4.0% | 120.3/147.7→90.6/132.0 | 26.9/31.4→25.8/32.3 | 22000→22000 | 6/6/Y/6 |
| `05_functions` | 1953 | 254,726 | 237,577 | -6.7% | 124,851→118,792 (-4.8%) | 79,708→75,910 (-4.8%) | 71→28 | 92.0(46.5)→10.0(0.0) | 5.7%→5.3% | 156.5/204.9→166.3/216.6 | 554.6/682.5→485.4/688.9 | 55928→48208 | 6/6/Y/6 |
| `06_control` | 1511 | 179,236 | 166,255 | -7.2% | 82,734→77,588 (-6.2%) | 51,130→48,174 (-5.8%) | 71→28 | 88.5(51.0)→10.0(0.0) | 22.6%→3.8% | 123.4/172.6→111.1/130.1 | 56.6/67.5→40.7/60.4 | 22000→22000 | 6/6/Y/6 |
| `07_metatables` | 1711 | 225,699 | 207,012 | -8.3% | 108,504→101,460 (-6.5%) | 68,786→64,518 (-6.2%) | 71→28 | 87.5(53.5)→10.0(0.0) | 8.2%→4.3% | 140.3/209.8→134.8/173.1 | 67.8/95.7→51.5/65.0 | 22000→22000 | 6/6/Y/6 |
| `08_multival` | 1364 | 188,436 | 170,103 | -9.7% | 89,023→79,861 (-10.3%) | 55,418→49,772 (-10.2%) | 71→28 | 83.5(57.0)→10.0(0.0) | 18.3%→4.2% | 111.0/144.8→92.5/98.4 | 54.1/66.5→32.0/46.6 | 22000→22000 | 6/6/Y/6 |
| `09_coroutine` | 1448 | 190,404 | 166,026 | -12.8% | 91,630→79,078 (-13.7%) | 57,164→49,591 (-13.2%) | 71→28 | 86.5(54.0)→10.0(0.0) | 15.7%→4.0% | 114.4/152.7→87.2/119.5 | 39.9/55.3→28.9/40.7 | 22000→22000 | 6/6/Y/6 |
| `10_real_algo` | 787 | 174,660 | 161,198 | -7.7% | 82,423→75,958 (-7.8%) | 50,963→47,089 (-7.6%) | 71→28 | 86.0(55.5)→10.0(0.0) | 20.2%→3.0% | 102.0/143.8→104.7/147.7 | 43.2/55.5→38.4/53.6 | 22000→22000 | 6/6/Y/6 |

#### 表 B：`v15 --no-guard`（6-seed 中位数 / p90）

| Corpus | Base Out (B) | Cand Out (B) | Δ Bytes | Base→Cand Tokens | APIs (B→C) | Build ms Med/p90 (B→C) | Run ms Med/p90 (B→C) | Eq / Det / Rnd |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `01_micro_print6` | 141,954 | 100,709 | -29.1% | 65,855→44,567 (-32.3%) | 69→20 | 127.1/159.1→78.2/113.1 | 37.8/69.5→22.6/33.5 | 6/6/Y/6 |
| `02_basics` | 158,119 | 140,545 | -11.1% | 71,858→63,179 (-12.1%) | 69→21 | 100.4/129.9→92.5/97.9 | 32.7/48.1→25.9/37.3 | 6/6/Y/6 |
| `03_strings` | 163,602 | 137,816 | -15.8% | 73,217→61,237 (-16.4%) | 69→22 | 140.6/169.7→97.1/144.4 | 57.0/80.1→37.0/54.4 | 6/6/Y/6 |
| `04_luau_escapes` | 140,406 | 104,041 | -25.9% | 65,255→47,160 (-27.7%) | 69→20 | 103.4/126.4→69.6/87.9 | 24.7/33.6→18.1/27.1 | 6/6/Y/6 |
| `05_functions` | 250,145 | 232,435 | -7.1% | 122,787→115,769 (-5.7%) | 69→23 | 149.8/231.5→144.0/201.6 | 579.5/859.4→424.7/537.6 | 6/6/Y/6 |
| `06_control` | 173,057 | 159,496 | -7.8% | 80,243→74,362 (-7.3%) | 69→23 | 109.1/133.0→98.0/112.9 | 42.3/50.5→35.5/59.1 | 6/6/Y/6 |
| `07_metatables` | 222,396 | 202,865 | -8.8% | 107,500→98,783 (-8.1%) | 69→23 | 132.9/174.6→121.5/163.9 | 53.1/72.2→44.9/70.1 | 6/6/Y/6 |
| `08_multival` | 184,576 | 161,149 | -12.7% | 87,449→76,562 (-12.4%) | 69→22 | 115.0/146.1→101.0/136.8 | 53.4/64.6→38.8/56.4 | 6/6/Y/6 |
| `09_coroutine` | 183,733 | 158,314 | -13.8% | 88,470→75,702 (-14.4%) | 69→22 | 102.8/150.8→100.7/124.7 | 37.5/52.1→34.9/41.8 | 6/6/Y/6 |
| `10_real_algo` | 170,118 | 153,149 | -10.0% | 80,160→72,780 (-9.2%) | 69→23 | 168.6/204.8→160.1/188.7 | 61.4/75.3→54.6/73.1 | 6/6/Y/6 |

#### 表 C：Legacy 预设对照（`vm` 与 `high`，6-seed 中位数）

| Corpus | Stock Luau ms | `vm` Out B (B→C, Δ%) | `vm` Run ms Med/p90 (B→C) | `high` Out B (B→C, Δ%) | `high` Run ms Med/p90 (B→C) | Eq |
|---|---:|---:|---:|---:|---:|---:|
| `01_micro_print6` | 2.84 | 179,556→186,009 (+3.6%) | 59.7/72.2→43.1/56.0 | 13,405→16,598 (+23.8%) | 6.4/8.0→6.3/7.9 | 12/12 |
| `02_basics` | 3.15 | 213,399→215,266 (+0.9%) | 98.3/152.6→44.9/77.0 | 21,557→25,343 (+17.6%) | 6.3/8.3→6.8/8.2 | 12/12 |
| `03_strings` | 3.82 | 229,710→242,643 (+5.6%) | 139.0/181.9→53.1/79.1 | 34,211→38,117 (+11.4%) | 9.7/15.6→10.7/14.6 | 12/12 |
| `04_luau_escapes` | 3.47 | 181,149→193,715 (+6.9%) | 43.3/63.7→38.1/75.4 | 16,302→19,822 (+21.6%) | 6.1/9.4→6.6/8.9 | 12/12 |
| `05_functions` | 3.90 | 233,765→243,624 (+4.2%) | 573.0/1101.3→143.1/221.0 | 90,320→93,811 (+3.9%) | 94.8/129.6→37.3/43.4 | 12/12 |
| `06_control` | 2.98 | 231,206→241,755 (+4.6%) | 114.1/190.8→86.0/99.4 | 67,458→70,430 (+4.4%) | 21.0/26.5→18.9/22.9 | 12/12 |
| `07_metatables` | 2.88 | 239,902→249,411 (+4.0%) | 131.1/274.8→53.9/90.7 | 73,861→77,382 (+4.8%) | 13.7/16.2→14.3/23.6 | 12/12 |
| `08_multival` | 2.99 | 217,763→232,132 (+6.6%) | 100.1/183.7→48.4/64.9 | 55,420→58,801 (+6.1%) | 11.5/14.1→12.1/14.5 | 12/12 |
| `09_coroutine` | 2.73 | 215,446→230,188 (+6.8%) | 82.0/133.4→44.0/61.9 | 30,966→34,427 (+11.2%) | 7.9/12.0→7.8/8.3 | 12/12 |
| `10_real_algo` | 2.88 | 208,923→221,050 (+5.8%) | 79.7/152.1→46.3/76.5 | 53,214→57,304 (+7.7%) | 14.0/20.6→15.1/20.6 | 12/12 |

**说明（Legacy 预设体积微增原因）**：`vm` 与 `high` 预设默认开启 `guard_prelude` + 能力降级闭环与字符串解密缓存表，固定前置约增加 `~3.2 KB`–`12 KB`（小脚本相对百分比更高），换取沙箱兼容闭环与 **10/10 语料在 `vm` 下的显著运行提速**（如 `05_functions` 中位运行耗时由 `573.0 ms` 降至 `143.1 ms`）。

### 16.4 全量门禁结果与安全边界重申

- **全量验证结果**：`cargo test --release`（**75 passed / 0 failed / 1 ignored** + 水印集成测试 **4 passed**）；`capability_manifest_test.py`（**11/11 PASS**）；`guard_matrix_test.py`（**46/46 PASS**，含 `STAGE_MANIFEST` 双向同步校验）；`prod_audit_gate_test.sh`（**PASS**，4 模式双二进制等效 + 3 项反向自检）；`run_tests.sh`（**169 PASS / 0 FAIL**，含 `wtf-port` **23/23** 与 `R064` 字符串表挂钩 `0/0/0`）；`run_presets.sh`（**370 PASS / 0 FAIL**）；`multiseed.sh`（**FAIL=0**，含 R064 **24 seeds**）；`vm_semantics_regression.sh`（**PASS**）；`defense_audit.sh`（**20 PASS / 0 FAIL**）；`faithful_stub_test.sh`（**5/5 PASS**）；`v15_fingerprint.py`（`example.txt` 与 7 个校准 seed 全部 **32/32 PASS**）；`h2`..`h8`、`r061_bypass_audit.py`、`redteam_probe_regression.py`、`bench_presets.sh`（**56/56 PASS**）全部通过。
- **能力边界重申**：
  1. 静态字符串常量首次求值解码后缓存于对应函数生命周期内的代理表；在真实宿主环境下的运行期动态插桩、内存转储或录制重放仍可观测已求值明文常量，静态混淆对此不作过度安全承诺。
  2. 本轮全部兼容性与守卫矩阵验证基于 stock Luau `0.735`、`LUAU_READONLY_GLOBALS=1` 及受控模拟环境完成；未在特定第三方真实执行器（如 Delta 等）实机验证本轮产物前，不声称该执行器实机已验收。

## 17. Phase 17 安全优先加固（2026-11-23）

> 任务书：`obsucvim_phase17_security_first_tasks.md`（P0–P1、§4–§8）。本阶段原则：安全性优先；不得为体积更小或运行更快而削弱混淆强度、VM 保护、字符串保护或默认 Guard。唯一体积硬门槛：`print(6)\n` 默认 v15 产物 `<= 250,000 bytes`（§0.1/§7）。
> 基线 HEAD：`69dcfc9cb27772b097726225908b3a3e22959b88`（分支 `arena/3f7093d7-obsucvim`，开工时工作树 clean，`.tools/bin` 已重建）。
> 最终 HEAD：`47fcf268342220f0758b98f9af572c2782bae7fd`（Phase 17 主体提交，含本节 §17 初版；其后仅有本节 SHA 回填提交，以 `git log -1` 为准，见 §17.6）。
> 安全评估报告：`docs/phase17_security_evaluation.md`（§6 五维评估 + NOT_RUN 声明）。

### 17.1 能力契约定案（P0，方案 A：纯 Lua fallback）

- `engine/src/vmgen/caps.rs`：`VmPrim::Unpack::lua_bind_expr()` 由两级（`unpack` → `table.unpack`）扩展为 Scheme A 三级回退：`(type(unpack)=="function" and unpack) or (type(table)=="table" and type(table.unpack)=="function" and table.unpack) or <纯 Lua 递归切片解包器 _uf>`。两种 API 同时缺失属于**可兼容场景**（走完整 fallback），不再由 Guard 拒绝；`STAGE_MANIFEST` 按“可替代 API 组”表达该契约，不再把两个 API 标为互不相关、可同时缺失的普通 optional API。
- `engine/src/vmgen/handlers/loadstr.rs`：统一 `LoadStr` 原语名（`BYTE`/`SUB`/`CONC`/`CH`/`UNPK`），消除 `unpack or table.unpack` 旁路。
- `engine/src/vmgen/handlers/mod.rs` + `handlers/nop.rs`：接线 `"Nop" => nop::code(fmt)`，消除 `"Nop"` panic。
- `engine/tests/capability_manifest_test.py`：补上“`unpack` 与 `table.unpack` 同时缺失”场景与“`table.unpack` 存在但返回错误”场景；`table.unpack` 行为验证检查实际返回值、nil 保留、起止索引与空区间（不只 `pcall` 成功）；新增 `f64::MIN_POSITIVE = 2.2250738585072014e-308` 端到端运行期校验。结果 **11 PASS / 0 FAIL**。

### 17.2 Guard 正确性与误报控制（P1）

- `engine/src/guard.rs` + `engine/src/anti/mod.rs`：`abort()` 改为有界 65,536 步 fail-closed（消除无界循环导致的宿主线程挂起）；`STAGE_MANIFEST` 扩展并与 `guard_matrix_test.py` 保持双向精确同步（Rust 单元测试解析 Python 用例表 + Python 反向解析 `anti/mod.rs`，重复/孤儿 ID 即 CI 失败）；7-core `[C]` 检查（`pcall`、`xpcall`、`type`、`select`、`tostring`、`rawget`、`setmetatable`）+ 双 `checkUnpack`。
- `engine/src/vmgen/template.rs`：`PD()` 有界 65,536 步；`pcall_pre_guard` 在 `HBOOT` 之前；`--no-guard` 与 `hbootfail` 分离；IEEE-754 `-0.0` 保留（`1/x == -math.huge`）；`const_loop` 直接 `v = m` 缩放，移除 FTZ 危险的次正规中间值 `2^-1074`。
- `engine/tests/guard_matrix_test.py`：结果分类拆分 `GuardRejected` / `RuntimeError` / `Timeout` / `CompileError` / `HarnessError`；每个篡改用例带 `--no-guard` 差分对照（同输入同环境变体在 `--no-guard` 下通过、`--guard` 下拒绝）；payload 执行标记确认拒绝发生在 payload 运行前；方案 A 下“两种 unpack 均缺失”为允许场景，不再误判为 Guard 成功。结果 **72 PASS / 0 FAIL**（直方图 `{'OK': 26, 'GuardRejected': 39}` + 6 项负向自检）。

### 17.3 VM/ISA/字符串一致性与防篡改矩阵（§4）

- `engine/src/vmgen/isa.rs`：新增 IEEE-754（`-0.0`、`f64::MIN_POSITIVE`、`NaN`、`±Inf`）与二进制字符串往返测试 + 全 43 opcode 的 Luau/Lua5.1 handler 解析测试。
- `engine/src/vmgen/v15.rs`（关键安全修复）：外层 `v2h` 完整性门 `verifyslot ~= sumslot` 此前仅改写未读的 `C[sumslot]` 即返回 `sdone`（篡改可静默执行）；现改为 `C[entryslot]=nil; C[vmslot]=nil`，篡改必在 payload 前 fail-closed。
- `engine/tests/vm_tamper_matrix_test.py`（新增）：基于自有 fixture 的 60-case 黑盒篡改矩阵（3 seed × 20 case：15 VM 篡改 + 5 对照），覆盖 VM carrier、常量/密封字符串、opcode/operand 流、分派/micro-wrapper、完整性校验/阶段机；每 case 记录改动位置、字节数、预期分类、退出码、stdout/stderr、payload 标记。结果 **60 PASS / 0 FAIL**（`TamperRejectedFault 42`、`TamperRejectedPoisonedOutput 3`、`DeadDataIgnored 9`、`SyntaxCompileError 3`、`CleanExecution 3`），机器可读结果见 `engine/tests/results/vm_tamper_matrix.json`。
- 诚实边界：静态产物保护与“运行时明文不可观测”明确区分；字符串首次解码后在函数生命周期内缓存，运行期 hook/dump/重放仍可观测（见 §17.6）。

### 17.4 唯一 250 KB 门槛（§7）与生产隔离（§5）

- `engine/tests/print_size_gate.sh`（新增）：精确输入 `print(6)\n`，生产 release binary + `--preset v15 --dialect luau --seed <seed>`（默认 Guard 开启），6 个固定 seed `{1, 7, 42, 4242, 31337, 999999}` 逐个构建；每 seed 校验完整文件 `<= 250000 bytes`、水印存在、`luau-compile` 解析、stock Luau 执行输出 `6` 且退出码 0；记录每 seed 大小 + SHA-256、binary SHA-256、源码 commit SHA；以最大值作为 gate 结果。
  - seed=1：110,139 B，SHA-256 `f25cdd940285b84e99565035c2fdc0ad8d48a79ea110256df22029d333265b81`
  - seed=7：106,193 B，SHA-256 `57119cc95b7c0a617eaad24d22a3828c3cff0f7ce95369232a2383fe93fec293`
  - seed=42：106,252 B，SHA-256 `cbde6b7716c7f3844ba27babb68c9b113e991627e148ddd8f33243dd43c01a8f`
  - seed=4242：106,484 B，SHA-256 `a6d7d2acabf0753d1f01e0069f6db53d4eca08a70a0ff2c1fb0bea28060986c0`
  - seed=31337：110,332 B，SHA-256 `2873b188e01f202246916da7dd632cdb5012d28615e0b4a6470917ab21063a07`
  - seed=999999：111,351 B，SHA-256 `fce7b850f85854e3bf9c9661ba2f5eab3f2d09018697500630aafc01b0d4c436`
  - **最大值 111,351 B <= 250,000 B（余量 138,649 B），6/6 PASS。** 其他语料不设体积/耗时/RSS 门槛（`ab_compare.sh` 480/480 语义等价仅作观察数据，见 `engine/tests/results/ab_baseline_f583c2c3.{json,md}`）。
- `engine/tests/prod_audit_gate_test.sh`：覆盖全部 12 个 `OBSUCVIM_*` 钩子（含 `OBSUCVIM_VM_RAW` 早退语义：生产版注入后仍输出正常产物且与同配置 audit-hooks 正常构建逐字节一致；audit-hooks 正向调用显式设置 `OBSUCVIM_VM_RAW=1` 并核验 VM 中间表示结构/可解析性）；生产 binary 零审计文件落盘、零 `stderr` 内部标记；`audit-hooks` 构建缺失/过期即自动重建（失败即非零退出）。结果 **PASS**（5 构建模式双二进制等效 + 3 项反向自检）。
- 构建身份：生产 binary SHA-256 `df926259cebeae18eb49f7eca429d6eea30455827c6bde504ff8b380d81e4002`；audit-hooks binary SHA-256 `fe1ed6882ff35ac451e4fb3ccbdf88430ba37907f9dfcaa1c210abdf98761359`；`rustc 1.88.0`；stock Luau（`.tools/bin/luau`）。

### 17.5 真实运行过的验证矩阵（§8，逐项结果）

| 命令 | 结果 |
|---|---|
| `cargo check --all-targets` | PASS（仅既有 warnings） |
| `cargo test --release` | **77 passed / 0 failed / 1 ignored** + 水印集成 **4 passed** |
| `bash engine/tests/run_tests.sh` | **169 PASS / 0 FAIL**（含 wtf-port 23/23） |
| `bash engine/tests/run_presets.sh` | **370 PASS / 0 FAIL** |
| `bash engine/tests/multiseed.sh` | **FAIL=0**（含 R064 24 seeds） |
| `bash engine/tests/vm_semantics_regression.sh` | **PASS** |
| `bash engine/tests/defense_audit.sh` | **20 PASS / 0 FAIL** |
| `bash engine/tests/faithful_stub_test.sh` | **5/5 PASS** |
| `python3 engine/tests/capability_manifest_test.py` | **11 PASS / 0 FAIL** |
| `python3 engine/tests/guard_matrix_test.py` | **72 PASS / 0 FAIL** |
| `python3 engine/tests/vm_tamper_matrix_test.py` | **60 PASS / 0 FAIL** |
| `bash engine/tests/prod_audit_gate_test.sh` | **PASS** |
| `bash engine/tests/print_size_gate.sh` | **6/6 PASS**（max 111,351 B） |
| `bash engine/tests/env_bind_test.sh` | **6 PASS / 0 FAIL** |
| `python3 engine/tests/v15_fingerprint.py example.txt` | **32/32 PASS** |
| `python3 engine/tests/security_fingerprint.py example.txt <src>` | **5/5 PASS**（S1–S5） |
| `python3 engine/tests/h2_diff_audit.py` 等 H2–H8（7 脚本） | 全部 **PASS** |
| `python3 engine/tests/h4_loader_audit.py --enforce-zero` | **ZERO-LOADER GATE: PASS** |
| `python3 engine/tests/r061_bypass_audit.py` | **PASS** |
| `python3 engine/tests/redteam_probe_regression.py` | **PASS** |
| `python3 engine/tests/mobile_print_probe_test.py` | **6/6** |
| `bash engine/tests/wtf_port_test.sh` | **23 PASS / 0 FAIL** |
| `bash engine/tests/ab_compare.sh` | **480/480** 语义等价（观察数据） |
| `git diff --check` / `bash -n` / Python 语法 | 全部 clean |

- **失败与未执行项**：本轮无失败项。`v15_test.sh` 在当前仓库不存在（等价项为 `v15_fingerprint.py` + `v15_short_name_audit.py`，已执行前者）；§6.2 独立评估者流程 **NOT_RUN**（无独立评估者，见安全评估报告 §7）；第三方执行器实机 **未实测**。
- **样例重生成**：`engine/examples/*.v15.luau.lua`（37 例，`gen_examples.sh`，`--seed 42`）与根 `example.txt`（输入 `a = 999\n`，`--preset v15 --dialect luau --seed 42`，104,224 B，SHA-256 `9a9148c463516fc678967c07035fac759cb5402524c2377aea5cbac08a921867`）已用本轮引擎重生成；`example.txt` 通过 `luau-compile` 与 stock Luau 运行验证。

### 17.6 安全边界与未完成项

1. 运行时明文可观测性：字符串首次解码后缓存于函数生命周期内；运行期动态插桩、内存转储、录制重放仍可观测已求值明文。本阶段不承诺“运行时明文不可观测”，不声称“不可逆”“无法破解”。
2. 第三方执行器未实测：全部验证基于 stock Luau + `LUAU_READONLY_GLOBALS=1` 及受控模拟环境；未在特定第三方执行器实机验证前，不声称该执行器实机已验收。
3. §6.2 独立评估（未参与实现者的固定预算评估）：**NOT_RUN**，不以内部静态指标替代。
4. 退役路径未恢复：executor binding、key-host、EVKEY/EVKX、XTOK/XPROBE、EXG 与 executor 专用 tier-2 探测保持移除；未新增 crates.io 依赖（保持 std-only）；`audit-hooks` 保持显式 opt-in，未变为默认生产 feature。
5. 本阶段未为体积/速度削弱任何保护：默认 v15 Guard 保持开启，`--super-insn` 保持 opt-in，字符串密封与完整性校验未降级；`print(6)` 达标靠既有结构余量（max 111,351 B），无需任何保护删减。
