# Phase 17 安全评估报告（Security Evaluation）

> 任务书：`obsucvim_phase17_security_first_tasks.md` §6。
> 基线 HEAD：`69dcfc9cb27772b097726225908b3a3e22959b88`（分支 `arena/3f7093d7-obsucvim`，工作树 dirty，见 HANDOFF §17）。
> 评估环境：Linux x86_64，`rustc 1.88.0`，stock Luau（`.tools/bin/luau`，`LUAU_READONLY_GLOBALS=1`），生产 binary SHA-256 `df926259cebeae18eb49f7eca429d6eea30455827c6bde504ff8b380d81e4002`，audit-hooks binary SHA-256 `fe1ed6882ff35ac451e4fb3ccbdf88430ba37907f9dfcaa1c210abdf98761359`。
> 评估 fixture：仓库自有无害语料 `engine/tests/cases/*.lua`（37 例）+ `print(6)\n` 微基准 + `a = 999\n` 根样例。未使用任何有害 payload，未触碰第三方执行器或真实用户数据。

## 1. 静态可见性（Static Visibility）

- **证据**：`security_fingerprint.py` S1–S5 **5/5 PASS**（`example.txt` 对 `a = 999` 源）：S1 常量不可见（脱壳恢复=0，攻击失败于 `no LCG generator`）；S2 字节码不规则（规整布局不可解析）；S3 指令语义不可见（解释器加密）；S4 自描述消除（最大数字阵列 94 词、无 LCG 定形生成器、OC 形大表 0 个）；S5 动态加载防护（原生性探针 1 处，陷阱 0 / 钥匙污染 1）。
- **证据**：`v15_fingerprint.py` 对 `example.txt` **32/32 PASS**（F1–F32 结构形态回归）。
- **限制**：指纹计数是形态信号，不构成安全证明；静态字符串常量首次求值解码后缓存于函数生命周期内的代理表，运行期动态插桩/内存转储/录制重放仍可观测已求值明文（见 §3）。

## 2. 结构恢复难度（Structural Recovery Difficulty）

- **证据**：H2–H8 审计全部 **PASS**（`h2_diff_audit`、`h3_noise_audit`、`h5_dialect_audit`、`h5_p0_micro_audit`、`h6_window_audit`、`h7_decentralization_audit`、`h8_poison_audit`），`h4_loader_audit.py --enforce-zero` **ZERO-LOADER GATE: PASS**（`payload result survived: YES`），`r061_bypass_audit.py` **PASS**，`redteam_probe_regression.py` **PASS**（Tsn 注入已封堵）。
- **限制**：本报告不以 token 数下降、AST 节点下降、API 引用变少或 wrapper 签名变多宣称逆向难度提升；§6.2 独立评估（未参与该 pass 实现的评估者、固定时间预算）**NOT_RUN**——无独立评估者，不以内部静态指标替代。

## 3. 运行时边界（Runtime Boundary）

- **证据**：`guard_matrix_test.py` **72 PASS / 0 FAIL**（直方图 `{'OK': 26, 'GuardRejected': 39}`，含 6 项负向自检）；`capability_manifest_test.py` **11 PASS / 0 FAIL**（6 seed × `--guard`/`--no-guard` + `--preset vm --no-body`，含 IEEE-754 `-0.0` 与 `2.2250738585072014e-308` 端到端校验）；`mobile_print_probe_test.py` **6/6**（`print` hook clean、loader hook rejected）。
- **边界声明**：字符串首次解码后在运行时缓存中驻留；运行期 hook/trace、内存 dump、录制重放可观测明文。本阶段不承诺“运行时明文不可观测”，不声称“不可逆”“无法破解”，未在任何第三方执行器实机验证。

## 4. 篡改反应（Tamper Reaction）

- **证据**：`vm_tamper_matrix_test.py` **60 PASS / 0 FAIL**（3 seed × 20 case：15 VM 篡改 + 5 对照），机器可读结果见 `engine/tests/results/vm_tamper_matrix.json`。直方图：`TamperRejectedFault 42`、`TamperRejectedPoisonedOutput 3`、`DeadDataIgnored 9`、`SyntaxCompileError 3`、`CleanExecution 3`；表面分布：`vm_carrier_span 6`、`const_and_sealed_string_payload 9`、`opcode_and_operand_stream 9`、`dispatch_and_micro_wrapper 9`、`integrity_checksum_and_phase_machine 12`、`control_dead_data 9`、`control_syntax_damage 3`。
- **关键修复**：`v15.rs` 外层 `v2h` 完整性门（`verifyslot ~= sumslot`）此前仅改写未读的 `C[sumslot]` 即返回 `sdone`，现已改为 `C[entryslot]=nil; C[vmslot]=nil`，篡改必在 payload 前 fail-closed。
- **限制**：篡改矩阵仅覆盖仓库自有 fixture 的有限单点篡改；不要求所有篡改由同一种检查机制捕获；`DeadDataIgnored`（9 例）为已证明不可达的死数据（如 `#"..."` junk 字面量、`EP[i].dispatchS`、`st_slots` chaff），非校验遗漏。

## 5. 随机化质量（Randomization Quality）

- **证据**：`capability_manifest_test.py` `p3_semantics_6_seeds_readonly_globals`（6/6 seed 语义等价、6 个互异构建）；`multiseed.sh` **FAIL=0**（含 R064 24 seeds）；`r064_seed_stability_test.sh` PASS；`print_size_gate.sh` 6 seed 产物 SHA-256 互异（见 HANDOFF §17.4）。
- **限制**：seed 只改变允许随机化的产物结构，不改变程序语义；随机化质量以“同 seed 逐字节可复现 + 异 seed 关键结构互异”判定，不以噪声量判定。

## 6. 体积/性能观察（非门槛，§0.1/§6.3）

- 唯一体积硬门槛：`print(6)\n` 默认 v15 六 seed 最大 **111,351 B**（`<= 250,000 B`，余量 138,649 B），见 `print_size_gate.sh` 与 HANDOFF §17.4。
- 其他语料体积/耗时仅作观察数据（`ab_compare.sh` 480/480 语义等价，原始数据 `engine/tests/results/ab_baseline_f583c2c3.{json,md}`），未设上限，未据此删减 Guard、VM 语义保护、字符串密封或完整性校验。

## 7. 未验证项（NOT_RUN / 环境依赖）

1. §6.2 独立评估者流程：**NOT_RUN**（无独立评估者）。
2. 第三方执行器实机兼容：**未实测**（仅 stock Luau 0.735 + 受控模拟环境）。
3. 运行时明文不可观测性：**不承诺**（见 §3）。
