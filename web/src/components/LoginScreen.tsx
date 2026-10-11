import { useCallback, useEffect, useState } from "react";
import { BlackHoleSVG, Logo, LangSwitch } from "./Brand";
import { Button, Icon } from "./ui";
import { Captcha } from "./Captcha";
import { useI18n, type TKey } from "../lib/i18n";
import { useAuth, type CaptchaChallenge } from "../lib/store";

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function LoginScreen() {
  const { t } = useI18n();
  const { login, register, confirmRegister, resendCode, fetchCaptcha } = useAuth();
  const [mode, setMode] = useState<"login" | "register">("login");
  const [regStep, setRegStep] = useState<"form" | "code">("form");
  const [email, setEmail] = useState("");
  const [nickname, setNickname] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPwd, setConfirmPwd] = useState("");
  const [captchaInput, setCaptchaInput] = useState("");
  const [captcha, setCaptcha] = useState<CaptchaChallenge | null>(null);
  const [codeInput, setCodeInput] = useState("");
  const [demoCode, setDemoCode] = useState<string | null>(null);
  const [cooldown, setCooldown] = useState(0);
  const [lockLeft, setLockLeft] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refreshCaptcha = useCallback(() => {
    void fetchCaptcha().then((c) => setCaptcha(c));
  }, [fetchCaptcha]);

  useEffect(() => {
    refreshCaptcha();
  }, [refreshCaptcha]);

  // 重发冷却倒计时
  useEffect(() => {
    if (cooldown <= 0) return;
    const timer = setTimeout(() => setCooldown((c) => c - 1), 1000);
    return () => clearTimeout(timer);
  }, [cooldown]);

  // 服务端锁定倒计时
  useEffect(() => {
    if (lockLeft <= 0) return;
    const timer = setTimeout(() => setLockLeft((s) => s - 1), 1000);
    return () => clearTimeout(timer);
  }, [lockLeft]);

  const errText = (key: TKey, vars?: Record<string, string | number>) => {
    setErr(t(key, vars));
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (lockLeft > 0) return;
    if (!EMAIL_RE.test(email)) {
      errText("login.invalid");
      return;
    }

    // ---------- 注册 ----------
    if (mode === "register") {
      if (regStep === "code") {
        if (codeInput.trim().length !== 6) {
          errText("verify.wrong");
          return;
        }
        setErr(null);
        setBusy(true);
        try {
          const res = await confirmRegister(email.trim(), codeInput.trim());
          if (!res.ok) {
            const key: TKey =
              res.error === "code_expired" ? "verify.expired"
                : res.error === "code_exhausted" ? "verify.exhausted"
                  : res.error === "email_taken" ? "login.emailTaken"
                    : res.error === "register_expired" ? "verify.expired"
                      : "verify.wrong";
            errText(key);
            setCodeInput("");
            if (res.error === "code_expired" || res.error === "code_exhausted" || res.error === "register_expired") {
              setRegStep("form");
              refreshCaptcha();
              setCaptchaInput("");
            }
          }
          // 成功 → AuthProvider 切换 user，本组件卸载
        } finally {
          setBusy(false);
        }
        return;
      }
      // 第一步：表单 + 服务端验证码
      if (password.length < 8) {
        errText("login.pwdShort");
        return;
      }
      if (password !== confirmPwd) {
        errText("login.pwdMismatch");
        return;
      }
      if (!captcha || !captchaInput.trim()) {
        errText("login.captchaFail");
        return;
      }
      setErr(null);
      setBusy(true);
      try {
        const res = await register(email.trim(), nickname.trim(), password, captcha.captchaId, captchaInput.trim());
        refreshCaptcha();
        setCaptchaInput("");
        if (!res.ok) {
          const key: TKey =
            res.error === "email_taken" ? "login.emailTaken"
              : res.error === "captcha_fail" ? "login.captchaFail"
                : res.error === "rate_limited" ? "login.busy"
                  : res.error === "resend_cooldown" ? "verify.resendIn"
                    : "login.invalid";
          errText(key, res.retryAfter ? { s: res.retryAfter } : undefined);
          return;
        }
        setDemoCode(res.demoCode ?? null);
        setCooldown(60);
        setCodeInput("");
        setRegStep("code");
      } finally {
        setBusy(false);
      }
      return;
    }

    // ---------- 登录 ----------
    if (password.length < 1) {
      errText("login.invalid");
      return;
    }
    if (!captcha || !captchaInput.trim()) {
      errText("login.captchaFail");
      return;
    }
    setErr(null);
    setBusy(true);
    try {
      const res = await login(email.trim(), password, captcha.captchaId, captchaInput.trim());
      refreshCaptcha();
      setCaptchaInput("");
      if (!res.ok) {
        if (res.error === "locked" && res.retryAfter) {
          setLockLeft(res.retryAfter);
          errText("login.locked", { s: res.retryAfter });
          return;
        }
        const key: TKey =
          res.error === "no_user" ? "login.noUser"
            : res.error === "captcha_fail" ? "login.captchaFail"
              : res.error === "rate_limited" ? "login.busy"
                : "login.badPassword";
        errText(key);
      }
    } finally {
      setBusy(false);
    }
  };

  const doResend = async () => {
    if (cooldown > 0 || busy) return;
    setBusy(true);
    try {
      const res = await resendCode(email.trim());
      if (!res.ok) {
        if (res.error === "resend_cooldown" && res.retryAfter) setCooldown(res.retryAfter);
        return;
      }
      setDemoCode(res.demoCode ?? null);
      setCooldown(60);
      setErr(null);
      setCodeInput("");
    } finally {
      setBusy(false);
    }
  };

  const switchMode = (m: "login" | "register") => {
    setMode(m);
    setRegStep("form");
    setDemoCode(null);
    setErr(null);
    setCodeInput("");
    setCaptchaInput("");
    refreshCaptcha();
  };

  const field =
    "w-full rounded-xl border border-white/[0.09] bg-black/30 px-3 py-2.5 text-[13px] text-aqua-100 placeholder:text-slate-600 focus:border-aqua-300/45 focus:outline-none";

  const inCodeStep = mode === "register" && regStep === "code";
  const locked = lockLeft > 0;

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

          {!inCodeStep && (
            <div className="mb-5 flex items-center gap-1 rounded-xl border border-white/[0.08] bg-black/25 p-1">
              {(["login", "register"] as const).map((m) => (
                <button
                  key={m}
                  type="button"
                  onClick={() => switchMode(m)}
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
          )}

          <form onSubmit={submit} className="grid gap-3.5">
            {inCodeStep ? (
              <>
                <div className="flex items-center gap-2.5">
                  <Icon name="shield" className="h-4 w-4 text-aqua-300" />
                  <h2 className="text-[14px] font-semibold text-slate-100">{t("verify.title")}</h2>
                </div>
                <p className="text-[12px] leading-relaxed text-slate-400/85">{t("verify.sent", { email })}</p>
                {demoCode && (
                  <div className="rounded-xl border border-amber-300/25 bg-amber-400/[0.07] px-3 py-2.5 font-mono text-[12.5px] tracking-[0.3em] text-amber-200">
                    {t("verify.demo", { code: demoCode })}
                  </div>
                )}
                <label className="block">
                  <span className="mb-1.5 block text-[11.5px] text-slate-400/80">{t("verify.code")}</span>
                  <input
                    value={codeInput}
                    onChange={(e) => setCodeInput(e.target.value.replace(/\D/g, "").slice(0, 6))}
                    placeholder={t("verify.codePh")}
                    inputMode="numeric"
                    autoFocus
                    className={field + " text-center font-mono text-[18px] tracking-[0.5em]"}
                    disabled={busy}
                    autoComplete="one-time-code"
                  />
                </label>
                {err && (
                  <div className="flex items-start gap-2 rounded-xl border border-rose-400/25 bg-rose-500/[0.08] px-3 py-2.5 text-[12px] text-rose-200">
                    <Icon name="sparkle" className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                    {err}
                  </div>
                )}
                <Button type="submit" disabled={busy || codeInput.length !== 6} className="mt-1 w-full py-2.5">
                  {busy ? t("login.working") : t("verify.verify")}
                </Button>
                <div className="flex items-center justify-between text-[11px]">
                  <button
                    type="button"
                    onClick={() => {
                      setRegStep("form");
                      setDemoCode(null);
                      setCodeInput("");
                      setErr(null);
                      refreshCaptcha();
                      setCaptchaInput("");
                    }}
                    className="cursor-pointer text-slate-500 transition hover:text-aqua-200"
                  >
                    ← {t("verify.back")}
                  </button>
                  <button
                    type="button"
                    onClick={doResend}
                    disabled={cooldown > 0 || busy}
                    className={
                      "cursor-pointer transition " +
                      (cooldown > 0 ? "text-slate-600" : "text-slate-500 hover:text-aqua-200")
                    }
                  >
                    {cooldown > 0 ? t("verify.resendIn", { s: cooldown }) : t("verify.resend")}
                  </button>
                </div>
              </>
            ) : (
              <>
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
                      onChange={(e) => setCaptchaInput(e.target.value.replace(/[^\d-]/g, "").slice(0, 6))}
                      placeholder={t("login.captchaPh")}
                      inputMode="numeric"
                      className={field + " font-mono tracking-[0.2em]"}
                      disabled={locked}
                      autoComplete="off"
                    />
                    <Captcha question={captcha?.question ?? null} onRefresh={refreshCaptcha} />
                  </div>
                  <button
                    type="button"
                    onClick={refreshCaptcha}
                    className="mt-1.5 cursor-pointer text-[10.5px] text-slate-500 transition hover:text-aqua-200"
                  >
                    {t("login.refresh")}
                  </button>
                </div>

                {err && (
                  <div className="flex items-start gap-2 rounded-xl border border-rose-400/25 bg-rose-500/[0.08] px-3 py-2.5 text-[12px] text-rose-200">
                    <Icon name="sparkle" className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                    {locked ? t("login.locked", { s: lockLeft }) : err}
                  </div>
                )}

                <Button type="submit" disabled={busy || locked} className="mt-1 w-full py-2.5">
                  {busy ? (
                    <span className="animate-pulse">{t("login.working")}</span>
                  ) : locked ? (
                    t("login.locked", { s: lockLeft })
                  ) : mode === "login" ? (
                    t("common.login")
                  ) : (
                    t("verify.send")
                  )}
                </Button>
              </>
            )}
          </form>

          {!inCodeStep && <p className="mt-5 text-[10.5px] leading-relaxed text-slate-500">{t("login.secureNote")}</p>}
        </div>
      </div>
    </div>
  );
}
