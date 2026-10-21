import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { DAILY_FREE_QUOTA, type ObfuscationOptions } from "./engine";
import { clearLegacyAuthData } from "./token";

/* ------------------------------------------------------------------ */
/*            账户 + 每日免费额度（全部服务端裁决）                      */
/*  会话/密码/验证码/锁定/额度都在服务端裁决。会话仅由 HttpOnly Cookie 传输， */
/*  JavaScript 无法读取或伪造登录凭证。                                       */
/* ------------------------------------------------------------------ */
export interface User {
  email: string;
  nickname: string;
  plan: "free";
}

export interface QuotaInfo {
  usedToday: number;
  remaining: number;
  limit: number;
}

export type CaptchaChallenge =
  | {
      provider: "puzzle";
      captchaId: string;
      backgroundUrl: string;
      pieceUrl: string;
      width: number;
      height: number;
      pieceSize: number;
      pieceY: number;
      maxOffset: number;
    }
  | {
      provider: "turnstile";
      captchaId: string;
      siteKey: string;
    };

export type AuthError =
  | "bad_input"
  | "email_taken"
  | "ip_taken"
  | "no_user"
  | "bad_password"
  | "locked"
  | "captcha_fail"
  | "rate_limited"
  | "network"
  | "code_wrong"
  | "code_expired"
  | "code_exhausted"
  | "code_none"
  | "register_expired"
  | "resend_cooldown";

interface AuthResult {
  ok: boolean;
  error?: AuthError;
  retryAfter?: number;
  demoCode?: string;
}

interface AuthValue {
  user: User | null;
  remaining: number;
  usedToday: number;
  limit: number;
  fetchCaptcha: () => Promise<CaptchaChallenge | null>;
  register: (email: string, nickname: string, password: string, captchaId: string, captchaAnswer: string) => Promise<AuthResult>;
  confirmRegister: (email: string, code: string) => Promise<AuthResult>;
  resendCode: (email: string) => Promise<AuthResult>;
  login: (email: string, password: string, captchaId: string, captchaAnswer: string) => Promise<AuthResult>;
  logout: () => void;
  applyQuota: (q: QuotaInfo) => void;
  refreshQuota: () => void;
  outOfQuota: boolean;
}

const AuthContext = createContext<AuthValue | null>(null);

const ERROR_MAP: Record<string, AuthError> = {
  BAD_INPUT: "bad_input",
  EMAIL_TAKEN: "email_taken",
  IP_ALREADY_REGISTERED: "ip_taken",
  NO_USER: "no_user",
  BAD_PASSWORD: "bad_password",
  LOCKED: "locked",
  CAPTCHA_FAIL: "captcha_fail",
  RATE_LIMITED: "rate_limited",
  CODE_WRONG: "code_wrong",
  CODE_EXPIRED: "code_expired",
  CODE_EXHAUSTED: "code_exhausted",
  CODE_NONE: "code_none",
  REGISTER_EXPIRED: "register_expired",
  RESEND_COOLDOWN: "resend_cooldown",
};

async function apiPost<T>(url: string, body: unknown): Promise<{ status: number; data: T }> {
  const res = await fetch(url, {
    method: "POST",
    credentials: "same-origin",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  return { status: res.status, data: (await res.json()) as T };
}

interface ApiEnvelope {
  ok?: boolean;
  error?: string;
  retryAfter?: number;
  demoCode?: string;
  user?: User;
  quota?: QuotaInfo;
  captchaId?: string;
  provider?: "puzzle" | "turnstile";
  siteKey?: string;
  backgroundUrl?: string;
  pieceUrl?: string;
  width?: number;
  height?: number;
  pieceSize?: number;
  pieceY?: number;
  maxOffset?: number;
}

function mapError(d: ApiEnvelope, status: number): AuthError {
  if (d.error && ERROR_MAP[d.error]) return ERROR_MAP[d.error];
  if (status === 401) return "no_user";
  return "bad_input";
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [quota, setQuota] = useState<QuotaInfo>({ usedToday: 0, remaining: DAILY_FREE_QUOTA, limit: DAILY_FREE_QUOTA });
  const [booted, setBooted] = useState(false);

  // 启动时先删旧版 localStorage 令牌，再通过 HttpOnly Cookie 恢复会话。
  useEffect(() => {
    let alive = true;
    clearLegacyAuthData();
    void (async () => {
      try {
        const res = await fetch("/api/me", { credentials: "same-origin" });
        const d = (await res.json()) as ApiEnvelope;
        if (!alive) return;
        if (res.ok && d.ok && d.user) {
          setUser(d.user);
          if (d.quota) setQuota(d.quota);
        }
      } catch {
        /* 后端不可达：保持未登录 */
      } finally {
        if (alive) setBooted(true);
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  const fetchCaptcha = useCallback(async (): Promise<CaptchaChallenge | null> => {
    try {
      const { data } = await apiPost<ApiEnvelope>("/api/auth/captcha", {});
      if (!data.ok || !data.captchaId) return null;
      if (data.provider === "turnstile" && data.siteKey) {
        return { provider: "turnstile", captchaId: data.captchaId, siteKey: data.siteKey };
      }
      if (
        data.provider === "puzzle" && data.backgroundUrl && data.pieceUrl &&
        Number.isInteger(data.width) && Number.isInteger(data.height) &&
        Number.isInteger(data.pieceSize) && Number.isInteger(data.pieceY) &&
        Number.isInteger(data.maxOffset)
      ) {
        return {
          provider: "puzzle",
          captchaId: data.captchaId,
          backgroundUrl: data.backgroundUrl,
          pieceUrl: data.pieceUrl,
          width: data.width!,
          height: data.height!,
          pieceSize: data.pieceSize!,
          pieceY: data.pieceY!,
          maxOffset: data.maxOffset!,
        };
      }
      return null;
    } catch {
      return null;
    }
  }, []);

  const register = useCallback(
    async (email: string, nickname: string, password: string, captchaId: string, captchaAnswer: string): Promise<AuthResult> => {
      try {
        const { status, data } = await apiPost<ApiEnvelope>("/api/auth/register", {
          email,
          nickname,
          password,
          captchaId,
          captchaAnswer,
        });
        if (data.ok) return { ok: true, demoCode: data.demoCode };
        return { ok: false, error: mapError(data, status), retryAfter: data.retryAfter };
      } catch {
        return { ok: false, error: "network" };
      }
    },
    [],
  );

  const confirmRegister = useCallback(async (email: string, code: string): Promise<AuthResult> => {
    try {
      const { status, data } = await apiPost<ApiEnvelope>("/api/auth/register/confirm", { email, code });
      if (data.ok && data.user) {
        setUser(data.user);
        if (data.quota) setQuota(data.quota);
        return { ok: true };
      }
      return { ok: false, error: mapError(data, status), retryAfter: data.retryAfter };
    } catch {
      return { ok: false, error: "network" };
    }
  }, []);

  const resendCode = useCallback(async (email: string): Promise<AuthResult> => {
    try {
      const { status, data } = await apiPost<ApiEnvelope>("/api/auth/resend", { email });
      if (data.ok) return { ok: true, demoCode: data.demoCode };
      return { ok: false, error: mapError(data, status), retryAfter: data.retryAfter };
    } catch {
      return { ok: false, error: "network" };
    }
  }, []);

  const login = useCallback(
    async (email: string, password: string, captchaId: string, captchaAnswer: string): Promise<AuthResult> => {
      try {
        const { status, data } = await apiPost<ApiEnvelope>("/api/auth/login", {
          email,
          password,
          captchaId,
          captchaAnswer,
        });
        if (data.ok && data.user) {
          setUser(data.user);
          if (data.quota) setQuota(data.quota);
          return { ok: true };
        }
        return { ok: false, error: mapError(data, status), retryAfter: data.retryAfter };
      } catch {
        return { ok: false, error: "network" };
      }
    },
    [],
  );

  const logout = useCallback(() => {
    void apiPost("/api/auth/logout", {}).catch(() => {});
    clearLegacyAuthData();
    setUser(null);
    setQuota({ usedToday: 0, remaining: DAILY_FREE_QUOTA, limit: DAILY_FREE_QUOTA });
  }, []);

  const applyQuota = useCallback((q: QuotaInfo) => setQuota(q), []);

  const refreshQuota = useCallback(() => {
    if (!user) return;
    void (async () => {
      try {
        const res = await fetch("/api/me", { credentials: "same-origin" });
        const d = (await res.json()) as ApiEnvelope;
        if (res.ok && d.quota) setQuota(d.quota);
        else if (res.status === 401) {
          clearLegacyAuthData();
          setUser(null);
        }
      } catch {
        /* ignore */
      }
    })();
  }, [user]);

  // 页面获得焦点时刷新额度（防止长时间停留后状态过期）
  useEffect(() => {
    const onFocus = () => refreshQuota();
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") onFocus();
    });
    return () => window.removeEventListener("focus", onFocus);
  }, [refreshQuota]);

  // 引擎层 401 → 清用户状态回登录页（与 Workspace 的 logout 双保险）
  useEffect(() => {
    const onUnauth = () => {
      clearLegacyAuthData();
      setUser(null);
      setQuota({ usedToday: 0, remaining: DAILY_FREE_QUOTA, limit: DAILY_FREE_QUOTA });
    };
    window.addEventListener("obsucvim:unauthorized", onUnauth);
    return () => window.removeEventListener("obsucvim:unauthorized", onUnauth);
  }, []);

  const value = useMemo<AuthValue>(
    () => ({
      user: booted ? user : null,
      usedToday: quota.usedToday,
      limit: quota.limit,
      remaining: quota.remaining,
      outOfQuota: quota.remaining <= 0,
      fetchCaptcha,
      register,
      confirmRegister,
      resendCode,
      login,
      logout,
      applyQuota,
      refreshQuota,
    }),
    [booted, user, quota, fetchCaptcha, register, confirmRegister, resendCode, login, logout, applyQuota, refreshQuota],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used inside AuthProvider");
  return ctx;
}

/* ------------------------------------------------------------------ */
/*                            出包记录                                 */
/* ------------------------------------------------------------------ */
export interface HistoryItem {
  id: number;
  timestamp: number;
  seed: string;
  inputBytes: number;
  outputBytes: number;
  code: string;
  options: ObfuscationOptions;
}

interface HistoryValue {
  items: HistoryItem[];
  push: (item: Omit<HistoryItem, "id" | "timestamp">) => void;
  clear: () => void;
}

const HistoryContext = createContext<HistoryValue | null>(null);
const HISTORY_KEY = "obsucvim.history";
const HISTORY_MAX = 5;

export function HistoryProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<HistoryItem[]>(() => {
    try {
      const raw = localStorage.getItem(HISTORY_KEY);
      return raw ? (JSON.parse(raw) as HistoryItem[]) : [];
    } catch {
      return [];
    }
  });

  const persist = useCallback((next: HistoryItem[]) => {
    setItems(next);
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
    } catch {
      try {
        localStorage.setItem(HISTORY_KEY, JSON.stringify(next.slice(0, 2)));
      } catch {
        /* ignore */
      }
    }
  }, []);

  const push = useCallback(
    (item: Omit<HistoryItem, "id" | "timestamp">) => {
      const full: HistoryItem = { ...item, id: Date.now(), timestamp: Date.now() };
      persist([full, ...items].slice(0, HISTORY_MAX));
    },
    [items, persist],
  );

  const clear = useCallback(() => persist([]), [persist]);

  return <HistoryContext.Provider value={{ items, push, clear }}>{children}</HistoryContext.Provider>;
}

export function useHistory() {
  const ctx = useContext(HistoryContext);
  if (!ctx) throw new Error("useHistory must be used inside HistoryProvider");
  return ctx;
}

/* ------------------------------------------------------------------ */
/*                          默认参数（设置页）                          */
/* ------------------------------------------------------------------ */
const DEFAULTS_KEY = "obsucvim.defaults";

const OPTION_KEYS = [
  "minify", "mangle", "strings", "flatten", "junk", "numbers",
  "body", "vmSealStrings", "seed",
] as const;

export function loadDefaults(fallback: ObfuscationOptions): ObfuscationOptions {
  try {
    const raw = localStorage.getItem(DEFAULTS_KEY);
    if (!raw) return fallback;
    const saved = JSON.parse(raw) as Record<string, unknown>;
    if (!saved || typeof saved !== "object" || Array.isArray(saved)) return fallback;
    const picked: Partial<ObfuscationOptions> = {};
    for (const k of OPTION_KEYS) {
      if (k in saved && typeof saved[k] === typeof fallback[k]) (picked as Record<string, unknown>)[k] = saved[k];
    }
    return { ...fallback, ...picked };
  } catch {
    return fallback;
  }
}

export function saveDefaults(options: ObfuscationOptions) {
  try {
    localStorage.setItem(DEFAULTS_KEY, JSON.stringify(options));
  } catch {
    /* ignore */
  }
}
