#!/usr/bin/env node
/**
 * Obsucvim 本地引擎后端（开发用，使用 Nodemailer 发送验证邮件）。
 * 包装真实 obsucvim CLI（Rust），给 web 控制台提供 /api/*：
 *   GET  /api/health     → { ok, engine, bin }
 *   POST /api/obfuscate  → { ok, result: { code, log, stats } }
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
import { existsSync, readFileSync, writeFileSync, unlinkSync, mkdtempSync, rmSync, createReadStream, statSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { randomBytes, scryptSync, timingSafeEqual } from "node:crypto";
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

/* ---------------- 选项 → CLI 参数（与前端 buildCliArgs 对齐） ---------------- */
function buildCliArgs(o) {
  const args = ["--preset", "v15", "--env-check", "--bind-executor", "--bind-env", "roblox"];
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

function json(res, status, body) {
  const text = JSON.stringify(body);
  // 同源代理（Vite dev）访问，不需要 CORS；去掉通配符开放
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "x-content-type-options": "nosniff",
    "cache-control": "no-store",
  });
  res.end(text);
}

/* ================= 账户体系（服务端，2026-10-11 安全修复） =================
 * 审计结论修复：会话/密码/验证码/锁定/额度全部移到服务端——浏览器只持有
 * 随机令牌，devtools 无法再伪造登录态、旁路人机验证或清除锁定。
 * 状态持久化到 tools/.state.json（gitignored，含哈希）。 */
const STATE_FILE = process.env.OBSUCVIM_STATE_FILE || path.join(__dirname, ".state.json");
const SESSION_TTL_MS = 7 * 24 * 3600 * 1000;
const CODE_TTL_MS = 10 * 60_000;
const CODE_RESEND_MS = 60_000;
const CODE_MAX_FAILS = 5;
const LOGIN_MAX_FAILS = 5;
const LOGIN_LOCK_MS = 60_000;
const AUTH_BODY_LIMIT = 32 * 1024;
const MAX_ACCOUNTS = 1000;      // 演示环境账户上限
const EMAIL_MAX = 100;          // 邮箱长度上限（防长邮箱膨胀状态文件）
const MAX_PENDING_REG = 200;    // 同时待验证的注册草稿上限

let state = {
  users: {},      // email -> { email, nickname, salt, hash, created }
  sessions: {},   // token -> { email, expires }
  quotas: {},     // email -> { date, used }
  fails: {},      // email -> { fails, until }
  codes: {},      // email -> { code, expires, sentAt, fails }
  captchas: {},   // id -> { answer, expires }
  apiKeys: {},    // key -> { email, created }
  pendingRegistrations: {}, // email -> { nickname, salt, hash, expires }
};

function loadState() {
  try {
    const raw = readFileSync(STATE_FILE, "utf8");
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") state = { ...state, ...parsed };
  } catch {
    /* first run */
  }
}
function saveState() {
  try {
    writeFileSync(STATE_FILE, JSON.stringify(state));
  } catch {
    /* ignore */
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
  saveState();
}, 300_000).unref?.();

const now = () => Date.now();
const todayStr = () => {
  const d = new Date(); // server-local timezone; Windows Server clock/timezone is authoritative
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};
const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

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
  const h = req.headers["authorization"];
  if (typeof h !== "string" || !h.startsWith("Bearer ")) return null;
  const token = h.slice(7).trim();
  const s = state.sessions[token];
  if (s && now() <= s.expires) return state.users[s.email] ?? null;
  return apiKeyUser(token);
}

/* 两步注册的服务端草稿：第一步保存昵称/密码（10 分钟），确认步消费——
 * 确认请求只带 email+code，防止两步之间注册信息被改写 */
const pendingRegistrations = state.pendingRegistrations;

const API_KEY_PREFIX = "obk_";

function newApiKey() {
  return API_KEY_PREFIX + randomBytes(24).toString("hex");
}

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
  return { usedToday: q.used, remaining: Math.max(0, 5 - q.used), limit: 5 };
}

/* 服务端人机验证：算式问答——答案永不下发客户端（题面可见无妨） */
function issueCaptcha() {
  const a = 2 + Math.floor(Math.random() * 19);
  const b = 2 + Math.floor(Math.random() * 19);
  const ops = [
    ["+", (x, y) => x + y],
    ["-", (x, y) => x - y],
    ["×", (x, y) => x * y],
  ];
  const [sym, fn] = ops[Math.floor(Math.random() * ops.length)];
  const id = randomBytes(12).toString("hex");
  state.captchas[id] = { answer: String(fn(a, b)), expires: now() + 5 * 60_000 };
  saveState();
  return { captchaId: id, question: `${a} ${sym} ${b}` };
}

function consumeCaptcha(id, answer) {
  const c = state.captchas[id];
  delete state.captchas[id];
  saveState();
  if (!c || now() > c.expires) return false;
  return String(answer ?? "").trim() === c.answer;
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

/* 安全（2026-10-11 红队自测修复）：不信任 X-Forwarded-For——
 * 攻击者可每个请求伪造一个新 XFF 无限绕过限流（实测 40 连发全 200）。
 * 一律按 socket 地址计数：经 Vite 代理的所有流量共享一个桶（保守方向，
 * 无法伪造）。直连 127.0.0.1 的本机流量同桶。 */
function clientIp(req) {
  return req.socket?.remoteAddress ?? "unknown";
}

/* auth 专用限流（独立桶，比引擎更严格：防垃圾注册同时不影响混淆使用） */
const AUTH_RATE_LIMIT = 15;       // 15 次/分钟
const AUTH_RATE_WINDOW_MS = 60_000;
const authRateHits = new Map();
setInterval(() => {
  const n = Date.now();
  for (const [ip, h] of authRateHits) if (n > h.reset) authRateHits.delete(ip);
}, AUTH_RATE_WINDOW_MS).unref?.();

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

let cachedVersion; // obsucvim --version，启动后首次 health 探测并缓存

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
  if (req.method === "OPTIONS") return json(res, 204, {});

  if (req.method === "GET" && req.url === "/api/health") {
    /* 安全修复：health 也纳入限流（此前无限流 + 每请求 spawn 一次
     * obsucvim --version = 无限进程洪泛 DoS，实测 50 连发 0.64s）；
     * 版本改为启动时探测一次并缓存。 */
    if (rateLimited(clientIp(req))) {
      return json(res, 429, { ok: false, error: "RATE_LIMITED" });
    }
    const mode = engineMode();
    if (cachedVersion === undefined) {
      cachedVersion = mode === "native"
        ? (await run(BIN, ["--version"], { timeoutMs: 5000 })).out.trim() || "obsucvim native"
        : mode === "wasi" ? "obsucvim 0.2.0 (WASI)" : null;
    }
    return json(res, 200, {
      ok: mode !== "missing",
      engine: cachedVersion || "obsucvim (engine missing)",
      engineMode: mode,
      mail: demoMail ? "demo" : "smtp",
      quotaPerDay: 5,
    });
  }

  /* ================= 认证端点 ================= */
  const handleAuth = async () => {
    if (authRateLimited(clientIp(req))) {
      return json(res, 429, { ok: false, error: "RATE_LIMITED" });
    }
    let body;
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
    if (!body || typeof body !== "object") return json(res, 400, { ok: false, error: "BAD_REQUEST" });

    if (req.method === "POST" && req.url === "/api/auth/captcha") {
      return json(res, 200, { ok: true, ...issueCaptcha() });
    }

    if (req.method === "POST" && req.url === "/api/auth/register") {
      const { email, nickname, password, captchaId, captchaAnswer } = body;
      if (
        typeof email !== "string" || email.length > EMAIL_MAX || !EMAIL_RE.test(email) ||
        typeof password !== "string" || password.length < 8 || password.length > 128 ||
        (nickname !== undefined && (typeof nickname !== "string" || nickname.length > 24))
      ) return json(res, 400, { ok: false, error: "BAD_INPUT" });
      if (Object.keys(state.users).length >= MAX_ACCOUNTS) {
        return json(res, 503, { ok: false, error: "SERVICE_FULL" });
      }
      if (Object.keys(pendingRegistrations).length >= MAX_PENDING_REG) {
        return json(res, 503, { ok: false, error: "SERVICE_FULL" });
      }
      if (!consumeCaptcha(captchaId, captchaAnswer)) return json(res, 400, { ok: false, error: "CAPTCHA_FAIL" });
      const key = email.trim().toLowerCase();
      if (state.users[key]) return json(res, 409, { ok: false, error: "EMAIL_TAKEN" });
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
      if (typeof email !== "string" || !EMAIL_RE.test(email) || typeof code !== "string") {
        return json(res, 400, { ok: false, error: "BAD_INPUT" });
      }
      const key = email.trim().toLowerCase();
      const v = checkEmailCode(key, code);
      if (!v.ok) return json(res, 400, { ok: false, error: v.error });
      if (state.users[key]) return json(res, 409, { ok: false, error: "EMAIL_TAKEN" });
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
      saveState();
      const token = sessionFor(key);
      return json(res, 200, { ok: true, token, user: userPublic(state.users[key]) });
    }

    if (req.method === "POST" && req.url === "/api/auth/resend") {
      const { email } = body;
      if (typeof email !== "string" || !EMAIL_RE.test(email)) return json(res, 400, { ok: false, error: "BAD_INPUT" });
      const key = email.trim().toLowerCase();
      if (state.users[key]) return json(res, 409, { ok: false, error: "EMAIL_TAKEN" });
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
      if (!consumeCaptcha(captchaId, captchaAnswer)) return json(res, 400, { ok: false, error: "CAPTCHA_FAIL" });
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
      return json(res, 200, { ok: true, token, user: userPublic(u), quota: quotaJson(key) });
    }

    if (req.method === "POST" && req.url === "/api/auth/logout") {
      const h = req.headers["authorization"];
      if (typeof h === "string" && h.startsWith("Bearer ")) {
        delete state.sessions[h.slice(7).trim()];
        saveState();
      }
      return json(res, 200, { ok: true });
    }

    if (req.method === "GET" && req.url === "/api/auth/apikeys") {
      const u = authUser(req);
      if (!u) return json(res, 401, { ok: false, error: "UNAUTHORIZED" });
      const keys = Object.entries(state.apiKeys)
        .filter(([, v]) => v.email === u.email)
        .map(([k, v]) => ({ id: k.slice(0, 12) + "…", created: v.created }));
      return json(res, 200, { ok: true, keys });
    }

    if (req.method === "POST" && req.url === "/api/auth/apikeys") {
      const u = authUser(req);
      if (!u) return json(res, 401, { ok: false, error: "UNAUTHORIZED" });
      const key = newApiKey();
      state.apiKeys[key] = { email: u.email, created: now() };
      saveState();
      return json(res, 200, { ok: true, key });
    }

    if (req.method === "DELETE" && req.url === "/api/auth/apikeys") {
      const u = authUser(req);
      if (!u) return json(res, 401, { ok: false, error: "UNAUTHORIZED" });
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

    return json(res, 404, { ok: false, error: "NOT_FOUND" });
  };

  if (req.url.startsWith("/api/auth/")) return await handleAuth();

  if (req.method === "GET" && req.url === "/api/me") {
    if (rateLimited(clientIp(req))) return json(res, 429, { ok: false, error: "RATE_LIMITED" });
    const u = authUser(req);
    if (!u) return json(res, 401, { ok: false, error: "UNAUTHORIZED" });
    return json(res, 200, { ok: true, user: userPublic(u), quota: quotaJson(u.email) });
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
    if (!auth) return json(res, 401, { ok: false, error: "UNAUTHORIZED" });
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
  console.log(`[engine-server] web dist: ${WEB_ROOT} (${existsSync(path.join(WEB_ROOT, "index.html")) ? "found" : "MISSING"})`);
  console.log(`[engine-server] smtp: ${demoMail ? "demo fallback" : "configured"}`);
});
