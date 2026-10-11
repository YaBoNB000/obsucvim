/**
 * ============================================================================
 *  Obsucvim 引擎接入层  (src/lib/engine.ts)
 * ----------------------------------------------------------------------------
 *  选项与 obsucvim CLI 一一对应。
 *  引擎来源（自动探测）：本地 Rust 后端 / 占位 Mock。
 * ============================================================================
 */

export interface ObfuscationOptions {
  minify: boolean;
  mangle: boolean;
  strings: boolean;
  flatten: boolean;
  junk: boolean;
  numbers: boolean;
  body: boolean;
  vmSealStrings: boolean;
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
  quota?: QuotaInfo;
}

export interface ObfuscatorEngine {
  name: string;
  runtime: "wasm" | "worker" | "http";
  obfuscate(source: string, options: ObfuscationOptions): Promise<ObfuscationResult>;
}

export const MAX_SOURCE_BYTES = 1024 * 1024;
export const ENGINE_TIMEOUT_MS = 60_000;
export const DAILY_FREE_QUOTA = 30;
export const WATERMARK_LINE = "-- This file was obfuscated by Obsucvim.";

export function appliedNames(o: ObfuscationOptions): string[] {
  const list: string[] = ["Watermark", "Executor Binding", "Environment Check", "Environment Binding", "Anti-Tamper", "Anti-Debug Guard"];
  if (o.mangle) list.push("Identifier Renaming");
  if (o.strings) list.push("String Encryption");
  if (o.flatten) list.push("Control Flow Protection");
  if (o.junk) list.push("Junk Code Injection");
  if (o.numbers) list.push("Number Obfuscation");
  if (o.body) list.push("Whole-Program Encryption");
  if (o.vmSealStrings) list.push("VM String Sealing");
  if (o.minify) list.push("Minification");
  return list;
}

import { authHeaders, clearToken } from "./token";

async function httpObfuscate(source: string, options: ObfuscationOptions): Promise<ObfuscationResult> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), ENGINE_TIMEOUT_MS + 5_000);
  try {
    const res = await fetch("/api/obfuscate", {
      method: "POST",
      headers: { "content-type": "application/json", ...authHeaders() },
      body: JSON.stringify({ source, options }),
      signal: ctrl.signal,
    });
    if (res.status === 401) {
      clearToken();
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
    const esc = body.split("").map((c) => `\\${c.charCodeAt(0)}`).join("");
    return next() > 0.5 ? `"${esc}"` : `'${esc}'`;
  });
}

function hexNumbers(code: string) {
  return code.replace(/\b(\d{3,})\b/g, (m) => `0x${Number(m).toString(16)}`);
}

export const mockEngine: ObfuscatorEngine = {
  name: "Obsucvim Core · 占位",
  runtime: "wasm",
  async obfuscate(source, options) {
    const t0 = performance.now();
    const inputBytes = new TextEncoder().encode(source).length;
    if (inputBytes > MAX_SOURCE_BYTES) throw new Error(`SOURCE_TOO_LARGE:${inputBytes}`);

    const seed = options.seed || Math.random().toString(36).slice(2, 10).toUpperCase();
    const next = mulberry(seed);
    const log: LogLine[] = [];
    log.push({ level: "step", text: `[1/4] Parsing · ${inputBytes} B` });
    log.push({ level: "info", text: `Luau · seed ${seed}` });

    let out = source;
    if (options.mangle) out = renameIdentifiers(out, next);
    if (options.strings) out = encodeStrings(out, next);
    if (options.numbers) out = hexNumbers(out);
    log.push({ level: "step", text: "[2/4] Code generation" });
    out = out.split("\n").filter((l) => !/^\s*--(?!\[\[)/.test(l)).join("\n").replace(/\n{3,}/g, "\n\n").trim();
    const final = `${WATERMARK_LINE}\n${out}`;
    log.push({ level: "step", text: "[3/4] Integrity injection" });
    log.push({ level: "ok", text: `[4/4] Done · ${appliedNames(options).length} protections applied` });
    log.push({ level: "warn", text: "Placeholder engine — connect the Rust backend for real obfuscation" });

    const outputBytes = new TextEncoder().encode(final).length;
    return {
      code: final,
      log,
      stats: { inputBytes, outputBytes, ratio: outputBytes / Math.max(inputBytes, 1), elapsedMs: Math.round(performance.now() - t0), lines: final.split("\n").length, seeded: seed },
    };
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
  name: "Obsucvim Core",
  runtime: "http",
  async obfuscate(source, options) {
    if (await probeBackend()) {
      try {
        return await httpObfuscate(source, options);
      } catch (e) {
        if (e instanceof TypeError) {
          backendOnline = false;
          const r = await mockEngine.obfuscate(source, options);
          r.log.unshift({ level: "warn", text: "Backend unreachable — placeholder engine output (not real obfuscation)" });
          return r;
        }
        throw e;
      }
    }
    const r = await mockEngine.obfuscate(source, options);
    r.log.unshift({ level: "warn", text: "Backend not connected — placeholder engine output (not real obfuscation)" });
    return r;
  },
};

export async function runObfuscation(source: string, options: ObfuscationOptions) {
  return ENGINE.obfuscate(source, options);
}

export function resetEngineProbe() {
  backendOnline = null;
}

export const SAMPLE_LUA = `--!strict
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
