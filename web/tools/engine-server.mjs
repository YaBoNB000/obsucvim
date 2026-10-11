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
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "access-control-allow-origin": "*",
    "access-control-allow-headers": "content-type",
    "access-control-allow-methods": "GET, POST, OPTIONS",
  });
  res.end(text);
}

async function readBody(req, limit) {
  return new Promise((resolve, reject) => {
    let size = 0;
    const chunks = [];
    req.on("data", (c) => {
      size += c.length;
      if (size > limit) {
        reject(new Error("BODY_TOO_LARGE"));
        req.destroy();
        return;
      }
      chunks.push(c);
    });
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
    req.on("error", reject);
  });
}

const server = createServer(async (req, res) => {
  if (req.method === "OPTIONS") return json(res, 204, {});

  if (req.method === "GET" && req.url === "/api/health") {
    const version = existsSync(BIN)
      ? (await run(BIN, ["--version"], { timeoutMs: 5000 })).out.trim()
      : null;
    return json(res, 200, { ok: existsSync(BIN), engine: version || "obsucvim (binary missing)", bin: BIN });
  }

  if (req.method === "POST" && req.url === "/api/obfuscate") {
    let body;
    try {
      body = JSON.parse(await readBody(req, MAX_SOURCE_BYTES * 2 + 65536));
    } catch {
      return json(res, 400, { ok: false, error: "BAD_JSON" });
    }
    const { source, options } = body || {};
    if (typeof source !== "string" || !options || typeof options !== "object") {
      return json(res, 400, { ok: false, error: "BAD_REQUEST" });
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
    const tmpIn = path.join(tmpdir(), `obsucvim-web-${process.pid}-${Date.now()}.lua`);
    writeFileSync(tmpIn, source, "utf8");
    let r;
    try {
      r = await run(BIN, [...args, tmpIn], { timeoutMs: ENGINE_TIMEOUT_MS });
    } finally {
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
      const tmpOut = path.join(tmpdir(), `obsucvim-web-out-${process.pid}-${Date.now()}.lua`);
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
