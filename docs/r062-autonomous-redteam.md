# R062 自主红队：字符串封存路径

> 日期：2026-10-06
>
> 范围：R062 VM 默认开启的 AST literal seal / `LoadStr`。
>
> 结论口径：本层目标是让常量池直接转储不出现原始用户字面量；不是让纯离线客户端获得不可提取的秘密。

## 发现：固定 `LoadStr` recipe 可跨样本复用（P1）

R062 编译器对每个非空源码字符串生成密文常量，并把 `seed/step` 放入 `LoadStr` 的第三、第四操作数：

```text
LoadStr(dst, const_index, seed, step)
```

每字节变换固定为：

```text
state = (state * 257 + step + i) mod 65521
cipher[i] = plain[i] + state mod 256
```

模板侧执行完全镜像的减法。对应实现位于：

- `luraph-rs/src/vmgen/strseal.rs`：编译期编码；
- `luraph-rs/src/vmgen/compiler.rs`：cipher 常量与 seed/step 操作数；
- `luraph-rs/src/vmgen/template.rs`：微码 `M_LOAD_STR`。

**绕过边界：**只 dump 解码后的 C 常量表时，得到的是第二层密文；但若分析者已恢复 VM 的 `LoadStr` 线码及四列操作数，就能一次性读出每个 `const_index/seed/step`，用固定 recipe 离线还原全部封存字符串。工具可以跨构建复用 recipe。这是字符串封存层的静态批量解码面，不等于原始 VM 已被完全反编译。

当前没有单独的全局 Lua `decode()` 函数或共享明文字符串表；`LoadStr` 走现有随机 opcode/micro dispatch。因此我把它定为 **P1 静态成本不足**，不虚报为直接泄漏明文或整个 VM 的 P0。

## 另外发现的静态分析面

### P1-A：KeyEmitter 查表下标仍可做常量折叠

对已推送的 `handoff/R062_target.luau.lua` 做静态扫描：

```text
[A+B-C] 形式：159 处
常量折叠后的不同槽号：90 个
槽号范围：1..96
```

这与 R061 报告的 P1-A 同族：普通 AST constant-fold pass 可还原 KeyEmitter 查表下标，降低密钥碎片表的数据流分析成本。它本身不直接暴露表里的密钥值，但容易自动化清除，R062 字符串封存并未缓解。

**候选修补方向（需设计审批）：**只改安全关键查表槽的编码下标，使用编码端与读取端严格镜像、实际消费的非零 boot/session 状态；不采用 `state-state` 假依赖，不新增统一 getter/公开索引表。需要逐类确认状态在 KeyEmitter 各生成阶段的可用时序，避免依赖尚未初始化的 SALT/CHQ/HBD。

### P1-B：编译器合成的名称常量不经过 AST 封存

源码字面量会变成 `SealedStr`；但 `Expr::Dot.name`、`Expr::Method.name` 与未解析为局部变量的 `Expr::Ident.name` 是 AST 字段，不是 `Expr::Str`。`compiler.rs` 在编译 `Dot`、`Method`、全局读取/赋值时仍直接调用 `kidx(Const::Str(name.as_bytes()))`。因此像 `game:GetService(...)` 中的成员/全局名称不在 R062 第二层封存范围；常量区的原有 at-rest masking 仍保护静态文件，但首次触碰后 CDEC 会把这些普通字符串缓存为明文。

这是**覆盖面限制**，不是新出现的字面量明文泄漏。若要扩展到名称，需要改全局/属性读取协议，可能影响 `__index`、执行顺序和性能，不能只做一个遍历器就宣称修好。

### P2-A：固定 seed 下字符串层跨构建可复现

site 参数由 `Rng::new(seed ^ domain_tag)` 按 AST 遍历顺序抽取。固定 `--seed` 是支持的可复现功能；同 seed、同 AST 会重复相同的 site 参数与密文。默认的 time seed 通常会变化，但客户若长期固定 seed，会降低字符串层的跨构建差异。

## 验收证据补充

`r062_string_seal_experiment.sh` 的 canary grep 单独看只证明最终 Lua 源文本没有直接 canary；外层 VM carrier 本来就加密，不能独立证明 CDEC 之后的常量仍是密文。已新增 `sealed_literal_stays_ciphertext_after_proto_and_constant_unmasking` white-box 单测：解开测试原型 blob 与常量 keystream，确认常量区含 site ciphertext、没有原字节，并且编译器启用了 `LoadStr`。它验证的是 serializer/compiler 路径，不是完整输出的不可逆性，也不覆盖动态运行态。

## 建议的下一项（R063 候选，尚未实施）

研究 per-build / per-proto 的 codec dialect，并让字节流参数真实消费函数/指令上下文；保留密文 C 池、不加入全局 getter、不加固定失败分支、不缓存全量明文。若可行，限制运算中间值低于 `2^53` 并用独立配对编码/解码测试证明等价。

**风险与预期：**这仍只是增加跨构建工具复用成本；所有 decoder 与参数都在客户端，逆向当前构建仍可恢复。若仅随机替换 257/65521 常数，却留下相同固定结构，收益有限；若把 key 混进错误的中心状态或增加明文缓存，则会新造旁路/观察点。名称常量扩展还涉及全局和 metamethod 语义。因这些方案会改变默认 codec/读取协议与性能面，先列为审批候选，不在本阶段擅自启用。
