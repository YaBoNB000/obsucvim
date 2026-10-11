/**
 * ============================================================================
 *  Obsucvim 引擎接入层  (src/lib/engine.ts)
 * ----------------------------------------------------------------------------
 *  你的引擎：Rust 编译出的 WASM，直接在浏览器里调用（无后端、无 API）。
 *  接入方式（wasm-bindgen / wasm-pack 产物）：
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
 *  约束（与你的引擎对齐）：
 *   · 仅支持 Luau，目标固定，无多运行时切换
 *   · 一次性返回最终结果（不做流式进度）
 *   · 源文件上限 1 MB，超出直接拒绝
 *   · 超时不重试，直接抛错
 * ============================================================================
 */

export type PresetId = "light" | "balanced" | "extreme";
/** VM 虚拟化范围 */
export type VmMode = "off" | "function" | "script";
/** VM 指令分发方式 */
export type DispatchStyle = "switch" | "table" | "goto";

export interface VmOptions {
  /** 虚拟化范围：关闭 / 只虚拟化函数 / 全脚本字节码 */
  mode: VmMode;
  /** 指令集洗牌（每次出包 opcode 顺序不同） */
  opcodeShuffle: boolean;
  /** 字节码 + 常量池加密 */
  bytecodeEncryption: boolean;
  /** 指令分发方式 */
  dispatchStyle: DispatchStyle;
  /** 寄存器分配策略：寄存器机 / 栈机 */
  registerMachine: boolean;
}

export interface ObfuscationOptions {
  preset: PresetId;
  /* 变换 */
  renameIdentifiers: boolean;
  stringEncryption: boolean;
  controlFlowFlattening: boolean;
  constantPool: boolean;
  numbersToExpressions: boolean;
  /* VM 自定义 */
  vm: VmOptions;
  /* 运行时防护 */
  antiDebug: boolean;
  antiTamper: boolean;
  /* 可追溯 */
  seed: string;
  watermark: string;
}

export const DEFAULT_OPTIONS: ObfuscationOptions = {
  preset: "balanced",
  renameIdentifiers: true,
  stringEncryption: true,
  controlFlowFlattening: true,
  constantPool: true,
  numbersToExpressions: false,
  vm: {
    mode: "off",
    opcodeShuffle: true,
    bytecodeEncryption: true,
    dispatchStyle: "switch",
    registerMachine: true,
  },
  antiDebug: false,
  antiTamper: false,
  seed: "",
  watermark: "",
};

type PresetPatch = { label: string; hint: string } & Partial<
  Omit<ObfuscationOptions, "preset" | "vm" | "seed" | "watermark">
> & { vmMode?: VmMode };

export const PRESETS: Record<PresetId, PresetPatch> = {
  light: {
    label: "轻量",
    hint: "改名 + 字符串加密，体积与兼容性最优",
    renameIdentifiers: true,
    stringEncryption: true,
    controlFlowFlattening: false,
    constantPool: false,
    numbersToExpressions: false,
    antiDebug: false,
    antiTamper: false,
    vmMode: "off",
  },
  balanced: {
    label: "标准",
    hint: "改名 + 字符串加密 + 控制流平坦化 + 常量池",
    renameIdentifiers: true,
    stringEncryption: true,
    controlFlowFlattening: true,
    constantPool: true,
    numbersToExpressions: false,
    antiDebug: false,
    antiTamper: false,
    vmMode: "off",
  },
  extreme: {
    label: "极限",
    hint: "标准档 + VM 函数级虚拟化 + 数字表达式化 + 反调试防篡改",
    renameIdentifiers: true,
    stringEncryption: true,
    controlFlowFlattening: true,
    constantPool: true,
    numbersToExpressions: true,
    antiDebug: true,
    antiTamper: true,
    vmMode: "function",
  },
};

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

/* ---------------------------------------------------------------------------
 *  保护项名称（用于产物头部与统计）
 * ------------------------------------------------------------------------- */
export function appliedNames(o: ObfuscationOptions): string[] {
  const list: string[] = [];
  if (o.vm.mode === "script") list.push("VM 全脚本虚拟化");
  else if (o.vm.mode === "function") list.push("VM 函数虚拟化");
  if (o.vm.opcodeShuffle) list.push("指令集洗牌");
  if (o.vm.bytecodeEncryption) list.push("字节码加密");
  if (o.controlFlowFlattening) list.push("控制流平坦化");
  if (o.stringEncryption) list.push("字符串加密");
  if (o.constantPool) list.push("常量池混淆");
  if (o.renameIdentifiers) list.push("标识符重命名");
  if (o.numbersToExpressions) list.push("数字表达式化");
  if (o.antiDebug) list.push("反调试");
  if (o.antiTamper) list.push("防篡改校验");
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

function vmPrelude(o: ObfuscationOptions, seed: string, next: () => number) {
  const ops = Array.from({ length: 8 }, () => shortName(next));
  const shuffled = o.vm.opcodeShuffle ? [...ops].sort(() => next() - 0.5) : ops;
  const dispatch =
    o.vm.dispatchStyle === "switch"
      ? "while true do local ins = bc[pc] if ins == " + shuffled[0] + " then pc = ins + 1 elseif ins == 0 then break end end"
      : o.vm.dispatchStyle === "table"
        ? "while bc[pc] do local h = handlers[bc[pc]] pc = h(pc, reg) end"
        : "::vm_loop:: if bc[pc] == 0 then goto vm_end end pc = bc[pc] goto vm_loop ::vm_end::";
  return [
    `-- VM: mode=${o.vm.mode} dispatch=${o.vm.dispatchStyle} regs=${o.vm.registerMachine ? "register" : "stack"}`,
    `local ${shuffled[0]} = ${o.vm.registerMachine ? "{}" : "{}"} -- 寄存器/栈帧`,
    `local ${shuffled[1]} = "${seed}" -- 字节码校验种子`,
    `local function ${shuffled[2]}(bc, handlers, reg, pc)`,
    `  ${dispatch}`,
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
      log.push({ level: "step", text: `[1/4] Luau 词法/语法分析 · ${inputBytes} B` });
      log.push({ level: "info", text: `目标运行时 Luau 固定 · 预设「${PRESETS[options.preset].label}」· 种子 ${seed}` });

      let out = source;
      if (options.renameIdentifiers) out = renameIdentifiers(out, next);
      if (options.stringEncryption) out = encodeStrings(out, next);
      if (options.constantPool) log.push({ level: "info", text: "常量池：字符串/数字并入常量表，按索引取值" });
      if (options.controlFlowFlattening) log.push({ level: "info", text: "控制流平坦化：状态机调度" });
      if (options.numbersToExpressions) out = hexNumbers(out);
      if (options.antiDebug) log.push({ level: "info", text: "反调试：注入 debug hook / 环境指纹检测桩" });
      if (options.vm.mode !== "off") {
        out = `${vmPrelude(options, seed, next)}\n${out}`;
        log.push({
          level: "info",
          text: `VM 虚拟化：${options.vm.mode === "script" ? "全脚本字节码" : "函数级"} · 分发 ${options.vm.dispatchStyle} · 指令集${
            options.vm.opcodeShuffle ? "已洗牌" : "固定"
          } · 字节码${options.vm.bytecodeEncryption ? "已加密" : "明文"}`,
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
        `   preset  : ${options.preset}   seed: ${seed}   vm: ${options.vm.mode}/${options.vm.dispatchStyle}`,
        `   applied : ${appliedNames(options).join(" · ") || "-"}`,
        options.watermark ? `   watermark: ${options.watermark}` : "",
        "   do not modify: integrity checked",
        "]]",
      ]
        .filter(Boolean)
        .join("\n");

      const tamper = options.antiTamper
        ? `\nlocal ${shortName(next)} = function(s) local h = 5381 for i = 1, #s do h = (h * 33 + string.byte(s, i)) % 4294967296 end return h end\n`
        : "";

      const final = `${header}\n${out}${tamper}`;
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
