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
import { existsSync, readFileSync, writeFileSync, unlinkSync, mkdtempSync, rmSync, createReadStream, statSync } from "node:fs";
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
const MAX_SCRIPTS_PER_ACCOUNT = 50;
const MAX_LICENSES_PER_SCRIPT = 2000;
const LICENSE_VERIFY_LIMIT = 60;
const LICENSE_VERIFY_WINDOW_MS = 60_000;

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
  scripts: {}, // scriptId -> { ownerEmail, name, createdAt, active }
  licenseKeys: {}, // licenseId -> hashed key metadata; raw keys are never stored
  licenseKeyIndex: {}, // SHA-256(key) -> licenseId
  licenseHwidHashKey: "", // stable server-only HMAC key; raw device IDs are never stored
};

function loadState() {
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
  if (changed) saveState();
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

const licenseKeyDigest = (key) => createHash("sha256").update(key).digest("hex");
const licenseHwidDigest = (hwid) =>
  createHmac("sha256", state.licenseHwidHashKey).update(hwid.trim()).digest("hex");
const licensePublic = (license) => ({
  licenseId: license.licenseId,
  scriptId: license.scriptId,
  label: license.label,
  createdAt: license.createdAt,
  expiresAt: license.expiresAt,
  revokedAt: license.revokedAt,
  hwidBound: Boolean(license.hwidHash),
  firstActivatedAt: license.firstActivatedAt,
  lastUsedAt: license.lastUsedAt,
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
  return { usedToday: q.used, remaining: Math.max(0, 5 - q.used), limit: 5 };
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
    if (authRateLimited(clientIp(req))) {
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

    if (req.method === "GET" && req.url === "/api/auth/scripts") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const scripts = Object.values(state.scripts)
        .filter((script) => script.ownerEmail === owner.email)
        .map(({ scriptId, name, description, createdAt, active }) => ({ scriptId, name, description, createdAt, active }));
      return json(res, 200, { ok: true, scripts });
    }

    if (req.method === "POST" && req.url === "/api/auth/scripts") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const { name, description } = body;
      if (
        typeof name !== "string" || name.trim().length < 2 || name.trim().length > 80 ||
        (description !== undefined && (typeof description !== "string" || description.length > 500))
      ) return json(res, 400, { ok: false, error: "BAD_INPUT" });
      const owned = Object.values(state.scripts).filter((script) => script.ownerEmail === owner.email);
      if (owned.length >= MAX_SCRIPTS_PER_ACCOUNT) return json(res, 409, { ok: false, error: "SCRIPT_LIMIT" });
      const scriptId = SCRIPT_ID_PREFIX + randomBytes(12).toString("hex");
      const script = {
        scriptId,
        ownerEmail: owner.email,
        name: name.trim(),
        description: typeof description === "string" ? description.trim() : "",
        createdAt: now(),
        active: true,
      };
      state.scripts[scriptId] = script;
      saveState();
      return json(res, 201, { ok: true, script: { scriptId, name: script.name, description: script.description, createdAt: script.createdAt, active: true } });
    }

    if (req.method === "GET" && new URL(req.url, "http://localhost").pathname === "/api/auth/licenses") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const url = new URL(req.url, "http://localhost");
      const scriptId = url.searchParams.get("scriptId");
      if (scriptId && state.scripts[scriptId]?.ownerEmail !== owner.email) {
        return json(res, 404, { ok: false, error: "NOT_FOUND" });
      }
      const licenses = Object.values(state.licenseKeys)
        .filter((license) => license.ownerEmail === owner.email && (!scriptId || license.scriptId === scriptId))
        .map(licensePublic);
      return json(res, 200, { ok: true, licenses });
    }

    if (req.method === "POST" && req.url === "/api/auth/licenses") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const { scriptId, label, expiresAt } = body;
      const script = typeof scriptId === "string" ? state.scripts[scriptId] : null;
      if (!script || script.ownerEmail !== owner.email || !script.active) {
        return json(res, 404, { ok: false, error: "SCRIPT_NOT_FOUND" });
      }
      if (label !== undefined && (typeof label !== "string" || label.length > 80)) {
        return json(res, 400, { ok: false, error: "BAD_INPUT" });
      }
      let expiry = null;
      if (expiresAt !== undefined && expiresAt !== null && expiresAt !== "") {
        if (typeof expiresAt !== "string") return json(res, 400, { ok: false, error: "BAD_INPUT" });
        expiry = Date.parse(expiresAt);
        if (!Number.isFinite(expiry) || expiry <= now()) return json(res, 400, { ok: false, error: "BAD_EXPIRY" });
      }
      const scriptLicenses = Object.values(state.licenseKeys).filter((license) => license.scriptId === scriptId);
      if (scriptLicenses.length >= MAX_LICENSES_PER_SCRIPT) {
        return json(res, 409, { ok: false, error: "LICENSE_LIMIT" });
      }
      const licenseId = "lic_" + randomBytes(12).toString("hex");
      const licenseKey = LICENSE_KEY_PREFIX + randomBytes(32).toString("hex");
      const keyHash = licenseKeyDigest(licenseKey);
      const license = {
        licenseId,
        keyHash,
        ownerEmail: owner.email,
        scriptId,
        label: typeof label === "string" ? label.trim() : "",
        createdAt: now(),
        expiresAt: expiry,
        revokedAt: null,
        hwidHash: null,
        firstActivatedAt: null,
        lastUsedAt: null,
      };
      state.licenseKeys[licenseId] = license;
      state.licenseKeyIndex[keyHash] = licenseId;
      saveState();
      return json(res, 201, {
        ok: true,
        licenseKey, // raw key is returned once; only its hash is persisted
        license: licensePublic(license),
      });
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

    if (req.method === "POST" && req.url === "/api/auth/licenses/reset-hwid") {
      const owner = authUser(req);
      if (!owner) return unauthorized(res, req);
      const license = typeof body.licenseId === "string" ? state.licenseKeys[body.licenseId] : null;
      if (!license || license.ownerEmail !== owner.email) return json(res, 404, { ok: false, error: "NOT_FOUND" });
      license.hwidHash = null;
      license.firstActivatedAt = null;
      saveState();
      return json(res, 200, { ok: true, license: licensePublic(license) });
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

    return json(res, 404, { ok: false, error: "NOT_FOUND" });
  };

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
    const { scriptId, key, hwid, nonce } = body;
    if (
      typeof scriptId !== "string" || !/^scr_[\da-f]{24}$/i.test(scriptId) ||
      typeof key !== "string" || !/^osk_[\da-f]{64}$/i.test(key) ||
      typeof hwid !== "string" || hwid.trim().length < 8 || hwid.trim().length > 256 ||
      (nonce !== undefined && (typeof nonce !== "string" || !/^[\da-z]{16,64}$/i.test(nonce)))
    ) return json(res, 400, { ok: false, valid: false, error: "BAD_REQUEST" });

    const script = state.scripts[scriptId];
    const licenseId = state.licenseKeyIndex[licenseKeyDigest(key)];
    const license = licenseId ? state.licenseKeys[licenseId] : null;
    if (
      !script || !script.active || !license || license.scriptId !== scriptId ||
      license.revokedAt || (license.expiresAt && now() >= license.expiresAt)
    ) return json(res, 403, { ok: false, valid: false, error: "LICENSE_INVALID" });

    const hwidHash = licenseHwidDigest(hwid);
    if (license.hwidHash && license.hwidHash !== hwidHash) {
      return json(res, 403, { ok: false, valid: false, error: "DEVICE_MISMATCH" });
    }
    if (!license.hwidHash) {
      license.hwidHash = hwidHash;
      license.firstActivatedAt = now();
    }
    license.lastUsedAt = now();
    saveState();
    return json(res, 200, {
      ok: true,
      valid: true,
      scriptId,
      licenseId,
      expiresAt: license.expiresAt,
      deviceBound: true,
      ...(nonce === undefined ? {} : { nonce }),
    });
  }

  if (req.method === "GET" && req.url === "/api/me") {
    if (rateLimited(clientIp(req))) return json(res, 429, { ok: false, error: "RATE_LIMITED" });
    const u = authUser(req);
    if (!u) return unauthorized(res, req);
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
  console.log(`[engine-server] captcha: ${TURNSTILE_MISCONFIGURED ? "CONFIGURATION ERROR" : TURNSTILE_ENABLED ? "Cloudflare Turnstile" : "local puzzle fallback"}`);
});
