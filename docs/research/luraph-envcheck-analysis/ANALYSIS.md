# ⚠️ 历史研究存档

> 本文件属研究战役记录（结论按当时口径），路径对照见 [`../../README.md`](../../README.md) §3。
> 现状见 [`../../CURRENT-STATE.md`](../../CURRENT-STATE.md)。

---

# 真 Luraph 环境校验载荷分析（WTF.txt）

**来源：用户上传至 main 分支（f572977）。存档：`luraph-envcheck.lua`。1,131 行 / 45,671 字节。**

## 鉴定

真 Luraph 的**运行时环境校验模块**（很可能经提取/反混淆）。证据：
- 失败路径固定 `error("WTF,son", 0)`（文件名来源）；
- `loadstring(..., "Luraph")` / `"Luraph2"` chunk 名直接暴露厂商；
- 检查目标 = "当前是否为**真实 Roblox executor 环境**"，全部通过后 `print("You Passed")`。

## 检查清单（约 300+ 断言，10 大类）

| 类 | 内容 | 备注 |
|---|---|---|
| 核心语义 | getfenv/setfenv、error(s,0)/xpcall、select('#') 精确 arity（含 nil 洞）、闭包 per-iteration 捕获、元表全谱（__index/__newindex/__add/__tostring/raw*）、(-17%5)==3 floor-mod | 与我方 VM 语义回归同类 |
| 库指纹 | string（\0/负索引/reverse/format）、math（floor(-1.25)）、bit32 定值、buffer **全 26 函数**含小端序字节布局、越界错误路径 | bit32 是**常量挑战** |
| **库表键序扫描** | 用 `next(t, prev)` 按**原生迭代序**走 utf8/math/debug/task/script 的完整键列表；对 13 个"新键"（isnan/nfcnormalize/desynchronize 等）做**位置精确**匹配 | 环境版本指纹 |
| **全局身份终检** | 入口捕获 ~60 个引用（含 buffer 全家族），结尾一次性 `assert == refs.assert and ...` 全等 | 防运行中掉包（我方 exguard 周期复检同族） |
| executor 语义 | iscclosure/islclosure/newcclosure/clonefunction/isfunctionhooked/checkcaller（存在才测）；debug.info 六元组（"[C]"、line=-1、variadic、f==fn）；**错误现场栈深探测**（嵌套 4 层包装 + xpcall 内 debug.info(level,"f") 于 5..11 层必须非空） | debug 键集含 getproto/getconstants/setstack 等 executor 面 |
| **错误消息指纹** | ~60 个函数的缺参/错参调用，**精确匹配错误文本后缀**（含 "Expected ':' not '.'" 成员调用形态）；`normalizeError` 剥离 " to '[^']+'" 以容忍位置差异 | 52 号雷区实战证据 |
| Roblox 对象 | game/DataModel、GetService、Instance.new(Folder) 父子、Name、**锁定元表**（"The metatable is locked"）、Connect/Disconnect 生命周期、销毁后 Parent 锁定错误 | |
| **Enum 身份缓存** | FromName("R6") 两次 + FromValue(0) 三次必须返回**同一 userdata**；EnumItem/EnumType typeof/元表 | mock 最易破的点 |
| **确定性指纹** | Random.new(177465824) 的**精确浮点输出序列**（0.9598971928030796...）+ Clone 状态分叉；utf8 NFC/NFD 韩文规范化字节精确；Path2D 几何（GetLength=119.61338806152344 等浮点） | 版本敏感 |
| 编译/协程 | loadstring 正常/语法错误形态、setfenv 自引用片段编译、协程 106 轮 yield、task.spawn/delay/cancel 返回线程 | |

## 对照我方 C6 研究的结论

**强（值得吸收进 exguard 绑定态探针库）**：buffer 全族+端序+错误路径；Enum 身份缓存语义；Random 种子确定性；错误消息指纹（配 normalizeError 容差）；库表键序扫描；错误现场栈深探测；入口/结尾全局身份全等。

**弱（我方实验已证实的缺口）**：
1. **全部是常量挑战**——无任何 nonce/新颖查询。录制重放工具在真环境录一遍即可全答对（ROUND4 P1 实测同类形态）；
2. **失败路径是固定 error 分支**——`check()` 函数族 + `"WTF,son"` 可定位、可 patch（R061 教训：应走无分支代数 lane 静默投毒）；
3. `normalizeError` 的存在本身证明错误文本在 executor 间有漂移——正是 52 号增量翻车的域，且此文件比我们激进得多（60+ 条精确文本）；
4. 键序/浮点指纹强版本绑定——Roblox 更新即碎（维护成本高）。

**定位**：此文件 = L2 层（executor/Roblox 真实性认证），与我方 CORE-ATTEST 计划的 L1 层（杀录制重放）互补不冲突。吸收清单应并入 **exguard 绑定态**探针库扩充（Phase 3 联动），不进默认管线（H4 路线图红线：默认不堆 API 探针）。
