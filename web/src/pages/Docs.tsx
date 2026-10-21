import { useI18n } from "../lib/i18n";
import { GlassCard, Icon, Pill, Stat } from "../components/ui";
import { InlineCode } from "../components/CodeArea";
import { ENGINE, MAX_SOURCE_BYTES, ENGINE_TIMEOUT_MS, DAILY_FREE_QUOTA } from "../lib/engine";

const TS_SNIPPET = `// src/lib/engine.ts · HttpOnly same-origin session cookie
export async function obfuscate(source: string, options: ObfuscationOptions) {
  const response = await fetch("/api/obfuscate", {
    method: "POST",
    credentials: "same-origin",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ source, options }),
  });
  const payload = await response.json();
  if (!response.ok || !payload.ok) throw new Error(payload.error ?? "ENGINE_FAILED");
  return payload.result;
}`;

export function Docs() {
  const { t } = useI18n();

  const limits = [
    { k: t("docs.limitSource"), v: `${MAX_SOURCE_BYTES / 1024 / 1024} MB`, d: t("docs.limitSourceV") },
    { k: t("docs.limitReturn"), v: "once", d: t("docs.limitReturnV") },
    { k: t("docs.limitTimeout"), v: `${ENGINE_TIMEOUT_MS / 1000} s`, d: t("docs.limitTimeoutV") },
    { k: t("docs.limitApi"), v: "none", d: t("docs.limitApiV") },
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
          <InlineCode code={TS_SNIPPET} lang="json" label={t("docs.ts")} />
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
            <Icon name="sparkle" className="h-4 w-4 text-aqua-300" />
            <h2 className="text-[13.5px] font-semibold text-slate-100">{t("docs.quota")}</h2>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <Stat label={t("quota.daily")} value={`${DAILY_FREE_QUOTA}`} unit="/day" />
            <Stat label={t("docs.limitTimeout")} value={`${ENGINE_TIMEOUT_MS / 1000}`} unit="s" />
          </div>
        </GlassCard>
      </div>
    </div>
  );
}
