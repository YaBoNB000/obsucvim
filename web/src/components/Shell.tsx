import type { ReactNode } from "react";
import { Logo, LangSwitch } from "./Brand";
import { Icon, type IconName } from "./ui";
import { useI18n, type TKey } from "../lib/i18n";
import { useAuth } from "../lib/store";
import { ENGINE, MAX_SOURCE_BYTES } from "../lib/engine";
import { cn } from "../utils/cn";

export type PageId = "workspace" | "history" | "docs" | "account";

const NAV: { id: PageId; label: TKey; icon: IconName; group: "tool" | "info" }[] = [
  { id: "workspace", label: "nav.workspace", icon: "bolt", group: "tool" },
  { id: "history", label: "nav.history", icon: "clock", group: "tool" },
  { id: "docs", label: "nav.docs", icon: "terminal", group: "info" },
  { id: "account", label: "nav.account", icon: "shield", group: "info" },
];

/* ---------------------------- 额度卡 ---------------------------- */
function QuotaCard({ compact = false }: { compact?: boolean }) {
  const { t } = useI18n();
  const { remaining, limit, usedToday } = useAuth();
  const pct = Math.round((remaining / limit) * 100);
  return (
    <div className={cn("glass-soft rounded-2xl border border-white/[0.07] p-3.5", compact && "p-3")}>
      <div className="flex items-center justify-between text-[10.5px] tracking-[0.16em] text-slate-400/70 uppercase">
        <span>{t("quota.daily")}</span>
        <span className="tabular text-aqua-100">{t("quota.remaining", { n: remaining, t: limit })}</span>
      </div>
      <div className="mt-2.5 h-1.5 overflow-hidden rounded-full bg-white/[0.07]">
        <div
          className={cn(
            "h-full rounded-full transition-all duration-700",
            remaining > 2
              ? "bg-gradient-to-r from-aqua-500 to-aqua-200 shadow-[0_0_14px_rgba(34,226,213,0.8)]"
              : "bg-gradient-to-r from-amber-400 to-rose-400 shadow-[0_0_14px_rgba(251,191,36,0.7)]",
          )}
          style={{ width: `${pct}%` }}
        />
      </div>
      <p className="tabular mt-2 text-[10.5px] text-slate-500">
        {t("quota.usedToday", { n: usedToday })} · {t("quota.resetNote")}
      </p>
    </div>
  );
}

/* ---------------------------- 侧边栏 ---------------------------- */
export function Sidebar({
  page,
  setPage,
  mobileOpen,
  setMobileOpen,
}: {
  page: PageId;
  setPage: (p: PageId) => void;
  mobileOpen: boolean;
  setMobileOpen: (v: boolean) => void;
}) {
  const { t } = useI18n();
  const { user, logout } = useAuth();

  const groups: { key: "tool" | "info"; label: TKey }[] = [
    { key: "tool", label: "nav.group.tool" },
    { key: "info", label: "nav.group.info" },
  ];

  return (
    <>
      {mobileOpen && (
        <div
          className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm lg:hidden"
          onClick={() => setMobileOpen(false)}
        />
      )}
      <aside
        className={cn(
          "fixed inset-y-0 left-0 z-50 flex w-[272px] flex-col gap-5 border-r border-white/[0.07] px-4 py-5 transition-transform duration-300 lg:translate-x-0",
          "glass",
          mobileOpen ? "translate-x-0" : "-translate-x-full",
        )}
      >
        <div className="flex items-center justify-between">
          <button onClick={() => setPage("workspace")} className="cursor-pointer text-left">
            <Logo />
          </button>
          <button
            onClick={() => setMobileOpen(false)}
            className="cursor-pointer rounded-lg border border-white/10 p-1.5 text-slate-400 lg:hidden"
            aria-label="close"
          >
            <Icon name="chevron" className="h-4 w-4 rotate-90" />
          </button>
        </div>

        <nav className="flex flex-1 flex-col gap-5 overflow-y-auto">
          {groups.map((g) => (
            <div key={g.key}>
              <div className="mb-2 px-2 text-[10px] font-semibold tracking-[0.22em] text-slate-500 uppercase">
                {t(g.label)}
              </div>
              <div className="grid gap-1">
                {NAV.filter((n) => n.group === g.key).map((n) => {
                  const active = page === n.id;
                  return (
                    <button
                      key={n.id}
                      onClick={() => {
                        setPage(n.id);
                        setMobileOpen(false);
                      }}
                      className={cn(
                        "group relative flex cursor-pointer items-center gap-3 rounded-xl px-3 py-2.5 text-left text-[13px] transition-all duration-300",
                        active
                          ? "border border-aqua-300/30 bg-aqua-400/[0.1] text-aqua-50 shadow-[0_0_30px_-16px_rgba(34,226,213,0.9)_inset]"
                          : "border border-transparent text-slate-400 hover:bg-white/[0.05] hover:text-aqua-100",
                      )}
                    >
                      {active && (
                        <span className="absolute top-1/2 -left-px h-5 w-[2px] -translate-y-1/2 rounded-full bg-aqua-300 shadow-[0_0_12px_rgba(34,226,213,1)]" />
                      )}
                      <Icon name={n.icon} className={cn("h-4 w-4", active && "text-aqua-200")} />
                      {t(n.label)}
                    </button>
                  );
                })}
              </div>
            </div>
          ))}

          <div className="mt-auto grid gap-3">
            <QuotaCard />
            <div className="flex items-center gap-2 rounded-2xl border border-white/[0.07] bg-white/[0.02] px-3 py-2.5">
              <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-xl border border-aqua-300/25 bg-aqua-400/10 font-mono text-[12px] text-aqua-200">
                {user ? user.nickname.slice(0, 2).toUpperCase() : "--"}
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[12px] text-slate-200">{user?.email ?? "-"}</span>
                <span className="block text-[10.5px] text-slate-500">{t("account.planFree")}</span>
              </span>
              <button
                onClick={logout}
                title={t("common.logout")}
                className="cursor-pointer rounded-lg border border-white/10 p-1.5 text-slate-400 transition hover:border-rose-400/40 hover:text-rose-300"
              >
                <Icon name="eye" className="h-3.5 w-3.5" />
              </button>
            </div>
            <div className="flex items-center justify-between px-1">
              <LangSwitch />
              <span className="flex items-center gap-1.5 text-[10px] text-slate-500">
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-300 shadow-[0_0_8px_rgba(110,231,183,0.9)]" />
                {ENGINE.runtime.toUpperCase()}
              </span>
            </div>
          </div>
        </nav>
      </aside>
    </>
  );
}

/* ---------------------------- 顶栏 ---------------------------- */
export function TopBar({
  title,
  subtitle,
  actions,
  onMenu,
}: {
  title: string;
  subtitle?: string;
  actions?: ReactNode;
  onMenu: () => void;
}) {
  const { t } = useI18n();
  const { remaining, limit } = useAuth();

  return (
    <header className="glass specular sticky top-0 z-30 flex flex-wrap items-center gap-3 rounded-none border-x-0 border-t-0 px-4 py-3 sm:px-6">
      <button
        onClick={onMenu}
        className="cursor-pointer rounded-xl border border-white/10 p-2 text-slate-300 lg:hidden"
        aria-label="menu"
      >
        <Icon name="layers" className="h-4 w-4" />
      </button>

      <div className="min-w-0">
        <h1 className="truncate text-[15px] font-semibold text-slate-100">{title}</h1>
        {subtitle && <p className="mt-0.5 truncate text-[11.5px] text-slate-400/70">{subtitle}</p>}
      </div>

      <div className="ml-auto flex flex-wrap items-center gap-2">
        <span className="hidden items-center gap-1.5 rounded-xl border border-white/[0.07] bg-white/[0.02] px-2.5 py-1.5 font-mono text-[11px] text-slate-400 sm:inline-flex">
          {t("ws.runtime")}
          <span className="text-slate-600">|</span>
          {Math.round(MAX_SOURCE_BYTES / 1024)} KB
        </span>
        <span
          className={cn(
            "tabular inline-flex items-center gap-1.5 rounded-xl border px-2.5 py-1.5 text-[11px]",
            remaining > 2
              ? "border-aqua-300/25 bg-aqua-400/[0.08] text-aqua-100"
              : "border-amber-300/30 bg-amber-400/[0.08] text-amber-200",
          )}
        >
          <Icon name="bolt" className="h-3.5 w-3.5" />
          {t("quota.remaining", { n: remaining, t: limit })}
        </span>
        {actions}
        <div className="lg:hidden">
          <LangSwitch />
        </div>
      </div>
    </header>
  );
}
