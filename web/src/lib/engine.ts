/**
 * ============================================================================
 *  Obsucvim 引擎接入层  (src/lib/engine.ts)
 * ----------------------------------------------------------------------------
 *  产品形态唯一：v15 管线（无强度预设）。选项与 obsucvim CLI 旗标一一对应：
 *    变换   改名 · 压缩 · 字符串加密 · 平坦化 · 垃圾代码 · 数字重写 · 全程序加密
 *    VM     字符串封存（--vm-seal-strings，v15 默认开）
 *    固定   防篡改 · 反调试环境守卫 · executor 认证绑定 · 环境检查组
 *           tier-1+tier-2 · 环境绑定 roblox（五项运行时保护全开，非选项）
 *    水印   产物首行固定英文水印（引擎行为，非选项）
 *
 *  引擎来源（自动探测）：
 *   · 本地 Rust 后端（web/tools/engine-server.mjs 包装 obsucvim CLI，
 *     开发时由 Vite 代理 /api/*）——真实引擎
 *   · 后端不可达时回退到占位 Mock（界面仍可演示）
 *
 *  未来接 WASM（wasm-bindgen / wasm-pack 产物）时替换 httpObfuscate 即可：
 *    import init, { obfuscate } from "../wasm/obsucvim_core.js";
 *    await init();
 *    const out = obfuscate(source, JSON.stringify(options)); // 返回 JSON 字符串
 *
 *  约束（与引擎对齐）：
 *   · 仅支持 Luau（5.1 自 2026-09-10 起停止支持），目标固定
 *   · 一次性返回最终结果（不做流式进度）
 *   · 源文件上限 1 MB，超出直接拒绝
 *   · 超时不重试，直接抛错
 * ============================================================================
 */

export interface ObfuscationOptions {
  /* ---- 变换开关（对应 CLI --no-* 旗标，默认全开）---- */
  /** L1 单行压缩（--minify / --no-minify） */
  minify: boolean;
  /** L1 标识符改名（--no-mangle） */
  mangle: boolean;
  /** L2 字符串加密（--no-strings） */
  strings: boolean;
  /** L3 控制流平坦化（--no-flatten） */
  flatten: boolean;
  /** L3 垃圾代码注入（--no-junk） */
  junk: boolean;
  /** L4 数字字面量重写（--no-numbers） */
  numbers: boolean;
  /** L5 全程序加密（--no-body） */
  body: boolean;
  /* ---- VM 自定义 ---- */
  /** VM 字符串封存（--vm-seal-strings / --no-vm-seal-strings，v15 默认开） */
  vmSealStrings: boolean;
  /* ---- 元信息 ---- */
  seed: string;
}

export const DEFAULT_OPTIONS: ObfuscationOptions = {
  minify: true,
  mangle: true,
  strings: true,
  flatten: true,
  junk: true,
  numbers: true,
  body: true,
  vmSealStrings: true,
  seed: "",
};

/**
 * 把 UI 选项映射成 obsucvim CLI 参数（与引擎 help 的旗标面一一对应；
 * 后端 /api/obfuscate 使用同一映射）。产品形态固定 v15。
 */
export function buildCliArgs(o: ObfuscationOptions): string[] {
  const args: string[] = ["--preset", "v15", "--env-check", "--bind-executor", "--bind-env", "roblox"];
  if (o.seed) args.push("--seed", o.seed);
  args.push(o.minify ? "--minify" : "--no-minify");
  if (!o.mangle) args.push("--no-mangle");
  if (!o.strings) args.push("--no-strings");
  if (!o.flatten) args.push("--no-flatten");
  if (!o.junk) args.push("--no-junk");
  if (!o.numbers) args.push("--no-numbers");
  if (!o.body) args.push("--no-body");
  args.push(o.vmSealStrings ? "--vm-seal-strings" : "--no-vm-seal-strings");
  return args;
}

export interface LogLine {
  level: "info" | "ok" | "warn" | "step";
  text: string;
}

export interface ObfuscationStats {
  inputBytes: number;
  outputBytes: number;
  ratio: number;
  elapsedMs: number;
  lines: number;
  seeded: string;
}

export interface QuotaInfo {
  usedToday: number;
  remaining: number;
  limit: number;
}

export interface ObfuscationResult {
  code: string;
  log: LogLine[];
  stats: ObfuscationStats;
  /** 服务端裁决后的当日额度（真实引擎路径携带） */
  quota?: QuotaInfo;
}

export interface ObfuscatorEngine {
  name: string;
  runtime: "wasm" | "worker" | "http";
  /** 一次性返回最终结果 */
  obfuscate(source: string, options: ObfuscationOptions): Promise<ObfuscationResult>;
}

import { authHeaders, clearToken } from "./token";

/* ------------------------------- 约束常量 ------------------------------- */
export const MAX_SOURCE_BYTES = 1024 * 1024; // 1 MB
export const ENGINE_TIMEOUT_MS = 60_000; // 超时直接失败，不重试
export const DAILY_FREE_QUOTA = 10; // 每日免费额度（次）

/** 引擎固定的产物首行水印（非选项） */
export const WATERMARK_LINE = "-- This file was obfuscated by Obsucvim.";

/* --------------------------------------------------------------------------- */
export function appliedNames(o: ObfuscationOptions): string[] {
  const list: string[] = [
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
  if (o.vmSealStrings) list.push("VM 字符串封存");
  if (o.minify) list.push("单行压缩");
  return list;
}

/* ========================= 真实引擎：本地 Rust 后端 ========================= */
const API_TIMEOUT_MS = ENGINE_TIMEOUT_MS + 5_000;

async function httpObfuscate(source: string, options: ObfuscationOptions): Promise<ObfuscationResult> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), API_TIMEOUT_MS);
  try {
    const res = await fetch("/api/obfuscate", {
      method: "POST",
      headers: { "content-type": "application/json", ...authHeaders() },
      body: JSON.stringify({ source, options }),
      signal: ctrl.signal,
    });
    if (res.status === 401) {
      clearToken(); // 会话失效：清除本地令牌，界面回到登录页
      throw new Error("UNAUTHORIZED");
    }
    const payload = (await res.json()) as
      | { ok: true; result: ObfuscationResult; quota?: QuotaInfo }
      | { ok: false; error: string; detail?: string; quota?: QuotaInfo };
    if (!res.ok || !payload.ok) {
      const err = payload && "error" in payload ? payload.error : `HTTP_${res.status}`;
      const detail = payload && "detail" in payload ? payload.detail : "";
      throw new Error(detail ? `${err}:${String(detail).slice(0, 160)}` : err);
    }
    if (payload.quota) payload.result.quota = payload.quota;
    return payload.result;
  } finally {
    clearTimeout(timer);
  }
}

/* ============================ 占位 Mock 引擎 ============================ */
const KEYWORDS = new Set([
  "and","break","do","else","elseif","end","false","for","function","goto","if","in","local",
  "nil","not","or","repeat","return","then","true","until","while","self","continue","export","type",
]);

const GLOBALS = new Set([
  "print","pairs","ipairs","tostring","tonumber","type","typeof","table","string","math","os","io",
  "coroutine","pcall","xpcall","error","assert","select","rawget","rawset","rawequal","setmetatable",
  "getmetatable","next","unpack","require","game","workspace","script","Instance","Vector3","CFrame",
  "UDim2","task","wait","spawn","Enum","Color3","tick","loadstring","getfenv","setfenv","_G","_ENV",
  "buffer","utf8","bit32","bits","os","string","debug",
]);

const ALPHABET = "IlO0oQ1";

function mulberry(seedStr: string) {
  let h = 2166136261 >>> 0;
  const s = seedStr || Math.random().toString(36).slice(2);
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619) >>> 0;
  return function next() {
    h += 0x6d2b79f5;
    let t = h;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function shortName(next: () => number) {
  let out = "";
  for (let i = 0; i < 4 + Math.floor(next() * 4); i++) out += ALPHABET[Math.floor(next() * ALPHABET.length)];
  return /^[0-9]/.test(out) ? `l${out.slice(1)}` : out;
}

function renameIdentifiers(code: string, next: () => number) {
  const seen = new Map<string, string>();
  const holes: string[] = [];
  const shielded = code
    .replace(/--\[\[[\s\S]*?\]\]/g, (m) => `\u0000${holes.push(m) - 1}\u0000`)
    .replace(/(["'])(?:\\.|(?!\1)[^\\\n])*\1/g, (m) => `\u0000${holes.push(m) - 1}\u0000`);

  const renamed = shielded.replace(/\b[A-Za-z_]\w*\b/g, (word) => {
    if (KEYWORDS.has(word) || GLOBALS.has(word)) return word;
    if (!seen.has(word)) seen.set(word, shortName(next));
    return seen.get(word)!;
  });
  return renamed.replace(/\u0000(\d+)\u0000/g, (_m, i: string) => holes[Number(i)]);
}

function encodeStrings(code: string, next: () => number) {
  return code.replace(/(["'])((?:\\.|(?!\1)[^\\\n]){1,240})\1/g, (m, _q, body: string) => {
    if (body.includes("\\") || /^\s*$/.test(body)) return m;
    const esc = body
      .split("")
      .map((c) => `\\${c.charCodeAt(0)}`)
      .join("");
    return next() > 0.5 ? `"${esc}"` : `'${esc}'`;
  });
}

function hexNumbers(code: string) {
  return code.replace(/\b(\d{3,})\b/g, (m) => `0x${Number(m).toString(16)}`);
}

export const mockEngine: ObfuscatorEngine = {
  name: "Obsucvim Core · 占位 (Rust 后端未连接)",
  runtime: "wasm",
  async obfuscate(source, options) {
    const t0 = performance.now();

    // 约束：源文件上限
    const inputBytes = new TextEncoder().encode(source).length;
    if (inputBytes > MAX_SOURCE_BYTES) {
      throw new Error(`SOURCE_TOO_LARGE:${inputBytes}`);
    }

    const run = async (): Promise<ObfuscationResult> => {
      const seed = options.seed || Math.random().toString(36).slice(2, 10).toUpperCase();
      const next = mulberry(seed);
      const log: LogLine[] = [];
      log.push({ level: "step", text: `[1/4] Luau 词法/语法分析 · ${inputBytes} B` });
      log.push({
        level: "info",
        text: `目标运行时 Luau 固定 · v15 产品形态 · 种子 ${seed} · CLI: ${buildCliArgs(options).join(" ")}`,
      });

      let out = source;
      if (options.mangle) out = renameIdentifiers(out, next);
      if (options.strings) out = encodeStrings(out, next);
      if (options.numbers) out = hexNumbers(out);
      if (options.flatten) log.push({ level: "info", text: "控制流平坦化：状态机调度" });
      if (options.junk) log.push({ level: "info", text: "垃圾代码注入：死分支 + 不变量噪声" });
      if (options.body) log.push({ level: "info", text: "L5 全程序加密：载荷整体封存" });
      if (options.vmSealStrings) log.push({ level: "info", text: "VM 字符串封存：编译前封存源字符串字面量" });
      log.push({ level: "info", text: "防篡改：完整性校验 lane（固定开启）" });
      log.push({ level: "info", text: "反调试环境守卫前导（固定开启）" });
      log.push({ level: "info", text: "v15 管线：模块表 + CPS 引导装配" });
      log.push({ level: "info", text: "executor 认证绑定：EXG 守卫令牌折入钥匙流（默认底层保护）" });
      log.push({ level: "info", text: "环境检查组：tier-1+tier-2 常量挑战，差值静默投毒" });
      log.push({ level: "info", text: "环境绑定：Roblox API（Vector3/task）嵌进原语槽" });

      log.push({ level: "step", text: "[2/4] 代码生成" });
      out = out
        .split("\n")
        .filter((l) => !/^\s*--(?!\[\[)/.test(l))
        .join("\n")
        .replace(/\n{3,}/g, "\n\n")
        .trim();

      const header = [
        "--[[ Obsucvim · Luau obfuscated output",
        `   engine  : obsucvim-core (placeholder, rust backend not connected)`,
        `   preset  : v15 (product form)   seed: ${seed}`,
        `   cli     : obsucvim ${buildCliArgs(options).join(" ")} in.lua out.lua`,
        `   applied : ${appliedNames(options).join(" · ") || "-"}`,
        "   do not modify: integrity checked",
        "]]",
      ].join("\n");

      const final = `${WATERMARK_LINE}\n${header}\n${out}`;
      log.push({ level: "step", text: "[3/4] 完整性校验注入 / 压缩" });
      log.push({ level: "ok", text: `[4/4] 完成 · 已应用 ${appliedNames(options).length} 项保护` });
      log.push({ level: "warn", text: "占位引擎：请启动本地 Rust 后端（web/tools/engine-server.mjs）或接入 obsucvim-core（Rust/WASM）" });

      const outputBytes = new TextEncoder().encode(final).length;
      return {
        code: final,
        log,
        stats: {
          inputBytes,
          outputBytes,
          ratio: outputBytes / Math.max(inputBytes, 1),
          elapsedMs: Math.round(performance.now() - t0),
          lines: final.split("\n").length,
          seeded: seed,
        },
      };
    };

    // 超时直接失败（不重试）
    return await Promise.race([
      run(),
      new Promise<ObfuscationResult>((_, reject) =>
        setTimeout(() => reject(new Error("ENGINE_TIMEOUT")), ENGINE_TIMEOUT_MS),
      ),
    ]);
  },
};

/* ================== 自动引擎：真实后端优先，失联回退 Mock ================== */
let backendOnline: boolean | null = null;

async function probeBackend(): Promise<boolean> {
  if (backendOnline !== null) return backendOnline;
  try {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 2500);
    const res = await fetch("/api/health", { signal: ctrl.signal });
    clearTimeout(timer);
    backendOnline = res.ok;
  } catch {
    backendOnline = false;
  }
  return backendOnline;
}

export const ENGINE: ObfuscatorEngine = {
  name: "Obsucvim Core (Rust · 自动探测)",
  runtime: "http",
  async obfuscate(source, options) {
    if (await probeBackend()) {
      try {
        return await httpObfuscate(source, options);
      } catch (e) {
        // 网络层失败（后端掉线）→ 回退 Mock 并明确标注；业务错误照常抛出
        if (e instanceof TypeError) {
          backendOnline = false;
          const r = await mockEngine.obfuscate(source, options);
          r.log.unshift({ level: "warn", text: "Rust 后端失联，已回退占位引擎（本次输出非真实混淆）" });
          return r;
        }
        throw e;
      }
    }
    const r = await mockEngine.obfuscate(source, options);
    r.log.unshift({ level: "warn", text: "Rust 后端未连接，当前为占位引擎输出（非真实混淆）" });
    return r;
  },
};

export async function runObfuscation(source: string, options: ObfuscationOptions) {
  return ENGINE.obfuscate(source, options);
}

/** 手动重置后端探测（连接恢复后调用） */
export function resetEngineProbe() {
  backendOnline = null;
}

export const SAMPLE_LUA = `--!strict
-- 示例：一个带校验的简易授权模块（Luau）
local LicenseCore = {}
local SIGNATURE = "obsucvim_2026_signature"

local function verify(x: string, y: string, salt: string): boolean
	local digest = 0
	for i = 1, #x do
		digest = (digest * 31 + string.byte(x, i, i)) % 2147483647
	end
	return digest == tonumber(y) and salt == SIGNATURE
end

function LicenseCore.check(key: string): (boolean, string)
	local parts: { string } = {}
	for segment in key:gmatch("[^-]+") do
		table.insert(parts, segment)
	end
	if #parts ~= 3 then
		return false, "invalid format"
	end
	return verify(parts[1], parts[2], parts[3]), "ok"
end

return LicenseCore
`;
