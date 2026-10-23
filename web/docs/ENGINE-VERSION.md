# 引擎版本：R067 与 af7fc32 的差异，以及怎么恢复

> 结论先说：**引擎源码已恢复到 R067，`web/tools/obsucvim.wasm` 也已重新编译为 R067**
> （2026-10-23）。本文件记录差异、锚点校验与重编方式。
>
> 校验结果：`print("suda") --preset v15 --env-check --bind-executor --seed 67067`
> → **229,422 B / `3775f3b5…`**，与 `handoff/README-R067.md` 记录的 R067 靶标逐字节一致。
>
> 作为对照，我也在同一台机器上编了 af7fc32 版本，它复现了线上旧产物的
> 212,137 B / `01099ddb…`（`--bind-env roblox` 参数下），证明工具链本身是保真的。

## 1. 两个版本的差别

`af7fc32`（2026-10-19，*Reduce v15 output size: 232KB → 212KB for print("suda")*）
为了压缩体积，改了 3 个文件、约 12 行，**全部是降低加密噪声密度**：

| 项 | R067（当前） | af7fc32（旧二进制） | 影响 |
| --- | --- | --- | --- |
| 噪声指令密度 | `base_code_len/12` ~ `/5` | `/30` ~ `/15` | 每个函数的伪指令少 ~2.5 倍 |
| 活跃噪声比例 | `1/2` | `1/3` | 有效噪声更少 |
| 诱饵常量 | 每函数 2–5 | 1–3 | 常量池反推更容易 |
| 死寄存器范围 | 2–4 | 2–3 | H3 噪声覆盖变小 |
| junk_n | 2 | 1 | junk 注入减半 |
| `obf_num` 随机范围 | 40 bit | 28 bit | 数字混淆的搜索空间缩小 ~4096 倍 |

也就是说：**这不是「优化」，这是拿保护换体积**（提交信息自己也写明
「Defense audit: D3 structural fingerprint 22/32 (was 23/32)」）。用户看到的
「少了 40 KB」正来自这里——不是种子抖动，是引擎被削了。

R067 之前的那次 `b82f14f`（剥注释）是零行为变化；所以
`af7fc32^` 的引擎源码 == R067。

## 2. 现在仓库的状态

```
obsucvim/src/main.rs             junk_n = 2                ← R067
obsucvim/src/vmgen/compiler.rs   /12, /5, 1/2, 2..4, 2..5  ← R067
obsucvim/src/vmgen/template.rs   obf_num 40 bit            ← R067
```

校验方式（应输出为空）：

```bash
git diff af7fc32^ -- obsucvim/src
```

## 3. 二进制是怎么换过来的

`obsucvim.wasm` 是构建产物。`rustup` 在沙箱里装不上
（`static.rust-lang.org` 返回 000），apt 无权限，GitHub Actions 又缺少
`workflows` 写权限。**最后是从 npm 拿到了预编译工具链**（`registry.npmjs.org` 可达）：

```bash
mkdir -p /tmp/tc && cd /tmp/tc && npm init -y
npm install @rustbin/rustc-1.88.0-x86_64-unknown-linux-gnu \
            @rustbin/rust-std-1.88.0-wasm32-wasip1 \
            @rustbin/rust-std-1.88.0-x86_64-unknown-linux-gnu
mkdir -p sysroot/lib/rustlib
cp -r node_modules/@rustbin/rust-std-1.88.0-x86_64-unknown-linux-gnu/rust-std-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu sysroot/lib/rustlib/
cp -r node_modules/@rustbin/rust-std-1.88.0-wasm32-wasip1/rust-std-wasm32-wasip1/lib/rustlib/wasm32-wasip1 sysroot/lib/rustlib/
RUSTC=/tmp/tc/node_modules/@rustbin/rustc-1.88.0-x86_64-unknown-linux-gnu/rustc/bin/rustc
cd obsucvim
CARGO_PKG_VERSION=0.2.0 $RUSTC --edition 2021 \
  -C opt-level=3 -C lto -C codegen-units=1 -C panic=abort -C strip=symbols \
  --target wasm32-wasip1 --sysroot /tmp/tc/sysroot \
  src/main.rs -o ../web/tools/obsucvim.wasm
```

产物 1,172,743 B（与旧的 1,182,882 B 同量级），输出与 R067 锚点完全一致。
这是**无 rustup 环境下的备用路线**；正常情况仍建议用下面 §5 的方式（cargo）编。

## 4. 怎么验证当前装的是哪个版本

```bash
node web/tools/verify-engine-version.mjs
# 或用脚本：
./web/tools/build-engine-wasm.sh --check
```

输出示例（当前实际状态）：

```
engine   : /…/web/tools/obsucvim.wasm
probe    : print("suda") --env-check --bind-executor --bind-env roblox --seed 67067
output   : 229422 bytes  3775f3b557ca4c95ee16d9d013f290d735b6c022af23ac17f1d0b2339fc689f6
verdict  : R067 — full noise density (junk_n=2, /12 /5 noise, obf_num 40 bit)
```

服务端启动时也会打印一行 `[engine-server] engine build: R067 | af7fc32 | UNKNOWN`。

**锚点**

| 版本 | 产物大小 | SHA-256 |
| --- | --- | --- |
探测命令：`--preset v15 --env-check --bind-executor --seed 67067`

| 版本 | 产物大小 | SHA-256 |
| --- | --- | --- |
| **R067.1（当前）** | 230,368 B | `8b43132eb0297a9017689acfb3a1f80950b2fc269a62630dc4cb4483c014c8fb` |
| R067（回退点，无前置自检） | 229,422 B | `3775f3b557ca4c95ee16d9d013f290d735b6c022af23ac17f1d0b2339fc689f6` |
| af7fc32（旧） | 229,900 B | `12c2c05b572d5ede5d8992684ed4aa724096f9254b66d81563848b99fd91d638` |

（同一 af7fc32 引擎在发布流水线那种 `--bind-env roblox` 参数下就是 212,137 B / `01099ddb…`。）

## 5. 重编 R067（三条路，任选）

### A. 本机（Windows / macOS / Linux 均可）

```bash
rustup target add wasm32-wasip1
cd obsucvim
cargo build --release --target wasm32-wasip1
```

产物 `obsucvim/target/wasm32-wasip1/release/obsucvim.wasm`。或者直接用仓库脚本：

```bash
./web/tools/build-engine-wasm.sh          # 编译 + 校验 R067 锚点 + 安装
```

脚本会在锚点不匹配时**拒绝安装**（除非 `--force`）。

### B. GitHub Actions（推荐，零本地环境）

把 `web/tools/ci/build-engine.yml` 复制到 `.github/workflows/build-engine.yml`
推到仓库即可。它会用 GitHub 的 runner（自带 Rust）编译、跑 R067 锚点校验，
并把 `obsucvim.wasm` 作为 run artifact 上传。

### C. 只换二进制

任何人给你一个 229,422 B、SHA 为 `3775f3b5…` 的 `obsucvim.wasm`，直接覆盖
`web/tools/obsucvim.wasm` 即可，其余代码不用动。

## 6. 换完之后

```bash
node web/tools/verify-engine-version.mjs        # 应显示 R067
cd web && ./tools/rebuild-package.sh --base releases/Obsucvim-Web-Windows-x64.zip
```

然后升级服务器并**重新发布所有脚本**——产物是引擎生成的，不重发就还是旧引擎的字节。


## 7. R067.1 变更（2026-10-23）

**问题**：`--bind-executor` 产物在缺 API 的执行器上（如 Delta）不是「拒绝」，而是
`TRIP()` 投毒后 VM 随机位置崩溃，表现为
`attempt to perform concatenation on a nil value` —— 与真实原因无关，线上无法诊断。

**改动**：`obsucvim/src/vmgen/exguard.rs` 在生成代码的 `VKC = {}` 之后插入一段只读自检
（约 680 B，仅出现在 `--bind-executor` 产物中）：逐个 `pcall` 检查
`identifyexecutor / checkcaller / iscclosure / islclosure / newcclosure / loadstring / typeof`
与 `debug.getconstants / getupvalues / setupvalue`，缺失则直接 `error()` 给出清单。

**影响面（实测）**：

| 构建 | 改动前 | 改动后 |
| --- | --- | --- |
| `--env-check --no-bind-executor`（safe） | 193,044 B | **193,044 B，逐字节一致** |
| `--env-check --bind-executor`（hard） | 212,089 B | 212,770 B（+681 B） |
| R067 探测锚点 | 229,422 B / `3775f3b5…` | 230,368 B / `8b43132e…` |

`web/tools/verify-engine-version.mjs` 同时认 R067.1 与 R067 两个锚点。
