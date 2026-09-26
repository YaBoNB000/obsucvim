# R028/R029 加固实施计划（基于 HARDENING_DETAIL.md 三板斧）

> 依据 `HARDENING_DETAIL.md`（`R028` 146KB `R028_target.luau.lua` + `R029` 1220778）的三重取证（`work/r29_h2.out` + `work/r29_h4.out` + 逐字节段比对），全部本地构建种子可复现，无服务依赖。

## 现状（`b18dc30` 基线，`169` 全绿）

- `S1` 四表真随机（`s_indices` 前置 `if scatter` + `encode_soa_for` + `HW2` 联合 `172` + `closure` 四线 `Wt[j]==w1 or w2 or w3 or w4`）已落地
- `A3` 跨代理冗余（`C={[0]=rawA,[1]=rawB}` 同源 `ct/cn/csb/csl/co`，`__index` 双零，`dummy` 读 `rawB[1][j]`）已落地
- `A4` 五流 `W/SA/SB/SC/SD` 置 `0` 已落地 `027d276`

## 待落地三板斧

### 一、泄漏封堵（`return...end` 最优先，`jz` 8780B 尾六路 `return vAv, FO, pZB, Zr, tl, Jh`）

**漏点**：`jz` 尾 `return vAv, FO, pZB, Zr, tl, Jh` 六路内部对象（`vAv` 52 段表/`FO` 主 `proto` 原始字/`pZB=zMR`/`Zr/tl` 链终态/`Jh`）经宿主 `S(function(){...return(function(...) return ... end)(pQ()(34参))})` 的 `varargs` 透传裸奔，`hook4` 一针拿全；`pQ()(TK,Ck,...,MK)` 34 参入口 `Sd` 包装可换 `spy` 一次收走 `vU/zx/fg/Gg/Ba/MK`。

**落地**：
- `jz` 尾改为单出口闭包：`local entry=function(ub,pF,jM) return Tl(ub,pF,{},jM,0) end; return entry`（`vAv/pZB/Zr/tl` 封为 `upvalue`，宿主只拿 `entry`，`e_,ub,PC,Vh,jd,CQ` 六路解构整段删除，`Luau` 无 `debug` 取 `upvalue` 值）
- `Zr/tl` 不外送，搬进 `jz/seg03` 内部自检（`t1/t2` 校验失败即 `Zo` 终止）
- `34` 参入口改为 `vault[nonce]` 一次性 `getter`（`chunk` 首行 `local K=GET(nonce)`，`Sd` 只见 `nonce`），壳补 `cw` 双模自哈希，禁 `return(function(...)` 透传 `lint`

### 二、构建差异化（`53/54` 逐字节相同 → 知识迁移 `100%`）

**现状**：`54` 段 `53/54` 相同，仅 `seg53` `iF` 常量 `355008141195→355008040966` 变；`av→段` 映射/`K` 常量组 `46064`/`别名表`/`QJS`/`tag` 全固定。

**落地**（构建种子驱动，纯离线）：
1. `lI` 三元组序重排
2. `av→段` 语义↔编号随机映射（`seg03` 的 `Cz` 构建与 `seg53` 的 `op` 写出同源）
3. 别名表组数/每组别名数/别名值随机
4. `K` 常量组每构建重掷（`seg03/seg53` 同源）
5. `LCG` 参 `fg/Gg` 乘加子/步数随机
6. 链种子构成 `vU+dfl·zx+Cu·Ba+zMR` 系数随机化
7. `t1/t2` 删或真随机（恒零假密封已实证 `5,625,356×2^28` 整除）
8. 指令字 `5×u16` 字段数/宽度/顺序每构建变
9. 载荷模板骨架寄存器分配/死代码形状变
10. `tag` 流格式 `221/244/233/56/33/129` 边界随机
11. 壳 `34` 参名/序重排
12. 保留 `seg53` 自哈希常量随机化

**自测**：与上一构建 `diff` 相同段数>10% 或相同噪声字>5% 即拒发布。

### 三、噪声随机化（`w3/w5` 模板对照 `5` 秒对齐）

**现状**：`K2=819970`/`K1=230257`/`w3/w5=54298/44585`/`NOP` 全零×2/`[22]` 死存储/`LOADK K5=0` 全跨构建逐字相同，真操作数 `0-23` 小值 vs 噪声 `10^4` 均匀大随机，阈值分类器即滤。

**落地**：
1. 噪声 `PRNG` 接构建种子
2. 分布贴近 `0-24` 寄存器/`0-8` 常量下标
3. `NOP` 多形态（假算术/假跳转/假比较）且数量随机
4. 活噪声（噪声常量偶尔 `LOADK` 到死寄存器）
5. `nil` 槽位随机
6. 双向均匀化（真操作数偶尔像噪声）

## 验证（落地后）

- `R028`/`R029` 同 `seed` 构建 `diff` 相同段数 `<10%`
- `hook4` 探针（`Sd` 包装/`return` 透传/`Tsn` 注入/`pQ` 返回值）抓不到 `proto`/`密钥`/`handler` 源码
- `169/169 + 370/370 + multiseed 5×30` 全绿，`R028` 6 行无打印 `143KB`（`--preset v15 --seed 58217439`）`luau` 直通 `exit 0`
