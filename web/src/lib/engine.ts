/**
 * ============================================================================
 *  Obsucvim 引擎接入层  (src/lib/engine.ts)
 * ----------------------------------------------------------------------------
 *  选项模型与 obsucvim CLI（Rust 引擎）完全对齐：
 *    预设   v15（产品默认）| low | medium | high | vm | max（legacy）
 *    变换   L1 改名/压缩 · L2 字符串加密 · L3 平坦化/垃圾代码 · L4 数字重写
 *           L5 全程序加密 · L7 防篡改 · 反调试环境守卫 · VM 字符串封存（实验）
 *    保护   环境检查组 tier-1+tier-2（--env-check）· executor 认证绑定
 *           （--bind-executor，默认开启的底层保护层）· 环境绑定 roblox
 *    水印   产物首行固定英文水印（引擎行为，非选项）
 *
 *  接入真实引擎（wasm-bindgen / wasm-pack 产物）：
 *
 *    import init, { obfuscate } from "../wasm/obsucvim_core.js";
 *    let ready: Promise<void> | null = null;
 *    async function load() { if (!ready) ready = init(); return ready; }
 *
 *    export const ENGINE: ObfuscatorEngine = {
 *      name: "Obsucvim Core (Rust → WASM)",
 *      runtime: "wasm",
 *      async obfuscate(source, options) {          // 一次性返回，无流式
 *        await load();
 *        const out = obfuscate(source, JSON.stringify(options)); // 返回 JSON 字符串
 *        const parsed = JSON.parse(out) as { code: string; log?: LogLine[] };
 *        return { code: parsed.code, log: parsed.log ?? defaultLog(options, parsed.code) };
 *      },
 *    };
 *
 *  约束（与引擎对齐）：
 *   · 仅支持 Luau（5.1 自 2026-09-10 起停止支持），目标固定
 *   · 一次性返回最终结果（不做流式进度）
 *   · 源文件上限 1 MB，超出直接拒绝
 *   · 超时不重试，直接抛错
 * ============================================================================
 */

/** 与 CLI --preset 一致；v15 = 产品默认形态（Luraph v15 结构克隆） */
export type PresetId = "v15" | "low" | "medium" | "high" | "vm" | "max";

export const PRESET_IDS: PresetId[] = ["v15", "low", "medium", "high", "vm", "max"];

export interface ObfuscationOptions {
  preset: PresetId;
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
  /** L7 防篡改（--no-antidbg） */
  antidbg: boolean;
  /** 反调试环境守卫前导（--no-guard） */
  guard: boolean;
  /** VM 字符串封存 · 实验（--vm-seal-strings，VM 预设默认开） */
  vmSealStrings: boolean;
  /* ---- v15 运行时保护（仅 v15 管线；legacy 预设自动降级）---- */
  /** 环境检查组 tier-1+tier-2（--env-check） */
  envCheck: boolean;
  /** executor 认证绑定（--bind-executor，默认开启；--no-bind-executor 退出） */
  bindExecutor: boolean;
  /** 环境绑定：Roblox API 原语槽（--bind-env roblox） */
  bindEnv: boolean;
  /* ---- 元信息 ---- */
  seed: string;
}

export const DEFAULT_OPTIONS: ObfuscationOptions = {
  preset: "v15",
  minify: true,
  mangle: true,
  strings: true,
  flatten: true,
  junk: true,
  numbers: true,
  body: true,
  antidbg: true,
  guard: true,
  vmSealStrings: true,
  envCheck: false,
  bindExecutor: true,
  bindEnv: false,
  seed: "",
};

type PresetPatch = { label: string; hint: string } & Partial<ObfuscationOptions>;

/** 与 CLI apply_preset 的档位语义一致 */
export const PRESETS: Record<PresetId, PresetPatch> = {
  v15: {
    label: "v15",
    hint: "产品默认形态：Luraph v15 结构克隆（Luau/Roblox 专用，模块表 + CPS 引导）；executor 认证绑定默认开启",
    minify: true,
    mangle: true,
    strings: true,
    flatten: true,
    junk: true,
    numbers: true,
    body: true,
    antidbg: true,
    guard: true,
    bindExecutor: true,
  },
  low: {
    label: "low",
    hint: "legacy 轻量：L1+L2（改名 + 压缩 + 字符串加密）",
    minify: true,
    mangle: true,
    strings: true,
    flatten: false,
    junk: false,
    numbers: false,
    body: false,
    antidbg: false,
    guard: false,
    envCheck: false,
    bindExecutor: false,
    bindEnv: false,
  },
  medium: {
    label: "medium",
    hint: "legacy 中等：low + L3（控制流平坦化 + 垃圾代码）",
    minify: true,
    mangle: true,
    strings: true,
    flatten: true,
    junk: true,
    numbers: false,
    body: false,
    antidbg: false,
    guard: false,
    envCheck: false,
    bindExecutor: false,
    bindEnv: false,
  },
  high: {
    label: "high",
    hint: "legacy 高强度：medium + L4 数字重写 + L5 全程序加密 + L7 防篡改",
    minify: true,
    mangle: true,
    strings: true,
    flatten: true,
    junk: true,
    numbers: true,
    body: true,
    antidbg: true,
    guard: true,
    envCheck: false,
    bindExecutor: false,
    bindEnv: false,
  },
  vm: {
    label: "vm",
    hint: "legacy VM：high + L6 私有字节码虚拟机（字符串封存实验默认开）",
    minify: true,
    mangle: true,
    strings: true,
    flatten: true,
    junk: true,
    numbers: true,
    body: true,
    antidbg: true,
    guard: true,
    vmSealStrings: true,
    envCheck: false,
    bindExecutor: false,
    bindEnv: false,
  },
  max: {
    label: "max",
    hint: "legacy 最强出货档（= vm）",
    minify: true,
    mangle: true,
    strings: true,
    flatten: true,
    junk: true,
    numbers: true,
    body: true,
    antidbg: true,
    guard: true,
    vmSealStrings: true,
    envCheck: false,
    bindExecutor: false,
    bindEnv: false,
  },
};

/**
 * 把 UI 选项映射成 obsucvim CLI 参数（接入后端 / WASM 时直接复用）。
 * 与引擎 help 的旗标面一一对应。
 */
export function buildCliArgs(o: ObfuscationOptions): string[] {
  const args: string[] = ["--preset", o.preset];
  if (o.seed) args.push("--seed", o.seed);
  args.push(o.minify ? "--minify" : "--no-minify");
  if (!o.mangle) args.push("--no-mangle");
  if (!o.strings) args.push("--no-strings");
  if (!o.flatten) args.push("--no-flatten");
  if (!o.junk) args.push("--no-junk");
  if (!o.numbers) args.push("--no-numbers");
  if (!o.body) args.push("--no-body");
  if (!o.antidbg) args.push("--no-antidbg");
  if (!o.guard) args.push("--no-guard");
  if (o.preset === "vm" || o.preset === "max") {
    args.push(o.vmSealStrings ? "--vm-seal-strings" : "--no-vm-seal-strings");
  }
  if (o.preset === "v15") {
    if (o.envCheck) args.push("--env-check");
    args.push(o.bindExecutor ? "--bind-executor" : "--no-bind-executor");
    if (o.bindEnv) args.push("--bind-env", "roblox");
  }
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

export interface ObfuscationResult {
  code: string;
  log: LogLine[];
  stats: ObfuscationStats;
}

export interface ObfuscatorEngine {
  name: string;
  runtime: "wasm" | "worker";
  /** 一次性返回最终结果 */
  obfuscate(source: string, options: ObfuscationOptions): Promise<ObfuscationResult>;
}

/* ------------------------------- 约束常量 ------------------------------- */
export const MAX_SOURCE_BYTES = 1024 * 1024; // 1 MB
export const ENGINE_TIMEOUT_MS = 60_000; // 超时直接失败，不重试
export const DAILY_FREE_QUOTA = 10; // 每日免费额度（次）

/** 引擎固定的产物首行水印（非选项） */
export const WATERMARK_LINE = "-- This file was obfuscated by Obsucvim.";

/* --------------------------------------------------------------------------- */
export function appliedNames(o: ObfuscationOptions): string[] {
  const list: string[] = ["英文水印首行"];
  if (o.preset === "v15") {
    list.push("v15 结构克隆（模块表 + CPS 引导）");
    if (o.bindExecutor) list.push("executor 认证绑定");
    if (o.envCheck) list.push("环境检查组（tier-1 + tier-2）");
    if (o.bindEnv) list.push("环境绑定（Roblox）");
  }
  if (o.preset === "vm" || o.preset === "max") {
    list.push("私有字节码 VM (L6)");
    if (o.vmSealStrings) list.push("VM 字符串封存（实验）");
  }
  if (o.mangle) list.push("标识符重命名 (L1)");
  if (o.strings) list.push("字符串加密 (L2)");
  if (o.flatten) list.push("控制流平坦化 (L3)");
  if (o.junk) list.push("垃圾代码注入 (L3)");
  if (o.numbers) list.push("数字字面量重写 (L4)");
  if (o.body) list.push("全程序加密 (L5)");
  if (o.antidbg) list.push("防篡改 (L7)");
  if (o.guard) list.push("反调试环境守卫");
  if (o.minify) list.push("单行压缩 (L1)");
  return list;
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

function vmPrelude(seed: string, next: () => number) {
  const ops = Array.from({ length: 8 }, () => shortName(next));
  return [
    `-- VM: L6 private bytecode (legacy preset)`,
    `local ${ops[0]} = {}`,
    `local ${ops[1]} = "${seed}"`,
    `local function ${ops[2]}(bc, handlers, reg, pc)`,
    `  while true do local ins = bc[pc] if ins == ${ops[3]} then pc = ins + 1 elseif ins == 0 then break end end`,
    `end`,
  ].join("\n");
}

export const mockEngine: ObfuscatorEngine = {
  name: "Obsucvim Core · 占位 (Rust → WASM 未接入)",
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
      const isV15 = options.preset === "v15";
      const isVm = options.preset === "vm" || options.preset === "max";
      log.push({ level: "step", text: `[1/4] Luau 词法/语法分析 · ${inputBytes} B` });
      log.push({
        level: "info",
        text: `目标运行时 Luau 固定 · 预设「${PRESETS[options.preset].label}」· 种子 ${seed} · CLI: ${buildCliArgs(options).join(" ")}`,
      });

      let out = source;
      if (options.mangle) out = renameIdentifiers(out, next);
      if (options.strings) out = encodeStrings(out, next);
      if (options.numbers) out = hexNumbers(out);
      if (options.flatten) log.push({ level: "info", text: "控制流平坦化：状态机调度" });
      if (options.junk) log.push({ level: "info", text: "垃圾代码注入：死分支 + 不变量噪声" });
      if (options.body) log.push({ level: "info", text: "L5 全程序加密：载荷整体封存" });
      if (options.antidbg) log.push({ level: "info", text: "L7 防篡改：完整性校验 lane" });
      if (options.guard) log.push({ level: "info", text: "反调试环境守卫前导" });
      if (isV15) {
        log.push({ level: "info", text: "v15 管线：模块表 + CPS 引导装配" });
        if (options.bindExecutor) log.push({ level: "info", text: "executor 认证绑定：EXG 守卫令牌折入钥匙流（默认底层保护）" });
        if (options.envCheck) log.push({ level: "info", text: "环境检查组：tier-1+tier-2 常量挑战，差值静默投毒" });
        if (options.bindEnv) log.push({ level: "info", text: "环境绑定：Roblox API（Vector3/task）嵌进原语槽" });
      }
      if (isVm) {
        out = `${vmPrelude(seed, next)}\n${out}`;
        log.push({
          level: "info",
          text: `L6 私有字节码 VM · 字符串封存${options.vmSealStrings ? "开启（实验）" : "关闭"}`,
        });
      }

      log.push({ level: "step", text: "[2/4] 代码生成" });
      out = out
        .split("\n")
        .filter((l) => !/^\s*--(?!\[\[)/.test(l))
        .join("\n")
        .replace(/\n{3,}/g, "\n\n")
        .trim();

      const header = [
        "--[[ Obsucvim · Luau obfuscated output",
        `   engine  : obsucvim-core (placeholder, rust→wasm not wired)`,
        `   preset  : ${options.preset}   seed: ${seed}`,
        `   cli     : obsucvim ${buildCliArgs(options).join(" ")} in.lua out.lua`,
        `   applied : ${appliedNames(options).join(" · ") || "-"}`,
        "   do not modify: integrity checked",
        "]]",
      ].join("\n");

      const final = `${WATERMARK_LINE}\n${header}\n${out}`;
      log.push({ level: "step", text: "[3/4] 完整性校验注入 / 压缩" });
      log.push({ level: "ok", text: `[4/4] 完成 · 已应用 ${appliedNames(options).length} 项保护` });
      log.push({ level: "warn", text: "占位引擎：请在 src/lib/engine.ts 接入 obsucvim-core（Rust/WASM）" });

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

/** 把 ENGINE 换成你的 Rust/WASM 引擎即可，界面无需改动。 */
export const ENGINE: ObfuscatorEngine = mockEngine;

export async function runObfuscation(source: string, options: ObfuscationOptions) {
  return ENGINE.obfuscate(source, options);
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
