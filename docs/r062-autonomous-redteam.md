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

## 已确认的边界

- 默认 VM 构建中，普通非空 `Str/LongStr` 字面量会封存；空字符串不封存。
- 全局名、点成员名及编译器合成字符串不属于源码字面量 AST，本阶段不保证封存。
- 用法执行后，明文必然进入目标寄存器；完整动态跟踪仍可观察。
- R062 的测试只证明产物没有直接包含测试 canary、运行语义等价；不证明拥有完整 VM 解码器的分析者不能还原。

## 建议的下一项（R063 候选，尚未实施）

研究 per-build / per-proto 的 codec dialect，并让字节流参数真实消费函数/指令上下文；保留密文 C 池、不加入全局 getter、不加固定失败分支、不缓存全量明文。若可行，限制运算中间值低于 `2^53` 并用独立配对编码/解码测试证明等价。

**风险与预期：**这仍只是增加跨构建工具复用成本；所有 decoder 与参数都在客户端，逆向当前构建仍可恢复。若仅随机替换 257/65521 常数，却留下相同固定结构，收益有限；若把 key 混进错误的中心状态或增加明文缓存，则会新造旁路/观察点。因该方案会改变默认 codec 与性能面，先列为审批候选，不在本阶段擅自启用。
