import { useI18n } from "../lib/i18n";
import { useHistory, type HistoryItem } from "../lib/store";
import { appliedNames, type ObfuscationOptions } from "../lib/engine";
import { Button, GlassCard, Icon, Pill } from "../components/ui";
import { presetLabel } from "../components/OptionsPanel";
import { cn } from "../utils/cn";

const fmt = (n: number) => (n < 1024 ? `${n} B` : `${(n / 1024).toFixed(1)} KB`);

function chips(o: ObfuscationOptions) {
  const all = appliedNames(o);
  return all.slice(0, 5);
}

export function History({ onLoad }: { onLoad: (item: HistoryItem) => void }) {
  const { t, lang } = useI18n();
  const { items, clear } = useHistory();

  if (items.length === 0) {
    return (
      <GlassCard className="p-10">
        <div className="flex flex-col items-center text-center">
          <div className="animate-floaty mb-4 flex h-14 w-14 items-center justify-center rounded-2xl border border-aqua-300/20 bg-aqua-400/[0.07]">
            <Icon name="clock" className="h-6 w-6 text-aqua-300" />
          </div>
          <p className="text-[13px] text-slate-300/90">{t("history.empty")}</p>
        </div>
      </GlassCard>
    );
  }

  return (
    <div className="grid gap-4">
      <div className="flex items-center justify-between">
        <Pill className="font-mono">
          {items.length} / 5 · {t("nav.history")}
        </Pill>
        <Button variant="ghost" className="px-3 py-1.5 text-[12px]" onClick={clear}>
          <Icon name="trash" className="h-4 w-4" />
          {t("history.clearAll")}
        </Button>
      </div>

      {items.map((item) => (
        <GlassCard key={item.id} className="p-5">
          <div className="flex flex-wrap items-start gap-4">
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-mono text-[12px] text-aqua-100">
                  obsucvim_{item.seed}.luau
                </span>
                <span className="rounded-lg border border-white/[0.08] bg-black/25 px-2 py-0.5 font-mono text-[10.5px] text-slate-400">
                  {presetLabel(item.preset, t)}
                </span>
                <span className="tabular text-[10.5px] text-slate-500">
                  {new Date(item.timestamp).toLocaleString(lang === "zh" ? "zh-CN" : "en-US", {
                    hour12: false,
                  })}
                </span>
              </div>

              <div className="mt-3 flex flex-wrap gap-1.5">
                {chips(item.options).map((c) => (
                  <span
                    key={c}
                    className="rounded-lg border border-aqua-300/15 bg-aqua-400/[0.06] px-2 py-0.5 font-mono text-[10px] text-aqua-100/85"
                  >
                    {c}
                  </span>
                ))}
              </div>

              <div className="mt-3 flex flex-wrap items-center gap-4 text-[11px] text-slate-400/80">
                <span>
                  {t("result.inputSize")}: <span className="tabular text-slate-300">{fmt(item.inputBytes)}</span>
                </span>
                <span>
                  {t("history.size")}: <span className="tabular text-slate-300">{fmt(item.outputBytes)}</span>
                </span>
                <span>
                  {t("ws.appliedCount", { n: appliedNames(item.options).length })}
                </span>
                <span>
                  {t("history.seed")}: <span className="font-mono text-slate-300">{item.seed}</span>
                </span>
              </div>
            </div>

            <Button variant="outline" className="px-3 py-2" onClick={() => onLoad(item)}>
              <Icon name="download" className="h-4 w-4 rotate-180" />
              {t("history.load")}
            </Button>
          </div>

          <details className="group mt-4">
            <summary
              className={cn(
                "flex cursor-pointer items-center gap-2 rounded-xl border border-white/[0.07] bg-white/[0.02] px-3 py-2 text-[11.5px] text-slate-400 transition hover:border-aqua-300/25 hover:text-aqua-100",
              )}
            >
              <Icon name="chevron" className="h-3.5 w-3.5 transition-transform group-open:rotate-180" />
              {t("history.options")}
            </summary>
            <pre className="code-hl mt-2 max-h-44 overflow-auto rounded-xl border border-white/[0.06] bg-black/35 p-3 text-[11.5px] leading-[19px]">
              {JSON.stringify(item.options, null, 2)}
            </pre>
          </details>
        </GlassCard>
      ))}
    </div>
  );
}
