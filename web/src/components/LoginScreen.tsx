import { useRef, useState } from "react";
import { BlackHoleSVG, Logo, LangSwitch } from "./Brand";
import { Button, Icon } from "./ui";
import { Captcha, type CaptchaHandle } from "./Captcha";
import { useI18n, type TKey } from "../lib/i18n";
import { useAuth } from "../lib/store";

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function LoginScreen() {
  const { t } = useI18n();
  const { login, register, lockRemaining } = useAuth();
  const [mode, setMode] = useState<"login" | "register">("login");
  const [email, setEmail] = useState("");
  const [nickname, setNickname] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPwd, setConfirmPwd] = useState("");
  const [captchaInput, setCaptchaInput] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const captchaRef = useRef<CaptchaHandle>(null);

  const locked = lockRemaining > 0;

  const fail = (key: TKey) => {
    setErr(t(key));
    captchaRef.current?.refresh();
    setCaptchaInput("");
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (locked) return;
    if (!EMAIL_RE.test(email)) {
      fail("login.invalid");
      return;
    }
    if (mode === "register") {
      if (password.length < 8) {
        fail("login.pwdShort");
        return;
      }
      if (password !== confirmPwd) {
        fail("login.pwdMismatch");
        return;
      }
    } else if (password.length < 1) {
      fail("login.invalid");
      return;
    }
    if (!captchaRef.current?.check(captchaInput)) {
      fail("login.captchaFail");
      return;
    }
    setErr(null);
    setBusy(true);
    try {
      const res =
        mode === "register"
          ? await register(email.trim(), nickname.trim(), password)
          : await login(email.trim(), password);
      if (!res.ok) {
        const key: TKey =
          res.error === "email_taken"
            ? "login.emailTaken"
            : res.error === "no_user"
              ? "login.noUser"
              : res.error === "locked"
                ? "login.locked"
                : "login.badPassword";
        fail(key);
      }
      // 成功则 AuthProvider 切换 user，本组件自动卸载
    } finally {
      setBusy(false);
    }
  };

  const field =
    "w-full rounded-xl border border-white/[0.09] bg-black/30 px-3 py-2.5 text-[13px] text-aqua-100 placeholder:text-slate-600 focus:border-aqua-300/45 focus:outline-none";

  return (
    <div className="relative flex min-h-screen items-center justify-center px-5 py-10">
      <div className="absolute top-5 right-5 z-10">
        <LangSwitch size="md" />
      </div>

      <div className="grid w-full max-w-4xl gap-6 lg:grid-cols-[1fr_1.05fr]">
        {/* 视觉侧 */}
        <div className="glass specular relative hidden flex-col justify-between overflow-hidden rounded-3xl p-8 lg:flex">
          <Logo />
          <div className="relative my-6 flex items-center justify-center">
            <div className="animate-drift-slow absolute h-64 w-64 rounded-full bg-[radial-gradient(circle,rgba(34,226,213,0.35),transparent_66%)] blur-2xl" />
            <img
              src="/blackhole.png"
              alt=""
              className="animate-floaty relative h-52 w-52 rounded-full object-cover opacity-90"
              draggable={false}
            />
          </div>
          <BlackHoleSVG />
        </div>

        {/* 表单侧 */}
        <div className="glass flex flex-col justify-center rounded-3xl p-7 sm:p-9">
          <div className="mb-6 lg:hidden">
            <Logo />
          </div>

          <div className="mb-5 flex items-center gap-1 rounded-xl border border-white/[0.08] bg-black/25 p-1">
            {(["login", "register"] as const).map((m) => (
              <button
                key={m}
                type="button"
                onClick={() => {
                  setMode(m);
                  setErr(null);
                  captchaRef.current?.refresh();
                  setCaptchaInput("");
                }}
                className={
                  "flex-1 cursor-pointer rounded-lg px-3 py-2 text-[13px] font-medium transition " +
                  (mode === m
                    ? "bg-aqua-400/20 text-aqua-100 shadow-[0_0_24px_-10px_rgba(34,226,213,0.95)_inset]"
                    : "text-slate-400 hover:text-aqua-100")
                }
              >
                {m === "login" ? t("common.login") : t("common.register")}
              </button>
            ))}
          </div>

          <form onSubmit={submit} className="grid gap-3.5">
            <label className="block">
              <span className="mb-1.5 block text-[11.5px] text-slate-400/80">{t("login.email")}</span>
              <input
                type="email"
                autoComplete="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
                className={field}
                disabled={locked}
              />
            </label>

            {mode === "register" && (
              <label className="block">
                <span className="mb-1.5 block text-[11.5px] text-slate-400/80">{t("login.nickname")}</span>
                <input
                  value={nickname}
                  onChange={(e) => setNickname(e.target.value)}
                  placeholder={t("login.nicknamePh")}
                  className={field}
                  disabled={locked}
                  maxLength={24}
                />
              </label>
            )}

            <label className="block">
              <span className="mb-1.5 block text-[11.5px] text-slate-400/80">{t("login.password")}</span>
              <input
                type="password"
                autoComplete={mode === "login" ? "current-password" : "new-password"}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={mode === "register" ? t("login.pwdRule") : "••••••••"}
                className={field}
                disabled={locked}
              />
            </label>

            {mode === "register" && (
              <label className="block">
                <span className="mb-1.5 block text-[11.5px] text-slate-400/80">{t("login.confirmPwd")}</span>
                <input
                  type="password"
                  autoComplete="new-password"
                  value={confirmPwd}
                  onChange={(e) => setConfirmPwd(e.target.value)}
                  placeholder="••••••••"
                  className={field}
                  disabled={locked}
                />
              </label>
            )}

            <div>
              <span className="mb-1.5 block text-[11.5px] text-slate-400/80">{t("login.captcha")}</span>
              <div className="flex items-center gap-2.5">
                <input
                  value={captchaInput}
                  onChange={(e) => setCaptchaInput(e.target.value)}
                  placeholder={t("login.captchaPh")}
                  maxLength={8}
                  className={field + " font-mono tracking-[0.25em] uppercase"}
                  disabled={locked}
                  autoComplete="off"
                />
                <Captcha ref={captchaRef} />
              </div>
              <button
                type="button"
                onClick={() => captchaRef.current?.refresh()}
                className="mt-1.5 cursor-pointer text-[10.5px] text-slate-500 transition hover:text-aqua-200"
              >
                {t("login.refresh")}
              </button>
            </div>

            {err && (
              <div className="flex items-start gap-2 rounded-xl border border-rose-400/25 bg-rose-500/[0.08] px-3 py-2.5 text-[12px] text-rose-200">
                <Icon name="sparkle" className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                {locked ? t("login.locked", { s: lockRemaining }) : err}
              </div>
            )}

            <Button type="submit" disabled={busy || locked} className="mt-1 w-full py-2.5">
              {busy ? (
                <span className="animate-pulse">{t("login.working")}</span>
              ) : locked ? (
                t("login.locked", { s: lockRemaining })
              ) : mode === "login" ? (
                t("common.login")
              ) : (
                t("common.register")
              )}
            </Button>
          </form>

          <p className="mt-5 text-[10.5px] leading-relaxed text-slate-500">{t("login.secureNote")}</p>
        </div>
      </div>
    </div>
  );
}
