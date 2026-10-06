# H5-P0：R057 Micro Descriptor 静态恢复整改

> 日期：2026-10-06
>
> 适用范围：`luraph-rs --preset v15` 纯离线 Luau
>
> 状态：**P0/H5-2 已闭环；静态 descriptor table、默认外层固定壳和 raw micro opcode baseline 已退役**

## 1. R057 失守结论

对 R057 和生成器复核后，攻击反馈的三个静态面均成立：

1. R057 可匹配约 26 处固定平方/取模恒真谓词；
2. 可匹配约 120 个纯字面量 `A+B-C` 下标；
3. 最致命的是每个 micro shard 静态发射约 47 组：

```text
{ masked_program, mask, step }
```

R057 有四个 shard，因此攻击者可同时取得四套 descriptor、对应 mask/step，
并用同文件内的 micro interpreter 直接恢复宏 opcode 语义。多 shard 只增加重复
解析量，没有阻止静态映射。

H4 的 zero-loader、block window、Closure/Call 去中心化和分散投毒仍然有效，
但它们不等价于“静态 Micro-ISA 不可恢复”。此前对静态恢复成本的判断偏高。

## 2. H5-P0 已实施

### 2.1 退役静态 descriptor table

以下结构已从生成器和 TSRC 中删除：

```text
_mdN[index] = { { program bytes... }, mask, step }
_mo[] checksum traversal
wrapper -> _mdN[index] -> _mxN(...)
```

现在每个 shard 的全部 descriptor 被编码为一个 packed binary carrier：

```text
[length | micro bytes | authenticated random padding]...
```

wrapper 只携带 shard 路由和 carrier offset，不再持有 program table、mask 或
step。

### 2.2 At-rest 加密与启动期重密封

每个 shard 使用独立的：

```text
MICRO_AT_KM_n
MICRO_AT_KC_n
MICRO_AT_SEED_n
```

进行构建态 LCG 加密。启动时按单字节窗口恢复，并立即使用运行期：

```text
SALT + CHQ + per-shard session coefficient/step
```

重密封为 session blob。转换过程只出现标量 `_v`，不创建完整明文 descriptor
数组；构建态 carrier、LCG 状态和临时输出引用在重密封后清除。

### 2.3 使用点即时解码

每个 shard 保留独立 micro interpreter。解释器根据 wrapper offset 直接从
session blob 解出当前 descriptor 的 length 和当前三元组，执行后继续清理：

```text
T[1..3]
O[1..4]
```

没有新增统一 descriptor getter，也没有重新引入 Lua 源码 loader。

### 2.4 完整性接线

Micro checksum 现在覆盖 packed carrier 的：

- length；
- micro bytes；
- 随机 padding。

启动期重密封时同步折叠恢复字节，再与 `CHQ` 合并进入既有 `MZD` 分散投毒
lane。修改 carrier 不会只影响一个无关密文表。

## 3. 验收

新增：

```bash
python3 luraph-rs/tests/h5_p0_micro_audit.py
```

固定检查：

- 同 seed 确定；
- `3..5` 个 shard 的 at-rest/session key 独立；
- SALT/CHQ session reseal 接线；
- 模板中无 `micro_variant_assign`；
- TSRC 中旧 `_mdN[index]` 与 `{{program},mask,step}` 提取器命中为 0；
- 构建态 carrier/钥匙引用清场；
- 双 seed 闭包、递归、变参运行等价；
- zero-loader 保持 0。

当前结果：

```text
旧 `_mdN[index]` 命中：0
旧 `{{program},mask,step}` 命中：0
内部 Loader：0
cargo test：28 passed / 0 failed / 1 ignored
run_tests：169/169
run_presets：370/370
multiseed：FAIL=0
H2/H3/H4/H5/H6/H7/H8：PASS
H5-P0 audit：PASS
防御审计：PASS 21 / FAIL 0
R057 同源码/同 seed：196299B → 204355B（+4.10%）
R058：187607B，m=134034，r=9827
```

## 4. H5-2：第二轮静态特征整改

### 4.1 全 shard 动态 Micro-op 映射

每个 shard 现在具有独立的模 `251`：

```text
op_mul / op_add / op_ctx
```

解释器先计算函数上下文 `ox = f(E.vd)`，再将解出的 micro opcode 映射到
`(op*mul+add+ox)%251`；语义分支目标也通过同一 shard 专属表达式生成。第一
套解释器不再保留 `op == raw literal` 基线，其余 shard 也不共享变换参数。

### 4.2 MkStr 绑定函数与指令上下文

编译器原有 `mk1*a+mk2` 只作为第一层。BSS 根据真实 MkStr wire，在 per-function
lane permutation 归一前将以下状态叠加到 b/c/d：

```text
qf + proto id + absolute PC
```

运行时从 `E.sk/E.si` 镜像恢复。仅提取一组全局乘数和加数不能再解出全部短
字符串。

### 4.3 默认外层壳退役

默认 v15 不再生成固定 staging/CPS module shell：

```text
C[1]..C[15]
b[n](b)
setmetatable(...):entry()(...)
外层 range tree + continue forest
```

解释器完成既有 mangle/string/number passes 后直接接收 encrypted carriers。
绑定态暂保 compatibility shell，以免破坏 module-table 环境捕获。

同源码、同 seed 实测：

```text
R058：187607B → 137102B（-26.92%，减少 50505B）
R059：121992B，m=249153，r=5342
```

专项门禁：

```text
python3 luraph-rs/tests/h5_context_audit.py
python3 luraph-rs/tests/h5_shell_audit.py <artifact>
```

最终矩阵：

```text
cargo test：28 passed / 0 failed / 1 ignored
run_tests：169/169
run_presets：370/370
multiseed：FAIL=0
H2/H3/H4/H5/H6/H7/H8/H5-P0/H5-2：PASS
防御审计：PASS 21 / FAIL 0
R059 luau-compile + 运行等价：PASS
```

### 4.4 R060 四环与双完整性门禁

R059 的 `osum/osum2 → OS/OS2 → CK/CK2 → BD fan-out` 未被删除；R060 将其
固化为 `r060_integrity_audit.py`。Micro shard 从随机 3..5 改为精确四套，模数
固定为 `251/241/239/229`，但 multiplier/addend/context 与 carrier/session key
继续逐构建重掷。

```text
R060：138615B，m=251932，r=4227
```

## 5. 诚实边界与后续项

本阶段修复的是 R057 最便宜、最致命的“47 组 descriptor 与解码参数并排静态
交付”路径，不宣称纯离线 Micro-ISA 永远无法恢复。

攻击者仍可通过完整模拟启动过程、动态记录 session blob，或跟踪 micro
interpreter 恢复语义。下一阶段按优先级处理：

1. **P1：函数级 Superoperator**，打破宏 opcode 与标准 Lua 操作的一对一映射；
2. **P1：运行期绑定的下标/钥匙配方**，减少纯 `A+B-C` constant folding；
3. **P2：退役固定 junk/opaque predicate 模板**，不再把可一键清除的死代码
   计入安全强度。

在上述阶段完成前，不使用“不可破解”或“95 分满血”等不可验证表述。
