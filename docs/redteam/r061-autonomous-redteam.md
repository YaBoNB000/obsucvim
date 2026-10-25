# ⚠️ 战役记录（历史）

> 本文件记录当时一轮红队/加固的完整过程与结论，路径与版本号均为当时口径。
> 路径对照见 [`../README.md`](../README.md) §3；现状见 [`../CURRENT-STATE.md`](../CURRENT-STATE.md)。

---

# R061 自主红队：R058 易绕过面审计

> 日期：2026-10-06
>
> 基线：R058 / commit tree `6d74031`
>
> 原则：先找“强防护的单点旁路”，安全修复已知缺陷；新架构只提交审批方案。

## 1. 已确认并修复：OS/OS2 单分支旁路（P0）

R058 的 operand 完整性本体很强：编译期计算四列无权和与全局位置加权和，
解析期恢复 `CK/CK2` 并与 `OS/OS2` 对照。但两个校验最终汇入同一个显式分支：

```text
if ck ~= OS or ck2 ~= OS2 then
    BD += poison
end
```

攻击者无需理解四列折叠，只需删除这个分支，即可同时绕过两条校验。后续
block/constant/metadata 六面 fan-out 再强，也收不到非零差值。

### R061 修复

- 删除 `if ck~=OS or ck2~=OS2`；
- 无条件计算两个 32-bit 差值 `d1/d2`；
- 将每个差值拆为 low/high 16-bit；
- 分别使用 `PS/PT/KM/KC + proto index` 生成四条独立活系数；
- 生成 per-prototype `iz1/iz2`；
- 每个 prototype 再混入自身 `bz`，进入既有六面 fan-out；
- `iz1/iz2` 编码完成后立即清除。

干净程序 `d1=d2=0`，语义不变。现在不存在一个同时关闭 Ou/De 的条件跳转。

新增门禁：

```bash
python3 engine/tests/r061_bypass_audit.py
```

## 2. 仍存在的静态/旁路面

### P1-A：纯字面量下标仍可折叠

R061 审计语料中仍发现约 154 个：

```text
[A + B - C]
```

它们不会直接泄漏全部 key，但会显著降低 KeyEmitter 表、slot 和调用关系的恢复
成本。

**建议方案（需审批）**：只对安全关键索引引入 boot-state 编码索引。编码侧与
使用侧真实消费 `HBD/CHQ/SALT` 派生的非零状态，不采用可相消的 `state-state`
装饰。普通非关键索引不处理，避免体积和性能失控。

### P1-B：Micro interpreter 语义体仍可作为基线

Packed Carrier 已关闭静态 47 组 descriptor 表，但 micro interpreter 中每种
micro-op 的语义实现必须存在。攻击者完整模拟 at-rest 解码与 session reseal 后，
仍可建立 recipe → semantic body 映射。

**建议方案（需审批）**：函数级安全 Superoperator 小步试验，仅在基本块内融合
无异常边界、无 Call/Return、无 metamethod 顺序风险的短序列，例如纯寄存器
Move/LoadK 与后续确定性写回。不得先做全 Proto Mega-VM。

### P1-C：Micro checksum 仍有可识别汇合点

`MZD = actual - expected` 已进入 RT 多面投毒，但赋值点本身可定位。直接把 MZD
改为 0 能绕过 checksum 的投毒效果，不过被修改的 descriptor carrier 本身仍会
改变执行语义。

**建议方案（需审批）**：把 micro checksum 分成每 shard 独立 lane，并分别进入
该 shard session mask、route delay 和 block seed，取消单一 MZD 汇合变量。

### P2-A：固定 junk/opaque predicate 可批量清除

`junk.rs` 的纯局部平方恒真式与外层 opaque predicate 不承担真实密钥或状态，
可被 DCE/SMT 快速删除。它们主要增加体积，不应再计入安全强度。

**建议方案（无需新增假恒等式）**：逐步减少纯死 junk；仅保留已存在的
key-carrying chaff，或让噪声结果真实进入独立完整性 lane。禁止使用数学错误的
OpaqueZero。

### P2-B：外层 staging/CPS 壳可机械剥离

R058 保留 `C[1..15] / b[n](b)` 兼容壳。此前移除时节省约 50KB，但改变了文件
入口形态并可能影响 chunk return 兼容，因此已按要求回退。

**建议方案（需审批）**：若再次处理，使用轻量 `return (function(...) ... end)(...)`
保留顶层多返回，且先为 bind-env/bind-executor 设计等价捕获层，不直接删除。

## 3. 未采用方向

- 内存耗尽/永久锁死：稳定 oracle + 客户端 DoS；
- 一次性 Mega-VM 全 Proto 替换：中央 hook 咽喉点及 closure/coroutine 语义风险；
- `(x OR 1)-(x XOR 1)` OpaqueZero：奇数输入时为 1，数学不成立；
- 任何最终相消为零的假状态依赖：符号执行可直接删除。

## 4. R061 验收

```text
OS/OS2 单条件旁路：0
双差值 per-prototype lane：2
完整性 fan-out：block/constant/child/upvalue/nparams/tag
cargo test：28 passed / 0 failed / 1 ignored
run_tests：169/169
run_presets：370/370
multiseed：FAIL=0
H2/H3/H4/H5/H6/H7/H8/H5-P0/R061：PASS
防御审计：PASS 21 / FAIL 0
内部 Loader：0
R061 luau-compile + 运行等价：PASS
R061：205310B，m=165942，r=2703
```

## 5. 待审批开发顺序

1. **P1-C：每 shard 独立 checksum lane**（风险最低，直接减少单点旁路）；
2. **P1-A：仅关键索引的 boot-state 编码**；
3. **P1-B：安全子集 Superoperator 实验 profile**；
4. **P2-A：削减无效 junk**；
5. **P2-B：保留 return 语义的轻量入口研究**。
