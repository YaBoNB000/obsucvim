# R028/R029 加固实施计划（基于 HARDENING_DETAIL.md 三板斧）

> 依据 `HARDENING_DETAIL.md`（`R028` 146KB `R028_target.luau.lua` + `R029` 1220778）的三重取证（`work/r29_h2.out` + `work/r29_h4.out` + 逐字节段比对），全部本地构建种子可复现，无服务依赖。

## 实施状态（2026-09-26）

- `S1` 四表真随机（`s_indices` + `encode_soa_for` + `HW2` 联合四线）已落地。
- `A3` 跨代理冗余、`A4` 五流销毁均已落地。
- **H1 泄漏封堵已闭环**：HBOOT 单出口闭包 + 阅后即焚 + `vault[nonce]` 一次性 getter；`Sd/return/Tsn/pQ` 四钩回归均抓不到 proto/密钥。
- **H2 构建差异化持续实施中**：已落地段语义编号随机、壳参数名/序双随机、五字段块布局随机、链种子真交叉项；既有的段序、K/LCG/tag/模板随机面已纳入审计。
- **H3 噪声随机化尚未开始独立增量**，不得把既有 unused-operand 噪声夸报为 H3 六项完成。

## 三板斧实施清单

### 一、泄漏封堵（`return...end` 最优先，`jz` 8780B 尾六路 `return vAv, FO, pZB, Zr, tl, Jh`）

**漏点**：`jz` 尾 `return vAv, FO, pZB, Zr, tl, Jh` 六路内部对象（`vAv` 52 段表/`FO` 主 `proto` 原始字/`pZB=zMR`/`Zr/tl` 链终态/`Jh`）经宿主 `S(function(){...return(function(...) return ... end)(pQ()(34参))})` 的 `varargs` 透传裸奔，`hook4` 一针拿全；`pQ()(TK,Ck,...,MK)` 34 参入口 `Sd` 包装可换 `spy` 一次收走 `vU/zx/fg/Gg/Ba/MK`。

**已落地**：
- HBOOT 尾部改为单出口闭包，内部 `HW/MPF/SALT/SH1/SH2/HPH` 只作闭包 upvalue；原局部立即置 `nil`。
- 宿主 `PCAL` 只接收闭包，再由 `_r1()` 取内部对象；旧多值直返已删除。
- `RPK1..5 + CKJ` 经 `vault[nonce]` 一次性 getter 进入 HBOOT，读取后删除槽；`_hbf` 的可见参数面只剩 getter + nonce。
- `redteam_probe_regression.py` 固化四钩结论：`Sd/return透传/Tsn/pQ` 全部 PASS。

### 二、构建差异化（`53/54` 逐字节相同 → 知识迁移 `100%`）

**取证基线**：旧 `R028/R029` 曾出现 `54` 段中 `53/54` 完全相同。现役 `h2_diff_audit.py` 固定以同源码双 seed 度量逐字节、长串、骨架 token、24-gram，并验证运行等价与结构随机面。

**12 项状态**（构建种子驱动，纯离线）：
1. ✅ 段三元组解码顺序：`frags` 的 `order` 每构建洗牌，哈希链与增量字典同序镜像。
2. ✅ 段语义↔编号：BSS/RT/EXG 从低区非 opcode 空档抽取；续行/执行器从 `256..511` 抽取。
3. 🟡 别名面：四套 opcode wire + NOP alias 值已随机；“组数/每组数量可变”仍待做。
4. ✅ K 常量组：`KeyEmitter` 的 KA/KB/KC/KM 内容、槽位、组装配方每构建重掷。
5. ✅ LCG：常量、blob、HQ、meta、块钥匙流的乘加子/种子/步数均由构建 seed 生成。
6. ✅ 链种子构成：随机 `wire`/`rv` 系数之外，新增真实 `wire×rv×CHAIN_CROSS` 交叉项；Rust 加密与 HBOOT 解密共同消费，非恒零装饰。
7. ✅ `t1/t2` 假密封已由双车道源码链 + 两条独立归零约束取代；约束实际进入块/常量钥匙流。
8. 🟡 指令字布局：块密文 `W/SA/SB/SC/SD` 五字段物理顺序及块缓存槽序每构建重排；字段数/宽度可变仍待做。
9. 🟡 载荷模板骨架：handler 语义等价格式与 dispatch/fetch 形态已有随机族；寄存器分配和更大形状族仍可加深。
10. ✅ tag 流：section tag 值、section 顺序、decoy section 数量/内容均已 seed 化。
11. ✅ 壳参数名/序：30 个 HBOOT 入口形参使用独立随机名，形参与实参按同一随机排列同步装配。
12. ✅ 自哈希常量：链乘数、初值、双归零系数与 meta/HQ 自哈希常量均每构建重掷。

**当前实测闸**：双 seed 长串完全相同段 `0`；同 seed 必须逐字节确定；双构建必须运行等价。具体百分比随代码骨架变化，以审计脚本当次输出为准，不在文档硬编码。

### 三、噪声随机化（`w3/w5` 模板对照 `5` 秒对齐）

**现状**：`K2=819970`/`K1=230257`/`w3/w5=54298/44585`/`NOP` 全零×2/`[22]` 死存储/`LOADK K5=0` 全跨构建逐字相同，真操作数 `0-23` 小值 vs 噪声 `10^4` 均匀大随机，阈值分类器即滤。

**落地**：
1. 噪声 `PRNG` 接构建种子
2. 分布贴近 `0-24` 寄存器/`0-8` 常量下标
3. `NOP` 多形态（假算术/假跳转/假比较）且数量随机
4. 活噪声（噪声常量偶尔 `LOADK` 到死寄存器）
5. `nil` 槽位随机
6. 双向均匀化（真操作数偶尔像噪声）

## 验证闸

- `python3 luraph-rs/tests/h2_diff_audit.py`：确定性、跨 seed 差异、五字段排列、参数名/序、wire/链系数、双构建运行等价。
- `python3 luraph-rs/tests/redteam_probe_regression.py`：`Sd/return/Tsn/pQ` 四钩抓不到 `proto`/密钥。
- `bash luraph-rs/tests/run_tests.sh`：全量语言/VM 回归必须全绿。
- H2 尚未因本页状态表而宣告“12 项全部满规格”；🟡 项与 H3 六项仍需后续增量及回归。
