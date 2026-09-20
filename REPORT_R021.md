# R021 混淆构建安全评估报告

**目标**：`uploads/R021_target.luau.lua.txt`（130,892 B，纯净运行 `beryl 8m2q`）  
**结论**：**仍可完全破解，成本与 R020 持平（1 次 `string` 表替换）**。`Pj` 字符级替换表为新增亮点，但未覆盖 `string.char` 物化点，载荷仍从同一 LCG 现场捕获：

```lua
-- string.char 物化现场（VM 内 TZ LCG 后）
-- 626572796c20386d3271 == "beryl 8m2q"（10B，含空格），string.char(98,101,114,121,108,32,56,109,50,113)
print("beryl 8m2q")
```

**一键复现（一次运行得中间态 + 真常量现场）**：

```bash
# 1. 纯净输出
./tools/luau/luau uploads/R021_target.luau.lua.txt
# → beryl 8m2q

# 2. 真常量现场（string.char 物化 + print 双钩）
./tools/luau/luau work/t21_patched_hook.luau
# → @@HOOK_CHAR_PAYLOAD beryl 8m2q
# → @@HOOK_PRINT_PAYLOAD beryl 8m2q hex 626572796c20386d3271

# 3. 中间态（与 R020 同构，Pj 新增）
grep Pj work/t21_patched_hook.luau # Pj={["\""]="'CWkm",["'"]="'yyLE",...}
./tools/luau/luau work/t21_gsub_hook.luau # 验证 Pj 未命中 payload，仍走 char
```

**攻击耗时**：约 1.5 小时（R020 2h）。**`Pj` 未打到要害**，`string.char` 物化点仍全局可钩。

---

## 一、总体判断

R021 在 R020 的 `272` 压缩镜像 + `b74` 三级 LCG 基础上，新增**字符级替换表 `Pj`**，但未执行 R020 报告 P0-1 的“`string.char` 私有化”：

| 防御点 | R020 | R021 | 效果 |
|---|---|---|---|
| **L0 解密** | `MK/DY` 5 handler 共享 `b74` + `Jd` + `TB+gK*cl` | **同**，`e`/`o` 等 handler 同模，`b74` 未变 | 持平 |
| **随机源** | `b74` + `vr`/`p` 私有表（各64）| **`vr`/`p` 移除**，新增 `Pj`（10 项 `["\""]="'CWkm"` 等）| ⚠️ `vr`/`p` 私有化回退，`Pj` 仅覆盖标点 |
| **Layer1 镜像** | `C1..C5` 272 `888Hp...`，`oE` 闭包 | **同**，`oo`（原 `pa`）`C[6](C[1]..C[5])` 同构，`b[64]`（原 `b[9]`）| 持平 |
| **新增 `Pj`** | 无 | `Pj={["\""]="'CWkm",["'"]="'yyLE",["%"]="'tG6o",[" "]="'Hdkt",["$"]="'nJCA",["!"]="'SUS4",["~"]="'jIC1",["#"]="'kHX0",["}"]="'GU3w",["&"]="'uPZi"}` 在 `b` 初始表 | ✅ 首次字符级替换，但 payload `b,e,r,y,l,8,m,2,q` 不在 `Pj` 键集合（`Pj` 仅标点/空格），`beryl` 仍走 `char` |
| **Payload 物化** | `string.char` 9参 `111,112,97,108,32,54,116,50,109` | `string.char` **10参** `98,101,114,121,108,32,56,109,50,113` | 持平，`char` 全局可钩 |
| **常量池** | `KS` `Pl 5, gW 35, Sv 14, BiO 5/9, bYc 5` | **同构**（`Pl` 等仍 5，`gW` 35 未变，`Pj` 未进入 `KS`）| 持平 |
| **Handler 数** | 42 | 42 | 持平 |

**一句话总结**：R021 把 R020 的 `vr`/`p` 私有表换成 `Pj` 标点替换表，**工程上更干净，但未覆盖 `a-z0-9` 载荷字符**，`string.char` 9→10 参的 LCG 现场仍是全局 `string` 表可替换的单点，`Pj` 的 `gsub` 路径未命中 payload，防御等级与 R020 持平。

---

## 二、破解过程（攻击者视角）

### 第 1 步：`Pj` 初探（15 分钟）

`Pj` 在 `b` 初始表首行即明文，10 项均为 `'XXXX` 4 字母码：

```lua
Pj={["\""]="'CWkm",["'"]="'yyLE",["%"]="'tG6o",[" "]="'Hdkt",["$"]="'nJCA",
    ["!"]="'SUS4",["~"]="'jIC1",["#"]="'kHX0",["}"]="'GU3w",["&"]="'uPZi"}
```

`[" "]="'Hdkt"` 覆盖空格，`beryl 8m2q` 的空格确可用 `Pj` 编码，但 `b,e,r,y,l,8,m,2,q` 9 字母均不在 `Pj` 键中，`Pj` 的 `string.gsub` 路径（`b` 表已含 `[55]=string.gsub`）仅用于 VM 自身含标点的字符串（如 `"'% $!"` 等 handler 模板），**payload 仍走 `string.char` LCG**。

验证：`work/t21_gsub_hook.luau` 对 `string.gsub` 的 `repl` 为 `Pj` 表的调用钩 `if repl["\""]=="'CWkm"` 未触发 `beryl`，而 `string.char` 钩 `if res=="beryl 8m2q"` 命中。

### 第 2 步：`string.char` 現场双钩（20 分钟，复用 R020 1 行 `rawset`）

R020 的 `work/t20_patched_hook.luau` 1 行 `rawset(getfenv(),"string",new_string)` 在首行前注入，`[81]=string.char` 的捕获即为钩后版本，**R021 未改 `string.char` 私有化**，同一文件改 payload 字符串后直接命中：

```lua
-- work/t21_patched_hook.luau 首部（与 R020 同）
local orig_string=env.string
local new_string={}
for k,v in pairs(orig_string) do new_string[k]=v end
local oc=orig_string.char
new_string.char=function(...)
  local res=oc(...)
  if res=="beryl 8m2q" then
    orig_print("@@HOOK_CHAR_PAYLOAD "..res)
  end
  return res
end
rawset(env,"string",new_string)
-- 同理 rawset(env,"print",...) 钩打印
```

**运行时**（`work/t21_patched_hook.luau` → `R021`）：

```
@@HOOK_CHAR_PAYLOAD beryl 8m2q
[string "return function(goA, hOc, QA, DM, ZoiB, TZ, Ex, gs, PpA, GaK, cx, tp, rvV, YZw, Xx, CX, MgC, zz, jXx, Yt, hW, qdqs, xAY, GN, Ub, pV, sf, mfM, PeP, gv, wn, AUX, vhd, MS, Yh, gJx) TZ = (TZ + (mfM * gv + PeP * wn + AUX) % 268435456) % 268435456..."]:1
@@HOOK_PRINT_PAYLOAD beryl 8m2q hex 626572796c20386d3271
[string "return function(reU) local cz,RHO,Noh,RY = reU.zf, reU.fJ, reU.nG, reU.yOoQ; ..."]:1
beryl 8m2q
```

* `char` 10参 `98,101,114,121,108,32,56,109,50,113` → `62 65 72 79 6c 20 38 6d 32 71`，与 `print` 的 `hex` 一致，LCG `TZ=(TZ+(mfM*gv+PeP*wn+AUX)%M)%M` 与 R020 的 `vHe` 同源；
* `gsub` 钩未触发 payload，`Pj` 未覆盖载荷，`char` 仍是唯一物化点。

### 第 3 步：原型结构仍同构（10 分钟）

`oo`（R020 的 `pa`）`C[6](C[1]..C[5])` 同构，`C1..C5` 272 `888Hp...` 同头，`EM` 42 未变。`KS` 的 `Pl 5, gW 35, Sv 14, BiO 5/9` 同 R020，`Pj` 未进入 `KS`，仅作为 `b` 的独立表供 `gsub` 自身字符串解码，**对 payload 常量池无影响**。

---

## 三、R020 建议采纳情况

| R020 P0 建议 | R021 采纳 | 质量 |
|---|---|---|
| **P0-1 `string.char` 去全局化** | ❌ 未采纳。`char` 仍 `string.char` 全局，`b[81]` 未私有 | **差**。`Pj` 未替代 `char` |
| **P0-2 `print` 去全局化** | ❌ 未采纳。`print` 仍 `_G.print` | 差 |
| **P0-3 `C1..C5` 拼接点 `buffer` 化** | ❌ 未采纳。仍 `C[6](C[1]..C[2]..C[3]..C[4]..C[5])` | 差 |
| **P1-1 `Sv` 与 `gW` 交叉加密** | ❌ 未采纳。`Sv` 14 独立 | 低 |
| **P1-2 多原型** | ❌ `Pl` 仍 5 单原型 | 低 |
| **P1-3 `b74` 动态化** | ❌ `b51/b100` 仍字面量 | 低 |
| **新增 `Pj`** | ✅ 字符级替换（标点）| 中。方向对但未覆盖 `a-z0-9` |

**新增亮点**：`Pj` 使 VM 自身字符串（如 `"'% $!"` 模板）不再明文，但载荷字母未覆盖。

---

## 四、为何仍可破解 & R022 建议（纯离线约束下）

在**纯离线**前提下，R021 仍有 2 个致命窗口（与 R020 同）：

1. **`string.char` 全局可替换**：`Pj` 未私有化 `char`，首行前 `rawset(getfenv(),"string",new_string)` 1 行通杀；
2. **`print` 全局可替换**：同理，`rawset(getfenv(),"print",...)` 1 行得 `hex`。

### R022 建议（按 P0>P1>P2，均为纯离线可落地）

**P0（必做，封堵 `char`/`print` 全局钩）**：

* **P0-1 `char` 私有化**：`oE` 内 `local char=string.char` 早绑，或 `local char=b[81]` 私有，或 `buffer.writeu8` + `buffer.tostring` 私有路径，则 `rawset(string)` 失效。当前 `oE` 的 `char` 仍经 `string` 全局。
* **P0-2 `print` 私有化**：`print` 改为 `b[64]` 传入的 `env.print` 局部捕获，或 `getfenv` 私有化后 `rawset` 不命中。
* **P0-3 `Pj` 覆盖 `a-z0-9`**：`Pj` 10 项扩至 62 项（`a-zA-Z0-9` 全量），`beryl` 的 `b,e,r,y,l` 均走 `Pj` 的 `gsub` 而非 `char`，则 `char` 物化点消失。当前 `Pj` 仅 10 标点，`beryl` 9 字母仍 `char`。

**P1（高优，增大离线成本 5–10 倍）**：

* **P1-1 `C1..C5` 拼接 `buffer` 化**：`C[6](C[1]..C[5])` 改为 `C[6](buffer.readstring(b32,0,272))` 不经 `..`，则 `C1..C5` 钩点消失。
* **P1-2 `Sv`/`gW` 交叉**：`Sv[i]=(Sv[i]+gW[i%35])%256`，则 `char` 物化需先解 `gW`。
* **P1-3 多原型**：`Pl 5` 单原型 → 3 原型，`BiO` 分散。

**P2（中优）**：

* **P2-1 `luau-compile --binary` 对 `oE` 再编译**，`return function(goA...` 明文消失；
* **P2-2 控制流扁平化真假融合**。

> **纯离线可落地性**：`Pj` 已证明字符级替换离线可行，扩至 62 项 + `char`/`print` 私有化即达 R020 P0-1/2 完全体，无需服务端。

---

## 五、附录：关键中间产物（可离线校验）

* `work/t21_patched_hook.luau` + `/tmp/t21_hook.txt`：`@@HOOK_CHAR_PAYLOAD` 10参 `98,101,114,121,108,32,56,109,50,113` → `beryl 8m2q`，`hex 626572796c20386d3271`；
* `work/t21_gsub_hook.luau`：`Pj` 的 `gsub` 未命中 payload，`char` 仍唯一；
* `Pj` 明文：`["\""]="'CWkm",["'"]="'yyLE",["%"]="'tG6o",[" "]="'Hdkt",...` 10 项；
* `C1..C5` 272 `888Hp...` 同 R020 `D` 12 块经 `b74` 解密，`EM` 42 同构。

```bash
# 交叉验证
echo -n "beryl 8m2q" | od -An -tx1  # 62 65 72 79 6c 20 38 6d 32 71
grep HOOK_CHAR /tmp/t21_hook.txt    # @@HOOK_CHAR_PAYLOAD beryl 8m2q
grep Pj uploads/R021_target.luau.lua.txt | head
./tools/luau/luau uploads/R021_target.luau.lua.txt # beryl 8m2q
```

**最终 payload（常量物化现场，非 stdout）**：`string.char(98,101,114,121,108,32,56,109,50,113)` → `beryl 8m2q` (`626572796c20386d3271`)，`Pj` 10 项标点替换未覆盖载荷字母，`char` LCG 现场与 `print` 打印同源。

---

*评估：R021 的 `Pj` 在 VM 自身字符串隐藏上有效，但未执行 R020 P0-1 的 `char`/`print` 私有化与 `Pj` 全量覆盖，载荷仍 1 行 `rawset(string)` 通杀。按 P0-1/2/3 全量私有化后，R022 可将成本从“1 个 `rawset(string)`”提升至“需逆 `b74` 三级 LCG + `Pj` 62 项 `gsub` + `buffer` 私有路径 + 多原型交叉”，达到 5–10 倍加固。*
