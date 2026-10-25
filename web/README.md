# web —— 控制台 + 托管服务

React 控制台（Vite + Tailwind，单文件打包）与 Node 托管服务（账户、卡密、
脚本、发布、验卡、webhook），以及引擎 WASM 与全部验证套件。

> ⚠️ **部署契约路径**：已部署的 Windows 客户端按固定 raw URL 拉取
> `web/deploy/self-update.mjs` 与 `release-assets/…/Obsucvim-Web-Windows-x64.zip`。
> **不要移动 / 改名 `web/` 与 `release-assets/`**，否则线上自动升级 404。
> 详见 [`../docs/README.md`](../docs/README.md) §5。

---

## 本地运行

```bash
cd web
npm install
npm run engine     # 托管服务（默认 127.0.0.1:8787；引擎优先原生二进制，缺失回退 WASI）
npm run dev        # 控制台（Vite，/api 代理到 8787）
npm run build      # 产物到 web/dist（单文件 index.html）
```

引擎来源优先级：`OBSUCVIM_BIN`（默认 `engine/target/release/obsucvim`）→
`web/tools/obsucvim.wasm`（WASI + `tools/wasi-runner.mjs`）。
常用环境变量见 `tools/engine-server.mjs` 文件头。

## 目录

| 路径 | 内容 |
| --- | --- |
| `src/` | 控制台源码：`pages/`（工作台/脚本/历史/卡密/文档/账户）、`components/`、`lib/`（引擎接入 `engine.ts`、i18n、状态、卡密） |
| `tools/` | `engine-server.mjs`（托管服务，60+ 路由）、`obsucvim.wasm`（引擎）、`wasi-runner.mjs`、`attest.luau`、`loader-template.luau`、`license-guard.luau`、`key-check-library.luau`、`mistweave.py`，以及下列套件 |
| `deploy/` | Windows 一键包：启动器、远程升级、说明（`README-Windows.md` / `README-UPDATE.md`） |
| `public/` | 静态资源 |

## 验证套件（`tools/`）

```bash
node tools/verify-engine-version.mjs    # 引擎锚点（当前 R079）
node tools/test-hosting.mjs             # 托管主套件
node tools/test-engine-retry.mjs        # 发布作业崩溃重试
python3 tools/test-loader.py            # loader 模板
python3 tools/test-attest.py            # attest 证明层
python3 tools/test-live-loader.py       # 真 HTTP + 真服务器 + 真 loader
python3 tools/test-artifact-runtime.py  # 产物运行等价（需 LUAU_BIN）
python3 tools/mistweave-selftest.py     # MW-1 输入层门禁（11/11）
```

记录值（本轮沙箱未重跑，见 `../docs/CURRENT-STATE.md` §7）：hosting 78/78、
engine-retry 9/9、loader 24/24、attest 18/18、live-loader 13/13。

## 相关文档

- [`../docs/hosting/HOSTING-V2-PLAN.md`](../docs/hosting/HOSTING-V2-PLAN.md) —— 托管能力与实施记录
- [`../docs/hosting/SCRIPT-KEYS.md`](../docs/hosting/SCRIPT-KEYS.md) —— 卡密 / 接口全集
- [`../docs/hosting/SECURITY-AUDIT.md`](../docs/hosting/SECURITY-AUDIT.md) —— 控制台红队自测
- [`../docs/hosting/EMAIL-SETUP.md`](../docs/hosting/EMAIL-SETUP.md) —— 163 邮箱验证码
- [`../docs/engine/ENGINE-VERSION.md`](../docs/engine/ENGINE-VERSION.md) —— 引擎版本与重编配方
