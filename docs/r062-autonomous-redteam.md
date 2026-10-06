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

### P0（KeyEmitter 局部）：常量折叠后可直接恢复多组密钥值

对已推送的 `handoff/R062_target.luau.lua` 做静态扫描，定位到四个 96 项的局部数值表（3 张 28-bit fragment 表 + 1 张小乘数表）。查表索引仍是 `[A+B-C]` 纯字面量形式：

```text
[A+B-C] 下标：159 处
常量折叠后的不同槽号：90 个
槽号范围：1..96

离线验证：折叠下标 → 查明文数字表 → 计算表组合式
table-driven mod expression：52 个
与本次构建 key manifest 精确匹配：48 个
```

这不只是“还原索引后更容易看数据流”：表值本身在输出中是明文数字，组合 recipe 也只是加法/乘法/取模。一个普通常量折叠器加数组读取器就能静态恢复多组 KeyEmitter 值，包括若干常量流/原型流参数。**这是 KeyEmitter 密钥封装层的直接静态旁路（局部 P0）；它仍不等于动态 SALT/HBD 等所有运行期约束都被绕过，也不应夸大成完整源码已自动还原。**取证脚本只在本地临时文件运行，没有把样本密钥值写入仓库。

**候选修补方向（需设计审批）：**必须同时处理明文表值和可折叠下标；只改 `[A+B-C]` 外形不够。研究分阶段密封表项、分散即用，并使用读取/构建两侧真实消费的非零 boot/session 状态；不新建统一 getter、公开索引表或可批量 dump 的明文表。KeyEmitter 在 HBOOT/RT/BSS 等多个初始化阶段都会被使用，必须先逐类核对可用状态时序，避免未初始化 key、共享解码咽喉点或新的固定 oracle。

### P1-B：编译器合成的名称常量不经过 AST 封存

源码字面量会变成 `SealedStr`；但 `Expr::Dot.name`、`Expr::Method.name` 与未解析为局部变量的 `Expr::Ident.name` 是 AST 字段，不是 `Expr::Str`。`compiler.rs` 在编译 `Dot`、`Method`、全局读取/赋值时仍直接调用 `kidx(Const::Str(name.as_bytes()))`。因此像 `game:GetService(...)` 中的成员/全局名称不在 R062 第二层封存范围；常量区的原有 at-rest masking 仍保护静态文件，但首次触碰后 CDEC 会把这些普通字符串缓存为明文。

这是**覆盖面限制**，不是新出现的字面量明文泄漏。若要扩展到名称，需要改全局/属性读取协议，可能影响 `__index`、执行顺序和性能，不能只做一个遍历器就宣称修好。

### P1-C（条件性动态观察点）：LoadStr 的 native `char` 汇合面

`M_LOAD_STR` 对每个已解码字节调用一次字符串元表的 `char` 函数；因此在提供 native-function hook 的 executor/调试环境里，若 `hookfunction(string.char)` 能拦截元表返回的同一函数对象，单钩子即可逐字节收集所有已经触碰的封存字符串。现有 `charhook_test.sh` 只覆盖 `setfenv` 下替换 `env.string.char`，而 `("")["char"]` 正是为躲这类环境表替换设计；它**没有**验证 executor 的函数对象级 hook。Lua 5.1 探针确认字符串元表索引可走同一个 `string.char` 函数对象；R062 Luau 目标的 executor 级 hook 仍需在真实目标环境实测，因此标成条件性风险，不夸大成所有 Luau 沙箱都能直接 hook。

若要提高成本，可研究按站点分散字节物化原语（而不是一条统一 `char` 路径）；这只能让单 API hook 少收一部分值，不能阻止完整动态跟踪，也可能引入新的固定汇合点。

### P1-D：入口 `setmetatable` 可透明截获整个模块表

R062 产物从 `setmetatable({...}, {}):<entry>(...)` 开始。用 loader 控制的环境把 `setmetatable` 换成“先浅拷贝参数表、再调用原函数”的透明 wrapper，样本仍正确运行（返回 `1713`），guard 未报错；wrapper 在首次入口时已捕获 **216 个字段**，其中 93 个数字键、187 个函数值和 16 个字符串值。复现脚本：`luraph-rs/tests/r062_root_table_hook_probe.sh`。它不直接解出全部用户字节码，但让攻击者在启动前一次性拿到整个模块入口表及其中的闭包/载体字段，是比逐点插桩便宜得多的动态观察点。

这依赖攻击者能控制 chunk 的加载环境（如自建 loader/executor）；普通 Luau 全局表只读不等于加载环境不可控。潜在修法是研究不经过单一全局 `setmetatable` 的入口构造/入口路由，但会改变 v15 外形且必须保留 chunk return 与环境绑定语义，需先审批。

### P2-A：固定 seed 下字符串层跨构建可复现

site 参数由 `Rng::new(seed ^ domain_tag)` 按 AST 遍历顺序抽取。固定 `--seed` 是支持的可复现功能；同 seed、同 AST 会重复相同的 site 参数与密文。默认的 time seed 通常会变化，但客户若长期固定 seed，会降低字符串层的跨构建差异。

## 验收证据补充

`r062_string_seal_experiment.sh` 的 canary grep 单独看只证明最终 Lua 源文本没有直接 canary；外层 VM carrier 本来就加密，不能独立证明 CDEC 之后的常量仍是密文。已新增 `sealed_literal_stays_ciphertext_after_proto_and_constant_unmasking` white-box 单测：解开测试原型 blob 与常量 keystream，确认常量区含 site ciphertext、没有原字节，并且编译器启用了 `LoadStr`。它验证的是 serializer/compiler 路径，不是完整输出的不可逆性，也不覆盖动态运行态。

## 建议的下一项（R063 候选，尚未实施）

研究 per-build / per-proto 的 codec dialect，并让字节流参数真实消费函数/指令上下文；保留密文 C 池、不加入全局 getter、不加固定失败分支、不缓存全量明文。若可行，限制运算中间值低于 `2^53` 并用独立配对编码/解码测试证明等价。

**风险与预期：**这仍只是增加跨构建工具复用成本；所有 decoder 与参数都在客户端，逆向当前构建仍可恢复。若仅随机替换 257/65521 常数，却留下相同固定结构，收益有限；若把 key 混进错误的中心状态或增加明文缓存，则会新造旁路/观察点。名称常量扩展还涉及全局和 metamethod 语义。因这些方案会改变默认 codec/读取协议与性能面，先列为审批候选，不在本阶段擅自启用。
