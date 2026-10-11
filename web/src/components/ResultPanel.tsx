import { useEffect, useState } from "react";
import { appliedNames, type LogLine, type ObfuscationOptions, type ObfuscationResult } from "../lib/engine";
import { Button, GlassCard, Icon, Pill, Stat } from "./ui";
import { CodeViewer } from "./CodeArea";
import { useI18n } from "../lib/i18n";
import { cn } from "../utils/cn";

const fmt = (n: number) => (n < 1024 ? `${n} B` : `${(n / 1024).toFixed(1)} KB`);

function Empty({ icon, title, desc }: { icon: Parameters<typeof Icon>[0]["name"]; title: string; desc: string }) {
  return (
    <div className="flex h-[380px] flex-col items-center justify-center px-6 text-center">
      <div className="animate-floaty mb-4 flex h-14 w-14 items-center justify-center rounded-2xl border border-aqua-300/20 bg-aqua-400/[0.07]">
        <Icon name={icon} className="h-6 w-6 text-aqua-300" />
      </div>
      <p className="text-[13px] text-slate-300/90">{title}</p>
      <p className="mt-1 max-w-xs text-[11.5px] leading-relaxed text-slate-400/60">{desc}</p>
    </div>
  );
}

export function ResultPanel({
  result,
  options,
  running,
  hasInput,
}: {
  result: ObfuscationResult | null;
  options: ObfuscationOptions;
  running: boolean;
  hasInput: boolean;
}) {
  const { t } = useI18n();
  const [tab, setTab] = useState<"out" | "log" | "stats">("out");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (result) setTab("out");
  }, [result]);

  const output = result?.code ?? "";

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(output);
    } catch {
      /* ignore */
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const download = () => {
    if (!output) return;
    const blob = new Blob([output], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `obsucvim_${result?.stats.seeded ?? "out"}.luau`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const applied = appliedNames(options);
  const strength = result ? Math.min(100, applied.length * 11 + (result.stats.ratio > 1.4 ? 6 : 0)) : 0;

  const logColor: Record<LogLine["level"], string> = {
    info: "text-slate-400",
    step: "text-aqua-200",
    ok: "text-emerald-300",
    warn: "text-amber-300/90",
  };

  return (
    <GlassCard padded={false} className="overflow-hidden">
      <div className="flex flex-wrap items-center gap-3 border-b border-white/[0.07] px-4 py-3">
        <div className="flex items-center gap-1 rounded-xl border border-white/[0.07] bg-black/25 p-1">
          {(
            [
              { id: "out", label: t("result.output"), icon: "code" },
              { id: "log", label: t("result.log"), icon: "terminal" },
              { id: "stats", label: t("result.stats"), icon: "sparkle" },
            ] as const
          ).map((tabs) => (
            <button
              key={tabs.id}
              onClick={() => setTab(tabs.id)}
              className={cn(
                "flex cursor-pointer items-center gap-1.5 rounded-lg px-3 py-1.5 text-[12px] transition",
                tab === tabs.id ? "bg-aqua-400/15 text-aqua-100" : "text-slate-400 hover:text-aqua-100",
              )}
            >
              <Icon name={tabs.icon} className="h-3.5 w-3.5" />
              {tabs.label}
              {tabs.id === "log" && result && (
                <span className="tabular rounded-full bg-aqua-400/20 px-1.5 text-[10px] text-aqua-100">
                  {result.log.length}
                </span>
              )}
            </button>
          ))}
        </div>

        {result && (
          <Pill className="hidden font-mono sm:inline-flex">
            <span className="tabular">{fmt(result.stats.outputBytes)}</span>
            <span className="text-slate-400/70">
              {result.stats.ratio >= 1
                ? `+${Math.round((result.stats.ratio - 1) * 100)}%`
                : `-${Math.round((1 - result.stats.ratio) * 100)}%`}
            </span>
          </Pill>
        )}

        <div className="ml-auto flex items-center gap-2">
          <Button variant="outline" onClick={copy} disabled={!output} className="px-3 py-2">
            <Icon name={copied ? "check" : "copy"} className={cn("h-4 w-4", copied && "text-emerald-300")} />
            <span className="hidden sm:inline">{copied ? t("common.copied") : t("common.copy")}</span>
          </Button>
          <Button onClick={download} disabled={!output} className="px-3 py-2">
            <Icon name="download" className="h-4 w-4" />
            <span className="hidden sm:inline">{t("common.download")}</span>
          </Button>
        </div>
      </div>

      <div className="min-h-[380px]">
        {tab === "out" &&
          (result ? (
            <CodeViewer code={result.code} height={380} />
          ) : (
            <Empty
              icon="code"
              title={hasInput ? t("result.empty") : t("result.needInput")}
              desc={hasInput ? t("result.emptyDesc") : t("result.needInputDesc")}
            />
          ))}

        {tab === "log" && (
          <div className="h-[380px] overflow-auto bg-[#01060b]/60 p-4 font-mono text-[12px] leading-[21px]">
            {running && (
              <div className="mb-2 flex items-center gap-2 text-aqua-200">
                <span className="h-3 w-2 animate-pulse bg-aqua-300" />
                <span className="text-slate-400/70">{t("ws.running")}</span>
              </div>
            )}
            {result ? (
              result.log.map((l, i) => (
                <div key={i} className={cn("whitespace-pre-wrap", logColor[l.level])}>
                  <span className="mr-2 text-aqua-400/50">{String(i + 1).padStart(2, "0")}</span>
                  {l.text}
                </div>
              ))
            ) : (
              !running && <div className="text-slate-500/70">{t("result.noLogs")}</div>
            )}
          </div>
        )}

        {tab === "stats" && (
          <div className="p-5">
            {result ? (
              <div className="grid gap-4">
                <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                  <Stat label={t("result.inputSize")} value={fmt(result.stats.inputBytes)} />
                  <Stat label={t("result.outputSize")} value={fmt(result.stats.outputBytes)} />
                  <Stat
                    label={t("result.expansion")}
                    value={`${result.stats.ratio >= 1 ? "+" : "-"}${Math.abs(Math.round((result.stats.ratio - 1) * 100))}`}
                    unit="%"
                  />
                  <Stat label={t("result.elapsed")} value={`${result.stats.elapsedMs}`} unit="ms" />
                </div>

                <div className="glass-soft rounded-2xl border border-white/[0.07] p-4">
                  <div className="flex items-center justify-between text-[10.5px] tracking-[0.18em] text-slate-400/70 uppercase">
                    <span>{t("result.strength")}</span>
                    <span className="tabular text-aqua-200">{strength}/100</span>
                  </div>
                  <div className="mt-3 h-2 overflow-hidden rounded-full bg-white/[0.06]">
                    <div
                      className="h-full rounded-full bg-gradient-to-r from-aqua-500 via-aqua-300 to-aqua-100 shadow-[0_0_18px_rgba(34,226,213,0.8)] transition-all duration-700"
                      style={{ width: `${strength}%` }}
                    />
                  </div>
                  <div className="mt-4 flex flex-wrap gap-1.5">
                    {applied.map((a) => (
                      <Pill key={a}>{a}</Pill>
                    ))}
                  </div>
                  <div className="mt-4 flex flex-wrap items-center gap-4 border-t border-white/[0.06] pt-3 text-[11px] text-slate-400/70">
                    <span>
                      {t("result.seed")}: <span className="font-mono text-aqua-100">{result.stats.seeded}</span>
                    </span>
                    <span className="tabular">{result.stats.lines} lines</span>
                  </div>
                  <p className="mt-3 text-[10.5px] leading-relaxed text-slate-500">{t("result.strengthNote")}</p>
                </div>
              </div>
            ) : (
              <Empty icon="sparkle" title={t("result.noStats")} desc={t("result.noStatsDesc")} />
            )}
          </div>
        )}
      </div>
    </GlassCard>
  );
}
