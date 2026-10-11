import { useState } from "react";
import { useI18n } from "../lib/i18n";
import { useAuth, loadDefaults, saveDefaults } from "../lib/store";
import { DEFAULT_OPTIONS, ENGINE, DAILY_FREE_QUOTA, type ObfuscationOptions } from "../lib/engine";
import { Button, GlassCard, Icon, Pill, Stat } from "../components/ui";
import { LangSwitch } from "../components/Brand";
import { presetLabel } from "../components/OptionsPanel";
import { Toggle } from "../components/ui";

export function Account() {
  const { t, lang } = useI18n();
  const { user, usedToday, limit, remaining, logout } = useAuth();
  const [defaults, setDefaults] = useState<ObfuscationOptions>(() => loadDefaults(DEFAULT_OPTIONS));
  const [saved, setSaved] = useState(false);

  const save = () => {
    saveDefaults(defaults);
    setSaved(true);
    setTimeout(() => setSaved(false), 1600);
  };

  return (
    <div className="grid items-start gap-5 xl:grid-cols-[minmax(0,1.2fr)_minmax(320px,1fr)]">
      <div className="grid min-w-0 content-start gap-5">
        <GlassCard className="p-6">
          <div className="mb-5 flex items-center gap-3">
            <Icon name="shield" className="h-4 w-4 text-aqua-300" />
            <h2 className="text-[13.5px] font-semibold text-slate-100">{t("account.profile")}</h2>
          </div>
          <div className="flex flex-wrap items-center gap-4">
            <span className="flex h-14 w-14 items-center justify-center rounded-2xl border border-aqua-300/25 bg-aqua-400/10 font-mono text-[18px] text-aqua-200">
              {user ? user.nickname.slice(0, 2).toUpperCase() : "--"}
            </span>
            <div className="min-w-0">
              <p className="truncate font-mono text-[13px] text-slate-100">{user?.email ?? "-"}</p>
              <div className="mt-1.5 flex flex-wrap items-center gap-2">
                <Pill>{t("account.planFree")}</Pill>
                <span className="text-[11px] text-slate-500">{t("account.used")}: {usedToday} / {limit}</span>
              </div>
            </div>
            <Button variant="outline" className="ml-auto px-3 py-2" onClick={logout}>
              <Icon name="eye" className="h-4 w-4" />
              {t("common.logout")}
            </Button>
          </div>

          <div className="mt-5 grid grid-cols-2 gap-3 sm:grid-cols-3">
            <Stat label={t("quota.daily")} value={`${limit}`} unit="/day" />
            <Stat label={t("quota.usedToday", { n: usedToday })} value={`${remaining}`} unit="left" />
            <Stat label={t("account.language")} value={lang === "zh" ? "中文" : "EN"} />
          </div>
          <p className="mt-4 text-[11px] leading-relaxed text-slate-500">{t("account.loginNote")}</p>
        </GlassCard>

        <GlassCard className="p-6">
          <div className="mb-4 flex items-center gap-3">
            <Icon name="bolt" className="h-4 w-4 text-aqua-300" />
            <h2 className="text-[13.5px] font-semibold text-slate-100">{t("account.defaults")}</h2>
          </div>
          <p className="mb-4 text-[12px] leading-relaxed text-slate-400/80">{t("account.defaultsDesc")}</p>

          <div className="grid gap-2.5">
            <div className="grid grid-cols-3 gap-2">
              {(["light", "balanced", "extreme"] as const).map((p) => (
                <button
                  key={p}
                  onClick={() => setDefaults({ ...defaults, preset: p })}
                  className={
                    "cursor-pointer rounded-xl border px-3 py-2 text-[12px] transition-all duration-300 " +
                    (defaults.preset === p
                      ? "border-aqua-300/40 bg-aqua-400/[0.12] text-aqua-100"
                      : "border-white/[0.07] bg-white/[0.02] text-slate-400 hover:border-aqua-300/25 hover:text-aqua-100")
                  }
                >
                  {presetLabel(p, t)}
                </button>
              ))}
            </div>
            <Toggle
              label={t("ws.vmMode")}
              hint={t("ws.vmModeHint")}
              checked={defaults.vm.mode !== "off"}
              onChange={(v) => setDefaults({ ...defaults, vm: { ...defaults.vm, mode: v ? "function" : "off" } })}
            />
            <Toggle
              label={t("ws.watermark")}
              hint={defaults.watermark || t("ws.watermarkPh")}
              checked={defaults.watermark.length > 0}
              onChange={(v) => setDefaults({ ...defaults, watermark: v ? (defaults.watermark || "user#00001") : "" })}
            />
            <label className="block">
              <span className="mb-1 block text-[11px] text-slate-400/70">{t("ws.watermark")}</span>
              <input
                value={defaults.watermark}
                onChange={(e) => setDefaults({ ...defaults, watermark: e.target.value })}
                placeholder={t("ws.watermarkPh")}
                className="w-full rounded-xl border border-white/[0.09] bg-black/30 px-3 py-2 font-mono text-[12px] text-aqua-100 placeholder:text-slate-600 focus:border-aqua-300/45"
              />
            </label>
          </div>

          <div className="mt-5 flex items-center gap-3">
            <Button onClick={save} className="px-4 py-2">
              <Icon name={saved ? "check" : "download"} className={saved ? "h-4 w-4 text-emerald-100" : "h-4 w-4"} />
              {saved ? t("common.copied") : t("common.reset")}
            </Button>
            <Button
              variant="ghost"
              className="px-3 py-2 text-[12px]"
              onClick={() => setDefaults(DEFAULT_OPTIONS)}
            >
              {t("common.reset")}
            </Button>
          </div>
        </GlassCard>
      </div>

      <div className="grid content-start gap-5">
        <GlassCard className="p-6">
          <div className="mb-4 flex items-center gap-3">
            <Icon name="flow" className="h-4 w-4 text-aqua-300" />
            <h2 className="text-[13.5px] font-semibold text-slate-100">{t("account.language")}</h2>
          </div>
          <LangSwitch size="md" />
          <div className="mt-5 border-t border-white/[0.06] pt-4">
            <div className="flex items-center gap-3">
              <span className="h-1.5 w-1.5 rounded-full bg-emerald-300 shadow-[0_0_8px_rgba(110,231,183,0.9)]" />
              <span className="text-[12px] text-slate-300">{t("account.engine")}</span>
            </div>
            <p className="mt-2 font-mono text-[11.5px] text-aqua-100/90">{ENGINE.name}</p>
            <p className="mt-1 text-[11px] text-slate-500">
              {t("account.engineNote")} · runtime {ENGINE.runtime} · {DAILY_FREE_QUOTA}/day
            </p>
          </div>
        </GlassCard>

        <GlassCard className="border-amber-300/20 bg-amber-400/[0.05] p-6">
          <div className="flex items-start gap-3">
            <Icon name="shield" className="mt-0.5 h-4 w-4 shrink-0 text-amber-300" />
            <div>
              <h2 className="text-[12.5px] font-semibold text-amber-100">{t("account.about")}</h2>
              <p className="mt-2 text-[11.5px] leading-relaxed text-amber-100/80">{t("account.aboutText")}</p>
            </div>
          </div>
        </GlassCard>
      </div>
    </div>
  );
}
