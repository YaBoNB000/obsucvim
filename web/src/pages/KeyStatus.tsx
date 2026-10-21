import { useCallback, useEffect, useState } from "react";
import { BlackHoleSVG, LangSwitch, Logo } from "../components/Brand";
import { Button, GlassCard, Icon } from "../components/ui";
import { useI18n, type TKey } from "../lib/i18n";
import { checkKey, selfResetHwid, type KeyError, type KeyStatus } from "../lib/key";

const INPUT =
  "w-full rounded-xl border border-white/[0.09] bg-black/30 px-3 py-2.5 font-mono text-[12px] text-aqua-100 placeholder:text-slate-600 transition focus:border-aqua-300/45 focus:outline-none";

const ERROR_KEY: Record<KeyError, TKey> = {
  invalid: "key.errInvalid",
  rate_limited: "key.errRateLimited",
  self_reset_disabled: "key.errSelfResetDisabled",
  reset_cooldown: "key.errCooldown",
  bad_input: "key.errBadInput",
  network: "key.errNetwork",
  unknown: "key.errUnknown",
};

function when(ts: number | null) {
  if (!ts) return "—";
  return new Date(ts).toLocaleString();
}

function countdown(seconds: number) {
  if (seconds <= 0) return "—";
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export function KeyStatusPage() {
  const { t } = useI18n();
  const [key, setKey] = useState("");
  const [status, setStatus] = useState<KeyStatus | null>(null);
  const [error, setError] = useState<TKey | null>(null);
  const [busy, setBusy] = useState(false);
  const [resetDone, setResetDone] = useState(false);

  const refresh = useCallback(async (value: string) => {
    setBusy(true);
    setError(null);
    setResetDone(false);
    const res = await checkKey(value.trim());
    setBusy(false);
    if (!res.ok || !res.data) {
      setStatus(null);
      setError(ERROR_KEY[res.error ?? "unknown"]);
      return;
    }
    setStatus(res.data);
  }, []);

  const handleReset = async () => {
    if (!key.trim() || busy) return;
    setBusy(true);
    setError(null);
    const res = await selfResetHwid(key.trim());
    setBusy(false);
    if (!res.ok) {
      setError(ERROR_KEY[res.error ?? "unknown"]);
      return;
    }
    setResetDone(true);
    await refresh(key);
  };

  // 支持从外部链接直接带入卡密：#key=<卡密>
  useEffect(() => {
    const hash = window.location.hash.replace(/^#/, "");
    const params = new URLSearchParams(hash.includes("=") ? hash : "");
    const preset = params.get("key");
    if (preset) {
      setKey(preset);
      void refresh(preset);
    }
  }, [refresh]);

  const stateTone =
    status?.state === "active"
      ? "border-emerald-300/25 bg-emerald-400/[0.08] text-emerald-200"
      : status?.state === "expired"
        ? "border-amber-300/25 bg-amber-400/[0.08] text-amber-200"
        : "border-rose-300/25 bg-rose-400/[0.08] text-rose-200";

  return (
    <main className="auth-stage relative flex min-h-screen items-center justify-center px-4 py-10">
      <div className="auth-ambience pointer-events-none fixed inset-0 overflow-hidden" aria-hidden="true">
        <div className="auth-glow auth-glow-cyan" />
        <div className="auth-glow auth-glow-violet" />
      </div>

      <GlassCard className="relative z-10 w-full max-w-[560px] p-6 sm:p-8">
        <header className="flex items-center justify-between gap-4">
          <Logo />
          <LangSwitch />
        </header>

        <div className="mt-7 flex items-center gap-3">
          <span className="flex h-11 w-11 items-center justify-center rounded-2xl border border-aqua-300/25 bg-aqua-400/10 text-aqua-200">
            <Icon name="lock" className="h-5 w-5" />
          </span>
          <div>
            <h1 className="text-[19px] font-semibold tracking-tight text-slate-50">{t("key.title")}</h1>
            <p className="mt-0.5 text-[11.5px] text-slate-400/80">{t("key.subtitle")}</p>
          </div>
        </div>

        <form
          className="mt-5 grid gap-2.5"
          onSubmit={(e) => {
            e.preventDefault();
            if (key.trim()) void refresh(key);
          }}
        >
          <label className="auth-field-label">{t("key.field")}</label>
          <input
            className={INPUT}
            value={key}
            onChange={(e) => setKey(e.target.value)}
            placeholder="osk_..."
            spellCheck={false}
            autoComplete="off"
          />
          <Button type="submit" className="w-full py-2.5" disabled={busy || key.trim().length < 16}>
            {busy ? <span className="auth-spinner" /> : <Icon name="eye" className="h-4 w-4" />}
            {busy ? t("key.checking") : t("key.check")}
          </Button>
        </form>

        {error && (
          <div className="mt-4 flex items-start gap-2 rounded-xl border border-rose-400/25 bg-rose-500/[0.08] px-3 py-2.5 text-[11.5px] text-rose-200">
            <Icon name="sparkle" className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            {t(error)}
          </div>
        )}

        {resetDone && (
          <div className="mt-4 flex items-center gap-2 rounded-xl border border-emerald-300/25 bg-emerald-400/[0.08] px-3 py-2.5 text-[11.5px] text-emerald-200">
            <Icon name="check" className="h-3.5 w-3.5" />
            {t("key.resetDone")}
          </div>
        )}

        {status && (
          <div className="mt-5 grid gap-3">
            <div className={`flex items-center justify-between rounded-xl border px-3 py-2 text-[12px] ${stateTone}`}>
              <span className="flex items-center gap-2">
                <span className="h-1.5 w-1.5 rounded-full bg-current" />
                {status.state === "active" ? t("key.stateActive") : status.state === "expired" ? t("key.stateExpired") : t("key.stateRevoked")}
              </span>
              <span className="font-mono text-[11px] opacity-80">
                {status.secondsLeft !== null ? countdown(status.secondsLeft) : t("key.never")}
              </span>
            </div>

            <dl className="grid grid-cols-2 gap-2 text-[11.5px]">
              {[
                [t("key.script"), status.scriptName || "—"],
                [t("key.folder"), status.folderName || "—"],
                [t("key.note"), status.note || "—"],
                [t("key.device"), status.deviceBound ? t("key.bound") : t("key.unbound")],
                [t("key.expires"), status.expiresAt ? when(status.expiresAt) : t("key.never")],
                [t("key.executions"), String(status.totalExecutions)],
                [t("key.firstUsed"), when(status.firstActivatedAt)],
                [t("key.lastUsed"), when(status.lastUsedAt)],
              ].map(([label, value]) => (
                <div key={label} className="glass-soft rounded-xl border border-white/[0.06] px-3 py-2">
                  <dt className="text-[10px] tracking-[0.12em] text-slate-500 uppercase">{label}</dt>
                  <dd className="mt-0.5 truncate text-slate-200">{value}</dd>
                </div>
              ))}
            </dl>

            {status.selfReset ? (
              <div className="rounded-xl border border-white/[0.07] bg-white/[0.02] p-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <p className="text-[11.5px] font-medium text-slate-200">{t("key.selfReset")}</p>
                    <p className="mt-0.5 text-[10.5px] text-slate-500">
                      {status.resetAvailableIn > 0
                        ? t("key.cooldownLeft", { v: countdown(status.resetAvailableIn) })
                        : t("key.selfResetHint", { d: status.resetCooldownDays })}
                    </p>
                  </div>
                  <Button
                    variant="outline"
                    className="shrink-0 px-3 py-1.5 text-[11.5px]"
                    disabled={busy || status.resetAvailableIn > 0}
                    onClick={handleReset}
                  >
                    {t("key.reset")}
                  </Button>
                </div>
              </div>
            ) : (
              <p className="text-[10.5px] leading-relaxed text-slate-500">{t("key.selfResetOff")}</p>
            )}
          </div>
        )}

        <div className="mt-6 flex items-center justify-between border-t border-white/[0.07] pt-4">
          <a href="#" className="text-[11px] text-slate-500 transition hover:text-aqua-200">
            ← {t("key.back")}
          </a>
          <span className="flex items-center gap-1.5 text-[10px] text-slate-600">
            <BlackHoleSVG className="h-3.5 w-3.5" />
            Obsucvim
          </span>
        </div>
      </GlassCard>
    </main>
  );
}
