import { useCallback, useEffect, useRef, useState } from "react";
import { Logo, LangSwitch } from "./Brand";
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
  const [captchaUnavailable, setCaptchaUnavailable] = useState(false);
  const [codeInput, setCodeInput] = useState("");
  const [demoCode, setDemoCode] = useState<string | null>(null);
  const [cooldown, setCooldown] = useState(0);
  const [lockLeft, setLockLeft] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const captchaRequest = useRef(0);
  const refreshCaptcha = useCallback(() => {
    const requestId = ++captchaRequest.current;
    setCaptcha(null);
    setCaptchaUnavailable(false);
    setCaptchaInput("");
    void fetchCaptcha().then((c) => {
      if (requestId === captchaRequest.current) {
        setCaptcha(c);
        setCaptchaUnavailable(!c);
      }
    });
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
                    : res.error === "ip_taken" ? "login.ipTaken"
                      : res.error === "register_expired" ? "verify.expired"
                        : "verify.wrong";
            errText(key);
            setCodeInput("");
            if (
              res.error === "code_expired" || res.error === "code_exhausted" ||
              res.error === "register_expired" || res.error === "ip_taken" || res.error === "email_taken"
            ) {
              setRegStep("form");
              setDemoCode(null);
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
              : res.error === "ip_taken" ? "login.ipTaken"
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
        else if (res.error === "ip_taken") {
          errText("login.ipTaken");
          setRegStep("form");
          setDemoCode(null);
          refreshCaptcha();
          setCaptchaInput("");
        } else if (res.error === "email_taken") {
          errText("login.emailTaken");
          setRegStep("form");
          setDemoCode(null);
          refreshCaptcha();
          setCaptchaInput("");
        }
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

  const field = "auth-input";

  const inCodeStep = mode === "register" && regStep === "code";
  const locked = lockLeft > 0;

  return (
    <main className="auth-stage relative flex min-h-screen items-center justify-center px-4 py-10 sm:px-6 sm:py-14">
      <div className="auth-ambience pointer-events-none fixed inset-0 overflow-hidden" aria-hidden="true">
        <div className="auth-glow auth-glow-cyan" />
        <div className="auth-glow auth-glow-violet" />
        <div className="auth-orbit auth-orbit-large"><span /></div>
        <div className="auth-orbit auth-orbit-small"><span /></div>
        <div className="auth-star auth-star-one" />
        <div className="auth-star auth-star-two" />
        <div className="auth-star auth-star-three" />
      </div>

      <section className="auth-card glass specular relative z-10 w-full max-w-[520px] overflow-hidden rounded-[30px] p-5 sm:rounded-[34px] sm:p-8 lg:p-9">
        <div className="auth-card-grid pointer-events-none absolute inset-0" aria-hidden="true" />
        <div className="auth-card-sheen pointer-events-none absolute inset-y-0 left-[-55%] w-[45%]" aria-hidden="true" />

        <div className="relative z-10">
          <header className="flex items-center justify-between gap-4">
            <Logo />
            <LangSwitch size="sm" />
          </header>

          {!inCodeStep ? (
            <>
              <div className="auth-heading mt-8 text-center sm:mt-9">
                <div className="auth-kicker mx-auto">
                  <span className="auth-kicker-dot" />
                  <span>{t("login.portal")}</span>
                  <span className="auth-kicker-divider" />
                  <span className="auth-kicker-product">{t("login.badge")}</span>
                </div>
                <h1 className="mt-4 text-[27px] font-semibold tracking-[-0.04em] text-slate-50 sm:text-[32px]">
                  {mode === "login" ? t("login.title") : t("login.registerTitle")}
                </h1>
                <p className="mx-auto mt-2 max-w-[390px] text-[12px] leading-relaxed text-slate-400/85 sm:text-[13px]">
                  {t("login.subtitle")}
                </p>
              </div>

              <div className="auth-mode-switch relative mt-7 grid grid-cols-2 rounded-2xl p-1" role="tablist" aria-label={t("login.portal")}>
                <span className={`auth-mode-indicator ${mode === "register" ? "is-register" : ""}`} aria-hidden="true" />
                {(["login", "register"] as const).map((m) => (
                  <button
                    key={m}
                    type="button"
                    role="tab"
                    aria-selected={mode === m}
                    onClick={() => switchMode(m)}
                    className={`auth-mode-tab relative z-10 rounded-xl px-3 py-3 text-[13px] font-semibold transition-colors duration-300 ${mode === m ? "is-active" : ""}`}
                  >
                    {m === "login" ? t("common.login") : t("common.register")}
                  </button>
                ))}
              </div>
            </>
          ) : (
            <div className="auth-verify-heading mt-8 sm:mt-9">
              <div className="auth-verify-icon">
                <Icon name="shield" className="h-6 w-6" />
              </div>
              <div className="min-w-0">
                <p className="auth-step-label">EMAIL VERIFICATION · 02</p>
                <h1 className="mt-1 text-[23px] font-semibold tracking-tight text-slate-50 sm:text-[27px]">
                  {t("verify.title")}
                </h1>
                <p className="mt-1.5 text-[12px] leading-relaxed text-slate-400/85">
                  {t("verify.sent", { email })}
                </p>
              </div>
            </div>
          )}

          <form onSubmit={submit} className="auth-form-enter mt-6 grid gap-4">
            {inCodeStep ? (
              <>
                {demoCode && (
                  <div className="auth-demo-code rounded-2xl px-4 py-3 font-mono text-[13px] tracking-[0.26em] text-amber-100">
                    {t("verify.demo", { code: demoCode })}
                  </div>
                )}
                <label className="auth-field-group block">
                  <span className="auth-field-label">{t("verify.code")}</span>
                  <input
                    value={codeInput}
                    onChange={(e) => setCodeInput(e.target.value.replace(/\D/g, "").slice(0, 6))}
                    placeholder={t("verify.codePh")}
                    inputMode="numeric"
                    autoFocus
                    className={`${field} text-center font-mono text-[19px] tracking-[0.48em]`}
                    disabled={busy}
                    autoComplete="one-time-code"
                  />
                </label>
                {err && (
                  <div className="auth-alert auth-alert-error" role="alert">
                    <Icon name="sparkle" className="mt-0.5 h-4 w-4 shrink-0" />
                    <span>{err}</span>
                  </div>
                )}
                <Button type="submit" disabled={busy || codeInput.length !== 6} className="auth-submit-button mt-1 w-full py-3.5">
                  {busy ? <span className="auth-spinner" /> : <Icon name="shield" className="h-4 w-4" />}
                  {busy ? t("login.working") : t("verify.verify")}
                </Button>
                <div className="flex items-center justify-between pt-1 text-[11.5px]">
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
                    className="auth-text-link"
                  >
                    ← {t("verify.back")}
                  </button>
                  <button
                    type="button"
                    onClick={doResend}
                    disabled={cooldown > 0 || busy}
                    className="auth-text-link disabled:cursor-not-allowed disabled:opacity-40"
                  >
                    {cooldown > 0 ? t("verify.resendIn", { s: cooldown }) : t("verify.resend")}
                  </button>
                </div>
              </>
            ) : (
              <>
                <label className="auth-field-group block">
                  <span className="auth-field-label">{t("login.email")}</span>
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
                  <label className="auth-field-group auth-field-reveal block">
                    <span className="auth-field-label">{t("login.nickname")}</span>
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

                <label className="auth-field-group block">
                  <span className="auth-field-label">{t("login.password")}</span>
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
                  <label className="auth-field-group auth-field-reveal block">
                    <span className="auth-field-label">{t("login.confirmPwd")}</span>
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

                <div className="auth-captcha-block">
                  <div className="mb-2 flex items-center justify-between gap-2">
                    <span className="auth-field-label !mb-0">{t("login.captcha")}</span>
                    <button type="button" onClick={refreshCaptcha} className="auth-text-link !text-[10.5px]">
                      {t("login.refresh")}
                    </button>
                  </div>
                  <Captcha
                    challenge={captcha}
                    onAnswerChange={setCaptchaInput}
                    disabled={locked}
                    failed={captchaUnavailable}
                    className="mx-auto"
                  />
                </div>

                {err && (
                  <div className="auth-alert auth-alert-error" role="alert">
                    <Icon name="sparkle" className="mt-0.5 h-4 w-4 shrink-0" />
                    <span>{locked ? t("login.locked", { s: lockLeft }) : err}</span>
                  </div>
                )}

                <Button type="submit" disabled={busy || locked} className="auth-submit-button mt-1 w-full py-3.5">
                  {busy ? <span className="auth-spinner" /> : <Icon name={mode === "login" ? "lock" : "bolt"} className="h-4 w-4" />}
                  {busy ? (
                    <span className="animate-pulse">{t("login.working")}</span>
                  ) : locked ? (
                    t("login.locked", { s: lockLeft })
                  ) : mode === "login" ? (
                    t("common.login")
                  ) : (
                    t("verify.send")
                  )}
                  {!busy && !locked && <Icon name="chevron" className="ml-1 h-4 w-4 -rotate-90" />}
                </Button>
              </>
            )}
          </form>

          {!inCodeStep && (
            <footer className="auth-footer mt-6 border-t border-white/[0.07] pt-4">
              <div className="flex items-start gap-2.5">
                <Icon name="shield" className="mt-0.5 h-4 w-4 shrink-0 text-aqua-300/80" />
                <p className="text-[10.5px] leading-relaxed text-slate-400/75">{t("login.secureNote")}</p>
              </div>
            </footer>
          )}
        </div>
      </section>
    </main>
  );
}
