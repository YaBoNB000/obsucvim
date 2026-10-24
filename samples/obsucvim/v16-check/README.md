# v16-check：v16-a `--key-host delta` 真 Delta 矩阵件（2026-10-24）

这两件是 **Phase-2 真机验收件**：引擎 R068 用 `--key-host delta` 构建
（转录绑定解码钥：Vector3 tostring/typeof 折叠 + IsStudio/checkcaller 布尔 +
identify 名长折入 SCHALL 转录；观测不符 → 全链失步 → 错钥静默死亡）。
输入载荷 `print("kkksu")`，种子 `20261024`，其余参数与发布流水线 hard 同构。

## 用法

1. **`Kkksu.v16keyhost.hard.lua`** —— 正式形态。在真 Delta 整段运行：
   - **预期：正常打印 `kkksu`**（Phase-0 实测你的宿主 a/b/c 三项与编译期模型
     逐值匹配：a=1143546169 b=1317214663 c=22）。
   - 如果正常打印：v16-a 正向验收通过，回一句话即可。
   - 如果任何报错/静默：换 trace 件跑第 2 步。
2. **`Kkksu.v16keyhost-trace.hard.lua`** —— 诊断形态（仅失败时才需要）。
   跑完把完整报错行原样发回；若是转录失配，错误里的 `(R-xxxx)` 段会带
   `heavy/tr/KHXA/KHXB/KHXC` 五值，可与 GateProbe 的 `[XG-KH]` 行直接对表定位
   是哪一个观测项在真机上偏了。

## 第二轮（Phase-2 要求：中型脚本，覆盖多函数/多常量/多字符串的解码全链）

- **`Kkksu-mid.v16keyhost.hard.lua`** —— 递归 fib + 循环 + table + 拼接，
  **预期精确打印**：
  `kkksu-mid:1,1,2,3,5,8,13,21,34,55|143`
  任何一位不对/报错都算失败（错位即投毒的表现）；打印完全一致讲一声，
  两轮齐了就拍板 `--key-host` 翻默认 + 进发布管线。
- `Kkksu-mid.v16keyhost-trace.hard.lua` —— 同样的诊断形态，仅失败时用。

## 旁证

- 跑 `diag-tools/Obsucvim-GateProbe.lua`（现行版），`[XG-KH]` 行应打印
  `match(delta a=1143546169 b=1317214663 c=22)=true`；false 就不用试上面两件了，
  把 `[XG-KH]` 行发回。
- 沙箱侧已证（非真机）：关态产物与 R067.5 逐字节相同；公式双层互验（client 匹配 /
  studio 失步可复算，见 `web/tools/test-keyhost-fold.py`）；precheck 先于转录链；
  载荷字符串不出现在产物字节里（`grep -c kkksu` = 0）。

## 边界（诚实声明）

- 正向「真 Delta 打印 kkksu」在本仓库沙箱**无法自证**（hard 需要
  debug.getconstants/getupvalues/getproto/setupvalue，Lua 层无法伪造）——
  这正是这两件要你跑的原因。
- v16-a 抬的是攻击工时（静态解混淆者必须完整仿真 VM+闸门+seal 栈才能还原文本），
  不是密码学意义上的不可解；服务端持钥（v16-b）才是「物理拿不到」，已拍板不做。
