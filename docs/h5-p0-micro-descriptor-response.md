# H5-P0：R057 Micro Descriptor 静态恢复整改

> 日期：2026-10-06
>
> 适用范围：`luraph-rs --preset v15` 纯离线 Luau
>
> 状态：**P0 已闭环；静态 47 组 descriptor table 已退役；R064 已将所有多步 handler recipe 收为 ordered superoperator；同进程 trace 仍可观察**

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

## 4. 诚实边界与后续项

本阶段修复的是 R057 最便宜、最致命的“47 组 descriptor 与解码参数并排静态
交付”路径，不宣称纯离线 Micro-ISA 永远无法恢复。

攻击者仍可通过完整模拟启动过程、动态记录 session blob，或跟踪 micro
interpreter 恢复语义。后续项按优先级处理：

1. **P1：跨-handler/跨 VM-instruction superoperator**，继续研究打破宏 opcode 与标准 Lua
   操作的一对一映射；R064 已把每个多步 handler recipe 收为一个有序 record，但尚未 fusion 相邻 VM instructions；
2. **P1：运行期绑定的下标/钥匙配方**，减少纯 `A+B-C` constant folding；
3. **P2：退役固定 junk/opaque predicate 模板**，不再把可一键清除的死代码
   计入安全强度。

在上述阶段完成前，不使用“不可破解”或“95 分满血”等不可验证表述。

## 5. R064 动态追踪缓解跟进（2026-10-07）

R064 第一层为 10 种内部短序列（LoadK/Move/NewTab/Not/GetUp/SetUp/VarArgC/Jf/Jt/VarArgTab）加专用单记录 fused op。第二层将所有 23 种多 primitive handler recipe 在 `micro_super_body` 中按原序展开到一个 per-shard superoperator record；不跨 VM instruction，也不重排回调/元方法/错误边界。Call/PostCall/Return 本身已是独立 phase，继续保持分离。

同 seed `64075` 的临时白盒计数：原始 recipe **929,567** 条，第一层 **701,915** 条，完整 recipe fusion **412,706** 条；三者均 412,706 次 shard invocation 且输出一致。探针未进入仓库或正式产物。它减少 micro decode/dispatch 的中间记录，但仍留下大规模 handler trace；拥有同进程插桩能力者仍可看到 superoperator 类型、参数及执行顺序，不能声称“阻止 trace”。详细风险口径见 `docs/r064-deep-self-audit-r2.md` 和 `docs/r064-vuln-1-2-research.md`。
