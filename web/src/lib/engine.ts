/**
 * ============================================================================
 *  Obsucvim 引擎接入层  (src/lib/engine.ts)
 * ----------------------------------------------------------------------------
 *  选项与 obsucvim CLI 一一对应。
 *  引擎来源：同源服务端 API（Rust/WASI 引擎），服务不可用时明确报错。
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
  /** 环境证明（env-check + bind-env + bind-executor），仅在确认执行器支持时开启。 */
  hardening?: boolean;
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
  hardening: false,
};

/**
 * 环境证明（--env-check / --bind-env / --bind-executor）默认关闭：
 * 它们会在运行时核验 Roblox/执行器的具体 API（debug.getconstants、getupvalues、
 * typeof(game)…），任何一项对不上不是跳过而是自我投毒，脚本会直接跑不起来。
 * 只有确认目标执行器支持时才打开。
 */
export function buildCliArgs(o: ObfuscationOptions): string[] {
  const args: string[] = ["--preset", "v15", "--env-check"];
  // 这两层要求宿主 API 形状完全一致，缺一项就会自我投毒，默认不开。
  // 引擎默认是绑定执行器，所以必须显式 --no-bind-executor。
  if (o.hardening) args.push("--bind-executor", "--bind-env", "roblox");
  else args.push("--no-bind-executor");
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
export const DAILY_FREE_QUOTA = 5;
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

import { clearLegacyAuthData } from "./token";

async function httpObfuscate(source: string, options: ObfuscationOptions): Promise<ObfuscationResult> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), ENGINE_TIMEOUT_MS + 5_000);
  try {
    const res = await fetch("/api/obfuscate", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ source, options }),
      signal: ctrl.signal,
    });
    if (res.status === 401) {
      clearLegacyAuthData();
      // 通知 AuthProvider 重置用户状态（双保险：Workspace 也会调 logout）
      window.dispatchEvent(new CustomEvent("obsucvim:unauthorized"));
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

/* 产品仅调用服务端混淆 API；移除占位 Mock，避免把非真实混淆结果交付用户。 */
export const ENGINE: ObfuscatorEngine = {
  name: "Obsucvim Core",
  runtime: "http",
  obfuscate: httpObfuscate,
};

export async function runObfuscation(source: string, options: ObfuscationOptions) {
  return ENGINE.obfuscate(source, options);
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
