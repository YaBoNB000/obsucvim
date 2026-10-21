import { useState } from "react";
import { Background } from "./components/Brand";
import { LangSwitch } from "./components/Brand";
import { Sidebar, TopBar, type PageId } from "./components/Shell";
import { LoginScreen } from "./components/LoginScreen";
import { Workspace } from "./pages/Workspace";
import { History } from "./pages/History";
import { Scripts } from "./pages/Scripts";
import { Docs } from "./pages/Docs";
import { Account } from "./pages/Account";
import { I18nProvider, useI18n, type TKey } from "./lib/i18n";
import { AuthProvider, HistoryProvider, useAuth } from "./lib/store";

const TITLES: Record<PageId, { title: TKey; subtitle: TKey }> = {
  workspace: { title: "ws.title", subtitle: "ws.subtitle" },
  scripts: { title: "scripts.title", subtitle: "scripts.subtitle" },
  history: { title: "history.title", subtitle: "history.subtitle" },
  docs: { title: "docs.title", subtitle: "docs.subtitle" },
  account: { title: "account.title", subtitle: "account.subtitle" },
};

function Console() {
  const { t } = useI18n();
  const { user } = useAuth();
  const [page, setPage] = useState<PageId>("workspace");
  const [mobileOpen, setMobileOpen] = useState(false);

  if (!user) return <LoginScreen />;

  const meta = TITLES[page];

  return (
    <div className="min-h-screen lg:pl-[272px]">
      <Sidebar page={page} setPage={setPage} mobileOpen={mobileOpen} setMobileOpen={setMobileOpen} />

      <TopBar title={t(meta.title)} subtitle={t(meta.subtitle)} onMenu={() => setMobileOpen(true)} />

      <main className="px-4 py-5 sm:px-6 sm:py-6">
        <div key={page} className="animate-rise">
          {page === "workspace" && <Workspace />}
          {page === "scripts" && <Scripts />}
          {page === "history" && <History />}
          {page === "docs" && <Docs />}
          {page === "account" && <Account />}
        </div>

        <footer className="mt-10 flex flex-wrap items-center justify-between gap-3 border-t border-white/[0.06] pt-5 text-[11px] text-slate-500">
          <span>© {new Date().getFullYear()} Obsucvim · Luau obfuscation suite</span>
          <span className="flex items-center gap-3">
            <span className="flex items-center gap-1.5">
              <span className="h-1.5 w-1.5 rounded-full bg-emerald-300 shadow-[0_0_8px_rgba(110,231,183,0.9)]" />
              {t("status.local")}
            </span>
            <span className="lg:hidden">
              <LangSwitch />
            </span>
          </span>
        </footer>
      </main>
    </div>
  );
}

export default function App() {
  return (
    <I18nProvider>
      <AuthProvider>
        <HistoryProvider>
          <div className="relative">
            <Background />
            <Console />
          </div>
        </HistoryProvider>
      </AuthProvider>
    </I18nProvider>
  );
}
