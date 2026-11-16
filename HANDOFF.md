# 交接文档（HANDOFF）

> **仓库现在只有混淆引擎。** 2026-11-15 第二次规整：除本文件外不再保留任何 markdown；
> 控制台（`web/`）、托管服务（`services/`）、研究数据（`docs/`）、证据链（`artifacts/`）、
> 发布包（`release-assets/`）与 Render/Docker 部署文件已从工作树删除。测试仍需的 4 个靶标
> 已移入 `engine/tests/fixtures/`。需要旧文件时从基线提交 `68b9187` 取回（见 §10）。
>
> 最后更新：2026-11-15。所有数字来自本沙箱实际构建与执行，不是旧文档转抄。
> 当前分支以会话说明为准；仓库是 **shallow clone**，本地只能看到基线 `68b9187`。

---

## 1. 项目是什么

**obsucvim**：Luau / Roblox 混淆引擎。Rust 把输入脚本编译成私有字节码，再由随产物分发的
「Lua 写的加密解释器」执行；可选 v15 结构克隆、环境检查、executor 认证绑定与 key-host 转录链。

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
  src/vmgen/            compiler / template / v15 / isa / envcheck / exguard /
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

常用旗标：`--env-check`、`--bind-env roblox`、`--bind-executor` / `--no-bind-executor`
（后者仅原生开发构建可用）、`--super-insn`（默认关）、`--vm-seal-strings` /
`--no-vm-seal-strings`，以及 `--no-mangle/-minify/-strings/-flatten/-junk/-numbers/-body/-antidbg/-guard`。

- `--key-host delta` 自 R070 起恒开（仅兼容保留）；`--no-key-host` 已移除。
- 绑定态产物（`--bind-env` / `--env-check` / `--bind-executor`）**不进运行等价矩阵**：
  只做语法校验 + 裸 Luau「干净拒绝」验证；矩阵与示例用 `--no-bind-executor`。
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
4. **真机未验证**：只在自编 Luau 0.735 + 宿主 mock 上跑过；Roblox / Delta / Synapse
   等真实 executor 的正向运行没做。EVKEY 失败形态是「静默失步挂起」，可能被同宿主旁录。
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
6. 代码里没有注释（历史剥离）；设计看 git 历史，不要惊讶。
7. Shell 小坑：`/tmp/t_$s_$d.lua` 会把 `$s_` 当变量名，用 `${s}_${d}`。
8. 守卫/探针改动要过「净化后形态」：标识符会被改名，字符串字面量不会；
   外部对象成员只能 `["字符串键"]` 访问。
9. `strseal` 三处镜像（Rust encode/decode、v15 micro-op、handlers/loadstr.rs）必须同步改。

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

`68b9187` 是整理前基线；仓库是 shallow clone，取不到更早历史。需要更早的文档/版本时去远端
仓库或完整克隆查。

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
3. **数字字符串算术语义丢失（v15/vm）。** `print(("5")+0)` 源码 `5`，产物报
   `attempt to perform arithmetic on a string value`；`1+"2"`、`-"5"`、`"10"/"2"` 同理。
   `handlers/mod.rs::bin_op_body` 只接受 `type==number`，未做 Lua 的 tonumber 强制转换。
4. **括号调用截断语义丢失（所有 preset）。** `local function f() return 1,2,3 end;
   print((f()))` 源码输出 `1`，产物输出 `1 2 3`；`return (f())`、`select('#',(f()))`
   同样错。解析器 `parser.rs` 的 `(` 分支直接返回内部表达式，AST 无 Paren 节点，
   打印/编译器无从恢复“截断为一个值”。
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
12. **测试给出错误安全感。** `string_primitive_hook_test.sh`/D17 只换副本表；
    `run_tests.sh` 不覆盖上面 §12.3/12.4 的语义；`v15_fingerprint.py` 对 basics
    仅 21/32，`defense_audit.sh` 18 PASS / 3 FAIL（D3 指纹、D18 E4；D13 是本轮
    PATH 下 debug 构建未编成）。现有 169/2 是“现有语料”的绿，不是产品完备性。

### P2（工程/运维）

13. **RNG 只有 31 位状态**（Park-Miller，`rng.rs`）；默认 seed 取系统时间纳秒，
    不是密码学随机。种子碰撞/可预测性不适合作为“每用户唯一”的安全边界。
14. **调试环境变量是生产脚枪**：`OBSUCVIM_KEY_MANIFEST` 可把密钥写文件，
    `OBSUCVIM_VM_RAW`/`OBSUCVIM_VM_TSRC` 可导出中间形态；发布构建应编译期禁用。
15. **重建脚本从 GitHub `master` 拉 Lua 5.1.5 源码**（非固定 commit/无校验和），
    供应链与可复现性弱；Rust/Luau 版本已钉。
16. **无 CI、无 fuzz、无 benchmark、无默认产物正向测试**；20,534 行 Rust 中
    输入可达的 `panic!/assert!/unwrap` 不少，建议加入结构化限制和 fuzz 门禁。

### 仍然成立的部分

- 37 语料 × v15/vm 的运行等价与语法矩阵 169/2，`cargo test` 65 绿；
- v15 同一 seed 逐字节确定，随机化覆盖面广，能挡住 grep/字符串直读/初级 dump；
- VM 字符串密封的 legacy 解码 bug 已修，`--vm` 74/74；
- 协程、元表、多值、upvalue、变参等语料语义通过（但未覆盖上述两个核心缺失）。

**最低限度的下一步**：先修 §12.3/12.4 并补回归用例；再决定“单 executor 绑定”是
产品策略还是缺陷（至少给出 `--key-host` 多档案/通用模式）；随后做真实 Delta 正向
验收与性能预算；最后重写 S1–S5/D17 的攻击口径，避免继续用副本表测试宣称安全。
