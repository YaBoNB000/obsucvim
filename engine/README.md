# engine —— Rust 引擎本体

Luau 混淆器的核心：源码 → 私有字节码 → 随产物分发的加密解释器。
**std-only，零第三方 crate**（沙箱拿不到 crates.io，这是设计约束）。

- 版本/锚点等「现在到哪了」→ [`../docs/CURRENT-STATE.md`](../docs/CURRENT-STATE.md)
- 逐轮决策台账 → [`../docs/engine/ENGINE-VERSION.md`](../docs/engine/ENGINE-VERSION.md)
- 改 `src/vmgen/` 之前必读 → [`../docs/engine/vm-l6-implementation.md`](../docs/engine/vm-l6-implementation.md)
- 干活规矩/验收 → [`../HANDOFF.md`](../HANDOFF.md)

---

## 构建

```bash
export PATH=/home/user/obsucvim/.tools/bin:$PATH   # 工具链 gitignored，见 HANDOFF §4
CARGO_NET_OFFLINE=true cargo build --release        # 产物 target/release/obsucvim
cargo test                                          # 单测 + CLI 集成测试
```

WASM（控制台/托管用）：`../web/tools/build-engine-wasm.sh`（编译 + 锚点校验 + 安装）。

## 模块地图（`src/`）

| 阶段 | 模块 | 作用 |
| --- | --- | --- |
| 前端 | `lexer.rs` `parser.rs` `ast.rs` `symtab.rs` | Luau 词法/语法（`//`、`continue`、复合赋值、反引号插值、类型注解剥离） |
| 前端 | `desugar.rs` `printer.rs` | 循环脱糖、AST 打印（`%.17g` 浮点口径） |
| L1 | `mangle.rs` `minify.rs` | 名称混淆、压缩 |
| L2 | `strings.rs` | 字符串加密（逐字节状态链） |
| L3 | `flatten.rs` `junk.rs` | 控制流扁平化、垃圾代码注入 |
| L4 | `numbers.rs` | 数值字面量改写 |
| L5 | `body.rs` | 整段程序加密容器 |
| L7 | `antidbg.rs` `guard.rs` `anti/` | 反调试、环境守卫前奏、金丝雀/校验和/行号重映射/时间陷阱 |
| L6 | `vmgen/compiler.rs` | AST → 私有字节码（噪声、死寄存器、常量散布、`call_window_mask` 不变量） |
| L6 | `vmgen/isa.rs` | 指令集、操作数置换、码位映射 |
| L6 | `vmgen/handlers/*.rs` | **43 个指令 handler**（各自加密成碎片，运行时装配） |
| L6 | `vmgen/template.rs` `vmgen/v15.rs` | 解释器骨架生成、去中心化尾链/蹦床、v15 结构装配 |
| L6 | `vmgen/envcheck.rs` | 环境检查（tier-1 纯 Luau 语义 / tier-2 Roblox+executor 档） |
| L6 | `vmgen/exguard.rs` | executor 认证守卫 + 宿主钥（R070 起恒开） |
| L6 | `vmgen/fuse.rs` `strseal.rs` `strpool.rs` | 超指令融合、字符串封存、字符串池 |
| 基础 | `rng.rs` `rng_check.rs` `main.rs` | 种子 PRNG（同 seed 逐字节可复现）、CLI 与选项、输出水印 |

> `src/` 内**没有注释**（2026-10-11 整体剥离 3,155 条）；设计说明全在 `../docs/engine/`。
> 恢复带注释版本：`git checkout 0358615 -- engine/src`（整理前的等价路径为 `obsucvim/src`）。

## 测试（`tests/`）

| 脚本 | 内容 | 何时必跑 |
| --- | --- | --- |
| `run_tests.sh` | 官方矩阵：全语料（37 例）× 方言 ×（`high` + `vm` + `v15`），含绑定态门控用例 | 任何改动 |
| `run_presets.sh` | 预设 × 全语料 | 改预设/开关时 |
| `multiseed.sh` | 多种子回归（默认 1 7 4242 31337 999999） | **改 `vmgen/` 必跑** |
| `defense_audit.sh` | 防御审计 D1–D20 | 改发射面时 |
| `security_fingerprint.py` / `v15_fingerprint.py` | 轮廓/安全指纹（S1–S5、32 条结构指纹） | 改发射面时 |
| `charhook_test.sh` `string_primitive_hook_test.sh` `executor_guard_test.sh` `faithful_stub_test.sh` `wtf_port_test.sh` `wtf_port_tier2_test.sh` | 各轮攻击面回归闸 | 触及对应机制时 |
| `call_window_noise_test.sh` | H3-6 调用窗口噪声不变量 | 改编译器时 |
| `gen_examples.sh` | 重生成 `examples/` | **改 pass 必跑** |

工具链要求：`lua51` / `luau` / `luau-compile` 在 `.tools/bin`（脚本会自动回退查找）。

## 目录

```
src/       引擎源码（71 个模块）
tests/     矩阵、审计、红队回归、语料（cases/）
tools/     工具链重建、剥注释脚本、luau CLI 主程序
examples/  全语料混淆示例（改 pass 必须重生成）
```
