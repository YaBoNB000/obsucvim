import { useI18n } from "../lib/i18n";
import { Button, GlassCard, Icon, Pill } from "../components/ui";
import { InlineCode } from "../components/CodeArea";
import { ENGINE, MAX_SOURCE_BYTES, ENGINE_TIMEOUT_MS, DAILY_FREE_QUOTA, PRESETS } from "../lib/engine";

const RUST_SNIPPET = `// src/lib.rs — obsucvim-core（选项模型与 obsucvim CLI 对齐）
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn obfuscate(source: &str, options_json: &str) -> Result<String, JsValue> {
    let opts: ObfuscateOptions = serde_json::from_str(options_json)
        .map_err(|e| JsValue::from_str(&format!("bad options: {e}")))?;

    // 1 MB 上限：超出直接拒绝，不返回部分结果
    if source.len() > 1024 * 1024 {
        return Err(JsValue::from_str("SOURCE_TOO_LARGE"));
    }

    // 与 CLI 等价：--preset v15（产品默认）| low..max（legacy）
    let mut cfg = obsucvim::Options::from_preset(opts.preset)?;
    if let Some(seed) = opts.seed() { cfg = cfg.with_seed(seed); }
    cfg.minify = opts.minify;          // --minify / --no-minify
    if !opts.mangle  { cfg = cfg.no_mangle(); }   // L1
    if !opts.strings { cfg = cfg.no_strings(); }  // L2
    if !opts.flatten { cfg = cfg.no_flatten(); }  // L3
    if !opts.junk    { cfg = cfg.no_junk(); }     // L3
    if !opts.numbers { cfg = cfg.no_numbers(); }  // L4
    if !opts.body    { cfg = cfg.no_body(); }     // L5
    if !opts.antidbg { cfg = cfg.no_antidbg(); }  // L7
    if !opts.guard   { cfg = cfg.no_guard(); }
    if opts.preset == "vm" || opts.preset == "max" {
        cfg = if opts.vm_seal_strings { cfg.vm_seal_strings() } else { cfg.no_vm_seal_strings() };
    }
    if opts.preset == "v15" {
        if opts.env_check     { cfg = cfg.env_check(); }    // tier-1+tier-2
        cfg = if opts.bind_executor { cfg.bind_executor() } else { cfg.no_bind_executor() }; // 默认开
        if opts.bind_env      { cfg = cfg.bind_env("roblox"); }
    }

    let code = obsucvim::obfuscate(source, &cfg)?;
    // 一次性返回：{ code, log } —— 没有流式进度；首行固定英文水印
    Ok(serde_json::to_string(&BuildOutput { code, log: cfg.log })?)
}`;

const TS_SNIPPET = `// src/lib/engine.ts —— 前端唯一需要改的地方
import init, { obfuscate } from "../wasm/obsucvim_core.js";

let ready: Promise<void> | null = null;
const load = () => (ready ??= init());

export const ENGINE: ObfuscatorEngine = {
  name: "Obsucvim Core (Rust → WASM)",
  runtime: "wasm",
  async obfuscate(source, options) {          // 一次性返回最终结果
    await load();
    const raw = obfuscate(source, JSON.stringify(options));
    const parsed = JSON.parse(raw) as { code: string; log?: LogLine[] };
    return {
      code: parsed.code,
      log: parsed.log ?? [],
      stats: buildStats(source, parsed.code),  // 体积 / 膨胀率 / 耗时在前端统计
    };
  },
};`;

const OPTIONS_JSON = `{
  "preset": "v15",            // v15（产品默认）| low | medium | high | vm | max
  "minify": true,             // L1 单行压缩        --minify / --no-minify
  "mangle": true,             // L1 标识符重命名     --no-mangle
  "strings": true,            // L2 字符串加密       --no-strings
  "flatten": true,            // L3 控制流平坦化     --no-flatten
  "junk": true,               // L3 垃圾代码注入     --no-junk
  "numbers": true,            // L4 数字字面量重写    --no-numbers
  "body": true,               // L5 全程序加密       --no-body
  "antidbg": true,            // L7 防篡改           --no-antidbg
  "guard": true,              // 反调试环境守卫前导   --no-guard
  "vmSealStrings": true,      // VM 字符串封存（实验，VM 预设）--vm-seal-strings
  "envCheck": false,          // 环境检查组 tier-1+tier-2（仅 v15）--env-check
  "bindExecutor": true,       // executor 认证绑定（仅 v15，默认开）--no-bind-executor 退绑
  "bindEnv": false,           // 环境绑定 roblox（仅 v15）--bind-env roblox
  "seed": "67067"             // 留空 = 时间种子
}`;

export function Docs() {
  const { t } = useI18n();

  const limits = [
    { k: t("docs.limitSource"), v: `${MAX_SOURCE_BYTES / 1024 / 1024} MB`, d: t("docs.limitSourceV") },
    { k: t("docs.limitReturn"), v: "once", d: t("docs.limitReturnV") },
    { k: t("docs.limitTimeout"), v: `${ENGINE_TIMEOUT_MS / 1000} s`, d: t("docs.limitTimeoutV") },
    { k: t("docs.limitApi"), v: "none", d: t("docs.limitApiV") },
  ];

  const vmRows = [
    ["preset", "v15 | low..max", PRESETS["v15"].hint],
    ["bindExecutor", "bool (default true)", t("ws.bindExecutorHint")],
    ["envCheck", "bool (default false)", t("ws.envCheckHint")],
    ["bindEnv", "bool (default false)", t("ws.bindEnvHint")],
    ["vmSealStrings", "bool (VM presets)", t("ws.vmSealHint")],
  ];

  return (
    <div className="grid items-start gap-5 xl:grid-cols-[minmax(0,1.35fr)_minmax(340px,1fr)]">
      <div className="grid min-w-0 content-start gap-5">
        <GlassCard className="p-6">
          <div className="mb-3 flex flex-wrap items-center gap-3">
            <Icon name="terminal" className="h-4 w-4 text-aqua-300" />
            <h2 className="text-[13.5px] font-semibold text-slate-100">{t("docs.contract")}</h2>
            <Pill className="ml-auto font-mono">
              {ENGINE.name} · {ENGINE.runtime}
            </Pill>
          </div>
          <p className="mb-4 text-[12px] leading-relaxed text-slate-400/80">{t("docs.contractDesc")}</p>
          <InlineCode code={TS_SNIPPET} lang="rust" label={t("docs.ts")} />
          <div className="mt-4">
            <InlineCode code={RUST_SNIPPET} lang="rust" label={t("docs.rust")} />
          </div>
        </GlassCard>

        <GlassCard className="p-6">
          <div className="mb-3 flex items-center gap-3">
            <Icon name="cpu" className="h-4 w-4 text-aqua-300" />
            <h2 className="text-[13.5px] font-semibold text-slate-100">{t("docs.vm")}</h2>
          </div>
          <p className="mb-4 text-[12px] leading-relaxed text-slate-400/80">{t("docs.vmDesc")}</p>
          <div className="overflow-hidden rounded-2xl border border-white/[0.07]">
            <table className="w-full border-collapse text-left">
              <tbody>
                {vmRows.map(([k, type, desc], i) => (
                  <tr key={k} className={i % 2 ? "bg-white/[0.015]" : ""}>
                    <td className="border-b border-white/[0.05] px-4 py-2.5 font-mono text-[11.5px] whitespace-nowrap text-aqua-100/90">
                      {k}
                    </td>
                    <td className="border-b border-white/[0.05] px-4 py-2.5 font-mono text-[11px] whitespace-nowrap text-slate-400">
                      {type}
                    </td>
                    <td className="border-b border-white/[0.05] px-4 py-2.5 text-[11.5px] leading-relaxed text-slate-400/85">
                      {desc}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </GlassCard>
      </div>

      <div className="grid content-start gap-5">
        <GlassCard className="p-6">
          <div className="mb-3 flex items-center gap-3">
            <Icon name="shield" className="h-4 w-4 text-aqua-300" />
            <h2 className="text-[13.5px] font-semibold text-slate-100">{t("docs.limits")}</h2>
          </div>
          <div className="grid gap-2.5">
            {limits.map((l) => (
              <div
                key={l.k}
                className="flex items-start gap-3 rounded-2xl border border-white/[0.07] bg-white/[0.02] px-4 py-3"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[11px] tracking-[0.14em] text-slate-400/70 uppercase">{l.k}</span>
                    <span className="ml-auto rounded-lg border border-aqua-300/20 bg-aqua-400/[0.07] px-2 py-0.5 font-mono text-[10.5px] text-aqua-100">
                      {l.v}
                    </span>
                  </div>
                  <p className="mt-1.5 text-[11.5px] leading-relaxed text-slate-400/80">{l.d}</p>
                </div>
              </div>
            ))}
          </div>
        </GlassCard>

        <GlassCard className="p-6">
          <div className="mb-3 flex items-center gap-3">
            <Icon name="code" className="h-4 w-4 text-aqua-300" />
            <h2 className="text-[13.5px] font-semibold text-slate-100">{t("docs.options")}</h2>
          </div>
          <InlineCode code={OPTIONS_JSON} lang="json" />
          <div className="mt-4 flex flex-wrap items-center gap-3 border-t border-white/[0.06] pt-4 text-[11px] text-slate-400/75">
            <span className="flex items-center gap-1.5">
              <Icon name="bolt" className="h-3.5 w-3.5 text-aqua-300" />
              {t("quota.daily")}: {DAILY_FREE_QUOTA}
            </span>
            <Button
              variant="outline"
              className="ml-auto px-3 py-1.5 text-[11.5px]"
              onClick={() => navigator.clipboard?.writeText(OPTIONS_JSON)}
            >
              <Icon name="copy" className="h-3.5 w-3.5" />
              {t("common.copy")}
            </Button>
          </div>
        </GlassCard>

        <GlassCard className="border-amber-300/20 bg-amber-400/[0.05] p-5">
          <div className="flex items-start gap-3">
            <Icon name="sparkle" className="mt-0.5 h-4 w-4 shrink-0 text-amber-300" />
            <p className="text-[11.5px] leading-relaxed text-amber-100/85">{t("docs.note")}</p>
          </div>
        </GlassCard>
      </div>
    </div>
  );
}
