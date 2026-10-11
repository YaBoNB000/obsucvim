#!/usr/bin/env node
/**
 * Obsucvim 本地引擎后端（开发用，零依赖）。
 * 包装真实 obsucvim CLI（Rust），给 web 控制台提供 /api/*：
 *   GET  /api/health     → { ok, engine, bin }
 *   POST /api/obfuscate  → { ok, result: { code, log, stats } }
 *
 * 选项映射与前端 buildCliArgs() 一致（产品形态固定 v15）。
 * 环境变量：
 *   OBSUCVIM_BIN   obsucvim 二进制路径（默认 <repo>/obsucvim/target/release/obsucvim）
 *   PORT           监听端口（默认 8787，仅 127.0.0.1，由 Vite dev 代理）
 *   LUAUC          luau-compile 路径（默认 <repo>/.tools/bin/luau-compile，可选语法校验）
 */
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, writeFileSync, unlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(__dirname, "..", "..");
const BIN = process.env.OBSUCVIM_BIN || path.join(REPO, "obsucvim", "target", "release", "obsucvim");
const LUAUC = process.env.LUAUC || path.join(REPO, ".tools", "bin", "luau-compile");
const PORT = Number(process.env.PORT || 8787);
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
    if (cachedVersion === undefined) {
      cachedVersion = existsSync(BIN)
        ? (await run(BIN, ["--version"], { timeoutMs: 5000 })).out.trim() || null
        : null;
    }
    return json(res, 200, { ok: existsSync(BIN), engine: cachedVersion || "obsucvim (binary missing)", bin: BIN });
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
    const optErr = validateOptions(options);
    if (optErr) {
      return json(res, 400, { ok: false, error: optErr });
    }
    const inputBytes = Buffer.byteLength(source, "utf8");
    if (inputBytes > MAX_SOURCE_BYTES) {
      return json(res, 413, { ok: false, error: "SOURCE_TOO_LARGE", detail: String(inputBytes) });
    }
    if (!existsSync(BIN)) {
      return json(res, 503, { ok: false, error: "ENGINE_BINARY_MISSING", detail: BIN });
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
    const tmpIn = path.join(tmpdir(), tmpName("in"));
    writeFileSync(tmpIn, source, "utf8");
    let r;
    try {
      r = await run(BIN, [...args, tmpIn], { timeoutMs: ENGINE_TIMEOUT_MS });
    } finally {
      releaseSlot();
      try { unlinkSync(tmpIn); } catch { /* ignore */ }
    }
    if (r.timedOut) return json(res, 504, { ok: false, error: "ENGINE_TIMEOUT" });
    if (r.code !== 0 || !r.out) {
      return json(res, 500, { ok: false, error: "ENGINE_FAIL", detail: r.err.trim().split("\n").slice(-3).join(" | ") });
    }
    const code = r.out;
    const elapsedMs = Date.now() - t0;
    log.push({ level: "info", text: `[2/4] Rust 引擎执行完成 · rc=0 · ${elapsedMs} ms` });
    log.push({ level: "info", text: `保护通道：${appliedNames(options).join(" · ")}` });
    log.push({ level: "info", text: "运行时保护全开（executor 绑定 + 环境检查组 + Roblox 环境绑定 + 防篡改 + 反调试守卫）：产物只在完整 Roblox executor 环境内正确运行" });

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

    return json(res, 200, {
      ok: true,
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

  json(res, 404, { ok: false, error: "NOT_FOUND" });
});

server.listen(PORT, "127.0.0.1", () => {
  console.log(`[engine-server] obsucvim backend on http://127.0.0.1:${PORT}`);
  console.log(`[engine-server] binary: ${BIN} (${existsSync(BIN) ? "found" : "MISSING"})`);
  console.log(`[engine-server] luau-compile: ${LUAUC} (${existsSync(LUAUC) ? "found" : "missing"})`);
});
