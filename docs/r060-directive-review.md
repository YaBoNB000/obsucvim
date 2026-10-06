# R060 指令审计与安全实施记录

> 日期：2026-10-06
>
> 原则：恢复真实防线，但不引入破坏用户设备、错误数学恒等式或未经分阶段验证
> 的中央 Mega-VM。

## 1. 审计结论纠偏

### 1.1 Ou/De 双完整性没有在 R059 丢失

R059 精简的是外层 staging/CPS module shell，内层 prototype 编译与 BSS parser
未被删除。现有代码一直保留：

```text
compiler.rs:
  osum  = Σ(a+b+c+d) mod 2^32
  osum2 = Σ(global_position × operand) mod 2^32

template.rs:
  OS / OS2
  CK / CK2
  PF_[id].ck / PF_[id].ck2 双比较
```

差值不是进入 `error()`，而是进入 `BD`，再分散影响：

```text
block seed
constant tag
constant payload
child token
upvalue descriptor
nparams
```

因此“R059 完全移除双校验”的结论不符合源码事实。R060 新增
`r060_integrity_audit.py`，将该事实固化为发布门禁。

### 1.2 三套解释器不是代码裁撤，但已按要求固定为四套

H4-5C 原设计每构建随机生成 3..5 套 shard；R059 的 seed 恰好选择 3，并非
重构时删除一套。为避免单样本多态空间波动，R060 改为精确四套，并使用四个
不同素数环：

```text
251 / 241 / 239 / 229
```

每套独立生成 multiplier、addend、context coefficient、packed carrier、at-rest
LCG 和 session reseal 参数。主路由 `MICRO_ROUTE_N` 固定为 4，route step 仍按
与 4 互质的步长选择。

## 2. 已实施项

- 编译期四列无权和/位置加权和门禁固化；
- 解析期 OS/OS2 双消费门禁固化；
- 校验差值继续进入六面 `BD` 静默投毒，不增加软错误 oracle；
- Micro shard 数量固定为 4；
- 四个独立素数模环与 affine/context 参数；
- 既有 packed carrier、SALT/CHQ reseal、MkStr qf/proto/PC 绑定保持；
- 新增 `r060_integrity_audit.py`。

## 3. 明确不实施的危险或错误要求

### 3.1 内存耗尽式死循环

不会加入循环分配 100000 元素 table 的 trap。该行为会：

- 主动耗尽客户端内存；
- 造成移动端/宿主进程级拒绝服务；
- 形成高度稳定的卡死 oracle；
- 可能影响同进程的其他正常脚本。

环境与数值完整性失败继续优先使用既有分散密钥投毒。gross bootstrap 认证拒绝
保持现状，不新增内存炸弹。

### 3.2 文档中的 OpaqueZero 公式不成立

所给：

```text
(x OR 1) - (x XOR 1)
```

只在 `x` 为偶数时等于 0；`x` 为奇数时等于 1。直接掺入真实计算会造成语义
错误。另一个 `x(x+1) mod 2 = 0` 是 SMT/常量传播极易证明的恒等式，不能实现
“剪除率严格 0%”。因此不将错误公式写入编译器。

### 3.3 一次性 Mega-VM 全 Proto 展平

强制所有程序恒为一个 proto 会破坏或极大复杂化：

- escaping closure；
- upvalue alias；
- 互递归；
- vararg/multireturn；
- coroutine/yield；
- pcall/xpcall 错误边界；
- 深递归帧预算。

同时单一 Mega-VM、单一 VStack/VSP 会重新制造中央 hook 咽喉点，与既定的
“去中心化、不新增统一 adapter”约束冲突。该方向如要研究，必须作为独立实验
profile，逐项证明语义和攻击成本，不能直接替换正式 v15。

## 4. R060 门禁

```bash
python3 luraph-rs/tests/r060_integrity_audit.py
python3 luraph-rs/tests/h5_context_audit.py
python3 luraph-rs/tests/h5_p0_micro_audit.py
python3 luraph-rs/tests/h5_shell_audit.py <artifact>
```

最终结果：

```text
Ou/De compile + parse + BD fan-out：PASS
MICRO_ROUTE_N：4
Micro rings：251/241/239/229
四套 affine 参数跨构建变化：PASS
内部 Loader：0
cargo test：28 passed / 0 failed / 1 ignored
run_tests：169/169
run_presets：370/370
multiseed：FAIL=0
防御审计：PASS 21 / FAIL 0
R060 luau-compile + 运行等价：PASS
R060：138615B，m=251932，r=4227
```

## 5. 诚实边界

四套素数 affine 仍可被完整符号执行消去；其价值是消灭单一 shard 明文基线并
增加跨 shard 对齐成本，不是密码学不可逆。真正进一步改变反编译语义映射仍需
安全、分阶段的 Superoperator 设计，而不是简单堆死循环或错误恒等式。
