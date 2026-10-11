import { useI18n } from "../lib/i18n";
import { useAuth } from "../lib/store";
import { ENGINE, DAILY_FREE_QUOTA } from "../lib/engine";
import { Button, GlassCard, Icon, Pill, Stat } from "../components/ui";
import { LangSwitch } from "../components/Brand";

export function Account() {
  const { t, lang } = useI18n();
  const { user, usedToday, limit, remaining, logout } = useAuth();

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
