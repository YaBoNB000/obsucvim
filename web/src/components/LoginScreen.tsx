import { useState } from "react";
import { BlackHoleSVG, Logo } from "./Brand";
import { Button, Icon } from "./ui";
import { LangSwitch } from "./Brand";
import { useI18n } from "../lib/i18n";
import { useAuth } from "../lib/store";

export function LoginScreen() {
  const { t } = useI18n();
  const { login } = useAuth();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email) || password.length < 4) {
      setErr(t("login.invalid"));
      return;
    }
    setErr(null);
    setBusy(true);
    // 演示版：本地模拟登录，替换此处即可接入真实账号体系
    setTimeout(() => {
      login(email.trim(), password);
      setBusy(false);
    }, 450);
  };

  return (
    <div className="relative flex min-h-screen items-center justify-center px-5 py-10">
      <div className="absolute top-5 right-5 z-10">
        <LangSwitch size="md" />
      </div>

      <div className="grid w-full max-w-4xl gap-6 lg:grid-cols-[1fr_1.05fr]">
        {/* 视觉侧 */}
        <div className="glass specular relative hidden flex-col justify-between overflow-hidden rounded-3xl p-8 lg:flex">
          {/* 玻璃反光条锚点 */}
          <Logo />
          <div className="relative my-6 flex items-center justify-center">
            <div className="animate-drift-slow absolute h-64 w-64 rounded-full bg-[radial-gradient(circle,rgba(34,226,213,0.35),transparent_66%)] blur-2xl" />
            <img
              src="/blackhole.png"
              alt="black hole"
              className="animate-floaty relative h-52 w-52 rounded-full object-cover opacity-95 shadow-[0_0_90px_-20px_rgba(34,226,213,0.9)]"
            />
            <span className="pointer-events-none absolute h-52 w-52 animate-spin rounded-full border border-aqua-300/20 [animation-duration:26s]" />
          </div>
          <div>
            <p className="font-mono text-[11.5px] tracking-[0.2em] text-aqua-300/90 uppercase">
              {t("login.badge")}
            </p>
            <p className="mt-2 text-[12.5px] leading-relaxed text-slate-400/80">{t("login.perks")}</p>
            <p className="mt-4 text-[11px] leading-relaxed text-amber-200/60">{t("login.foot")}</p>
          </div>
        </div>

        {/* 表单侧 */}
        <div className="glass specular relative rounded-3xl p-7 sm:p-8">
          <div className="mb-7 lg:hidden">
            <Logo />
          </div>
          <div className="mb-6 flex items-center gap-3">
            <BlackHoleSVG className="h-10 w-10 shrink-0" />
            <div>
              <h1 className="text-[19px] font-semibold text-slate-100">{t("login.title")}</h1>
              <p className="mt-1 text-[11.5px] leading-relaxed text-slate-400/75">{t("login.subtitle")}</p>
            </div>
          </div>

          <form onSubmit={submit} className="grid gap-4">
            <label className="block">
              <span className="mb-1.5 block text-[11px] tracking-[0.16em] text-slate-400/80 uppercase">
                {t("login.email")}
              </span>
              <input
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
                className="glass-soft w-full rounded-xl border border-white/[0.1] bg-black/30 px-3.5 py-2.5 font-mono text-[13px] text-aqua-50 placeholder:text-slate-600 focus:border-aqua-300/50"
              />
            </label>
            <label className="block">
              <span className="mb-1.5 block text-[11px] tracking-[0.16em] text-slate-400/80 uppercase">
                {t("login.password")}
              </span>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="••••••"
                className="glass-soft w-full rounded-xl border border-white/[0.1] bg-black/30 px-3.5 py-2.5 font-mono text-[13px] text-aqua-50 placeholder:text-slate-600 focus:border-aqua-300/50"
              />
            </label>

            {err && (
              <div className="flex items-center gap-2 rounded-xl border border-amber-300/25 bg-amber-400/[0.07] px-3 py-2 text-[11.5px] text-amber-200">
                <Icon name="shield" className="h-3.5 w-3.5" />
                {err}
              </div>
            )}

            <Button type="submit" disabled={busy} className="mt-1 w-full py-3 text-[13.5px]">
              {busy ? (
                <>
                  <span className="h-3.5 w-3.5 animate-spin rounded-full border-2 border-ink-1000/40 border-t-ink-1000" />
                  {t("ws.running")}
                </>
              ) : (
                <>
                  <Icon name="lock" className="h-4 w-4" />
                  {t("login.submit")}
                </>
              )}
            </Button>
          </form>

          <div className="mt-6 flex flex-wrap items-center gap-2 border-t border-white/[0.07] pt-4 text-[11px] text-slate-500">
            <span className="flex items-center gap-1.5">
              <span className="h-1.5 w-1.5 rounded-full bg-emerald-300" />
              {t("account.engineNote")}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
