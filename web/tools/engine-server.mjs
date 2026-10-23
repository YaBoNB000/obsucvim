#!/usr/bin/env node
/**
 * Obsucvim Web + 引擎服务（使用 Nodemailer 发送验证邮件）。
 * 为控制台提供账户认证、混淆等产品 API；不公开健康检查或调试端点。
 * POST /api/obfuscate → { ok, result: { code, log, stats } }
 *
 * 选项映射与前端 buildCliArgs() 一致（产品形态固定 v15）。
 * 环境变量：
 *   OBSUCVIM_BIN   obsucvim 二进制路径（默认 <repo>/obsucvim/target/release/obsucvim）
 *   PORT           监听端口（默认 8787，仅 127.0.0.1，由 Vite dev 代理）
 *   LUAUC          luau-compile 路径（默认 <repo>/.tools/bin/luau-compile，可选语法校验）
 *   SMTP_*         163 邮箱 SMTP 配置（SMTP_PASS 必须由服务端私密环境变量提供）
 *   OBSUCVIM_STATE_FILE 状态文件路径（默认 web/tools/.state.json）
 */
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, mkdirSync, readFileSync, writeFileSync, unlinkSync, mkdtempSync, renameSync, rmSync, createReadStream, statSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createHash, createHmac, randomBytes, randomInt, scryptSync, timingSafeEqual } from "node:crypto";
import { isIP } from "node:net";
import { deflateSync } from "node:zlib";
import nodemailer from "nodemailer";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(__dirname, "..", "..");
const BIN = process.env.OBSUCVIM_BIN || path.join(REPO, "obsucvim", "target", "release", "obsucvim");
const WASM_BIN = process.env.OBSUCVIM_WASM || path.join(__dirname, "obsucvim.wasm");
const WASI_RUNNER = path.join(__dirname, "wasi-runner.mjs");
const WEB_ROOT = process.env.OBSUCVIM_WEB_ROOT || path.resolve(__dirname, "..", "dist");
const HOST = process.env.HOST || "127.0.0.1";
const LUAUC = process.env.LUAUC || path.join(REPO, ".tools", "bin", "luau-compile");
const PORT = Number(process.env.PORT || 8787);

function engineMode() {
  if (existsSync(BIN)) return "native";
  if (existsSync(WASM_BIN) && existsSync(WASI_RUNNER)) return "wasi";
  return "missing";
}
const MAX_SOURCE_BYTES = 1024 * 1024;
const ENGINE_TIMEOUT_MS = 60_000;

/* ---------------- 选项 → CLI 参数（与前端 buildCliArgs 对齐） ----------------
 *
 * 固定不动的两层：
 *   --env-check   纯 Luau 语义自检（buffer/bit32/string/元表/协程/freeze），
 *                 只比较运行期前后取到的函数是否同一个，宿主差异不会误伤。
 *   --no-bind-executor
 *                 引擎默认值是「绑定执行器」，它会核验 debug.getconstants /
 *                 getupvalues / setupvalue / getproto(s) 等一整套 API，任何一项
 *                 对不上不是跳过而是自我投毒，整个脚本直接崩（2026-10-23 事故）。
 *                 所以这里恒定显式关闭，没有任何开关能打开它。
 *
 * 执行器证明 + Roblox 环境绑定改由 attest.luau 承担：它总是被注入到用户代码
 * 之前、和用户代码一起混淆，做同样的探测但**永不报错**，结果以
 * getgenv().OBS_Attested / OBS_Attest 的形式交给脚本，并随验卡上报服务端。
 * 换句话说：证明永远在、证据永远在，但宿主不合规不会让脚本死掉。 */
/* 两种构建模式，都不是「可选优化」：
 *   hardening=false → 兼容版：`--env-check --no-bind-executor`
 *   hardening=true  → 加固版：`--env-check --bind-executor --bind-env roblox`（原始保护，一层不少）
 * 加固版只有在 loader 的证明层确认宿主支持时才下发，否则会自我投毒。
 * 注意：引擎的默认值就是绑定执行器，所以两种模式都必须显式写明。 */
function hardeningEnabled(o) {
  return Boolean(o && o.hardening === true);
}

function buildCliArgs(o) {
  const args = ["--preset", "v15", "--env-check"];
  if (hardeningEnabled(o)) args.push("--bind-executor", "--bind-env", "roblox");
  else args.push("--no-bind-executor");
  const seed = normalizeSeed(o.seed);
  if (seed) args.push("--seed", seed);
  args.push(o.minify ? "--minify" : "--no-minify");
  if (!o.mangle) args.push("--no-mangle");
  if (!o.strings) args.push("--no-strings");
  if (!o.flatten) args.push("--no-flatten");
  if (!o.junk) args.push("--no-junk");
  if (!o.numbers) args.push("--no-numbers");
  if (!o.body) args.push("--no-body");
  args.push(o.vmSealStrings === false ? "--no-vm-seal-strings" : "--vm-seal-strings");
  return args;
}

/** 种子规整：空 → 随机数字种子；纯数字 → 原样；其他字符串 → djb2 哈希 */
function normalizeSeed(seed) {
  const s = typeof seed === "string" ? seed.trim() : "";
  if (!s) return String(BigInt(Date.now()) * 1000n + BigInt(Math.floor(Math.random() * 1000)));
  if (/^\d+$/.test(s) && s.length <= 19) return s;
  let h = 5381n;
  for (let i = 0; i < s.length; i++) h = ((h * 33n) ^ BigInt(s.charCodeAt(i))) & 0xffffffffffffffffn;
  return String(h % 18446744073709551615n);
}

function run(cmd, args, { input, timeoutMs }) {
  return new Promise((resolve) => {
    const child = spawn(cmd, args, { stdio: ["pipe", "pipe", "pipe"] });
    let out = "";
    let err = "";
    child.stdin.on("error", () => { /* child exited early: EPIPE is fine */ });
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      resolve({ code: null, out, err: err + "\n[engine-server] timeout", timedOut: true });
    }, timeoutMs);
    child.stdout.on("data", (d) => (out += d));
    child.stderr.on("data", (d) => (err += d));
    child.on("error", (e) => {
      clearTimeout(timer);
      resolve({ code: null, out, err: String(e), timedOut: false });
    });
    child.on("close", (code) => {
      clearTimeout(timer);
      resolve({ code, out, err, timedOut: false });
    });
    if (input !== undefined) child.stdin.end(input);
    else child.stdin.end();
  });
}

async function runEngine(args, workDir, timeoutMs) {
  const mode = engineMode();
  if (mode === "native") {
    const result = await run(BIN, [...args, path.join(workDir, "input.lua")], { timeoutMs });
    return { ...result, codeText: result.out };
  }
  if (mode === "wasi") {
    const wasmArgs = [...args, "/work/input.lua", "/work/output.lua"];
    const result = await run(
      process.execPath,
      [WASI_RUNNER, JSON.stringify({ wasmPath: WASM_BIN, workDir, args: wasmArgs })],
      { timeoutMs },
    );
    let codeText = "";
    const outputPath = path.join(workDir, "output.lua");
    if (result.code === 0 && existsSync(outputPath)) codeText = readFileSync(outputPath, "utf8");
    return { ...result, out: codeText, codeText };
  }
  return { code: null, out: "", err: "engine binary and WASM runtime are both missing", timedOut: false, codeText: "" };
}

function appliedNames(o) {
  const list = [
    "英文水印首行",
    "v15 结构克隆",
    "executor 认证绑定",
    "环境检查组",
    "环境绑定（Roblox）",
    "防篡改",
    "反调试环境守卫",
  ];
  if (o.mangle) list.push("标识符重命名");
  if (o.strings) list.push("字符串加密");
  if (o.flatten) list.push("控制流平坦化");
  if (o.junk) list.push("垃圾代码注入");
  if (o.numbers) list.push("数字字面量重写");
  if (o.body) list.push("全程序加密");
  if (o.vmSealStrings !== false) list.push("VM 字符串封存");
  if (o.minify) list.push("单行压缩");
  return list;
}

function json(res, status, body, extraHeaders = {}) {
  const text = JSON.stringify(body);
  // Same-origin API only; security headers are applied before routing.
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "x-content-type-options": "nosniff",
    "cache-control": "no-store",
    ...extraHeaders,
  });
  res.end(text);
}

/* ================= 账户体系（服务端裁决） =================
 * 密码、会话、验证码、锁定和额度均由服务端管理；浏览器会话只通过 HttpOnly
 * Cookie 传递，JavaScript 无法读取令牌。状态持久化到 tools/.state.json。 */
const STATE_FILE = process.env.OBSUCVIM_STATE_FILE || path.join(__dirname, ".state.json");
const SESSION_TTL_MS = 7 * 24 * 3600 * 1000;
const SESSION_COOKIE_NAME = "obsucvim_session";
const SESSION_COOKIE_MAX_AGE = Math.floor(SESSION_TTL_MS / 1000);
const CODE_TTL_MS = 10 * 60_000;
const CAPTCHA_TTL_MS = 3 * 60_000;
const CAPTCHA_MIN_AGE_MS = 700;
const TURNSTILE_SITE_KEY = (process.env.CLOUDFLARE_TURNSTILE_SITE_KEY || "").trim();
const TURNSTILE_SECRET_KEY = (process.env.CLOUDFLARE_TURNSTILE_SECRET_KEY || "").trim();
const TURNSTILE_HOSTNAME = (process.env.CLOUDFLARE_TURNSTILE_HOSTNAME || "").trim().toLowerCase();
const TURNSTILE_ENABLED = Boolean(TURNSTILE_SITE_KEY && TURNSTILE_SECRET_KEY);
const TURNSTILE_MISCONFIGURED = Boolean(TURNSTILE_SITE_KEY) !== Boolean(TURNSTILE_SECRET_KEY);
const TURNSTILE_SITEVERIFY_URL = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
const CODE_RESEND_MS = 60_000;
const CODE_MAX_FAILS = 5;
const LOGIN_MAX_FAILS = 5;
const LOGIN_LOCK_MS = 60_000;
const AUTH_BODY_LIMIT = 32 * 1024;
const MAX_ACCOUNTS = 1000;      // 演示环境账户上限
const EMAIL_MAX = 100;          // 邮箱长度上限（防长邮箱膨胀状态文件）
const MAX_PENDING_REG = 200;    // 同时待验证的注册草稿上限
const MAX_FOLDERS_PER_ACCOUNT = 10;
const MAX_BATCH_KEYS = 100;
const MAX_SCRIPTS_PER_ACCOUNT = 50;
const MAX_LICENSES_PER_SCRIPT = 2000;
const LICENSE_VERIFY_LIMIT = 60;
const LICENSE_VERIFY_WINDOW_MS = 60_000;
const LICENSE_NONCE_TTL_MS = 5 * 60_000;
const LICENSE_SESSION_TTL_MS = 2 * 3600_000;
const LICENSE_MAX_FAILS = 10;
const LICENSE_LOCK_MS = 10 * 60_000;
const MAX_RELEASE_BYTES = 4 * 1024 * 1024;
const MAX_RELEASES_PER_SCRIPT = 20;
const DAILY_QUOTA = 5;

let state = {
  users: {},      // email -> { email, nickname, salt, hash, created }
  sessions: {},   // server-side session token -> { email, expires }; browser token is HttpOnly cookie
  authSessionVersion: 0, // migration marker; invalidates old JS-readable bearer sessions once
  quotas: {},      // email -> { date, used }
  fails: {},      // email -> { fails, until }
  codes: {},      // email -> { code, expires, sentAt, fails }
  captchas: {},   // id -> { targetX, issuedAt, expires }; answer never leaves server
  apiKeys: {},    // key -> { email, created }
  pendingRegistrations: {}, // email -> { nickname, salt, hash, expires }
  registeredIps: {}, // HMAC(IP) -> { email, created }; lifetime one-account-per-IP index
  registrationIpHashKey: "", // stable per-install key; raw client IPs are never persisted
  folders: {}, // folderId -> { ownerEmail, name, createdAt, logWebhookUrl, alertWebhookUrl }
  scripts: {}, // scriptId -> { ownerEmail, folderId, name, createdAt, active }
  licenseKeys: {}, // licenseId -> hashed key metadata; raw keys are never stored
  licenseKeyIndex: {}, // SHA-256(key) -> licenseId
  licenseHwidHashKey: "", // stable server-only HMAC key; raw device IDs are never stored
  licenseKeyHashKey: "", // server-only HMAC key for license-key digests
  licenseNonces: {}, // `${licenseId}:${nonceDigest}` -> expiresAt; replay protection
  licenseSessions: {}, // short-lived token -> { licenseId, scriptId, hwidHash, expires }
  licenseAttempts: {}, // `${scriptId}:${ipHash}` -> { fails, until }; per-client abuse throttle
  releases: {}, // scriptId -> { version, sha256, size, createdAt, file }
  deviceBans: {}, // HMAC(hwid) -> { reason, expireAt, bannedAt }
  auditLog: [],   // [{ at, email, action, target, ip, meta }] newest last, capped
  metrics: {},    // "YYYY-MM-DD" -> { validations, rejected, executions, devices: {hash: 1} }
  webhooks: {},   // webhookId -> { folderId, ownerEmail, name, url, template, sanitizers, secret }
  webhookLog: [], // [{ at, webhookId, folderId, status, detail }] capped
  externalApps: {}, // appId -> { ownerEmail, name, secret, createdAt }
};

function loadState() {
  try {
    mkdirSync(path.dirname(STATE_FILE), { recursive: true });
  } catch {
    /* directory may be read-only; saveState reports nothing either */
  }
  try {
    const raw = readFileSync(STATE_FILE, "utf8");
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") state = { ...state, ...parsed };
  } catch {
    /* first run */
  }
  let changed = false;
  if (!state.registeredIps || typeof state.registeredIps !== "object" || Array.isArray(state.registeredIps)) {
    state.registeredIps = {};
    changed = true;
  }
  if (state.authSessionVersion !== 1) {
    state.sessions = {};
    state.authSessionVersion = 1;
    changed = true;
  }
  if (typeof state.registrationIpHashKey !== "string" || !/^[\da-f]{64}$/i.test(state.registrationIpHashKey)) {
    state.registrationIpHashKey = randomBytes(32).toString("hex");
    changed = true;
  }
  for (const key of ["scripts", "licenseKeys", "licenseKeyIndex"]) {
    if (!state[key] || typeof state[key] !== "object" || Array.isArray(state[key])) {
      state[key] = {};
      changed = true;
    }
  }
  if (typeof state.licenseHwidHashKey !== "string" || !/^[\da-f]{64}$/i.test(state.licenseHwidHashKey)) {
    state.licenseHwidHashKey = randomBytes(32).toString("hex");
    changed = true;
  }
  if (typeof state.licenseKeyHashKey !== "string" || !/^[\da-f]{64}$/i.test(state.licenseKeyHashKey)) {
    state.licenseKeyHashKey = randomBytes(32).toString("hex");
    changed = true;
  }
  for (const key of ["folders", "licenseNonces", "licenseSessions", "licenseAttempts", "releases", "deviceBans", "metrics", "webhooks", "externalApps"]) {
    if (!state[key] || typeof state[key] !== "object" || Array.isArray(state[key])) {
      state[key] = {};
      changed = true;
    }
  }
  for (const key of ["auditLog", "webhookLog"]) {
    if (!Array.isArray(state[key])) {
      state[key] = [];
      changed = true;
    }
  }
  // Legacy records predate project folders. Give each owner one folder so their
  // existing scripts and keys keep working without a manual migration step.
  for (const script of Object.values(state.scripts)) {
    if (script.folderId && state.folders[script.folderId]) continue;
    const folder = ensureDefaultFolder(script.ownerEmail);
    script.folderId = folder.folderId;
    changed = true;
  }
  for (const license of Object.values(state.licenseKeys)) {
    if (license.folderId) continue;
    const script = state.scripts[license.scriptId];
    if (script?.folderId) {
      license.folderId = script.folderId;
      changed = true;
    }
  }
  if (changed) saveState();
}

function ensureDefaultFolder(ownerEmail) {
  const existing = Object.values(state.folders).find((folder) => folder.ownerEmail === ownerEmail);
  if (existing) return existing;
  const folderId = "fld_" + randomBytes(12).toString("hex");
  state.folders[folderId] = {
    folderId,
    ownerEmail,
    name: "Default",
    createdAt: Date.now(),
    logWebhookUrl: "",
    alertWebhookUrl: "",
    selfReset: false,
    resetCooldownDays: 7,
  };
  return state.folders[folderId];
}

/* 原子写：先写临时文件再 rename。断电/崩溃只会留下旧文件，不会留下半截 JSON。
 * 单实例下足够；多实例部署仍需要真正的数据库（见 docs/HOSTING-V2-PLAN.md Phase 6）。 */
function saveState() {
  const payload = JSON.stringify(state);
  const tmp = `${STATE_FILE}.${process.pid}.tmp`;
  try {
    writeFileSync(tmp, payload);
    renameSync(tmp, STATE_FILE);
  } catch {
    try {
      writeFileSync(STATE_FILE, payload);
    } catch {
      /* 只读介质：忽略 */
    }
  }
}

const DATA_DIR = process.env.OBSUCVIM_DATA_DIR || path.dirname(STATE_FILE);
const RELEASE_DIR = path.join(DATA_DIR, "releases");
const GUARD_TEMPLATE_FILE = path.join(__dirname, "license-guard.luau");
const LOADER_TEMPLATE_FILE = path.join(__dirname, "loader-template.luau");
const ATTEST_TEMPLATE_FILE = path.join(__dirname, "attest.luau");

let cachedLoaderTemplate = null;
function loaderTemplate() {
  if (cachedLoaderTemplate === null) cachedLoaderTemplate = readFileSync(LOADER_TEMPLATE_FILE, "utf8");
  return cachedLoaderTemplate;
}

/** Stable public loader: same URL forever, contents swapped on each publish. */
/* 执行器证明 + Roblox 环境绑定是**强制**的：每次请求 /files/<id>.lua 都会把
 * attest.luau 拼在 loader 前面，没有任何开关能关掉。它跑在验卡之前，把结论写进
 * getgenv().OBS_Attest / OBS_Attested，并随 validate 上报服务端。
 *
 * 为什么放在 loader 而不是混淆产物里：WASI 引擎在「注入前言」里多加嵌套函数会
 * 段错误（实测 +2 个函数还稳、+4 个就开始崩），而 loader 是明文、不经过混淆，
 * 想加多少探测都安全。产物里的 guard 保持原样（6/6 稳定）。 */
function buildLoaderSource(script, origin) {
  return [attestTemplate(), loaderTemplate()]
    .join("\n")
    .replaceAll("__SCRIPT_ID__", script.scriptId)
    .replaceAll("__BASE_URL__", origin)
    .replaceAll("__HEARTBEAT__", script.heartbeat ? "true" : "false")
    .replaceAll("__SILENT__", script.silent ? "true" : "false");
}

let cachedGuardTemplate = null;
function guardTemplate() {
  if (cachedGuardTemplate === null) cachedGuardTemplate = readFileSync(GUARD_TEMPLATE_FILE, "utf8");
  return cachedGuardTemplate;
}

/* Prepends the licence guard so the check runs before any user code, then the
 * whole file is obfuscated as one unit. Only the public project id is embedded. */
let cachedAttestTemplate = null;
function attestTemplate() {
  if (cachedAttestTemplate === null) cachedAttestTemplate = readFileSync(ATTEST_TEMPLATE_FILE, "utf8");
  return cachedAttestTemplate;
}

/* 顺序很重要：证明先跑（拿到执行器/Roblox 证据），再由 guard 带着这份证据验卡，
 * 最后才是用户代码。三者一起进混淆器，谁都无法单独删掉其中一段。 */
function buildGuardedSource(source, scriptId, endpoint) {
  const template = guardTemplate()
    .replaceAll("__OBSUCVIM_SCRIPT_ID__", scriptId)
    .replaceAll("__OBSUCVIM_ENDPOINT__", endpoint);
  return `-- Injected by Obsucvim licence guard (runs before user code)\n${template}\n${source}`;
}

/* Absolute origin used inside generated guards. Prefers an explicit setting so a
 * spoofed Host header cannot redirect licence checks to another server. */
function publicOrigin(req) {
  const configured = (process.env.OBSUCVIM_PUBLIC_ORIGIN || "").trim().replace(/\/+$/, "");
  if (/^https:\/\/[A-Za-z0-9.-]+(:\d+)?$/.test(configured)) return configured;
  const host = req.headers.host;
  if (typeof host === "string" && /^[A-Za-z0-9.-]+(:\d+)?$/.test(host)) {
    const loopback = /^(localhost|127\.0\.0\.1)(:\d+)?$/.test(host);
    return `${loopback ? "http" : "https"}://${host}`;
  }
  return "https://obsucvim.l2.ink";
}

const DEFAULT_OBFUSCATION_OPTIONS = {
  minify: true, mangle: true, strings: true, flatten: true,
  junk: true, numbers: true, body: true, vmSealStrings: true, seed: "",
};

/* Runs the engine on in-memory source. The caller owns quota accounting. */
/* WASI 引擎偶尔会段错误（rc=139 或直接没有产物），跟输入长度、内容都有关，
 * 换一个种子重跑几乎总能成功。所以：崩溃自动重试（最多 3 次），
 * 语法错误之类「跑完了但失败」的不重试，免得把用户的等待时间翻倍。 */
const ENGINE_ATTEMPTS = 3;

function looksLikeCrash(job) {
  if (job.timedOut) return false;
  if (job.codeText) return false;
  if (job.code === 0) return true;              // 正常退出却没有产物
  if (job.code === null) return true;            // 被信号杀掉
  if (job.code < 0) return true;
  if (job.code >= 128) return true;              // 128+n = 信号
  // 引擎的语法错误会打印可读信息，崩溃只会留下 runtime 警告
  return !/error|expected|unexpected|syntax/i.test(job.err || "");
}

async function obfuscateSource(source, options) {
  if (!trySlot()) return { ok: false, status: 503, error: "ENGINE_BUSY" };
  const pinnedSeed = Boolean(options && typeof options.seed === "string" && options.seed.trim());
  let lastDetail = "";
  let seedNote;
  try {
    for (let attempt = 1; attempt <= ENGINE_ATTEMPTS; attempt++) {
      const attemptOptions = attempt === 1 || !pinnedSeed
        ? options
        : { ...options, seed: undefined }; // 固定种子重跑没意义，第二次起换成随机种子
      const workDir = mkdtempSync(path.join(tmpdir(), `obsucvim-job-${process.pid}-`));
      try {
        writeFileSync(path.join(workDir, "input.lua"), source, "utf8");
        const job = await runEngine(buildCliArgs(attemptOptions), workDir, ENGINE_TIMEOUT_MS);
        if (job.timedOut) return { ok: false, status: 504, error: "ENGINE_TIMEOUT" };
        if (job.code === 0 && job.codeText) {
          return {
            ok: true,
            code: job.codeText,
            attempts: attempt,
            ...(attempt > 1 && pinnedSeed ? { seedOverridden: true } : {}),
          };
        }
        lastDetail = (job.err || "").trim().split("\n").slice(-3).join(" | ");
        if (!looksLikeCrash(job)) {
          return { ok: false, status: 500, error: "ENGINE_FAIL", detail: lastDetail, attempts: attempt };
        }
        seedNote = attempt;
        // 崩溃：换个种子再来一次（默认种子本来就是随机的）
        if (attempt < ENGINE_ATTEMPTS) await new Promise((r) => setTimeout(r, 120 * attempt));
      } finally {
        try { rmSync(workDir, { recursive: true, force: true }); } catch { /* ignore */ }
      }
    }
    return {
      ok: false,
      status: 500,
      error: "ENGINE_CRASH",
      detail: `engine produced no output after ${ENGINE_ATTEMPTS} attempts (${lastDetail || "segfault"})`,
      attempts: seedNote ?? ENGINE_ATTEMPTS,
    };
  } finally {
    releaseSlot();
  }
}

function saveReleaseFile(scriptId, version, code, variant) {
  const dir = path.join(RELEASE_DIR, scriptId);
  mkdirSync(dir, { recursive: true });
  const suffix = variant ? `.${variant}` : "";
  const file = path.join(dir, `${version}${suffix}.lua`);
  writeFileSync(file, code, "utf8");
  return file;
}

/* ---------------- 异步发布作业 ----------------
 * 混淆是 1–3 秒的 CPU 活，同步等会顶到反代的超时。改成作业：
 * 提交立刻返回 jobId，前端轮询进度。额度在提交时预留，失败即退还，
 * 所以「失败扣额度」这种坑不会出现。作业只在内存里，进程重启即丢——
 * 这时前端会拿到 404 JOB_NOT_FOUND，提示重试即可。 */
const PUBLISH_STAGES = {
  queued: 5,
  injecting: 20,
  obfuscating: 65,
  storing: 90,
  completed: 100,
};
const MAX_ACTIVE_JOBS_PER_ACCOUNT = 2;
const PUBLISH_JOB_TTL_MS = 10 * 60_000;
const publishJobs = new Map();

function publishJobJson(job) {
  return {
    jobId: job.id,
    status: job.status,
    stage: job.stage,
    progress: job.progress,
    startedAt: job.startedAt,
    elapsedMs: (job.finishedAt ?? Date.now()) - job.startedAt,
    ...(job.result ? { result: job.result } : {}),
    ...(job.error ? { error: job.error, detail: job.detail } : {}),
  };
}

function activeJobsFor(email) {
  return [...publishJobs.values()].filter(
    (job) => job.ownerEmail === email && job.status === "processing",
  );
}

function setJobStage(job, stage) {
  job.stage = stage;
  job.progress = PUBLISH_STAGES[stage] ?? job.progress;
}

function refundQuota(email, job) {
  if (!job.quotaCharged) return;
  const q = quotaOf(email);
  if (q.date === job.quotaDate) {
    q.used = Math.max(0, q.used - 1);
    state.quotas[email] = q;
    job.quotaCharged = false;
  }
}

function failJob(job, error, detail) {
  job.status = "failed";
  job.stage = "failed";
  job.error = error;
  if (detail) job.detail = String(detail).slice(0, 300);
  job.finishedAt = now();
  refundQuota(job.ownerEmail, job);
  job.source = null;
  saveState();
}

function sweepPublishJobs() {
  const t = now();
  for (const [id, job] of publishJobs) {
    if (job.status === "processing" && t - job.startedAt > PUBLISH_JOB_TTL_MS) {
      failJob(job, "JOB_EXPIRED");
    } else if (job.status !== "processing" && job.finishedAt && t - job.finishedAt > PUBLISH_JOB_TTL_MS) {
      publishJobs.delete(id);
    }
  }
}

/* 真正的流水线。调用方已经校验过入参与额度。 */
/* 每个版本产两份产物，保护一点不丢：
 *   hard = 原来的完整加固（--env-check + --bind-executor + --bind-env roblox）
 *   safe = 兼容版（--env-check + --no-bind-executor），任何执行器都能跑
 * 客户端由 loader 先用证明层判断宿主能力，再决定取哪一份。宿主谎报只会让自己
 * 拿到跑不起来的 hard 版本，无法借此获得任何好处。 */
const RELEASE_VARIANTS = [
  { name: "safe", label: "compatible" },
  { name: "hard", label: "hardened" },
];

async function runPublishJob(job, { source, version, options, origin }) {
  try {
    setJobStage(job, "injecting");
    let guarded;
    try {
      guarded = buildGuardedSource(source, job.scriptId, `${origin}/api/licenses/validate`);
    } catch {
      return failJob(job, "GUARD_TEMPLATE_MISSING");
    }
    job.source = null; // 注入后立刻丢掉原始源码引用

    if (engineMode() === "missing") return failJob(job, "ENGINE_MISSING");
    setJobStage(job, "obfuscating");
    const started = Date.now();

    const variants = {};
    let attempts = 0;
    // 两份产物必须用同一个种子：这样它们只差「加固层」，体积/差异才可比，
    // 否则随机种子带来的 ±40 KB 抖动会把对比彻底淹没。
    const pinnedSeed = typeof options.seed === "string" && options.seed.trim()
      ? options.seed
      : normalizeSeed("");
    for (const variant of RELEASE_VARIANTS) {
      const variantOptions = variant.name === "hard"
        ? { ...options, seed: pinnedSeed, hardening: true }
        : { ...options, seed: pinnedSeed, hardening: false };
      const out = await obfuscateSource(guarded, variantOptions);
      if (out.ok) {
        attempts += out.attempts ?? 1;
        const bytes = Buffer.byteLength(out.code, "utf8");
        if (bytes > MAX_RELEASE_BYTES) return failJob(job, "RELEASE_TOO_LARGE");
        variants[variant.name] = {
          variant: variant.name,
          label: variant.label,
          code: out.code,
          bytes,
          sha256: createHash("sha256").update(out.code).digest("hex"),
        };
        continue;
      }
      // safe 版失败 = 这次发布彻底失败；hard 版失败 = 只少一层加固，不拖垮发布。
      if (variant.name === "safe") {
        return failJob(job, out.error, out.detail);
      }
      job.hardVariantError = out.error;
    }
    job.attempts = attempts;

    setJobStage(job, "storing");
    const key = `${job.scriptId}:${version}`;
    const primary = variants.safe;
    const stored = {};
    for (const [name, data] of Object.entries(variants)) {
      stored[name] = {
        variant: name,
        label: data.label,
        sha256: data.sha256,
        size: data.bytes,
        file: saveReleaseFile(job.scriptId, version, data.code, name),
      };
    }
    state.releases[key] = {
      scriptId: job.scriptId,
      ownerEmail: job.ownerEmail,
      version,
      // 兼容旧字段：顶层始终指向 compatible 版本
      sha256: primary.sha256,
      size: primary.bytes,
      createdAt: now(),
      file: stored.safe.file,
      pipeline: true,
      variants: stored,
    };
    if (state.scripts[job.scriptId]) state.scripts[job.scriptId].latestVersion = version;
    saveState();

    job.result = {
      scriptId: job.scriptId,
      version,
      sha256: primary.sha256,
      size: primary.bytes,
      sourceBytes: job.sourceBytes,
      guardedBytes: Buffer.byteLength(guarded, "utf8"),
      elapsedMs: Date.now() - started,
      variants: Object.fromEntries(
        Object.entries(stored).map(([name, data]) => [name, {
          sha256: data.sha256, size: data.size, label: data.label,
        }]),
      ),
    };
    job.status = "completed";
    setJobStage(job, "completed");
    job.finishedAt = now();
  } catch (error) {
    failJob(job, "ENGINE_FAIL", error instanceof Error ? error.message : String(error));
  }
}

loadState();

// 过期清扫（captchas / codes / sessions）
setInterval(() => {
  const n = now();
  for (const [k, c] of Object.entries(state.captchas)) if (n > c.expires) delete state.captchas[k];
  for (const [k, c] of Object.entries(state.codes)) if (n > c.expires) delete state.codes[k];
  for (const [k, s] of Object.entries(state.sessions)) if (n > s.expires) delete state.sessions[k];
  for (const [k, d] of Object.entries(state.pendingRegistrations)) if (n > d.expires) delete state.pendingRegistrations[k];
  for (const [k, expires] of Object.entries(state.licenseNonces)) if (n > expires) delete state.licenseNonces[k];
  for (const [k, session] of Object.entries(state.licenseSessions)) if (n > session.expires) delete state.licenseSessions[k];
  sweepPublishJobs();
  saveState();
}, 300_000).unref?.();

const now = () => Date.now();
const todayStr = () => {
  const d = new Date(); // server-local timezone; Windows Server clock/timezone is authoritative
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};
const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const normalizeEmail = (email) => email.trim().toLowerCase();
const registrationIpHash = (ip) =>
  createHmac("sha256", state.registrationIpHashKey).update(ip).digest("hex");

function hashPw(password, salt) {
  return scryptSync(password, salt, 64, { N: 16384, r: 8, p: 1 }).toString("hex");
}

function pwEqual(password, salt, expectHex) {
  const a = Buffer.from(hashPw(password, salt), "hex");
  const b = Buffer.from(expectHex, "hex");
  return a.length === b.length && timingSafeEqual(a, b);
}

const userPublic = (u) => ({ email: u.email, nickname: u.nickname, plan: "free" });

function sessionFor(email) {
  const token = randomBytes(32).toString("hex");
  state.sessions[token] = { email, expires: now() + SESSION_TTL_MS };
  saveState();
  return token;
}

function authUser(req) {
  const sessionToken = sessionTokenFromCookie(req);
  if (sessionToken) {
    const session = state.sessions[sessionToken];
    if (session && now() <= session.expires) return state.users[session.email] ?? null;
  }

  // Bearer tokens remain supported only for explicitly generated API keys.
  const h = req.headers["authorization"];
  if (typeof h !== "string" || !h.startsWith("Bearer ")) return null;
  return apiKeyUser(h.slice(7).trim());
}

/* 两步注册的服务端草稿：第一步保存昵称/密码（10 分钟），确认步消费——
 * 确认请求只带 email+code，防止两步之间注册信息被改写 */
const pendingRegistrations = state.pendingRegistrations;

const API_KEY_PREFIX = "obk_";
const SCRIPT_ID_PREFIX = "scr_";
const LICENSE_KEY_PREFIX = "osk_";

function newApiKey() {
  return API_KEY_PREFIX + randomBytes(24).toString("hex");
}

const licenseKeyDigest = (key) =>
  createHmac("sha256", state.licenseKeyHashKey).update(key).digest("hex");
const licenseNonceDigest = (nonce) => createHash("sha256").update(nonce).digest("hex");
const licenseHwidDigest = (hwid) =>
  createHmac("sha256", state.licenseHwidHashKey).update(hwid.trim()).digest("hex");

function digestsEqual(a, b) {
  if (typeof a !== "string" || typeof b !== "string" || a.length !== b.length) return false;
  try {
    return timingSafeEqual(Buffer.from(a, "hex"), Buffer.from(b, "hex"));
  } catch {
    return false;
  }
}

function licenseAttemptKey(scriptId, ip) {
  return `${scriptId}:${createHmac("sha256", state.licenseHwidHashKey).update(ip).digest("hex")}`;
}

function licenseBlockRemaining(attemptKey) {
  const entry = state.licenseAttempts[attemptKey];
  return entry && entry.until > now() ? Math.ceil((entry.until - now()) / 1000) : 0;
}

function noteLicenseFailure(attemptKey) {
  const entry = state.licenseAttempts[attemptKey] ?? { fails: 0, until: 0 };
  entry.fails += 1;
  if (entry.fails >= LICENSE_MAX_FAILS) {
    entry.until = now() + LICENSE_LOCK_MS;
    entry.fails = 0;
  }
  state.licenseAttempts[attemptKey] = entry;
}

function clearLicenseFailures(attemptKey) {
  if (state.licenseAttempts[attemptKey]) delete state.licenseAttempts[attemptKey];
}

const UNBAN_TOKEN_BYTES = 24;

/* A ban can be permanent (banExpireAt null) or lift on its own. Expired bans
 * are cleared lazily so we never keep rejecting someone after their time. */
function activeLicenseBan(license) {
  if (!license || !license.bannedAt) return null;
  if (license.banExpireAt && now() >= license.banExpireAt) {
    license.bannedAt = null;
    license.banReason = "";
    license.banExpireAt = null;
    return null;
  }
  return { reason: license.banReason || "", expireAt: license.banExpireAt ?? null };
}

function activeDeviceBan(hwidHash) {
  const entry = state.deviceBans[hwidHash];
  if (!entry) return null;
  if (entry.expireAt && now() >= entry.expireAt) {
    delete state.deviceBans[hwidHash];
    return null;
  }
  return entry;
}

/* Decide whether a license request is acceptable. Failures are counted per
 * client+script so a thief only throttles themselves; legitimate users are not
 * locked out by someone replaying their traffic. */
/* 客户端传来的 key/hwid 常常带复制时粘上的空格或换行，甚至大写十六进制。
 * 我们的 key 只会是小写 hex，所以统一 trim + 转小写既安全又不会放宽安全性
 * （hex 的 16 个符号本来就不区分大小写）。 */
const LICENSE_KEY_RE = /^osk_[\da-f]{64}$/i;
const SCRIPT_ID_RE = /^scr_[\da-f]{24}$/i;
const NONCE_RE = /^[\da-z]{16,64}$/i;

function normalizeLicenseKey(value) {
  if (typeof value !== "string") return { ok: true, key: "" };
  const trimmed = value.trim();
  if (trimmed === "") return { ok: true, key: "" };
  if (!LICENSE_KEY_RE.test(trimmed)) return { ok: false, key: trimmed, reason: "KEY_INVALID" };
  return { ok: true, key: trimmed.toLowerCase() };
}

function normalizeHwid(value) {
  if (typeof value !== "string") return { ok: true, hwid: "" };
  const trimmed = value.trim();
  if (trimmed.length > 256) return { ok: false, hwid: trimmed, reason: "HWID_INVALID" };
  return { ok: true, hwid: trimmed };
}

/** 脚本最新版本号，供 runtime.scriptVersion 使用。 */
function latestReleaseVersion(scriptId) {
  const release = latestReleaseFor(scriptId);
  return release ? release.version : null;
}

/* 谁算「最新版本」：优先用发布时记下的指针（同一毫秒内连续发布也不会打乱），
 * 老数据没有指针时退回按 createdAt 排序，再用版本号做稳定兜底。 */
function latestReleaseFor(scriptId) {
  const script = state.scripts[scriptId];
  if (script?.latestVersion) {
    const pinned = state.releases[`${scriptId}:${script.latestVersion}`];
    if (pinned) return pinned;
  }
  const releases = Object.values(state.releases)
    .filter((release) => release.scriptId === scriptId)
    .sort((a, b) => (b.createdAt ?? 0) - (a.createdAt ?? 0) || String(b.version).localeCompare(String(a.version)));
  return releases[0] ?? null;
}

function evaluateLicense({ scriptId, key, hwid, nonce, ip, dryRun = false }) {
  const script = state.scripts[scriptId];
  if (!script || !script.active) {
    return { ok: false, status: 404, error: "LICENSE_INVALID", code: "SCRIPT_NOT_FOUND" };
  }
  const folder = state.folders[script.folderId] ?? null;
  const hasHwid = typeof hwid === "string" && hwid.trim().length >= 8;
  const hwidHash = licenseHwidDigest(hasHwid ? hwid : "");

  // A banned device is refused before anything else, even with a fresh key.
  const deviceBan = hasHwid ? activeDeviceBan(hwidHash) : null;
  if (deviceBan) {
    return {
      ok: false, status: 403, error: "BANNED", code: "KEY_BANNED",
      folder, script, banReason: deviceBan.reason, banExpireAt: deviceBan.expireAt,
    };
  }

  const attemptKey = licenseAttemptKey(scriptId, ip || "unknown");
  const blankKey = typeof key !== "string" || key === "";

  // Free for All: no key required, nothing is bound and nothing is counted.
  if (blankKey) {
    if (script.ffa) return { ok: true, free: true, script, folder };
    return { ok: false, status: 401, error: "LICENSE_INVALID", code: "KEY_MISSING", folder, script };
  }

  if (!hasHwid) {
    return { ok: false, status: 400, error: "DEVICE_MISSING", code: "HWID_MISSING", folder, script };
  }

  const licenseId = state.licenseKeyIndex[licenseKeyDigest(key)];
  const license = licenseId ? state.licenseKeys[licenseId] : null;
  const keyMatches = Boolean(license && digestsEqual(license.keyHash, licenseKeyDigest(key)));

  // The guess throttle only applies to requests that do not present a valid key,
  // so correct credentials are never blocked by someone else's failed attempts.
  const blocked = keyMatches ? 0 : licenseBlockRemaining(attemptKey);
  if (blocked > 0) {
    return { ok: false, status: 429, error: "LICENSE_LOCKED", code: "KEY_LOCKED", retryAfter: blocked };
  }

  // Keys belong to a project folder, so one key unlocks every script inside it.
  const folderMatches = Boolean(
    license && keyMatches &&
    (license.folderId ? license.folderId === script.folderId : license.scriptId === scriptId),
  );
  if (!license || !keyMatches || !folderMatches) {
    noteLicenseFailure(attemptKey);
    return { ok: false, status: 403, error: "LICENSE_INVALID", code: "KEY_INCORRECT", folder, script };
  }

  if (license.revokedAt) {
    noteLicenseFailure(attemptKey);
    return { ok: false, status: 403, error: "LICENSE_INVALID", code: "KEY_REVOKED", folder, script };
  }

  const ban = activeLicenseBan(license);
  if (ban) {
    return {
      ok: false, status: 403, error: "BANNED", code: "KEY_BANNED",
      folder, script, banReason: ban.reason, banExpireAt: ban.expireAt,
    };
  }

  if (license.expiresAt && now() >= license.expiresAt) {
    noteLicenseFailure(attemptKey);
    return { ok: false, status: 403, error: "LICENSE_INVALID", code: "KEY_EXPIRED", folder, script };
  }

  if (license.hwidHash && !digestsEqual(license.hwidHash, hwidHash)) {
    // Not counted as abuse: the caller already holds the key.
    return { ok: false, status: 403, error: "DEVICE_MISMATCH", code: "KEY_HWID_LOCKED", folder, script };
  }

  const nonceKey = `${licenseId}:${licenseNonceDigest(nonce)}`;
  if (!dryRun && state.licenseNonces[nonceKey] && state.licenseNonces[nonceKey] > now()) {
    return { ok: false, status: 409, error: "NONCE_REPLAY", code: "NONCE_REPLAY", folder, script };
  }

  // dryRun 供 check_key 使用：只回报状态，不绑定设备、不计数、不消耗 nonce。
  if (dryRun) return { ok: true, license, script, folder, dryRun: true };

  if (!license.hwidHash) {
    license.hwidHash = hwidHash;
    license.firstActivatedAt = now();
    if (license.durationDays && !license.expiresAt) {
      license.expiresAt = now() + license.durationDays * 86400000;
    }
  }
  license.lastUsedAt = now();
  license.executions = (license.executions ?? 0) + 1;
  state.licenseNonces[nonceKey] = now() + LICENSE_NONCE_TTL_MS;
  clearLicenseFailures(attemptKey);
  return { ok: true, license, script, folder };
}

function issueLicenseSession(license) {
  const token = "lst_" + randomBytes(32).toString("hex");
  state.licenseSessions[token] = {
    licenseId: license.licenseId,
    scriptId: license.scriptId,
    hwidHash: license.hwidHash,
    expires: now() + LICENSE_SESSION_TTL_MS,
  };
  return token;
}
/* Webhook URLs are operator supplied, so treat them as untrusted input: only
 * https, only Discord's own hosts, and never carry a raw key or HWID. */
const WEBHOOK_HOST_SUFFIXES = ["discord.com", "discordapp.com"];
const webhookCooldown = new Map();

function normalizeWebhookUrl(value) {
  if (value === undefined || value === null || value === "") return "";
  if (typeof value !== "string" || value.length > 512) return null;
  let url;
  try { url = new URL(value); } catch { return null; }
  if (url.protocol !== "https:") return null;
  const host = url.hostname.toLowerCase();
  if (!WEBHOOK_HOST_SUFFIXES.some((suffix) => host === suffix || host.endsWith("." + suffix))) return null;
  return url.toString();
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/* ---------------- 审计日志（Phase 2） ---------------- */

const AUDIT_LIMIT = 500;

function recordAudit(ownerEmail, action, target, ip, meta) {
  state.auditLog.push({
    at: now(),
    email: ownerEmail,
    action,
    target: target ?? "",
    ip: typeof ip === "string" && ip ? registrationIpHash(ip).slice(0, 16) : "",
    meta: meta && typeof meta === "object" ? meta : undefined,
  });
  if (state.auditLog.length > AUDIT_LIMIT) state.auditLog.splice(0, state.auditLog.length - AUDIT_LIMIT);
}

function auditPublic(entry) {
  return {
    at: entry.at,
    action: entry.action,
    target: entry.target,
    ipHash: entry.ip,
    meta: entry.meta,
  };
}

/* ---------------- 指标（Phase 4） ---------------- */

const METRIC_DEVICE_CAP = 5000;

function bumpMetric(field, deviceHash) {
  const day = todayStr();
  const bucket = state.metrics[day] ?? (state.metrics[day] = {
    validations: 0, rejected: 0, executions: 0, devices: {}, attested: 0, unanattested: 0,
  });
  if (field === "devices") {
    if (deviceHash && Object.keys(bucket.devices).length < METRIC_DEVICE_CAP) bucket.devices[deviceHash] = 1;
    return;
  }
  bucket[field] = (bucket[field] ?? 0) + 1;
}

function metricsSnapshot(days) {
  const out = [];
  const today = new Date();
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(today.getTime() - i * 86400000);
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
    const bucket = state.metrics[key] ?? { validations: 0, rejected: 0, executions: 0, devices: {}, attested: 0, unanattested: 0 };
    out.push({
      date: key,
      validations: bucket.validations ?? 0,
      rejected: bucket.rejected ?? 0,
      executions: bucket.executions ?? 0,
      devices: Object.keys(bucket.devices ?? {}).length,
      attested: bucket.attested ?? 0,
      unanattested: bucket.unanattested ?? 0,
    });
  }
  return out;
}

/* ---------------- Webhook 投递（Phase 4） ---------------- */

const WEBHOOK_LOG_LIMIT = 200;

function webhookSecretFor(folder) {
  if (!folder.webhookSecret) {
    folder.webhookSecret = randomBytes(24).toString("hex");
    saveState();
  }
  return folder.webhookSecret;
}

function logWebhookDelivery(entry) {
  state.webhookLog.push({ at: now(), ...entry });
  if (state.webhookLog.length > WEBHOOK_LOG_LIMIT) {
    state.webhookLog.splice(0, state.webhookLog.length - WEBHOOK_LOG_LIMIT);
  }
}

/** 单次投递：带 HMAC-SHA256 签名，接收方可以用同一密钥验签。 */
async function postWebhook(url, payload, secret) {
  const body = JSON.stringify(payload);
  const headers = { "content-type": "application/json" };
  if (secret) headers["x-obsucvim-signature"] = `sha256=${createHmac("sha256", secret).update(body).digest("hex")}`;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 5000);
  try {
    const res = await fetch(url, { method: "POST", headers, body, signal: controller.signal });
    return { ok: res.ok, status: res.status };
  } catch (error) {
    return { ok: false, status: 0, detail: error instanceof Error ? error.message : String(error) };
  } finally {
    clearTimeout(timer);
  }
}

/** 最多 3 次尝试，250/500/1000ms 退避；每次都写一条投递日志。 */
async function deliverWebhook(url, payload, secret, meta = {}) {
  let last = { ok: false, status: 0 };
  for (let attempt = 0; attempt < 3; attempt++) {
    last = await postWebhook(url, payload, secret);
    if (last.ok) break;
    if (attempt < 2) await sleep(250 * 2 ** attempt);
  }
  logWebhookDelivery({
    ...meta,
    status: last.ok ? "delivered" : "failed",
    httpStatus: last.status,
    detail: last.detail ? String(last.detail).slice(0, 160) : undefined,
  });
  return last;
}

function dispatchWebhook(folder, kind, fields) {
  const target = kind === "alert" ? folder.alertWebhookUrl : folder.logWebhookUrl;
  if (!target) return;
  const cooldownKey = `${folder.folderId}:${kind}`;
  const last = webhookCooldown.get(cooldownKey) ?? 0;
  if (now() - last < 5000) return; // at most one event per 5s per folder+kind
  webhookCooldown.set(cooldownKey, now());
  const payload = {
    username: "Obsucvim",
    embeds: [{
      title: kind === "alert" ? "Licence rejected" : "Licence accepted",
      color: kind === "alert" ? 15158332 : 3066993,
      fields: fields.map(([name, value]) => ({ name, value: String(value).slice(0, 200), inline: true })),
      timestamp: new Date().toISOString(),
    }],
  };
  // Fire and forget: 投递失败绝不影响验卡。
  void deliverWebhook(target, payload, webhookSecretFor(folder), {
    kind, folderId: folder.folderId, target: kind,
  });
}

/* ---------------- Key Check 状态码（Phase 3，对齐 Luarmor） ---------------- */

const KEY_CHECK_MESSAGES = {
  KEY_VALID: "The provided key is valid.",
  KEY_EXPIRED: "The provided key has expired.",
  KEY_BANNED: "The provided key is blacklisted.",
  KEY_HWID_LOCKED: "The provided key has been locked to a different HWID. Reset your HWID to access it.",
  KEY_INCORRECT: "The provided key is incorrect / it does not exist.",
  KEY_INVALID: "The provided key is in an invalid format.",
  KEY_LOCKED: "Too many failed attempts, try again later.",
  SCRIPT_ID_INVALID: "The provided script ID is in an invalid format.",
  SCRIPT_ID_INCORRECT: "The provided script ID is incorrect / it does not exist.",
  INVALID_EXECUTOR: "HWID header contains invalid data. Executor might not be supported.",
  SECURITY_ERROR: "Request can not be validated by the server.",
  TIME_ERROR: "Client time is invalid.",
  UNKNOWN_ERROR: "Unknown server error.",
};

const KEY_CHECK_MAP = {
  SCRIPT_NOT_FOUND: "SCRIPT_ID_INCORRECT",
  HWID_MISSING: "INVALID_EXECUTOR",
  KEY_MISSING: "KEY_INVALID",
  KEY_LOCKED: "KEY_LOCKED",
  NONCE_REPLAY: "SECURITY_ERROR",
};

function toKeyCheckCode(code) {
  return KEY_CHECK_MAP[code] ?? code;
}

/* ---------------- 受保护的 webhook 模板（Phase 5） ---------------- */

const WEBHOOK_TEMPLATE_MAX = 4000;
const WEBHOOK_PAYLOAD_MAX = 7000;
const WEBHOOK_VALUE_MAX = 200;
const WEBHOOK_SEND_LIMIT = 30;
const WEBHOOK_SEND_WINDOW_MS = 60_000;

function normalizeSanitizers(input) {
  if (input === undefined || input === null) return {};
  if (typeof input !== "object" || Array.isArray(input)) return null;
  const keys = Object.keys(input);
  if (keys.length > 10) return null;
  const out = {};
  for (const name of keys) {
    if (!/^[A-Za-z_][A-Za-z0-9_]{0,31}$/.test(name)) return null;
    const pattern = input[name];
    if (typeof pattern !== "string" || pattern.length === 0 || pattern.length > 200) return null;
    try {
      // 服务端自己加锚点，模板里不要写 ^ $。
      new RegExp(`^(?:${pattern})$`);
    } catch {
      return null;
    }
    out[name] = pattern;
  }
  return out;
}

function sanitizeValue(value, pattern) {
  if (typeof value !== "string" && typeof value !== "number") return null;
  const text = String(value).slice(0, WEBHOOK_VALUE_MAX);
  if (!new RegExp(`^(?:${pattern})$`).test(text)) return null;
  return text;
}

/** 只替换白名单占位符：{{var}} 与 %SERVER_VAR%，其余原样保留。 */
function renderTemplate(template, values, serverVars) {
  let out = template;
  for (const [name, value] of Object.entries(values)) {
    out = out.split(`{{${name}}}`).join(JSON.stringify(value).slice(1, -1));
  }
  for (const [name, value] of Object.entries(serverVars)) {
    out = out.split(`%${name}%`).join(JSON.stringify(value).slice(1, -1));
  }
  return out;
}

/* ---------------- 外部验卡 API（Phase 6） ---------------- */

const EXTERNAL_CLOCK_SKEW_MS = 5 * 60_000;

function externalSignature(secret, ...parts) {
  return createHmac("sha256", secret).update(parts.join("|")).digest("hex");
}

/* ---------------- 批量操作（Phase 2） ---------------- */

const MAX_BATCH_TARGETS = 100;

function makeRateLimit(limit, windowMs) {
  const hits = new Map();
  setInterval(() => {
    const n = Date.now();
    for (const [key, hit] of hits) if (n > hit.reset) hits.delete(key);
  }, windowMs).unref?.();
  return (key) => {
    const n = Date.now();
    const hit = hits.get(key);
    if (!hit || n > hit.reset) {
      hits.set(key, { n: 1, reset: n + windowMs });
      return 0;
    }
    if (hit.n >= limit) return Math.max(1, Math.ceil((hit.reset - n) / 1000));
    hit.n += 1;
    return 0;
  };
}

const webhookSendLimited = makeRateLimit(WEBHOOK_SEND_LIMIT, WEBHOOK_SEND_WINDOW_MS);
const externalCheckLimited = makeRateLimit(60, 60_000);
const keyCheckLimited = makeRateLimit(60, 60_000);

function ownedLicenses(ownerEmail, licenseIds) {
  if (!Array.isArray(licenseIds) || licenseIds.length === 0 || licenseIds.length > MAX_BATCH_TARGETS) return null;
  const found = [];
  for (const id of licenseIds) {
    if (typeof id !== "string") return null;
    const license = state.licenseKeys[id];
    if (!license || license.ownerEmail !== ownerEmail) return null;
    found.push(license);
  }
  return found;
}

const folderPublic = (folder) => ({
  folderId: folder.folderId,
  name: folder.name,
  createdAt: folder.createdAt,
  logWebhook: Boolean(folder.logWebhookUrl),
  alertWebhook: Boolean(folder.alertWebhookUrl),
  selfReset: Boolean(folder.selfReset),
  resetCooldownDays: folder.resetCooldownDays ?? 7,
});

const licensePublic = (license) => ({
  licenseId: license.licenseId,
  scriptId: license.scriptId,
  folderId: license.folderId ?? null,
  label: license.label,
  createdAt: license.createdAt,
  expiresAt: license.expiresAt,
  revokedAt: license.revokedAt,
  hwidBound: Boolean(license.hwidHash),
  firstActivatedAt: license.firstActivatedAt,
  lastUsedAt: license.lastUsedAt,
  durationDays: license.durationDays ?? null,
  activated: Boolean(license.firstActivatedAt),
  bannedAt: license.bannedAt ?? null,
  banReason: license.banReason ?? "",
  banExpireAt: license.banExpireAt ?? null,
  banned: Boolean(activeLicenseBan(license)),
  totalResets: license.totalResets ?? 0,
  attested: license.lastAttested ?? null,
  attestScore: license.lastAttestScore ?? null,
  attestTotal: license.lastAttestTotal ?? null,
  attestedAt: license.lastAttestedAt ?? null,
  lastResetAt: license.lastHwidResetAt ?? null,
});

function apiKeyUser(key) {
  if (typeof key !== "string" || !key.startsWith(API_KEY_PREFIX)) return null;
  const k = state.apiKeys[key];
  return k ? state.users[k.email] ?? null : null;
}

function quotaOf(email) {
  const q = state.quotas[email];
  return q && q.date === todayStr() ? q : { date: todayStr(), used: 0 };
}

function quotaJson(email) {
  const q = quotaOf(email);
  return { usedToday: q.used, remaining: Math.max(0, DAILY_QUOTA - q.used), limit: DAILY_QUOTA };
}

/* 服务端生成 PNG 拼图验证码。答案坐标只保存在状态文件中；PNG 响应不含答案字段。
 * 使用内置 PNG 编码器，避免引入本地图片处理二进制或第三方 CAPTCHA SDK。 */
const CAPTCHA_WIDTH = 320;
const CAPTCHA_HEIGHT = 128;
const CAPTCHA_PIECE_SIZE = 56;
const CAPTCHA_MAX_OFFSET = CAPTCHA_WIDTH - CAPTCHA_PIECE_SIZE;
const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const PNG_CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = (c & 1) ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function pngCrc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) crc = PNG_CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const typeBytes = Buffer.from(type, "ascii");
  const body = Buffer.concat([typeBytes, data]);
  const length = Buffer.alloc(4);
  const checksum = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  checksum.writeUInt32BE(pngCrc32(body), 0);
  return Buffer.concat([length, body, checksum]);
}

function encodeCaptchaPng(width, height, pixels) {
  const rowBytes = width * 4;
  const raw = Buffer.alloc((rowBytes + 1) * height);
  for (let y = 0; y < height; y++) {
    const row = y * (rowBytes + 1);
    raw[row] = 0; // PNG filter: None
    Buffer.from(pixels.buffer, pixels.byteOffset + y * rowBytes, rowBytes).copy(raw, row + 1);
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8; // bit depth
  header[9] = 6; // RGBA
  header[10] = 0;
  header[11] = 0;
  header[12] = 0;
  return Buffer.concat([
    PNG_SIGNATURE,
    pngChunk("IHDR", header),
    pngChunk("IDAT", deflateSync(raw, { level: 6 })),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

function captchaBlend(pixels, width, height, x, y, color, opacity = 1) {
  x = Math.round(x);
  y = Math.round(y);
  if (x < 0 || y < 0 || x >= width || y >= height) return;
  const i = (y * width + x) * 4;
  const keep = 1 - opacity;
  pixels[i] = Math.round(pixels[i] * keep + color[0] * opacity);
  pixels[i + 1] = Math.round(pixels[i + 1] * keep + color[1] * opacity);
  pixels[i + 2] = Math.round(pixels[i + 2] * keep + color[2] * opacity);
  pixels[i + 3] = 255;
}

function captchaLine(pixels, width, height, x0, y0, x1, y1, color, opacity, thickness = 1) {
  x0 = Math.round(x0); y0 = Math.round(y0); x1 = Math.round(x1); y1 = Math.round(y1);
  const dx = Math.abs(x1 - x0), sx = x0 < x1 ? 1 : -1;
  const dy = -Math.abs(y1 - y0), sy = y0 < y1 ? 1 : -1;
  let error = dx + dy;
  const radius = Math.max(0, Math.floor(thickness / 2));
  while (true) {
    for (let oy = -radius; oy <= radius; oy++) {
      for (let ox = -radius; ox <= radius; ox++) {
        captchaBlend(pixels, width, height, x0 + ox, y0 + oy, color, opacity);
      }
    }
    if (x0 === x1 && y0 === y1) break;
    const twice = 2 * error;
    if (twice >= dy) { error += dy; x0 += sx; }
    if (twice <= dx) { error += dx; y0 += sy; }
  }
}

function captchaCircle(pixels, width, height, cx, cy, radius, color, opacity, filled = false, thickness = 1) {
  const r = Math.max(1, Math.ceil(radius));
  const outer = radius * radius;
  const inner = Math.max(0, radius - thickness) ** 2;
  for (let y = Math.floor(cy - r); y <= Math.ceil(cy + r); y++) {
    for (let x = Math.floor(cx - r); x <= Math.ceil(cx + r); x++) {
      const d2 = (x - cx) ** 2 + (y - cy) ** 2;
      if (d2 <= outer && (filled || d2 >= inner)) captchaBlend(pixels, width, height, x, y, color, opacity);
    }
  }
}

function captchaRandom(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function createCaptchaArtwork(seed) {
  const pixels = new Uint8Array(CAPTCHA_WIDTH * CAPTCHA_HEIGHT * 4);
  for (let y = 0; y < CAPTCHA_HEIGHT; y++) {
    for (let x = 0; x < CAPTCHA_WIDTH; x++) {
      const t = x / (CAPTCHA_WIDTH - 1);
      const wave = Math.sin((x + y * 1.7) / 27);
      const i = (y * CAPTCHA_WIDTH + x) * 4;
      pixels[i] = Math.round(5 + 13 * t + Math.max(0, wave) * 9);
      pixels[i + 1] = Math.round(25 + 30 * (1 - t) + Math.max(0, -wave) * 17);
      pixels[i + 2] = Math.round(46 + 50 * t + Math.max(0, wave) * 18);
      pixels[i + 3] = 255;
    }
  }
  const random = captchaRandom(seed);
  const colors = [[40, 220, 220], [88, 183, 255], [125, 244, 213], [182, 137, 255], [255, 189, 97]];
  for (let x = 14; x < CAPTCHA_WIDTH; x += 22) {
    captchaLine(pixels, CAPTCHA_WIDTH, CAPTCHA_HEIGHT, x, 0, x, CAPTCHA_HEIGHT - 1, [110, 220, 245], 0.055);
  }
  for (let y = 12; y < CAPTCHA_HEIGHT; y += 20) {
    captchaLine(pixels, CAPTCHA_WIDTH, CAPTCHA_HEIGHT, 0, y, CAPTCHA_WIDTH - 1, y, [110, 220, 245], 0.05);
  }
  for (let i = 0; i < 18; i++) {
    const x = random() * CAPTCHA_WIDTH;
    const y = random() * CAPTCHA_HEIGHT;
    const color = colors[Math.floor(random() * colors.length)];
    captchaCircle(pixels, CAPTCHA_WIDTH, CAPTCHA_HEIGHT, x, y, 3 + random() * 13, color, 0.12 + random() * 0.22, random() > 0.5, 1 + random() * 2);
  }
  for (let i = 0; i < 13; i++) {
    const color = colors[Math.floor(random() * colors.length)];
    captchaLine(
      pixels, CAPTCHA_WIDTH, CAPTCHA_HEIGHT,
      random() * CAPTCHA_WIDTH, random() * CAPTCHA_HEIGHT,
      random() * CAPTCHA_WIDTH, random() * CAPTCHA_HEIGHT,
      color, 0.18 + random() * 0.24, 1 + Math.floor(random() * 2),
    );
  }
  for (let i = 0; i < 90; i++) {
    captchaBlend(pixels, CAPTCHA_WIDTH, CAPTCHA_HEIGHT, random() * CAPTCHA_WIDTH, random() * CAPTCHA_HEIGHT, [190, 245, 255], 0.12 + random() * 0.3);
  }
  return pixels;
}

function captchaPieceContains(x, y) {
  const base = x >= 7 && x <= 49 && y >= 7 && y <= 49;
  const topKnob = (x - 28) ** 2 + (y - 7) ** 2 <= 9 ** 2;
  const bottomKnob = (x - 28) ** 2 + (y - 49) ** 2 <= 9 ** 2;
  const leftNotch = (x - 7) ** 2 + (y - 28) ** 2 < 8 ** 2;
  const rightNotch = (x - 49) ** 2 + (y - 28) ** 2 < 8 ** 2;
  return (base || topKnob || bottomKnob) && !leftNotch && !rightNotch;
}

function captchaPieceEdge(x, y) {
  return captchaPieceContains(x, y) && (
    !captchaPieceContains(x - 1, y) || !captchaPieceContains(x + 1, y) ||
    !captchaPieceContains(x, y - 1) || !captchaPieceContains(x, y + 1)
  );
}

function createCaptchaImages(targetX, pieceY, seed) {
  const board = createCaptchaArtwork(seed);
  const original = board.slice();
  const piece = new Uint8Array(CAPTCHA_PIECE_SIZE * CAPTCHA_PIECE_SIZE * 4);
  for (let y = 0; y < CAPTCHA_PIECE_SIZE; y++) {
    for (let x = 0; x < CAPTCHA_PIECE_SIZE; x++) {
      if (!captchaPieceContains(x, y)) continue;
      const source = ((pieceY + y) * CAPTCHA_WIDTH + targetX + x) * 4;
      const dest = (y * CAPTCHA_PIECE_SIZE + x) * 4;
      piece[dest] = original[source];
      piece[dest + 1] = original[source + 1];
      piece[dest + 2] = original[source + 2];
      piece[dest + 3] = 255;
      if (captchaPieceEdge(x, y)) captchaBlend(piece, CAPTCHA_PIECE_SIZE, CAPTCHA_PIECE_SIZE, x, y, [185, 250, 255], 0.82);

      if (captchaPieceEdge(x, y)) {
        captchaBlend(board, CAPTCHA_WIDTH, CAPTCHA_HEIGHT, targetX + x, pieceY + y, [161, 244, 255], 0.92);
      } else {
        captchaBlend(board, CAPTCHA_WIDTH, CAPTCHA_HEIGHT, targetX + x, pieceY + y, [1, 8, 17], 0.78);
      }
    }
  }
  const backgroundUrl = `data:image/png;base64,${encodeCaptchaPng(CAPTCHA_WIDTH, CAPTCHA_HEIGHT, board).toString("base64")}`;
  const pieceUrl = `data:image/png;base64,${encodeCaptchaPng(CAPTCHA_PIECE_SIZE, CAPTCHA_PIECE_SIZE, piece).toString("base64")}`;
  return { backgroundUrl, pieceUrl };
}

function issueCaptcha() {
  const id = randomBytes(16).toString("hex");
  const targetX = randomInt(36, CAPTCHA_MAX_OFFSET - 35);
  const pieceY = randomInt(24, CAPTCHA_HEIGHT - CAPTCHA_PIECE_SIZE - 24);
  const seed = randomBytes(4).readUInt32BE(0);
  const images = createCaptchaImages(targetX, pieceY, seed);
  state.captchas[id] = { targetX, issuedAt: now(), expires: now() + CAPTCHA_TTL_MS };
  saveState();
  return {
    provider: "puzzle",
    captchaId: id,
    ...images,
    width: CAPTCHA_WIDTH,
    height: CAPTCHA_HEIGHT,
    pieceSize: CAPTCHA_PIECE_SIZE,
    pieceY,
    maxOffset: CAPTCHA_MAX_OFFSET,
  };
}

function issueTurnstileChallenge() {
  return {
    provider: "turnstile",
    captchaId: randomBytes(16).toString("hex"),
    siteKey: TURNSTILE_SITE_KEY,
  };
}

function consumeCaptcha(id, answer) {
  if (typeof id !== "string" || !/^[\da-f]{32}$/.test(id)) return false;
  const c = state.captchas[id];
  if (!c) return false;
  delete state.captchas[id];
  saveState();
  if (
    !Number.isFinite(c.targetX) || !Number.isFinite(c.issuedAt) ||
    now() > c.expires || now() - c.issuedAt < CAPTCHA_MIN_AGE_MS
  ) return false;
  if (typeof answer !== "string" || !/^\d{1,3}$/.test(answer)) return false;
  const guess = Number(answer);
  return Math.abs(guess - c.targetX) <= 5;
}

function requestHostname(req) {
  const host = req.headers.host;
  if (typeof host !== "string" || !host) return "";
  try { return new URL(`http://${host}`).hostname.toLowerCase(); }
  catch { return ""; }
}

async function verifyTurnstileToken(req, captchaId, token) {
  if (typeof captchaId !== "string" || !/^[\da-f]{32}$/.test(captchaId)) return false;
  if (typeof token !== "string" || token.length < 20 || token.length > 4096) return false;
  const body = new URLSearchParams({ secret: TURNSTILE_SECRET_KEY, response: token });
  const ip = clientIp(req);
  if (ip !== "unknown") body.set("remoteip", ip);
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 5_000);
  try {
    const response = await fetch(TURNSTILE_SITEVERIFY_URL, {
      method: "POST",
      headers: { "content-type": "application/x-www-form-urlencoded" },
      body,
      signal: controller.signal,
    });
    if (!response.ok) return false;
    const result = await response.json();
    const expectedHostname = TURNSTILE_HOSTNAME || requestHostname(req);
    return result?.success === true &&
      typeof result.hostname === "string" && result.hostname.toLowerCase() === expectedHostname &&
      result.action === "auth";
  } catch {
    console.error("[engine-server] Turnstile verification request failed");
    return false;
  } finally {
    clearTimeout(timer);
  }
}

async function verifyCaptcha(req, captchaId, answer) {
  if (TURNSTILE_MISCONFIGURED) return false;
  if (TURNSTILE_ENABLED) return verifyTurnstileToken(req, captchaId, answer);
  return consumeCaptcha(captchaId, answer);
}

/* 邮箱验证码：签发/校验/冷却/尝试上限全在服务端。
 * demoMail=true（无真实邮件外发能力）时响应携带 demoCode 供界面明示。 */
const SMTP_HOST = process.env.SMTP_HOST || "smtp.163.com";
const SMTP_PORT = Number(process.env.SMTP_PORT || 465);
const SMTP_USER = process.env.SMTP_USER || "obsucvim@163.com";
const SMTP_PASS = process.env.SMTP_PASS || "";
const SMTP_FROM = process.env.SMTP_FROM || SMTP_USER;
const smtpAvailable = Boolean(SMTP_USER && SMTP_PASS);
const demoMail = !smtpAvailable;
const mailer = smtpAvailable
  ? nodemailer.createTransport({
      host: SMTP_HOST,
      port: SMTP_PORT,
      secure: SMTP_PORT === 465,
      requireTLS: SMTP_PORT === 587,
      auth: { user: SMTP_USER, pass: SMTP_PASS },
      tls: { minVersion: "TLSv1.2" },
      connectionTimeout: 10_000,
      greetingTimeout: 10_000,
      socketTimeout: 15_000,
    })
  : null;

async function deliverVerificationCode(email, code) {
  if (!mailer) return { ok: true, demoCode: code };
  try {
    await mailer.sendMail({
      from: `Obsucvim <${SMTP_FROM}>`,
      to: email,
      subject: "Obsucvim 邮箱验证码",
      text: `你的 Obsucvim 注册验证码是：${code}。验证码 10 分钟内有效，请勿转发。`,
      html: `<div style="font-family:Arial,sans-serif;max-width:520px;margin:auto;padding:28px;background:#07111b;color:#e8f4f7;border-radius:16px"><h2 style="color:#67e8f9">Obsucvim 邮箱验证</h2><p>你的验证码：</p><div style="font-size:30px;letter-spacing:8px;font-weight:bold;color:#a5f3fc;padding:16px 0">${code}</div><p>验证码 10 分钟内有效。若非本人操作，请忽略此邮件。</p></div>`,
    });
    return { ok: true };
  } catch (e) {
    console.error(`[engine-server] SMTP delivery failed (${e?.code || "unknown"})`);
    return { ok: false, error: "MAIL_SEND_FAILED" };
  }
}

function issueEmailCode(email) {
  const prev = state.codes[email];
  if (prev && now() - prev.sentAt < CODE_RESEND_MS) {
    return { ok: false, error: "RESEND_COOLDOWN", retryAfter: Math.ceil((CODE_RESEND_MS - (now() - prev.sentAt)) / 1000) };
  }
  const code = String(randomBytes(4).readUInt32BE(0) % 1_000_000).padStart(6, "0");
  state.codes[email] = { code, expires: now() + CODE_TTL_MS, sentAt: now(), fails: 0 };
  saveState();
  return { ok: true, code };
}

function checkEmailCode(email, input) {
  const c = state.codes[email];
  if (!c) return { ok: false, error: "CODE_NONE" };
  if (now() > c.expires) {
    delete state.codes[email];
    saveState();
    return { ok: false, error: "CODE_EXPIRED" };
  }
  if (String(input ?? "").trim() !== c.code) {
    c.fails += 1;
    if (c.fails >= CODE_MAX_FAILS) {
      delete state.codes[email];
      saveState();
      return { ok: false, error: "CODE_EXHAUSTED" };
    }
    saveState();
    return { ok: false, error: "CODE_WRONG" };
  }
  delete state.codes[email];
  saveState();
  return { ok: true };
}

/* 登录失败锁定：服务端按邮箱计数，localStorage 清不掉 */
function loginLocked(email) {
  const f = state.fails[email];
  return f && f.until > now() ? Math.ceil((f.until - now()) / 1000) : 0;
}

function noteLoginFail(email) {
  const f = state.fails[email] ?? { fails: 0, until: 0 };
  f.fails += 1;
  if (f.fails >= LOGIN_MAX_FAILS) {
    f.until = now() + LOGIN_LOCK_MS;
    f.fails = 0;
  }
  state.fails[email] = f;
  saveState();
}

/* ---------------- 安全：严格选项白名单 ---------------- */
const ALLOWED_OPTIONS = new Set([
  "minify", "mangle", "strings", "flatten", "junk",
  "numbers", "body", "vmSealStrings", "seed",
]);

function validateOptions(o) {
  if (!o || typeof o !== "object" || Array.isArray(o)) return "BAD_OPTIONS";
  for (const k of Object.keys(o)) {
    if (!ALLOWED_OPTIONS.has(k)) return "BAD_OPTIONS";
  }
  for (const k of ["minify", "mangle", "strings", "flatten", "junk", "numbers", "body", "vmSealStrings"]) {
    if (k in o && typeof o[k] !== "boolean") return "BAD_OPTIONS";
  }
  if ("seed" in o && (typeof o.seed !== "string" || o.seed.length > 64)) return "BAD_OPTIONS";
  return null;
}

/* ---------------- 安全：按 IP 限流（30 次/分钟） ---------------- */
const RATE_LIMIT = 30;
const RATE_WINDOW_MS = 60_000;
const rateHits = new Map(); // ip -> { n, reset }
setInterval(() => {
  const now = Date.now();
  for (const [ip, h] of rateHits) if (now > h.reset) rateHits.delete(ip);
}, RATE_WINDOW_MS).unref?.();

/* 默认只信 TCP 对端地址，忽略客户端可伪造的 X-Forwarded-For。
 * 只有当 TCP 对端命中 OBSUCVIM_TRUSTED_PROXY_IPS 时，才从 XFF 链中由右向左
 * 跳过已信任代理，取第一个非代理地址。代理必须覆盖或在末尾追加真实客户端 IP。 */
function normalizeIpAddress(value) {
  if (typeof value !== "string") return null;
  let address = value.trim().toLowerCase();
  if (!address) return null;
  const bracketed = address.match(/^\[([^\]]+)\](?::\d+)?$/);
  if (bracketed) address = bracketed[1];
  if (address.includes("%")) address = address.split("%", 1)[0];
  if (isIP(address) === 4) return address;
  if (isIP(address) !== 6) return null;

  let canonical;
  try {
    canonical = new URL(`http://[${address}]/`).hostname.slice(1, -1);
  } catch {
    return null;
  }
  if (canonical.startsWith("::ffff:")) {
    const mapped = canonical.slice(7);
    if (isIP(mapped) === 4) return mapped;
    const words = mapped.match(/^([\da-f]{1,4}):([\da-f]{1,4})$/);
    if (words) {
      const high = Number.parseInt(words[1], 16);
      const low = Number.parseInt(words[2], 16);
      return `${high >> 8}.${high & 255}.${low >> 8}.${low & 255}`;
    }
  }
  return canonical;
}

const TRUSTED_PROXY_IPS = new Set(
  (process.env.OBSUCVIM_TRUSTED_PROXY_IPS || "")
    .split(",")
    .map(normalizeIpAddress)
    .filter((ip) => ip !== null),
);

function isSecureRequest(req) {
  if (req.socket?.encrypted || process.env.OBSUCVIM_COOKIE_SECURE === "1") return true;
  const socketIp = normalizeIpAddress(req.socket?.remoteAddress);
  if (!socketIp || !TRUSTED_PROXY_IPS.has(socketIp)) return false;
  const forwardedProto = req.headers["x-forwarded-proto"];
  if (typeof forwardedProto !== "string") return false;
  return forwardedProto.split(",").at(-1)?.trim().toLowerCase() === "https";
}

function sessionCookieHeader(req, token, maxAge = SESSION_COOKIE_MAX_AGE) {
  const value = `${SESSION_COOKIE_NAME}=${token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=${maxAge}`;
  const secure = isSecureRequest(req) ? "; Secure" : "";
  const expires = maxAge === 0 ? "; Expires=Thu, 01 Jan 1970 00:00:00 GMT" : "";
  return `${value}${expires}${secure}`;
}

function collectInlineScriptHashes() {
  try {
    const html = readFileSync(path.join(WEB_ROOT, "index.html"), "utf8");
    const hashes = new Set();
    for (const match of html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script\s*>/gi)) {
      if (/\bsrc\s*=/i.test(match[1]) || !match[2].trim()) continue;
      const digest = createHash("sha256").update(match[2], "utf8").digest("base64");
      hashes.add(`'sha256-${digest}'`);
    }
    return [...hashes];
  } catch {
    return [];
  }
}

const INLINE_SCRIPT_HASHES = collectInlineScriptHashes();
const CONTENT_SECURITY_POLICY = [
  "default-src 'self'",
  `script-src 'self' ${INLINE_SCRIPT_HASHES.join(" ")}${TURNSTILE_ENABLED ? " https://challenges.cloudflare.com" : ""}`.trim(),
  "script-src-attr 'none'",
  "style-src 'self' 'unsafe-inline'",
  "style-src-attr 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "font-src 'self' data:",
  `connect-src 'self'${TURNSTILE_ENABLED ? " https://challenges.cloudflare.com" : ""}`,
  `frame-src ${TURNSTILE_ENABLED ? "https://challenges.cloudflare.com" : "'none'"}`,
  "frame-ancestors 'none'",
  "object-src 'none'",
  "base-uri 'self'",
  "form-action 'self'",
  "worker-src 'self' blob:",
  "manifest-src 'self'",
].join("; ");

function applySecurityHeaders(req, res) {
  res.setHeader("content-security-policy", CONTENT_SECURITY_POLICY);
  res.setHeader("x-frame-options", "DENY");
  res.setHeader("x-content-type-options", "nosniff");
  res.setHeader("referrer-policy", "strict-origin-when-cross-origin");
  res.setHeader("permissions-policy", "camera=(), microphone=(), geolocation=()");
  if (isSecureRequest(req)) res.setHeader("strict-transport-security", "max-age=31536000");
}

function sessionTokenFromCookie(req) {
  const cookieHeader = req.headers.cookie;
  if (typeof cookieHeader !== "string") return null;
  for (const part of cookieHeader.split(";")) {
    const separator = part.indexOf("=");
    if (separator < 0 || part.slice(0, separator).trim() !== SESSION_COOKIE_NAME) continue;
    const value = part.slice(separator + 1).trim();
    return /^[\da-f]{64}$/i.test(value) ? value : null;
  }
  return null;
}

function unauthorized(res, req) {
  const token = sessionTokenFromCookie(req);
  return json(
    res,
    401,
    { ok: false, error: "UNAUTHORIZED" },
    token ? { "set-cookie": sessionCookieHeader(req, "", 0) } : {},
  );
}

function clientIp(req) {
  const socketIp = normalizeIpAddress(req.socket?.remoteAddress) || "unknown";
  if (!TRUSTED_PROXY_IPS.has(socketIp)) return socketIp;
  const header = req.headers["x-forwarded-for"];
  if (typeof header !== "string") return socketIp;
  const forwarded = header.split(",").map(normalizeIpAddress).filter((ip) => ip !== null);
  let address = socketIp;
  for (let i = forwarded.length - 1; i >= 0 && TRUSTED_PROXY_IPS.has(address); i--) {
    address = forwarded[i];
  }
  return address;
}

/* auth 专用限流（独立桶，比引擎更严格：防垃圾注册同时不影响混淆使用） */
const AUTH_RATE_LIMIT = 15;       // 15 次/分钟
const AUTH_RATE_WINDOW_MS = 60_000;
const authRateHits = new Map();
setInterval(() => {
  const n = Date.now();
  for (const [ip, h] of authRateHits) if (n > h.reset) authRateHits.delete(ip);
}, AUTH_RATE_WINDOW_MS).unref?.();

/* Authenticated dashboard traffic gets its own, looser bucket: a single page
 * load legitimately fires several management calls, while anonymous login and
 * registration stay on the strict bucket. */
const AUTHED_RATE_LIMIT = 90;
const authedRateHits = new Map();
setInterval(() => {
  const n = Date.now();
  for (const [ip, h] of authedRateHits) if (n > h.reset) authedRateHits.delete(ip);
}, AUTH_RATE_WINDOW_MS).unref?.();

function authedRateLimited(ip) {
  const n = Date.now();
  const h = authedRateHits.get(ip);
  if (!h || n > h.reset) {
    authedRateHits.set(ip, { n: 1, reset: n + AUTH_RATE_WINDOW_MS });
    return false;
  }
  h.n += 1;
  return h.n > AUTHED_RATE_LIMIT;
}

function authRateLimited(ip) {
  const n = Date.now();
  const h = authRateHits.get(ip);
  if (!h || n > h.reset) {
    authRateHits.set(ip, { n: 1, reset: n + AUTH_RATE_WINDOW_MS });
    return false;
  }
  h.n += 1;
  return h.n > AUTH_RATE_LIMIT;
}

const CAPTCHA_ISSUE_LIMIT = 4;
const CAPTCHA_ISSUE_WINDOW_MS = 60_000;
const captchaIssueHits = new Map();
setInterval(() => {
  const n = Date.now();
  for (const [ip, h] of captchaIssueHits) if (n > h.reset) captchaIssueHits.delete(ip);
}, CAPTCHA_ISSUE_WINDOW_MS).unref?.();

function captchaIssueRetryAfter(ip) {
  const n = Date.now();
  const h = captchaIssueHits.get(ip);
  if (!h || n > h.reset) {
    captchaIssueHits.set(ip, { n: 1, reset: n + CAPTCHA_ISSUE_WINDOW_MS });
    return 0;
  }
  if (h.n >= CAPTCHA_ISSUE_LIMIT) return Math.max(1, Math.ceil((h.reset - n) / 1000));
  h.n += 1;
  return 0;
}

const licenseVerifyHits = new Map();
setInterval(() => {
  const n = Date.now();
  for (const [ip, h] of licenseVerifyHits) if (n > h.reset) licenseVerifyHits.delete(ip);
}, LICENSE_VERIFY_WINDOW_MS).unref?.();

function licenseVerifyRetryAfter(ip) {
  const n = Date.now();
  const h = licenseVerifyHits.get(ip);
  if (!h || n > h.reset) {
    licenseVerifyHits.set(ip, { n: 1, reset: n + LICENSE_VERIFY_WINDOW_MS });
    return 0;
  }
  if (h.n >= LICENSE_VERIFY_LIMIT) return Math.max(1, Math.ceil((h.reset - n) / 1000));
  h.n += 1;
  return 0;
}

function rateLimited(ip) {
  const now = Date.now();
  const h = rateHits.get(ip);
  if (!h || now > h.reset) {
    rateHits.set(ip, { n: 1, reset: now + RATE_WINDOW_MS });
    return false;
  }
  h.n += 1;
  return h.n > RATE_LIMIT;
}

async function readBody(req, limit) {
  return new Promise((resolve, reject) => {
    let size = 0;
    let overflowed = false;
    const chunks = [];
    req.on("data", (c) => {
      if (overflowed) return; // 已溢出：丢弃后续数据
      size += c.length;
      if (size > limit) {
        overflowed = true;
        reject(new Error("BODY_TOO_LARGE"));
        return;
      }
      chunks.push(c);
    });
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
    req.on("error", reject);
  });
}

/* 安全修复：并发引擎上限（实测 20 并发全过，无界 spawn = 资源耗尽）。
 * 超出立即 503 ENGINE_BUSY（可重试）。 */
const MAX_CONCURRENT = 4;
let inFlight = 0;
/* 安全修复（红队自测第 5 项）：临时文件名 pid+毫秒 在并发同毫秒到达时
 * 碰撞 → 请求互删文件 → 引擎读不到源码集体 500（实测 20 并发全 500）。
 * 追加原子计数器 + 随机后缀保证唯一。 */
let tmpSeq = 0;
const tmpName = (tag) =>
  `obsucvim-web-${tag}-${process.pid}-${process.hrtime.bigint()}-${tmpSeq++}-${
    Math.random().toString(36).slice(2, 10)
  }.lua`;

function trySlot() {
  if (inFlight >= MAX_CONCURRENT) return false;
  inFlight += 1;
  return true;
}

function releaseSlot() {
  inFlight = Math.max(0, inFlight - 1);
}

const server = createServer(async (req, res) => {
  applySecurityHeaders(req, res);
  if (req.method === "OPTIONS") return json(res, 204, {});

  /* ================= 认证端点 ================= */
  const handleAuth = async () => {
    const ip = clientIp(req);
    const alreadyAuthed = Boolean(authUser(req));
    const limited = alreadyAuthed ? authedRateLimited(ip) : authRateLimited(ip);
    if (limited) {
      return json(res, 429, { ok: false, error: "RATE_LIMITED" });
    }
    let body = {};
    if (req.method !== "GET" && req.method !== "HEAD") {
      try {
        body = JSON.parse(await readBody(req, AUTH_BODY_LIMIT));
      } catch (e) {
        if (e instanceof Error && e.message === "BODY_TOO_LARGE") {
          json(res, 413, { ok: false, error: "BODY_TOO_LARGE" });
          req.destroy();
          return;
        }
        return json(res, 400, { ok: false, error: "BAD_JSON" });
      }
      if (!body || typeof body !== "object" || Array.isArray(body)) {
        return json(res, 400, { ok: false, error: "BAD_REQUEST" });
      }
    }

    if (req.method === "POST" && req.url === "/api/auth/captcha") {
      const retryAfter = captchaIssueRetryAfter(clientIp(req));
      if (retryAfter > 0) {
        res.setHeader("retry-after", String(retryAfter));
        return json(res, 429, { ok: false, error: "CAPTCHA_RATE_LIMITED", retryAfter });
      }
      if (TURNSTILE_MISCONFIGURED) {
        return json(res, 503, { ok: false, error: "CAPTCHA_CONFIG_INVALID" });
      }
      const challenge = TURNSTILE_ENABLED ? issueTurnstileChallenge() : issueCaptcha();
      return json(res, 200, { ok: true, ...challenge });
    }

    if (req.method === "GET" && req.url === "/api/auth/folders") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      ensureDefaultFolder(owner.email);
      saveState();
      const folders = Object.values(state.folders)
        .filter((folder) => folder.ownerEmail === owner.email)
        .map(folderPublic);
      return json(res, 200, { ok: true, folders });
    }

    if (req.method === "POST" && req.url === "/api/auth/folders") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const { name } = body;
      if (typeof name !== "string" || name.trim().length < 2 || name.trim().length > 60) {
        return json(res, 400, { ok: false, error: "BAD_INPUT" });
      }
      const owned = Object.values(state.folders).filter((folder) => folder.ownerEmail === owner.email);
      if (owned.length >= MAX_FOLDERS_PER_ACCOUNT) return json(res, 409, { ok: false, error: "FOLDER_LIMIT" });
      const folderId = "fld_" + randomBytes(12).toString("hex");
      state.folders[folderId] = {
        folderId,
        ownerEmail: owner.email,
        name: name.trim(),
        createdAt: now(),
        logWebhookUrl: "",
        alertWebhookUrl: "",
        selfReset: false,
        resetCooldownDays: 7,
      };
      saveState();
      return json(res, 201, { ok: true, folder: folderPublic(state.folders[folderId]) });
    }

    if (req.method === "POST" && req.url === "/api/auth/folders/update") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const folder = typeof body.folderId === "string" ? state.folders[body.folderId] : null;
      if (!folder || folder.ownerEmail !== owner.email) return json(res, 404, { ok: false, error: "NOT_FOUND" });
      if (body.name !== undefined) {
        if (typeof body.name !== "string" || body.name.trim().length < 2 || body.name.trim().length > 60) {
          return json(res, 400, { ok: false, error: "BAD_INPUT" });
        }
        folder.name = body.name.trim();
      }
      if (body.selfReset !== undefined) {
        if (typeof body.selfReset !== "boolean") return json(res, 400, { ok: false, error: "BAD_INPUT" });
        folder.selfReset = body.selfReset;
      }
      if (body.resetCooldownDays !== undefined) {
        const days = Number(body.resetCooldownDays);
        if (!Number.isInteger(days) || days < 0 || days > 365) {
          return json(res, 400, { ok: false, error: "BAD_INPUT" });
        }
        folder.resetCooldownDays = days;
      }
      for (const [field, errorCode] of [["logWebhook", "BAD_WEBHOOK"], ["alertWebhook", "BAD_WEBHOOK"]]) {
        if (body[field] === undefined) continue;
        const normalized = normalizeWebhookUrl(body[field]);
        if (normalized === null) return json(res, 400, { ok: false, error: errorCode });
        if (field === "logWebhook") folder.logWebhookUrl = normalized;
        else folder.alertWebhookUrl = normalized;
      }
      saveState();
      return json(res, 200, { ok: true, folder: folderPublic(folder) });
    }

    if (req.method === "GET" && req.url === "/api/auth/scripts") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      // 每个脚本带上「最新版本 / 版本数 / 文件夹卡密数」，前端不用再逐个请求。
      const folderLicenseCounts = {};
      for (const license of Object.values(state.licenseKeys)) {
        if (license.ownerEmail !== owner.email) continue;
        folderLicenseCounts[license.folderId] = (folderLicenseCounts[license.folderId] ?? 0) + 1;
      }
      const scripts = Object.values(state.scripts)
        .filter((script) => script.ownerEmail === owner.email)
        .map(({ scriptId, folderId, name, description, createdAt, active, ffa, silent, heartbeat }) => {
          const releases = Object.values(state.releases).filter((release) => release.scriptId === scriptId);
          const latest = latestReleaseFor(scriptId);
          return {
            scriptId, folderId, name, description, createdAt, active,
            ffa: Boolean(ffa), silent: Boolean(silent), heartbeat: Boolean(heartbeat),
            releaseCount: releases.length,
            latestVersion: latest?.version ?? null,
            lastPublishedAt: latest?.createdAt ?? null,
            licenseCount: folderId ? (folderLicenseCounts[folderId] ?? 0) : 0,
          };
        });
      return json(res, 200, { ok: true, scripts });
    }

    if (req.method === "POST" && req.url === "/api/auth/scripts") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const { name, description, folderId } = body;
      if (
        typeof name !== "string" || name.trim().length < 2 || name.trim().length > 80 ||
        (description !== undefined && (typeof description !== "string" || description.length > 500))
      ) return json(res, 400, { ok: false, error: "BAD_INPUT" });
      const targetFolder = typeof folderId === "string" && folderId
        ? state.folders[folderId]
        : ensureDefaultFolder(owner.email);
      if (!targetFolder || targetFolder.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "FOLDER_NOT_FOUND" });
      }
      const owned = Object.values(state.scripts).filter((script) => script.ownerEmail === owner.email);
      if (owned.length >= MAX_SCRIPTS_PER_ACCOUNT) return json(res, 409, { ok: false, error: "SCRIPT_LIMIT" });
      const scriptId = SCRIPT_ID_PREFIX + randomBytes(12).toString("hex");
      const script = {
        scriptId,
        ownerEmail: owner.email,
        folderId: targetFolder.folderId,
        name: name.trim(),
        description: typeof description === "string" ? description.trim() : "",
        createdAt: now(),
        active: true,
      };
      state.scripts[scriptId] = script;
      saveState();
      return json(res, 201, {
        ok: true,
        script: {
          scriptId,
          folderId: script.folderId,
          name: script.name,
          description: script.description,
          createdAt: script.createdAt,
          active: true,
        },
      });
    }

    if (req.method === "POST" && req.url === "/api/auth/scripts/update") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const script = typeof body.scriptId === "string" ? state.scripts[body.scriptId] : null;
      if (!script || script.ownerEmail !== owner.email) return json(res, 404, { ok: false, error: "SCRIPT_NOT_FOUND" });
      if (body.name !== undefined) {
        if (typeof body.name !== "string" || body.name.trim().length < 2 || body.name.trim().length > 80) {
          return json(res, 400, { ok: false, error: "BAD_INPUT" });
        }
        script.name = body.name.trim();
      }
      for (const flag of ["ffa", "silent", "heartbeat"]) {
        if (body[flag] === undefined) continue;
        if (typeof body[flag] !== "boolean") return json(res, 400, { ok: false, error: "BAD_INPUT" });
        script[flag] = body[flag];
      }
      if (body.active !== undefined) {
        if (typeof body.active !== "boolean") return json(res, 400, { ok: false, error: "BAD_INPUT" });
        script.active = body.active;
      }
      saveState();
      return json(res, 200, {
        ok: true,
        script: {
          scriptId: script.scriptId, folderId: script.folderId, name: script.name,
          description: script.description, createdAt: script.createdAt,
          active: script.active, ffa: Boolean(script.ffa), silent: Boolean(script.silent),
          heartbeat: Boolean(script.heartbeat),
        },
      });
    }

    if (req.method === "GET" && new URL(req.url, "http://localhost").pathname === "/api/auth/licenses") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const url = new URL(req.url, "http://localhost");
      const scriptId = url.searchParams.get("scriptId");
      const folderId = url.searchParams.get("folderId");
      if (scriptId && state.scripts[scriptId]?.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "NOT_FOUND" });
      }
      if (folderId && state.folders[folderId]?.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "NOT_FOUND" });
      }
      const search = (url.searchParams.get("search") || "").trim().toLowerCase();
      const status = (url.searchParams.get("status") || "").trim().toLowerCase();
      const all = Object.values(state.licenseKeys).filter((license) => {
        if (license.ownerEmail !== owner.email) return false;
        if (folderId) return license.folderId === folderId;
        return !scriptId || license.scriptId === scriptId;
      });
      const decorated = all.map((license) => {
        const ban = activeLicenseBan(license);
        const stateLabel = ban
          ? "banned"
          : license.revokedAt
            ? "revoked"
            : license.expiresAt && now() >= license.expiresAt
              ? "expired"
              : "active";
        return { license, public: { ...licensePublic(license), status: stateLabel } };
      });
      const filtered = decorated.filter(({ license, public: view }) => {
        if (status && view.status !== status) return false;
        if (!search) return true;
        return (
          (license.label || "").toLowerCase().includes(search) ||
          license.licenseId.toLowerCase().includes(search)
        );
      });
      const total = filtered.length;
      const rawFrom = Number(url.searchParams.get("from"));
      const rawUntil = Number(url.searchParams.get("until"));
      const from = Number.isInteger(rawFrom) && rawFrom >= 0 ? rawFrom : 0;
      const until = Number.isInteger(rawUntil) && rawUntil > from
        ? Math.min(rawUntil, total)
        : total;
      const page = filtered.slice(from, until).map((entry) => entry.public);
      return json(res, 200, { ok: true, licenses: page, total, from, until });
    }

    if (req.method === "GET" && new URL(req.url, "http://localhost").pathname === "/api/auth/licenses/export") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const url = new URL(req.url, "http://localhost");
      const folderId = url.searchParams.get("folderId");
      const scriptId = url.searchParams.get("scriptId");
      if (folderId && state.folders[folderId]?.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "NOT_FOUND" });
      }
      if (scriptId && state.scripts[scriptId]?.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "NOT_FOUND" });
      }
      const rows = Object.values(state.licenseKeys).filter((license) => {
        if (license.ownerEmail !== owner.email) return false;
        if (folderId) return license.folderId === folderId;
        return !scriptId || license.scriptId === scriptId;
      });
      // Keys are stored as hashes, so an export can never leak a usable key.
      const header = "licenseId,label,scriptId,folderId,state,createdAt,expiresAt,durationDays,deviceBound,firstActivatedAt,lastUsedAt";
      const csvCell = (value) => {
        const text = value === null || value === undefined ? "" : String(value);
        return /[",\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
      };
      const body = rows.map((license) => {
        const stateLabel = license.revokedAt
          ? "revoked"
          : license.expiresAt && now() >= license.expiresAt ? "expired" : "active";
        return [
          license.licenseId, license.label, license.scriptId, license.folderId ?? "",
          stateLabel, new Date(license.createdAt).toISOString(),
          license.expiresAt ? new Date(license.expiresAt).toISOString() : "",
          license.durationDays ?? "",
          license.hwidHash ? "yes" : "no",
          license.firstActivatedAt ? new Date(license.firstActivatedAt).toISOString() : "",
          license.lastUsedAt ? new Date(license.lastUsedAt).toISOString() : "",
        ].map(csvCell).join(",");
      });
      const csv = [header, ...body].join("\n") + "\n";
      res.writeHead(200, {
        "content-type": "text/csv; charset=utf-8",
        "content-disposition": `attachment; filename="obsucvim-licenses-${now()}.csv"`,
        "cache-control": "no-store",
      });
      return res.end(csv);
    }

    if (req.method === "GET" && new URL(req.url, "http://localhost").pathname === "/api/auth/licenses/export.json") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const url = new URL(req.url, "http://localhost");
      const folderId = url.searchParams.get("folderId");
      const scriptId = url.searchParams.get("scriptId");
      if (folderId && state.folders[folderId]?.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "NOT_FOUND" });
      }
      if (scriptId && state.scripts[scriptId]?.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "NOT_FOUND" });
      }
      const rows = Object.values(state.licenseKeys).filter((license) => {
        if (license.ownerEmail !== owner.email) return false;
        if (folderId) return license.folderId === folderId;
        return !scriptId || license.scriptId === scriptId;
      });
      // 只导出元数据。卡密原文根本没存（只有 HMAC 摘要），所以这里不可能泄密。
      const licenses = rows.map((license) => {
        const ban = activeLicenseBan(license);
        return {
          licenseId: license.licenseId,
          label: license.label || "",
          folderId: license.folderId,
          scriptId: license.scriptId ?? null,
          createdAt: license.createdAt,
          expiresAt: license.expiresAt ?? null,
          durationDays: license.durationDays ?? null,
          revokedAt: license.revokedAt ?? null,
          deviceBound: Boolean(license.hwidHash),
          firstActivatedAt: license.firstActivatedAt ?? null,
          lastUsedAt: license.lastUsedAt ?? null,
          executions: license.executions ?? 0,
          ban: ban ? { reason: ban.reason || "", expiresAt: ban.expiresAt ?? null } : null,
        };
      });
      return json(res, 200, {
        ok: true,
        exportedAt: new Date(now()).toISOString(),
        folderId: folderId || null,
        count: licenses.length,
        note: "Keys themselves are only shown once at issuance; this export carries metadata only.",
        licenses,
      });
    }

    if (req.method === "POST" && req.url === "/api/auth/licenses/import") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const { folderId, entries } = body;
      const folder = typeof folderId === "string" ? state.folders[folderId] : null;
      if (!folder || folder.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "FOLDER_NOT_FOUND" });
      }
      if (!Array.isArray(entries) || entries.length === 0) {
        return json(res, 400, { ok: false, error: "BAD_INPUT" });
      }
      if (entries.length > MAX_BATCH_KEYS) {
        return json(res, 400, { ok: false, error: "BAD_COUNT" });
      }
      const existing = Object.values(state.licenseKeys).filter((license) => license.folderId === folder.folderId);
      if (existing.length + entries.length > MAX_LICENSES_PER_SCRIPT) {
        return json(res, 409, { ok: false, error: "LICENSE_LIMIT" });
      }
      const anchorScript = Object.values(state.scripts).find((script) => script.folderId === folder.folderId)?.scriptId ?? null;

      // 先把所有条目校验一遍，任何一条不合法就整批拒绝：不做半批导入。
      const parsed = [];
      for (const [index, entry] of entries.entries()) {
        if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
          return json(res, 400, { ok: false, error: "BAD_INPUT", index });
        }
        const label = entry.label === undefined ? "" : entry.label;
        if (typeof label !== "string" || label.length > 80) {
          return json(res, 400, { ok: false, error: "BAD_INPUT", index });
        }
        let expiresAt = null;
        let durationDays = null;
        if (entry.keyDays !== undefined && entry.keyDays !== null && entry.keyDays !== "") {
          const days = Number(entry.keyDays);
          if (!Number.isInteger(days) || days < 1 || days > 3650) {
            return json(res, 400, { ok: false, error: "BAD_KEY_DAYS", index });
          }
          durationDays = days;
        } else if (entry.expiresAt !== undefined && entry.expiresAt !== null && entry.expiresAt !== "") {
          const parsedMs = typeof entry.expiresAt === "number" ? entry.expiresAt : Date.parse(String(entry.expiresAt));
          if (!Number.isFinite(parsedMs) || parsedMs <= now()) {
            return json(res, 400, { ok: false, error: "BAD_EXPIRY", index });
          }
          expiresAt = parsedMs;
        }
        parsed.push({ label: label.trim(), expiresAt, durationDays });
      }

      const issued = parsed.map(({ label, expiresAt, durationDays }) => {
        const licenseId = "lic_" + randomBytes(12).toString("hex");
        const licenseKey = LICENSE_KEY_PREFIX + randomBytes(32).toString("hex");
        const keyHash = licenseKeyDigest(licenseKey);
        const license = {
          licenseId,
          keyHash,
          ownerEmail: owner.email,
          folderId: folder.folderId,
          scriptId: anchorScript,
          label,
          createdAt: now(),
          expiresAt,
          durationDays,
          revokedAt: null,
          hwidHash: null,
          firstActivatedAt: null,
          lastUsedAt: null,
          importedFrom: "json",
        };
        state.licenseKeys[licenseId] = license;
        state.licenseKeyIndex[keyHash] = licenseId;
        return { licenseKey, license: licensePublic(license) };
      });
      saveState();
      return json(res, 201, { ok: true, count: issued.length, licenses: issued });
    }

    if (req.method === "POST" && req.url === "/api/auth/licenses") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const { scriptId, folderId, label, expiresAt, keyDays, count } = body;

      // Keys live on a project folder so one key covers every script inside it.
      let folder = null;
      if (typeof folderId === "string" && folderId) {
        folder = state.folders[folderId] ?? null;
      } else if (typeof scriptId === "string") {
        const script = state.scripts[scriptId];
        if (script?.ownerEmail === owner.email) folder = state.folders[script.folderId] ?? null;
      }
      if (!folder || folder.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "SCRIPT_NOT_FOUND" });
      }

      if (label !== undefined && (typeof label !== "string" || label.length > 80)) {
        return json(res, 400, { ok: false, error: "BAD_INPUT" });
      }
      let batch = 1;
      if (count !== undefined && count !== null) {
        batch = Number(count);
        if (!Number.isInteger(batch) || batch < 1 || batch > MAX_BATCH_KEYS) {
          return json(res, 400, { ok: false, error: "BAD_COUNT" });
        }
      }

      let expiry = null;
      let durationDays = null;
      if (keyDays !== undefined && keyDays !== null && keyDays !== "") {
        const days = Number(keyDays);
        if (!Number.isInteger(days) || days < 1 || days > 3650) {
          return json(res, 400, { ok: false, error: "BAD_KEY_DAYS" });
        }
        if (expiresAt !== undefined && expiresAt !== null && expiresAt !== "") {
          return json(res, 400, { ok: false, error: "BAD_EXPIRY" });
        }
        durationDays = days;
      } else if (expiresAt !== undefined && expiresAt !== null && expiresAt !== "") {
        if (typeof expiresAt !== "string") return json(res, 400, { ok: false, error: "BAD_INPUT" });
        expiry = Date.parse(expiresAt);
        if (!Number.isFinite(expiry) || expiry <= now()) return json(res, 400, { ok: false, error: "BAD_EXPIRY" });
      }

      const folderLicenses = Object.values(state.licenseKeys).filter((license) => license.folderId === folder.folderId);
      if (folderLicenses.length + batch > MAX_LICENSES_PER_SCRIPT) {
        return json(res, 409, { ok: false, error: "LICENSE_LIMIT" });
      }

      // The first script in the folder is recorded for display/CSV only; the key
      // itself is folder scoped and works for every script in it.
      const anchorScript = typeof scriptId === "string" && state.scripts[scriptId]?.folderId === folder.folderId
        ? scriptId
        : Object.values(state.scripts).find((script) => script.folderId === folder.folderId)?.scriptId ?? null;

      const issued = [];
      for (let i = 0; i < batch; i++) {
        const licenseId = "lic_" + randomBytes(12).toString("hex");
        const licenseKey = LICENSE_KEY_PREFIX + randomBytes(32).toString("hex");
        const keyHash = licenseKeyDigest(licenseKey);
        const license = {
          licenseId,
          keyHash,
          ownerEmail: owner.email,
          folderId: folder.folderId,
          scriptId: anchorScript,
          label: typeof label === "string" ? label.trim() : "",
          createdAt: now(),
          expiresAt: expiry,
          durationDays,
          revokedAt: null,
          hwidHash: null,
          firstActivatedAt: null,
          lastUsedAt: null,
        };
        state.licenseKeys[licenseId] = license;
        state.licenseKeyIndex[keyHash] = licenseId;
        issued.push({ licenseKey, license: licensePublic(license) });
      }
      saveState();
      if (batch === 1) {
        return json(res, 201, { ok: true, licenseKey: issued[0].licenseKey, license: issued[0].license });
      }
      return json(res, 201, { ok: true, count: batch, licenses: issued });
    }

    if (req.method === "POST" && req.url === "/api/auth/licenses/revoke") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const license = typeof body.licenseId === "string" ? state.licenseKeys[body.licenseId] : null;
      if (!license || license.ownerEmail !== owner.email) return json(res, 404, { ok: false, error: "NOT_FOUND" });
      license.revokedAt = license.revokedAt || now();
      saveState();
      return json(res, 200, { ok: true, license: licensePublic(license) });
    }

    if (req.method === "POST" && req.url === "/api/auth/licenses/ban") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const license = typeof body.licenseId === "string" ? state.licenseKeys[body.licenseId] : null;
      if (!license || license.ownerEmail !== owner.email) return json(res, 404, { ok: false, error: "NOT_FOUND" });
      const reason = typeof body.reason === "string" ? body.reason.trim().slice(0, 200) : "";
      let expireAt = null;
      if (body.banDays !== undefined && body.banDays !== null && body.banDays !== "") {
        const days = Number(body.banDays);
        if (!Number.isInteger(days) || days < 1 || days > 3650) {
          return json(res, 400, { ok: false, error: "BAD_BAN_DAYS" });
        }
        expireAt = now() + days * 86400000;
      } else if (body.banExpireAt !== undefined && body.banExpireAt !== null && body.banExpireAt !== "") {
        const parsed = Date.parse(String(body.banExpireAt));
        if (!Number.isFinite(parsed) || parsed <= now()) return json(res, 400, { ok: false, error: "BAD_EXPIRY" });
        expireAt = parsed;
      }
      license.bannedAt = now();
      license.banReason = reason;
      license.banExpireAt = expireAt;
      license.unbanToken = randomBytes(UNBAN_TOKEN_BYTES).toString("hex");
      // Luarmor-style: the device that used this key is blocked too, so a fresh
      // key on the same machine does not get around the ban.
      if (license.hwidHash) {
        state.deviceBans[license.hwidHash] = {
          reason,
          expireAt,
          bannedAt: now(),
          licenseId: license.licenseId,
          ownerEmail: owner.email,
        };
      }
      saveState();
      return json(res, 200, {
        ok: true,
        unbanToken: license.unbanToken,
        license: licensePublic(license),
      });
    }

    if (req.method === "POST" && req.url === "/api/auth/licenses/unban") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const license = typeof body.licenseId === "string" ? state.licenseKeys[body.licenseId] : null;
      if (!license || license.ownerEmail !== owner.email) return json(res, 404, { ok: false, error: "NOT_FOUND" });
      license.bannedAt = null;
      license.banReason = "";
      license.banExpireAt = null;
      license.unbanToken = null;
      if (license.hwidHash && state.deviceBans[license.hwidHash]?.licenseId === license.licenseId) {
        delete state.deviceBans[license.hwidHash];
      }
      saveState();
      return json(res, 200, { ok: true, license: licensePublic(license) });
    }

    if (req.method === "POST" && req.url === "/api/auth/licenses/reset-hwid") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const license = typeof body.licenseId === "string" ? state.licenseKeys[body.licenseId] : null;
      if (!license || license.ownerEmail !== owner.email) return json(res, 404, { ok: false, error: "NOT_FOUND" });
      license.hwidHash = null;
      license.firstActivatedAt = null;
      license.lastHwidResetAt = now();
      license.totalResets = (license.totalResets ?? 0) + 1;
      saveState();
      return json(res, 200, { ok: true, license: licensePublic(license) });
    }

    if (req.method === "GET" && new URL(req.url, "http://localhost").pathname === "/api/auth/releases") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const scriptId = new URL(req.url, "http://localhost").searchParams.get("scriptId");
      if (scriptId && state.scripts[scriptId]?.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "NOT_FOUND" });
      }
      const releases = Object.values(state.releases)
        .filter((release) => release.ownerEmail === owner.email && (!scriptId || release.scriptId === scriptId))
        .map(({ scriptId: sid, version, sha256, size, createdAt }) => ({ scriptId: sid, version, sha256, size, createdAt }));
      return json(res, 200, { ok: true, releases });
    }

    if (req.method === "POST" && req.url === "/api/auth/releases") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const { scriptId, version, code } = body;
      const script = typeof scriptId === "string" ? state.scripts[scriptId] : null;
      if (!script || script.ownerEmail !== owner.email) return json(res, 404, { ok: false, error: "SCRIPT_NOT_FOUND" });
      if (typeof version !== "string" || !/^[A-Za-z0-9._-]{1,32}$/.test(version)) {
        return json(res, 400, { ok: false, error: "BAD_VERSION" });
      }
      if (typeof code !== "string" || !code.trim()) return json(res, 400, { ok: false, error: "BAD_CODE" });
      const bytes = Buffer.byteLength(code, "utf8");
      if (bytes > MAX_RELEASE_BYTES) return json(res, 413, { ok: false, error: "RELEASE_TOO_LARGE" });
      const key = `${scriptId}:${version}`;
      if (Object.keys(state.releases).filter((k) => k.startsWith(`${scriptId}:`)).length >= MAX_RELEASES_PER_SCRIPT && !state.releases[key]) {
        return json(res, 409, { ok: false, error: "RELEASE_LIMIT" });
      }
      const file = saveReleaseFile(scriptId, version, code);
      state.releases[key] = {
        scriptId,
        ownerEmail: owner.email,
        version,
        sha256: createHash("sha256").update(code).digest("hex"),
        size: bytes,
        createdAt: now(),
        file,
      };
      if (script) script.latestVersion = version;
      saveState();
      const { sha256 } = state.releases[key];
      return json(res, 201, { ok: true, release: { scriptId, version, sha256, size: bytes } });
    }

    if (req.method === "POST" && req.url === "/api/auth/register") {
      const { email, nickname, password, captchaId, captchaAnswer } = body;
      if (
        typeof email !== "string" || email.trim().length > EMAIL_MAX || !EMAIL_RE.test(normalizeEmail(email)) ||
        typeof password !== "string" || password.length < 8 || password.length > 128 ||
        (nickname !== undefined && (typeof nickname !== "string" || nickname.length > 24))
      ) return json(res, 400, { ok: false, error: "BAD_INPUT" });
      if (Object.keys(state.users).length >= MAX_ACCOUNTS) {
        return json(res, 503, { ok: false, error: "SERVICE_FULL" });
      }
      if (Object.keys(pendingRegistrations).length >= MAX_PENDING_REG) {
        return json(res, 503, { ok: false, error: "SERVICE_FULL" });
      }
      if (!(await verifyCaptcha(req, captchaId, captchaAnswer))) return json(res, 400, { ok: false, error: "CAPTCHA_FAIL" });
      const key = normalizeEmail(email);
      if (state.users[key]) return json(res, 409, { ok: false, error: "EMAIL_TAKEN" });
      const ipHash = registrationIpHash(clientIp(req));
      if (state.registeredIps[ipHash]) return json(res, 409, { ok: false, error: "IP_ALREADY_REGISTERED" });
      const issued = issueEmailCode(key);
      if (!issued.ok) return json(res, 429, { ok: false, error: issued.error, retryAfter: issued.retryAfter });
      const salt = randomBytes(16).toString("hex");
      pendingRegistrations[key] = {
        nickname: (typeof nickname === "string" && nickname.trim()) || key.split("@")[0],
        salt,
        hash: hashPw(password, salt),
        expires: now() + 10 * 60_000,
      };
      saveState();
      const sent = await deliverVerificationCode(key, issued.code);
      if (!sent.ok) {
        delete state.codes[key];
        delete pendingRegistrations[key];
        saveState();
        return json(res, 502, { ok: false, error: "MAIL_SEND_FAILED" });
      }
      return json(res, 200, {
        ok: true,
        needCode: true,
        ...(sent.demoCode ? { demoCode: sent.demoCode } : {}),
      });
    }

    if (req.method === "POST" && req.url === "/api/auth/register/confirm") {
      const { email, code } = body;
      if (typeof email !== "string" || typeof code !== "string" || !EMAIL_RE.test(normalizeEmail(email))) {
        return json(res, 400, { ok: false, error: "BAD_INPUT" });
      }
      const key = normalizeEmail(email);
      if (state.users[key]) return json(res, 409, { ok: false, error: "EMAIL_TAKEN" });
      const ipHash = registrationIpHash(clientIp(req));
      if (state.registeredIps[ipHash]) return json(res, 409, { ok: false, error: "IP_ALREADY_REGISTERED" });
      const v = checkEmailCode(key, code);
      if (!v.ok) return json(res, 400, { ok: false, error: v.error });
      const draft = pendingRegistrations[key];
      if (!draft || now() > draft.expires) return json(res, 400, { ok: false, error: "REGISTER_EXPIRED" });
      delete pendingRegistrations[key];
      state.users[key] = {
        email: key,
        nickname: draft.nickname,
        salt: draft.salt,
        hash: draft.hash,
        created: now(),
      };
      state.registeredIps[ipHash] = { email: key, created: now() };
      saveState();
      const token = sessionFor(key);
      return json(
        res,
        200,
        { ok: true, user: userPublic(state.users[key]) },
        { "set-cookie": sessionCookieHeader(req, token) },
      );
    }

    if (req.method === "POST" && req.url === "/api/auth/resend") {
      const { email } = body;
      if (typeof email !== "string" || !EMAIL_RE.test(normalizeEmail(email))) {
        return json(res, 400, { ok: false, error: "BAD_INPUT" });
      }
      const key = normalizeEmail(email);
      if (state.users[key]) return json(res, 409, { ok: false, error: "EMAIL_TAKEN" });
      const ipHash = registrationIpHash(clientIp(req));
      if (state.registeredIps[ipHash]) return json(res, 409, { ok: false, error: "IP_ALREADY_REGISTERED" });
      const issued = issueEmailCode(key);
      if (!issued.ok) return json(res, 429, { ok: false, error: issued.error, retryAfter: issued.retryAfter });
      const sent = await deliverVerificationCode(key, issued.code);
      if (!sent.ok) {
        delete state.codes[key];
        saveState();
        return json(res, 502, { ok: false, error: "MAIL_SEND_FAILED" });
      }
      return json(res, 200, { ok: true, ...(sent.demoCode ? { demoCode: sent.demoCode } : {}) });
    }

    if (req.method === "POST" && req.url === "/api/auth/login") {
      const { email, password, captchaId, captchaAnswer } = body;
      if (typeof email !== "string" || typeof password !== "string") {
        return json(res, 400, { ok: false, error: "BAD_INPUT" });
      }
      const key = email.trim().toLowerCase();
      const lock = loginLocked(key);
      if (lock > 0) return json(res, 429, { ok: false, error: "LOCKED", retryAfter: lock });
      if (!(await verifyCaptcha(req, captchaId, captchaAnswer))) return json(res, 400, { ok: false, error: "CAPTCHA_FAIL" });
      const u = state.users[key];
      if (!u) {
        pwEqual(password, "00".repeat(16), "00".repeat(128)); // 防时序枚举
        return json(res, 401, { ok: false, error: "NO_USER" });
      }
      if (!pwEqual(password, u.salt, u.hash)) {
        noteLoginFail(key);
        const lockAfter = loginLocked(key);
        if (lockAfter > 0) return json(res, 429, { ok: false, error: "LOCKED", retryAfter: lockAfter });
        return json(res, 401, { ok: false, error: "BAD_PASSWORD" });
      }
      delete state.fails[key];
      saveState();
      const token = sessionFor(key);
      return json(
        res,
        200,
        { ok: true, user: userPublic(u), quota: quotaJson(key) },
        { "set-cookie": sessionCookieHeader(req, token) },
      );
    }

    if (req.method === "POST" && req.url === "/api/auth/logout") {
      let changed = false;
      const cookieToken = sessionTokenFromCookie(req);
      if (cookieToken && state.sessions[cookieToken]) {
        delete state.sessions[cookieToken];
        changed = true;
      }
      // Expire any legacy bearer session if an old client still sends it.
      const h = req.headers["authorization"];
      if (typeof h === "string" && h.startsWith("Bearer ")) {
        const legacyToken = h.slice(7).trim();
        if (state.sessions[legacyToken]) {
          delete state.sessions[legacyToken];
          changed = true;
        }
      }
      if (changed) saveState();
      return json(res, 200, { ok: true }, { "set-cookie": sessionCookieHeader(req, "", 0) });
    }

    if (req.method === "GET" && req.url === "/api/auth/apikeys") {
      const u = authUser(req);
      if (!u) return unauthorized(res, req);
      const keys = Object.entries(state.apiKeys)
        .filter(([, v]) => v.email === u.email)
        .map(([k, v]) => ({ id: k.slice(0, 12) + "…", created: v.created }));
      return json(res, 200, { ok: true, keys });
    }

    if (req.method === "POST" && req.url === "/api/auth/apikeys") {
      const u = authUser(req);
      if (!u) return unauthorized(res, req);
      const key = newApiKey();
      state.apiKeys[key] = { email: u.email, created: now() };
      saveState();
      return json(res, 200, { ok: true, key });
    }

    if (req.method === "DELETE" && req.url === "/api/auth/apikeys") {
      const u = authUser(req);
      if (!u) return unauthorized(res, req);
      const { key } = body;
      if (typeof key !== "string" || !state.apiKeys[key]) {
        return json(res, 404, { ok: false, error: "NOT_FOUND" });
      }
      if (state.apiKeys[key].email !== u.email) {
        return json(res, 403, { ok: false, error: "FORBIDDEN" });
      }
      delete state.apiKeys[key];
      saveState();
      return json(res, 200, { ok: true });
    }

    /* ---------------- Phase 2: 批量发布 ---------------- */

    if (req.method === "POST" && req.url === "/api/auth/publish/batch") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const { folderId, version, source, scriptIds } = body;
      const folder = typeof folderId === "string" ? state.folders[folderId] : null;
      if (!folder || folder.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "FOLDER_NOT_FOUND" });
      }
      if (typeof version !== "string" || !/^[A-Za-z0-9._-]{1,32}$/.test(version)) {
        return json(res, 400, { ok: false, error: "BAD_VERSION" });
      }
      if (typeof source !== "string" || !source.trim()) return json(res, 400, { ok: false, error: "BAD_SOURCE" });
      if (Buffer.byteLength(source, "utf8") > MAX_SOURCE_BYTES) {
        return json(res, 413, { ok: false, error: "SOURCE_TOO_LARGE" });
      }
      const inFolder = Object.values(state.scripts).filter(
        (script) => script.ownerEmail === owner.email && script.folderId === folder.folderId && script.active,
      );
      const targets = Array.isArray(scriptIds) && scriptIds.length
        ? inFolder.filter((script) => scriptIds.includes(script.scriptId))
        : inFolder;
      if (targets.length === 0) return json(res, 404, { ok: false, error: "SCRIPT_NOT_FOUND" });
      if (targets.length > 5) return json(res, 400, { ok: false, error: "BATCH_TOO_LARGE" });
      if (engineMode() === "missing") return json(res, 503, { ok: false, error: "ENGINE_MISSING" });

      const q = quotaOf(owner.email);
      if (q.used + targets.length > DAILY_QUOTA) {
        return json(res, 429, { ok: false, error: "QUOTA_EXCEEDED", quota: quotaJson(owner.email) });
      }
      const active = activeJobsFor(owner.email).length;
      if (active + targets.length > MAX_ACTIVE_JOBS_PER_ACCOUNT) {
        return json(res, 409, { ok: false, error: "PUBLISH_BUSY" });
      }

      const options = { ...DEFAULT_OBFUSCATION_OPTIONS };
      const origin = publicOrigin(req);
      const jobs = [];
      for (const script of targets) {
        q.used += 1;
        const job = {
          id: `pub_${randomBytes(9).toString("hex")}`,
          ownerEmail: owner.email,
          scriptId: script.scriptId,
          version,
          source,
          sourceBytes: Buffer.byteLength(source, "utf8"),
          status: "processing",
          stage: "queued",
          progress: PUBLISH_STAGES.queued,
          startedAt: now(),
          quotaCharged: true,
          quotaDate: q.date,
        };
        publishJobs.set(job.id, job);
        jobs.push({ jobId: job.id, scriptId: script.scriptId, name: script.name });
        void runPublishJob(job, { source, version, options, origin });
      }
      state.quotas[owner.email] = q;
      saveState();
      recordAudit(owner.email, "publish.batch", `${targets.length} scripts`, clientIp(req), { version });
      return json(res, 202, {
        ok: true,
        count: jobs.length,
        jobs,
        quota: quotaJson(owner.email),
      });
    }

    /* ---------------- Phase 2: 审计日志 / 批量操作 ---------------- */

    if (req.method === "GET" && new URL(req.url, "http://localhost").pathname === "/api/auth/audit") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const limit = Math.min(200, Math.max(1, Number(new URL(req.url, "http://localhost").searchParams.get("limit")) || 50));
      const entries = state.auditLog.filter((entry) => entry.email === owner.email).slice(-limit).reverse();
      return json(res, 200, { ok: true, entries: entries.map(auditPublic) });
    }

    if (req.method === "POST" && req.url === "/api/auth/licenses/batch") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const { action, licenseIds } = body;
      const targets = ownedLicenses(owner.email, licenseIds);
      if (!targets) return json(res, 400, { ok: false, error: "BAD_INPUT" });
      if (!["revoke", "unban", "reset-hwid", "ban"].includes(action)) {
        return json(res, 400, { ok: false, error: "BAD_INPUT" });
      }

      if (action === "ban") {
        const reason = typeof body.reason === "string" ? body.reason.trim().slice(0, 200) : "";
        let banExpireAt = null;
        if (body.banDays !== undefined && body.banDays !== null && body.banDays !== "") {
          const days = Number(body.banDays);
          if (!Number.isInteger(days) || days < 1 || days > 3650) {
            return json(res, 400, { ok: false, error: "BAD_BAN_DAYS" });
          }
          banExpireAt = now() + days * 86400000;
        }
        for (const license of targets) {
          // 与单个封禁保持同一套字段，licensePublic / evaluateLicense 才能识别。
          license.bannedAt = now();
          license.banReason = reason;
          license.banExpireAt = banExpireAt;
          license.unbanToken = randomBytes(UNBAN_TOKEN_BYTES).toString("hex");
          if (license.hwidHash) {
            state.deviceBans[license.hwidHash] = {
              reason,
              expireAt: banExpireAt,
              bannedAt: now(),
              licenseId: license.licenseId,
              ownerEmail: owner.email,
            };
          }
        }
      } else if (action === "unban") {
        for (const license of targets) {
          if (license.hwidHash) delete state.deviceBans[license.hwidHash];
          license.bannedAt = null;
          license.banReason = "";
          license.banExpireAt = null;
          license.unbanToken = null;
        }
      } else if (action === "reset-hwid") {
        for (const license of targets) {
          license.hwidHash = null;
          license.firstActivatedAt = null;
          license.lastResetAt = now();
          license.totalResets = (license.totalResets ?? 0) + 1;
        }
      } else {
        for (const license of targets) {
          license.revokedAt = license.revokedAt || now();
        }
      }
      saveState();
      recordAudit(owner.email, `licenses.${action}`, `${targets.length} keys`, clientIp(req), { count: targets.length });
      return json(res, 200, {
        ok: true,
        action,
        count: targets.length,
        licenses: targets.map(licensePublic),
      });
    }

    /* ---------------- Phase 4: 指标 / webhook 投递日志 ---------------- */

    if (req.method === "GET" && new URL(req.url, "http://localhost").pathname === "/api/auth/metrics") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const days = Math.min(30, Math.max(1, Number(new URL(req.url, "http://localhost").searchParams.get("days")) || 7));
      return json(res, 200, { ok: true, days: metricsSnapshot(days) });
    }

    if (req.method === "GET" && req.url === "/api/auth/webhooks/deliveries") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const owned = new Set(
        Object.values(state.folders).filter((f) => f.ownerEmail === owner.email).map((f) => f.folderId),
      );
      const entries = state.webhookLog
        .filter((entry) => owned.has(entry.folderId))
        .slice(-80)
        .reverse();
      return json(res, 200, { ok: true, entries });
    }

    /* ---------------- Phase 5: 受保护的 webhook ---------------- */

    if (req.method === "GET" && req.url === "/api/auth/webhooks") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const list = Object.values(state.webhooks)
        .filter((hook) => hook.ownerEmail === owner.email)
        .map((hook) => ({
          webhookId: hook.webhookId,
          folderId: hook.folderId,
          name: hook.name,
          urlHost: (() => { try { return new URL(hook.url).host; } catch { return "invalid"; } })(),
          variables: Object.keys(hook.sanitizers ?? {}),
          createdAt: hook.createdAt,
        }));
      return json(res, 200, { ok: true, webhooks: list });
    }

    if (req.method === "POST" && req.url === "/api/auth/webhooks") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const { folderId, name, url, template, sanitizers } = body;
      const folder = typeof folderId === "string" ? state.folders[folderId] : null;
      if (!folder || folder.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "FOLDER_NOT_FOUND" });
      }
      if (typeof name !== "string" || name.trim().length < 2 || name.trim().length > 60) {
        return json(res, 400, { ok: false, error: "BAD_INPUT" });
      }
      const normalizedUrl = normalizeWebhookUrl(url);
      if (!normalizedUrl) return json(res, 400, { ok: false, error: "BAD_WEBHOOK" });
      if (typeof template !== "string" || template.length === 0 || template.length > WEBHOOK_TEMPLATE_MAX) {
        return json(res, 400, { ok: false, error: "BAD_INPUT" });
      }
      const clean = normalizeSanitizers(sanitizers);
      if (!clean) return json(res, 400, { ok: false, error: "BAD_INPUT" });
      try {
        JSON.parse(template);
      } catch {
        return json(res, 400, { ok: false, error: "BAD_INPUT" });
      }
      if (Object.values(state.webhooks).filter((h) => h.ownerEmail === owner.email).length >= 20) {
        return json(res, 409, { ok: false, error: "WEBHOOK_LIMIT" });
      }
      const webhookId = "whk_" + randomBytes(9).toString("hex");
      // URL 只存在服务端，客户端永远拿不到，也就无法删除/刷屏你的 webhook。
      state.webhooks[webhookId] = {
        webhookId,
        ownerEmail: owner.email,
        folderId: folder.folderId,
        name: name.trim(),
        url: normalizedUrl,
        template,
        sanitizers: clean,
        createdAt: now(),
      };
      saveState();
      recordAudit(owner.email, "webhook.create", webhookId, clientIp(req), { folderId: folder.folderId });
      return json(res, 201, {
        ok: true,
        webhook: { webhookId, name: state.webhooks[webhookId].name, folderId: folder.folderId, variables: Object.keys(clean) },
      });
    }

    if (req.method === "POST" && req.url === "/api/auth/webhooks/delete") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const hook = typeof body.webhookId === "string" ? state.webhooks[body.webhookId] : null;
      if (!hook || hook.ownerEmail !== owner.email) return json(res, 404, { ok: false, error: "NOT_FOUND" });
      delete state.webhooks[hook.webhookId];
      saveState();
      recordAudit(owner.email, "webhook.delete", hook.webhookId, clientIp(req));
      return json(res, 200, { ok: true });
    }

    /* ---------------- Phase 6: 外部验卡应用 ---------------- */

    if (req.method === "GET" && req.url === "/api/auth/external-apps") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const apps = Object.values(state.externalApps)
        .filter((app) => app.ownerEmail === owner.email)
        .map((app) => ({ appId: app.appId, name: app.name, createdAt: app.createdAt }));
      return json(res, 200, { ok: true, apps });
    }

    if (req.method === "POST" && req.url === "/api/auth/external-apps") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const name = typeof body.name === "string" ? body.name.trim() : "";
      if (name.length < 2 || name.length > 60) return json(res, 400, { ok: false, error: "BAD_INPUT" });
      if (Object.values(state.externalApps).filter((app) => app.ownerEmail === owner.email).length >= 10) {
        return json(res, 409, { ok: false, error: "APP_LIMIT" });
      }
      const appId = "app_" + randomBytes(8).toString("hex");
      const secret = randomBytes(24).toString("hex");
      state.externalApps[appId] = { appId, ownerEmail: owner.email, name, secret, createdAt: now() };
      saveState();
      recordAudit(owner.email, "external-app.create", appId, clientIp(req));
      // 密钥只在这里出现一次。
      return json(res, 201, { ok: true, app: { appId, name, secret } });
    }

    if (req.method === "POST" && req.url === "/api/auth/external-apps/delete") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const app = typeof body.appId === "string" ? state.externalApps[body.appId] : null;
      if (!app || app.ownerEmail !== owner.email) return json(res, 404, { ok: false, error: "NOT_FOUND" });
      delete state.externalApps[app.appId];
      saveState();
      recordAudit(owner.email, "external-app.delete", app.appId, clientIp(req));
      return json(res, 200, { ok: true });
    }

    return json(res, 404, { ok: false, error: "NOT_FOUND" });
  };

  if (req.method === "GET" && req.url.startsWith("/api/publish/")) {
    const owner = authUser(req);
    if (!owner) return unauthorized(res, req);
    sweepPublishJobs();
    const jobId = new URL(req.url, "http://localhost").pathname.slice("/api/publish/".length);
    const job = publishJobs.get(jobId);
    // 别人的作业与不存在的作业返回同一个结果，避免撞库探测。
    if (!job || job.ownerEmail !== owner.email) {
      return json(res, 404, { ok: false, error: "JOB_NOT_FOUND" });
    }
    return json(res, 200, { ok: true, ...publishJobJson(job), quota: quotaJson(owner.email) });
  }

  if (req.url.startsWith("/api/auth/")) return await handleAuth();

  if (req.method === "POST" && req.url === "/api/licenses/validate") {
    const retryAfter = licenseVerifyRetryAfter(clientIp(req));
    if (retryAfter > 0) {
      res.setHeader("retry-after", String(retryAfter));
      return json(res, 429, { ok: false, valid: false, error: "RATE_LIMITED", retryAfter });
    }
    let body;
    try {
      body = JSON.parse(await readBody(req, AUTH_BODY_LIMIT));
    } catch (e) {
      if (e instanceof Error && e.message === "BODY_TOO_LARGE") {
        json(res, 413, { ok: false, valid: false, error: "BODY_TOO_LARGE" });
        req.destroy();
        return;
      }
      return json(res, 400, { ok: false, valid: false, error: "BAD_JSON" });
    }
    if (!body || typeof body !== "object" || Array.isArray(body)) {
      return json(res, 400, { ok: false, valid: false, error: "BAD_REQUEST" });
    }
    const { scriptId, nonce } = body;
    // 逐项校验并回报具体字段，避免把「卡密格式错」笼统说成 BAD_REQUEST。
    const bad = (code, message, field) =>
      json(res, 400, { ok: false, valid: false, error: code, code, message, field });
    if (typeof scriptId !== "string" || !SCRIPT_ID_RE.test(scriptId)) {
      return bad("SCRIPT_ID_INVALID", "script id is not a valid scr_ id", "scriptId");
    }
    // A blank key is legitimate for Free for All scripts; the server decides.
    const keyInfo = normalizeLicenseKey(body.key);
    if (!keyInfo.ok) {
      return bad("KEY_INVALID", "key must look like osk_ followed by 64 hex characters", "key");
    }
    const hwidInfo = normalizeHwid(body.hwid);
    if (!hwidInfo.ok) return bad("HWID_INVALID", "hwid is longer than 256 characters", "hwid");
    if (typeof nonce !== "string" || !NONCE_RE.test(nonce)) {
      return bad("NONCE_INVALID", "nonce must be 16-64 letters or digits", "nonce");
    }
    const key = keyInfo.key;
    const hwid = hwidInfo.hwid;

    const verdict = evaluateLicense({ scriptId, key, hwid, nonce, ip: clientIp(req) });
    // 执行器/Roblox 证明：只记录，绝不参与放行判定（合不合规都能跑）。
    const attested = body.attested === true;
    const attestScore = Number.isFinite(Number(body.attestScore)) ? Math.max(0, Math.min(999, Math.trunc(Number(body.attestScore)))) : 0;
    const attestTotal = Number.isFinite(Number(body.attestTotal)) ? Math.max(0, Math.min(999, Math.trunc(Number(body.attestTotal)))) : 0;
    if (verdict.ok) bumpMetric(attested ? "attested" : "unanattested");
    if (verdict.ok && verdict.license) {
      verdict.license.lastAttested = attested;
      verdict.license.lastAttestScore = attestScore;
      verdict.license.lastAttestTotal = attestTotal;
      verdict.license.lastAttestedAt = now();
    }
    if (verdict.ok) bumpMetric("validations", typeof hwid === "string" ? licenseHwidDigest(hwid) : undefined);
    else bumpMetric("rejected");
    if (verdict.ok && !verdict.free) bumpMetric("executions", licenseHwidDigest(hwid));
    if (verdict.ok) bumpMetric("devices", typeof hwid === "string" && hwid.length >= 8 ? licenseHwidDigest(hwid) : undefined);
    saveState();
    if (!verdict.ok) {
      if (verdict.folder) {
        dispatchWebhook(verdict.folder, "alert", [
          ["Script", verdict.script?.name ?? scriptId],
          ["Reason", verdict.error],
          ["Client IP", clientIp(req)],
        ]);
      }
      if (verdict.retryAfter) res.setHeader("retry-after", String(verdict.retryAfter));
      return json(res, verdict.status, {
        ok: false,
        valid: false,
        error: verdict.error,
        code: verdict.code ?? verdict.error,
        ...(verdict.retryAfter ? { retryAfter: verdict.retryAfter } : {}),
        ...(verdict.banReason ? { banReason: verdict.banReason } : {}),
        ...(verdict.banExpireAt ? { banExpireAt: verdict.banExpireAt } : {}),
      });
    }

    // Free for All: valid without a key, nothing is bound or metered.
    if (verdict.free) {
      return json(res, 200, {
        ok: true,
        valid: true,
        free: true,
        scriptId,
        nonce,
        runtime: {
          premium: false,
          note: "",
          scriptName: verdict.script?.name ?? "",
          folderName: verdict.folder?.name ?? "",
          scriptVersion: latestReleaseVersion(scriptId),
          secondsLeft: null,
          expiresAt: null,
          totalExecutions: 0,
          firstActivatedAt: null,
        },
      });
    }

    const token = issueLicenseSession(verdict.license);
    saveState();
    if (verdict.folder) {
      dispatchWebhook(verdict.folder, "log", [
        ["Script", verdict.script?.name ?? scriptId],
        ["Licence", verdict.license.label || verdict.license.licenseId],
        ["Expires", verdict.license.expiresAt ? new Date(verdict.license.expiresAt).toISOString() : "never"],
      ]);
    }
    const secondsLeft = verdict.license.expiresAt
      ? Math.max(0, Math.floor((verdict.license.expiresAt - now()) / 1000))
      : null;
    return json(res, 200, {
      ok: true,
      valid: true,
      scriptId,
      licenseId: verdict.license.licenseId,
      expiresAt: verdict.license.expiresAt,
      deviceBound: true,
      nonce,
      token,
      tokenExpiresIn: Math.floor(LICENSE_SESSION_TTL_MS / 1000),
      // Runtime variables for the injected guard. Purely informational: the
      // client can tamper with them, so nothing security critical relies on them.
      runtime: {
        premium: true,
        note: verdict.license.label || "",
        scriptName: verdict.script?.name ?? "",
        folderName: verdict.folder?.name ?? "",
        secondsLeft,
        expiresAt: verdict.license.expiresAt ?? null,
        scriptVersion: latestReleaseVersion(scriptId),
        attested,
        attestScore,
        attestTotal,
        totalExecutions: verdict.license.executions ?? 0,
        firstActivatedAt: verdict.license.firstActivatedAt ?? null,
      },
    });
  }

  if (req.method === "POST" && req.url === "/api/releases/fetch") {
    const retryAfter = licenseVerifyRetryAfter(clientIp(req));
    if (retryAfter > 0) {
      res.setHeader("retry-after", String(retryAfter));
      return json(res, 429, { ok: false, error: "RATE_LIMITED", retryAfter });
    }
    let body;
    try {
      body = JSON.parse(await readBody(req, AUTH_BODY_LIMIT));
    } catch {
      return json(res, 400, { ok: false, error: "BAD_JSON" });
    }
    if (!body || typeof body !== "object" || Array.isArray(body)) {
      return json(res, 400, { ok: false, error: "BAD_REQUEST" });
    }
    const { scriptId, token } = body;
    // 与 validate 保持一致：允许前后空格、大小写十六进制。
    const keyInfo = normalizeLicenseKey(body.key);
    const hwidInfo = normalizeHwid(body.hwid);
    const key = keyInfo.ok ? keyInfo.key : "\u0000invalid";
    const hwid = hwidInfo.ok ? hwidInfo.hwid : "";
    const nonce = body.nonce;
    if (typeof scriptId !== "string" || !/^scr_[\da-f]{24}$/i.test(scriptId)) {
      return json(res, 400, { ok: false, error: "BAD_REQUEST" });
    }

    const target = state.scripts[scriptId];

    let allowed = false;
    if (typeof token === "string" && /^lst_[\da-f]{64}$/.test(token)) {
      const session = state.licenseSessions[token];
      const licence = session ? state.licenseKeys[session.licenseId] : null;
      allowed = Boolean(
        session && session.scriptId === scriptId && now() <= session.expires &&
        licence && !licence.revokedAt && !activeLicenseBan(licence),
      );
    } else if (
      typeof key === "string" && typeof hwid === "string" && typeof nonce === "string" &&
      keyInfo.ok && (key === "" || LICENSE_KEY_RE.test(key)) &&
      hwid.trim().length >= 8 && /^[\da-z]{16,64}$/i.test(nonce)
    ) {
      const verdict = evaluateLicense({ scriptId, key, hwid, nonce, ip: clientIp(req) });
      saveState();
      allowed = verdict.ok;
      if (!verdict.ok) {
        if (verdict.retryAfter) res.setHeader("retry-after", String(verdict.retryAfter));
        return json(res, verdict.status, {
          ok: false,
          error: verdict.error,
          code: verdict.code ?? verdict.error,
          ...(verdict.retryAfter ? { retryAfter: verdict.retryAfter } : {}),
          ...(verdict.banReason ? { banReason: verdict.banReason } : {}),
        });
      }
    } else if (target?.ffa) {
      // Free for All: an anonymous caller may download the latest release.
      allowed = true;
    }
    if (!allowed) return json(res, 403, { ok: false, error: "LICENSE_INVALID", code: "KEY_MISSING" });

    const release = latestReleaseFor(scriptId);
    // 变体：hard（完整加固）/ safe（兼容）。缺省走 safe，未知变体一律回落。
    const wantedVariant = body.variant === "hard" ? "hard" : "safe";
    if (!release) return json(res, 404, { ok: false, error: "RELEASE_NOT_FOUND" });
    const variantInfo = release.variants && release.variants[wantedVariant]
      ? release.variants[wantedVariant]
      : null;
    let code;
    try {
      code = readFileSync(variantInfo ? variantInfo.file : release.file, "utf8");
    } catch {
      return json(res, 500, { ok: false, error: "RELEASE_UNREADABLE" });
    }
    return json(res, 200, {
      ok: true,
      scriptId,
      version: release.version,
      variant: variantInfo ? variantInfo.variant : "safe",
      sha256: variantInfo ? variantInfo.sha256 : release.sha256,
      size: variantInfo ? variantInfo.size : release.size,
      code,
    });
  }

  /* ---------------- public key check / self-service ----------------
   * These accept the key itself as the credential (it is a bearer secret), so
   * they are rate limited per client and never reveal another key's data. */
  const readKeyBody = async () => {
    let parsed;
    try {
      parsed = JSON.parse(await readBody(req, AUTH_BODY_LIMIT));
    } catch (e) {
      if (e instanceof Error && e.message === "BODY_TOO_LARGE") {
        json(res, 413, { ok: false, error: "BODY_TOO_LARGE" });
        req.destroy();
        return { error: true };
      }
      json(res, 400, { ok: false, error: "BAD_JSON" });
      return { error: true };
    }
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      json(res, 400, { ok: false, error: "BAD_REQUEST" });
      return { error: true };
    }
    const { key } = parsed;
    if (typeof key !== "string" || !/^osk_[\da-f]{64}$/i.test(key)) {
      json(res, 400, { ok: false, error: "BAD_REQUEST" });
      return { error: true };
    }
    const licenseId = state.licenseKeyIndex[licenseKeyDigest(key)];
    const license = licenseId ? state.licenseKeys[licenseId] : null;
    const keyMatches = Boolean(license && digestsEqual(license.keyHash, licenseKeyDigest(key)));
    if (!license || !keyMatches) {
      json(res, 404, { ok: false, error: "LICENSE_INVALID" });
      return { error: true };
    }
    return { license, folder: state.folders[license.folderId] ?? null };
  };

  if (req.method === "POST" && req.url === "/api/key/status") {
    const retryAfter = licenseVerifyRetryAfter(clientIp(req));
    if (retryAfter > 0) {
      res.setHeader("retry-after", String(retryAfter));
      return json(res, 429, { ok: false, error: "RATE_LIMITED", retryAfter });
    }
    const parsed = await readKeyBody();
    if (parsed.error) return;
    const { license, folder } = parsed;
    const script = license.scriptId ? state.scripts[license.scriptId] : null;
    const expired = Boolean(license.expiresAt && now() >= license.expiresAt);
    const stateLabel = license.revokedAt ? "revoked" : expired ? "expired" : "active";
    const cooldownMs = (folder?.resetCooldownDays ?? 7) * 86400000;
    const lastReset = license.lastHwidResetAt ?? 0;
    const resetAvailableIn = Math.max(0, Math.ceil((lastReset + cooldownMs - now()) / 1000));
    return json(res, 200, {
      ok: true,
      state: stateLabel,
      note: license.label || "",
      scriptName: script?.name ?? "",
      folderName: folder?.name ?? "",
      expiresAt: license.expiresAt,
      durationDays: license.durationDays ?? null,
      secondsLeft: license.expiresAt ? Math.max(0, Math.floor((license.expiresAt - now()) / 1000)) : null,
      deviceBound: Boolean(license.hwidHash),
      firstActivatedAt: license.firstActivatedAt ?? null,
      lastUsedAt: license.lastUsedAt ?? null,
      totalExecutions: license.executions ?? 0,
      selfReset: Boolean(folder?.selfReset),
      resetCooldownDays: folder?.resetCooldownDays ?? 7,
      resetAvailableIn,
    });
  }

  if (req.method === "POST" && req.url === "/api/key/unban") {
    const retryAfter = licenseVerifyRetryAfter(clientIp(req));
    if (retryAfter > 0) {
      res.setHeader("retry-after", String(retryAfter));
      return json(res, 429, { ok: false, error: "RATE_LIMITED", retryAfter });
    }
    let parsed;
    try {
      parsed = JSON.parse(await readBody(req, AUTH_BODY_LIMIT));
    } catch {
      return json(res, 400, { ok: false, error: "BAD_JSON" });
    }
    const { key, token } = parsed ?? {};
    if (
      typeof key !== "string" || !/^osk_[\da-f]{64}$/i.test(key) ||
      typeof token !== "string" || !/^[\da-f]{32,64}$/i.test(token)
    ) return json(res, 400, { ok: false, error: "BAD_REQUEST" });

    const licenseId = state.licenseKeyIndex[licenseKeyDigest(key)];
    const license = licenseId ? state.licenseKeys[licenseId] : null;
    const keyMatches = Boolean(license && digestsEqual(license.keyHash, licenseKeyDigest(key)));
    // Same answer for "no such key" and "wrong token" so this is not an oracle.
    if (!license || !keyMatches || !license.unbanToken || !digestsEqual(
      createHmac("sha256", state.licenseHwidHashKey).update(String(license.unbanToken)).digest("hex"),
      createHmac("sha256", state.licenseHwidHashKey).update(String(token)).digest("hex"),
    )) return json(res, 401, { ok: false, error: "INVALID_UNBAN_TOKEN" });

    license.bannedAt = null;
    license.banReason = "";
    license.banExpireAt = null;
    license.unbanToken = null;
    if (license.hwidHash && state.deviceBans[license.hwidHash]?.licenseId === license.licenseId) {
      delete state.deviceBans[license.hwidHash];
    }
    saveState();
    return json(res, 200, { ok: true, note: license.label || "" });
  }

  if (req.method === "POST" && req.url === "/api/key/reset-hwid") {
    const retryAfter = licenseVerifyRetryAfter(clientIp(req));
    if (retryAfter > 0) {
      res.setHeader("retry-after", String(retryAfter));
      return json(res, 429, { ok: false, error: "RATE_LIMITED", retryAfter });
    }
    const parsed = await readKeyBody();
    if (parsed.error) return;
    const { license, folder } = parsed;
    if (!folder?.selfReset) return json(res, 403, { ok: false, error: "SELF_RESET_DISABLED" });
    if (license.revokedAt) return json(res, 403, { ok: false, error: "LICENSE_INVALID" });
    const cooldownMs = (folder.resetCooldownDays ?? 7) * 86400000;
    const waitMs = (license.lastHwidResetAt ?? 0) + cooldownMs - now();
    if (waitMs > 0) {
      return json(res, 429, {
        ok: false,
        error: "RESET_COOLDOWN",
        retryAfter: Math.ceil(waitMs / 1000),
      });
    }
    license.hwidHash = null;
    license.firstActivatedAt = null;
    license.lastHwidResetAt = now();
    saveState();
    if (folder) {
      dispatchWebhook(folder, "alert", [
        ["Script", license.scriptId ?? "-"],
        ["Event", "Self-service HWID reset"],
        ["Licence", license.label || license.licenseId],
      ]);
    }
    return json(res, 200, {
      ok: true,
      note: license.label || "",
      resetCooldownDays: folder.resetCooldownDays ?? 7,
    });
  }

  /* ---------------- Phase 3: Key Check（Luarmor 兼容状态码） ---------------- */

  if (req.method === "POST" && req.url === "/api/key/check") {
    const retryAfter = keyCheckLimited(clientIp(req));
    if (retryAfter > 0) {
      res.setHeader("retry-after", String(retryAfter));
      return json(res, 429, { ok: false, code: "KEY_LOCKED", message: KEY_CHECK_MESSAGES.KEY_LOCKED, retryAfter });
    }
    let body;
    try {
      body = JSON.parse(await readBody(req, AUTH_BODY_LIMIT));
    } catch {
      return json(res, 400, { ok: false, code: "UNKNOWN_ERROR", message: KEY_CHECK_MESSAGES.UNKNOWN_ERROR });
    }
    const scriptId = typeof body?.scriptId === "string" ? body.scriptId : "";
    if (!/^scr_[\da-f]{24}$/i.test(scriptId)) {
      return json(res, 400, { ok: false, code: "SCRIPT_ID_INVALID", message: KEY_CHECK_MESSAGES.SCRIPT_ID_INVALID });
    }
    const keyInfo = normalizeLicenseKey(body?.key);
    if (!keyInfo.ok) {
      return json(res, 400, { ok: false, code: "KEY_INVALID", message: KEY_CHECK_MESSAGES.KEY_INVALID });
    }
    const key = keyInfo.key;
    const hwid = typeof body?.hwid === "string"
      ? body.hwid
      : typeof req.headers["clienthwid"] === "string" ? req.headers["clienthwid"] : "";
    if (hwid.trim().length < 8 || hwid.trim().length > 256) {
      return json(res, 400, { ok: false, code: "INVALID_EXECUTOR", message: KEY_CHECK_MESSAGES.INVALID_EXECUTOR });
    }
    const script = state.scripts[scriptId];
    if (!script || !script.active) {
      return json(res, 404, { ok: false, code: "SCRIPT_ID_INCORRECT", message: KEY_CHECK_MESSAGES.SCRIPT_ID_INCORRECT });
    }
    const verdict = evaluateLicense({
      scriptId, key, hwid, ip: clientIp(req),
      nonce: randomBytes(16).toString("hex"), dryRun: true,
    });
    saveState();
    if (!verdict.ok) {
      const code = toKeyCheckCode(verdict.code ?? "UNKNOWN_ERROR");
      return json(res, verdict.status === 200 ? 400 : verdict.status, {
        ok: false,
        code,
        message: KEY_CHECK_MESSAGES[code] ?? KEY_CHECK_MESSAGES.UNKNOWN_ERROR,
        ...(verdict.retryAfter ? { retryAfter: verdict.retryAfter } : {}),
      });
    }
    return json(res, 200, {
      ok: true,
      code: "KEY_VALID",
      message: KEY_CHECK_MESSAGES.KEY_VALID,
      data: {
        auth_expire: verdict.license.expiresAt ?? -1,
        note: verdict.license.label || "Not specified",
        total_executions: verdict.license.executions ?? 0,
      },
    });
  }

  /* 供脚本内 loadstring 的轻量库：check_key / load_script / purge_cache */
  if ((req.method === "GET" || req.method === "HEAD") && req.url === "/library.lua") {
    const library = readFileSync(path.join(__dirname, "key-check-library.luau"), "utf8")
      .replaceAll("__BASE_URL__", publicOrigin(req));
    res.writeHead(200, {
      "content-type": "text/plain; charset=utf-8",
      "cache-control": "no-store",
      "x-content-type-options": "nosniff",
    });
    return res.end(req.method === "HEAD" ? undefined : library);
  }

  /* ---------------- Phase 5: 受保护的 webhook 发送 ---------------- */

  if (req.method === "POST" && req.url === "/api/webhooks/send") {
    const retryAfter = webhookSendLimited(clientIp(req));
    if (retryAfter > 0) {
      res.setHeader("retry-after", String(retryAfter));
      return json(res, 429, { ok: false, error: "RATE_LIMITED", retryAfter });
    }
    let body;
    try {
      body = JSON.parse(await readBody(req, 32 * 1024));
    } catch {
      return json(res, 400, { ok: false, error: "BAD_JSON" });
    }
    const token = typeof body?.token === "string" ? body.token : "";
    const session = state.licenseSessions[token];
    if (!session || session.expires < now()) {
      return json(res, 401, { ok: false, error: "INVALID_SESSION" });
    }
    const hook = typeof body?.webhookId === "string" ? state.webhooks[body.webhookId] : null;
    const script = state.scripts[session.scriptId];
    // 受保护的 webhook 只对同项目文件夹的脚本开放。
    if (!hook || !script || hook.folderId !== script.folderId) {
      return json(res, 404, { ok: false, error: "NOT_FOUND" });
    }
    const license = state.licenseKeys[session.licenseId];
    const values = body?.values && typeof body.values === "object" && !Array.isArray(body.values) ? body.values : {};
    const sanitized = {};
    for (const [name, pattern] of Object.entries(hook.sanitizers ?? {})) {
      if (values[name] === undefined) continue;
      const clean = sanitizeValue(values[name], pattern);
      if (clean === null) {
        return json(res, 400, { ok: false, error: "SANITIZE_FAILED", variable: name });
      }
      sanitized[name] = clean;
    }
    const serverVars = {
      CLIENT_IP: clientIp(req),
      SCRIPT_NAME: script.name ?? "",
      SCRIPT_ID: script.scriptId,
      USER_NOTE: license?.label || "Not specified",
      LICENSE_ID: license?.licenseId ?? "",
      TOTAL_EXECUTIONS: String(license?.executions ?? 0),
      COUNTRY_CODE: (req.headers["cf-ipcountry"] || req.headers["x-country-code"] || "").toString().slice(0, 2).toLowerCase(),
    };
    const rendered = renderTemplate(hook.template, sanitized, serverVars);
    if (rendered.length > WEBHOOK_PAYLOAD_MAX) {
      return json(res, 413, { ok: false, error: "PAYLOAD_TOO_LARGE" });
    }
    let payload;
    try {
      payload = JSON.parse(rendered);
    } catch {
      return json(res, 400, { ok: false, error: "BAD_TEMPLATE" });
    }
    if (Array.isArray(payload.embeds) && payload.embeds.length > 3) {
      return json(res, 400, { ok: false, error: "TOO_MANY_EMBEDS" });
    }
    const result = await deliverWebhook(hook.url, payload, null, {
      webhookId: hook.webhookId, folderId: hook.folderId, kind: "protected", target: hook.name,
    });
    saveState();
    return json(res, result.ok ? 200 : 502, { ok: result.ok, delivered: result.ok, status: result.status });
  }

  /* ---------------- Phase 6: 外部验卡 API ---------------- */

  if (req.method === "GET" && req.url.startsWith("/external/sync")) {
    return json(res, 200, {
      ok: true,
      st: now(),
      algorithm: "hmac-sha256",
      nodes: [publicOrigin(req)],
      version: 1,
    });
  }

  if (req.method === "GET" && req.url.startsWith("/external/check_key")) {
    const retryAfter = externalCheckLimited(clientIp(req));
    if (retryAfter > 0) {
      res.setHeader("retry-after", String(retryAfter));
      return json(res, 429, { ok: false, code: "KEY_LOCKED" });
    }
    const url = new URL(req.url, "http://localhost");
    const appId = (url.searchParams.get("by") || "").trim();
    const key = (url.searchParams.get("key") || "").trim();
    const app = state.externalApps[appId];
    if (!app) return json(res, 403, { ok: false, code: "SECURITY_ERROR" });

    const header = (name) => {
      const value = req.headers[name];
      return typeof value === "string" ? value : "";
    };
    const clientNonce = header("clientnonce");
    const clientTime = header("clienttime");
    const clientHwid = header("clienthwid");
    const signature = header("externalsignature");
    if (!clientNonce || !clientTime || !signature) {
      return json(res, 400, { ok: false, code: "SECURITY_ERROR" });
    }
    const sentAt = Number(clientTime);
    if (!Number.isFinite(sentAt) || Math.abs(now() - sentAt) > EXTERNAL_CLOCK_SKEW_MS) {
      return json(res, 409, { ok: false, code: "TIME_ERROR" });
    }
    const expected = externalSignature(app.secret, clientNonce, key, clientTime, clientHwid);
    if (!digestsEqual(expected, signature.toLowerCase())) {
      return json(res, 403, { ok: false, code: "SECURITY_ERROR" });
    }

    const sign = (code) => ({ signature: externalSignature(app.secret, clientNonce, code) });
    if (!/^osk_[\da-f]{64}$/i.test(key)) {
      return json(res, 400, { ok: false, code: "KEY_INVALID", ...sign("KEY_INVALID") });
    }
    const licenseId = state.licenseKeyIndex[licenseKeyDigest(key)];
    const license = licenseId ? state.licenseKeys[licenseId] : null;
    if (!license || license.ownerEmail !== app.ownerEmail) {
      return json(res, 404, { ok: false, code: "KEY_INCORRECT", ...sign("KEY_INCORRECT") });
    }
    if (license.revokedAt) {
      return json(res, 403, { ok: false, code: "KEY_INCORRECT", ...sign("KEY_INCORRECT") });
    }
    const ban = activeLicenseBan(license);
    if (ban) return json(res, 403, { ok: false, code: "KEY_BANNED", ...sign("KEY_BANNED") });
    if (license.expiresAt && now() >= license.expiresAt) {
      return json(res, 403, { ok: false, code: "KEY_EXPIRED", ...sign("KEY_EXPIRED") });
    }
    if (license.hwidHash && clientHwid && !digestsEqual(license.hwidHash, licenseHwidDigest(clientHwid))) {
      return json(res, 403, { ok: false, code: "KEY_HWID_LOCKED", ...sign("KEY_HWID_LOCKED") });
    }
    return json(res, 200, {
      ok: true,
      code: "KEY_VALID",
      data: {
        auth_expire: license.expiresAt ?? -1,
        note: license.label || "Not specified",
        total_executions: license.executions ?? 0,
        folder_id: license.folderId,
      },
      ...sign("KEY_VALID"),
    });
  }

  if (req.method === "GET" && req.url === "/api/me") {
    if (rateLimited(clientIp(req))) return json(res, 429, { ok: false, error: "RATE_LIMITED" });
    const u = authUser(req);
    if (!u) return unauthorized(res, req);
    return json(res, 200, { ok: true, user: userPublic(u), quota: quotaJson(u.email) });
  }

  if (req.method === "POST" && req.url === "/api/publish") {
    let body;
    try {
      body = JSON.parse(await readBody(req, MAX_SOURCE_BYTES * 2 + 65536));
    } catch (e) {
      if (e instanceof Error && e.message === "BODY_TOO_LARGE") {
        json(res, 413, { ok: false, error: "BODY_TOO_LARGE" });
        req.destroy();
        return;
      }
      return json(res, 400, { ok: false, error: "BAD_JSON" });
    }
    if (!body || typeof body !== "object" || Array.isArray(body)) {
      return json(res, 400, { ok: false, error: "BAD_REQUEST" });
    }
    if (rateLimited(clientIp(req))) return json(res, 429, { ok: false, error: "RATE_LIMITED" });
    const owner = authUser(req);
    if (!owner) return unauthorized(res, req);

    const { scriptId, version, source, options } = body;
    const script = typeof scriptId === "string" ? state.scripts[scriptId] : null;
    if (!script || script.ownerEmail !== owner.email || !script.active) {
      return json(res, 404, { ok: false, error: "SCRIPT_NOT_FOUND" });
    }
    if (typeof version !== "string" || !/^[A-Za-z0-9._-]{1,32}$/.test(version)) {
      return json(res, 400, { ok: false, error: "BAD_VERSION" });
    }
    if (typeof source !== "string" || !source.trim()) return json(res, 400, { ok: false, error: "BAD_SOURCE" });
    const sourceBytes = Buffer.byteLength(source, "utf8");
    if (sourceBytes > MAX_SOURCE_BYTES) return json(res, 413, { ok: false, error: "SOURCE_TOO_LARGE" });

    const key = `${scriptId}:${version}`;
    const existing = Object.keys(state.releases).filter((k) => k.startsWith(`${scriptId}:`));
    if (existing.length >= MAX_RELEASES_PER_SCRIPT && !state.releases[key]) {
      return json(res, 409, { ok: false, error: "RELEASE_LIMIT" });
    }

    const mergedOptions = { ...DEFAULT_OBFUSCATION_OPTIONS, ...(options && typeof options === "object" && !Array.isArray(options) ? options : {}) };
    const optErr = validateOptions(mergedOptions);
    if (optErr) return json(res, 400, { ok: false, error: optErr });
    if (engineMode() === "missing") return json(res, 503, { ok: false, error: "ENGINE_MISSING" });

    if (activeJobsFor(owner.email).length >= MAX_ACTIVE_JOBS_PER_ACCOUNT) {
      return json(res, 409, { ok: false, error: "PUBLISH_BUSY" });
    }

    const q = quotaOf(owner.email);
    if (q.used >= DAILY_QUOTA) {
      return json(res, 429, { ok: false, error: "QUOTA_EXCEEDED", quota: quotaJson(owner.email) });
    }
    // 预留额度：失败由 failJob 退还，成功才真正消耗。
    q.used += 1;
    state.quotas[owner.email] = q;
    saveState();

    const job = {
      id: `pub_${randomBytes(9).toString("hex")}`,
      ownerEmail: owner.email,
      scriptId,
      version,
      source,
      sourceBytes,
      status: "processing",
      stage: "queued",
      progress: PUBLISH_STAGES.queued,
      startedAt: now(),
      quotaCharged: true,
      quotaDate: q.date,
    };
    publishJobs.set(job.id, job);

    const origin = publicOrigin(req);
    // 不 await：先把 jobId 还给前端，混淆在后台继续。
    void runPublishJob(job, { source, version, options: mergedOptions, origin });

    return json(res, 202, {
      ok: true,
      jobId: job.id,
      stage: job.stage,
      progress: job.progress,
      quota: quotaJson(owner.email),
    });
  }

  if (req.method === "POST" && req.url === "/api/obfuscate") {
    let body;
    try {
      body = JSON.parse(await readBody(req, MAX_SOURCE_BYTES * 2 + 65536));
    } catch (e) {
      // 超限：回 413 并断开残留上传流（此前对已销毁 socket 写响应 → 空 500）
      if (e instanceof Error && e.message === "BODY_TOO_LARGE") {
        json(res, 413, { ok: false, error: "BODY_TOO_LARGE" });
        req.destroy();
        return;
      }
      return json(res, 400, { ok: false, error: "BAD_JSON" });
    }
    const { source, options } = body || {};
    if (typeof source !== "string" || !options || typeof options !== "object") {
      return json(res, 400, { ok: false, error: "BAD_REQUEST" });
    }
    if (rateLimited(clientIp(req))) {
      return json(res, 429, { ok: false, error: "RATE_LIMITED" });
    }
    const auth = authUser(req);
    if (!auth) return unauthorized(res, req);
    const q = quotaOf(auth.email);
    if (q.used >= 5) return json(res, 429, { ok: false, error: "QUOTA_EXCEEDED", quota: quotaJson(auth.email) });
    const optErr = validateOptions(options);
    if (optErr) {
      return json(res, 400, { ok: false, error: optErr });
    }
    const inputBytes = Buffer.byteLength(source, "utf8");
    if (inputBytes > MAX_SOURCE_BYTES) {
      return json(res, 413, { ok: false, error: "SOURCE_TOO_LARGE", detail: String(inputBytes) });
    }
    if (engineMode() === "missing") {
      return json(res, 503, { ok: false, error: "ENGINE_MISSING", detail: "obsucvim native binary or WASI bundle not found" });
    }

    const args = buildCliArgs(options);
    const seed = normalizeSeed(options.seed);
    const t0 = Date.now();
    const log = [];
    log.push({ level: "step", text: `[1/4] 接收源码 · ${inputBytes} B · obsucvim ${args.join(" ")}` });

    // CLI 读文件（无 stdin 模式）：源码落临时文件，产物走 stdout
    if (!trySlot()) {
      return json(res, 503, { ok: false, error: "ENGINE_BUSY", detail: "max concurrent obfuscations in flight" });
    }
    const workDir = mkdtempSync(path.join(tmpdir(), `obsucvim-job-${process.pid}-`));
    writeFileSync(path.join(workDir, "input.lua"), source, "utf8");
    let r;
    try {
      r = await runEngine(args, workDir, ENGINE_TIMEOUT_MS);
    } finally {
      releaseSlot();
      try { rmSync(workDir, { recursive: true, force: true }); } catch { /* ignore */ }
    }
    if (r.timedOut) return json(res, 504, { ok: false, error: "ENGINE_TIMEOUT" });
    if (r.code !== 0 || !r.codeText) {
      return json(res, 500, { ok: false, error: "ENGINE_FAIL", detail: r.err.trim().split("\n").slice(-3).join(" | ") });
    }
    const code = r.codeText;
    const elapsedMs = Date.now() - t0;
    log.push({ level: "info", text: `[2/4] Rust 引擎执行完成 · rc=0 · ${elapsedMs} ms` });
    log.push({ level: "info", text: `保护通道：${appliedNames(options).join(" · ")}` });
    log.push({ level: "info", text: "Runtime protections active." });

    // 可选：产物语法校验（luau-compile 存在时；无 stdin 模式，走临时文件）
    if (existsSync(LUAUC)) {
      const tmpOut = path.join(tmpdir(), tmpName("out"));
      writeFileSync(tmpOut, code, "utf8");
      let v;
      try {
        v = await run(LUAUC, [tmpOut], { timeoutMs: 15000 });
      } finally {
        try { unlinkSync(tmpOut); } catch { /* ignore */ }
      }
      if (v.code === 0) log.push({ level: "ok", text: "[3/4] luau-compile 语法校验通过" });
      else log.push({ level: "warn", text: `[3/4] luau-compile 语法校验失败：${v.err.trim().split("\n").slice(-1)[0]}` });
    } else {
      log.push({ level: "info", text: "[3/4] luau-compile 不可用，跳过语法校验" });
    }

    const outputBytes = Buffer.byteLength(code, "utf8");
    log.push({ level: "ok", text: `[4/4] 完成 · 已应用 ${appliedNames(options).length} 项保护` });

    q.used += 1;
    state.quotas[auth.email] = q;
    saveState();
    return json(res, 200, {
      ok: true,
      quota: quotaJson(auth.email),
      result: {
        code,
        log,
        stats: {
          inputBytes,
          outputBytes,
          ratio: outputBytes / Math.max(inputBytes, 1),
          elapsedMs,
          lines: code.split("\n").length,
          seeded: seed,
        },
      },
    });
  }

  /* Stable loader URL. Users copy one loadstring line and never change it. */
  let loaderPath = "";
  try { loaderPath = new URL(req.url, "http://localhost").pathname; } catch { loaderPath = ""; }
  const loaderMatch = /^\/files\/(scr_[\da-f]{24})\.lua$/i.exec(loaderPath);
  if (loaderMatch && (req.method === "GET" || req.method === "HEAD")) {
    const retryAfter = licenseVerifyRetryAfter(clientIp(req));
    if (retryAfter > 0) {
      res.setHeader("retry-after", String(retryAfter));
      return json(res, 429, { ok: false, error: "RATE_LIMITED", retryAfter });
    }
    const target = state.scripts[loaderMatch[1].toLowerCase()];
    if (!target || !target.active) return json(res, 404, { ok: false, error: "SCRIPT_NOT_FOUND" });
    const body = buildLoaderSource(target, publicOrigin(req));
    res.writeHead(200, {
      "content-type": "text/plain; charset=utf-8",
      "cache-control": "no-store",
      "x-content-type-options": "nosniff",
    });
    return res.end(req.method === "HEAD" ? undefined : body);
  }

  if (req.url.startsWith("/api/")) return json(res, 404, { ok: false, error: "NOT_FOUND" });
  return serveStatic(req, res);
});

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".ico": "image/x-icon",
  ".json": "application/json; charset=utf-8",
  ".wasm": "application/wasm",
};

function serveStatic(req, res) {
  if (req.method !== "GET" && req.method !== "HEAD") return json(res, 405, { ok: false, error: "METHOD_NOT_ALLOWED" });
  if (!existsSync(WEB_ROOT)) return json(res, 503, { ok: false, error: "WEB_DIST_MISSING", detail: WEB_ROOT });
  let pathname;
  try { pathname = decodeURIComponent(new URL(req.url, "http://localhost").pathname); }
  catch { return json(res, 400, { ok: false, error: "BAD_PATH" }); }
  const root = path.resolve(WEB_ROOT);
  const relative = pathname === "/" ? "index.html" : pathname.replace(/^\/+/, "");
  let file = path.resolve(root, relative);
  if (!file.startsWith(root + path.sep)) {
    return json(res, 403, { ok: false, error: "FORBIDDEN" });
  }
  if (!existsSync(file) || !statSync(file).isFile()) {
    // SPA route fallback only for extensionless paths
    if (path.extname(pathname)) return json(res, 404, { ok: false, error: "NOT_FOUND" });
    file = path.join(root, "index.html");
  }
  const ext = path.extname(file).toLowerCase();
  res.writeHead(200, {
    "content-type": MIME[ext] || "application/octet-stream",
    "x-content-type-options": "nosniff",
    "referrer-policy": "strict-origin-when-cross-origin",
    "cache-control": ext === ".html" ? "no-cache" : "public, max-age=3600",
  });
  if (req.method === "HEAD") return res.end();
  createReadStream(file).pipe(res);
}

server.listen(PORT, HOST, () => {
  console.log(`[engine-server] obsucvim web+engine on http://${HOST}:${PORT}`);
  console.log(`[engine-server] engine mode: ${engineMode()} (native: ${existsSync(BIN) ? "found" : "missing"}; WASI: ${existsSync(WASM_BIN) ? "found" : "missing"})`);
  // 引擎版本自检：v15 输出对固定输入+种子是确定的，所以能据此认出构建版本。
  // 这不影响服务启动——探测失败只记录，不阻断。
  void (async () => {
    const KNOWN = {
      R067: { size: 229422, sha256: "3775f3b557ca4c95ee16d9d013f290d735b6c022af23ac17f1d0b2339fc689f6" },
      af7fc32: { size: 212137, sha256: "01099ddbb04eb2608ca230b3b7e4ed1af476d2f9afa3a78f43024caa0eda51da" },
    };
    try {
      if (engineMode() !== "wasi") return;
      const out = await obfuscateSource('print("suda")\n', {
        ...DEFAULT_OBFUSCATION_OPTIONS, seed: "67067", hardening: true,
      });
      if (!out.ok) {
        console.log("[engine-server] engine build: unknown (probe failed)");
        return;
      }
      const digest = createHash("sha256").update(out.code).digest("hex");
      const size = Buffer.byteLength(out.code, "utf8");
      const match = Object.entries(KNOWN).find(([, v]) => v.size === size && v.sha256 === digest);
      if (match) {
        console.log(`[engine-server] engine build: ${match[0]}`);
      } else {
        console.log(`[engine-server] engine build: UNKNOWN (${size} B, ${digest.slice(0, 16)}…)`);
      }
    } catch {
      /* 探测只是提示，任何失败都不影响服务 */
    }
  })();

  console.log(`[engine-server] web dist: ${WEB_ROOT} (${existsSync(path.join(WEB_ROOT, "index.html")) ? "found" : "MISSING"})`);
  console.log(`[engine-server] smtp: ${demoMail ? "demo fallback" : "configured"}`);
  console.log(`[engine-server] captcha: ${TURNSTILE_MISCONFIGURED ? "CONFIGURATION ERROR" : TURNSTILE_ENABLED ? "Cloudflare Turnstile" : "local puzzle fallback"}`);
});
