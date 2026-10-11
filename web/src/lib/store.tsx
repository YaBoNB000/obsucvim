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

/* ------------------------------------------------------------------ */
/*                        账户 + 每日免费额度                          */
/* ------------------------------------------------------------------ */
export interface User {
  email: string;
  nickname: string;
  plan: "free";
}

export type AuthError =
  | "bad_input"
  | "email_taken"
  | "no_user"
  | "bad_password"
  | "locked";

interface AuthResult {
  ok: boolean;
  error?: AuthError;
}

interface AuthValue {
  user: User | null;
  remaining: number;
  usedToday: number;
  limit: number;
  register: (email: string, nickname: string, password: string) => Promise<AuthResult>;
  login: (email: string, password: string) => Promise<AuthResult>;
  logout: () => void;
  consumeQuota: () => void;
  outOfQuota: boolean;
  /** 登录锁定剩余秒数（连续失败 5 次触发） */
  lockRemaining: number;
}

/* ---------------- 本地账户注册表（密码加盐哈希，绝不存明文） ---------------- */
interface StoredUser {
  email: string;
  nickname: string;
  salt: string;
  hash: string;
  created: number;
}

const USERS_KEY = "obsucvim.users";
const LOCK_KEY = "obsucvim.authlock";
const MAX_FAILS = 5;
const LOCK_SECONDS = 60;

function randomSalt(): string {
  const bytes = new Uint8Array(16);
  if (globalThis.crypto?.getRandomValues) {
    globalThis.crypto.getRandomValues(bytes);
  } else {
    for (let i = 0; i < bytes.length; i++) bytes[i] = Math.floor(Math.random() * 256);
  }
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

async function hashPassword(password: string, salt: string): Promise<string> {
  const data = new TextEncoder().encode(salt + ":" + password);
  if (globalThis.crypto?.subtle) {
    const digest = await globalThis.crypto.subtle.digest("SHA-256", data);
    return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, "0")).join("");
  }
  // 非安全上下文降级（四轮 FNV）：弱于 SHA-256，但仍避免明文落盘
  let h1 = 0x811c9dc5 ^ 0xdeadbeef;
  let h2 = 0x01000193 ^ 0x41c6ce57;
  for (const b of data) {
    h1 = Math.imul(h1 ^ b, 16777619) >>> 0;
    h2 = Math.imul(h2 + b + 1, 2246822519) >>> 0;
    h1 = (h1 ^ h2) >>> 0;
  }
  return h1.toString(16).padStart(8, "0") + h2.toString(16).padStart(8, "0") + (h1 ^ h2).toString(16).padStart(8, "0");
}

function loadUsers(): Record<string, StoredUser> {
  try {
    const raw = localStorage.getItem(USERS_KEY);
    const parsed = raw ? (JSON.parse(raw) as Record<string, StoredUser>) : {};
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
}

function saveUsers(users: Record<string, StoredUser>) {
  try {
    localStorage.setItem(USERS_KEY, JSON.stringify(users));
  } catch {
    /* ignore */
  }
}

function readLock(): { fails: number; until: number } {
  try {
    const raw = localStorage.getItem(LOCK_KEY);
    const parsed = raw ? (JSON.parse(raw) as { fails: number; until: number }) : null;
    return parsed ?? { fails: 0, until: 0 };
  } catch {
    return { fails: 0, until: 0 };
  }
}

function writeLock(lock: { fails: number; until: number }) {
  try {
    localStorage.setItem(LOCK_KEY, JSON.stringify(lock));
  } catch {
    /* ignore */
  }
}

const AuthContext = createContext<AuthValue | null>(null);
const USER_KEY = "obsucvim.user";
/* 额度按账号独立计数：obsucvim.quota.<email>。
 * 旧的全局键 obsucvim.quota 已废弃（首次加载时清除）。 */
const LEGACY_QUOTA_KEY = "obsucvim.quota";
const quotaKey = (email: string) => `obsucvim.quota.${email.trim().toLowerCase()}`;

const today = () => new Date().toISOString().slice(0, 10);

function user0(): User | null {
  try {
    const raw = localStorage.getItem(USER_KEY);
    return raw ? (JSON.parse(raw) as User) : null;
  } catch {
    return null;
  }
}

function readQuota(email: string): number {
  try {
    const raw = localStorage.getItem(quotaKey(email));
    const parsed = raw ? (JSON.parse(raw) as { date: string; used: number }) : null;
    return parsed && parsed.date === today() ? parsed.used : 0;
  } catch {
    return 0;
  }
}

function writeQuota(email: string, used: number) {
  try {
    localStorage.setItem(quotaKey(email), JSON.stringify({ date: today(), used }));
  } catch {
    /* ignore */
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(() => {
    try {
      const raw = localStorage.getItem(USER_KEY);
      return raw ? (JSON.parse(raw) as User) : null;
    } catch {
      return null;
    }
  });
  const [usedToday, setUsedToday] = useState<number>(() => {
    try {
      localStorage.removeItem(LEGACY_QUOTA_KEY); // 旧全局计数废弃
    } catch {
      /* ignore */
    }
    return user0() ? readQuota(user0()!.email) : 0;
  });

  // 登录/登出/注册切换账号 → 重载该账号的当日额度
  const userEmail = user?.email;
  useEffect(() => {
    if (userEmail) setUsedToday(readQuota(userEmail));
    else setUsedToday(0);
  }, [userEmail]);

  // 日期变更即重置额度
  useEffect(() => {
    const timer = setInterval(() => {
      if (userEmail) setUsedToday(readQuota(userEmail));
    }, 60_000);
    return () => clearInterval(timer);
  }, [userEmail]);

  const [lockRemaining, setLockRemaining] = useState(0);

  useEffect(() => {
    const tick = () => {
      const lock = readLock();
      setLockRemaining(Math.max(0, Math.ceil((lock.until - Date.now()) / 1000)));
    };
    tick();
    const timer = setInterval(tick, 1000);
    return () => clearInterval(timer);
  }, []);

  const signIn = useCallback((u: StoredUser) => {
    const session: User = { email: u.email, nickname: u.nickname, plan: "free" };
    localStorage.setItem(USER_KEY, JSON.stringify(session));
    writeLock({ fails: 0, until: 0 });
    setUser(session);
  }, []);

  const register = useCallback(
    async (email: string, nickname: string, password: string): Promise<AuthResult> => {
      const key = email.trim().toLowerCase();
      const users = loadUsers();
      if (users[key]) return { ok: false, error: "email_taken" };
      const salt = randomSalt();
      const hash = await hashPassword(password, salt);
      const su: StoredUser = { email: key, nickname: nickname.trim() || key.split("@")[0], salt, hash, created: Date.now() };
      users[key] = su;
      saveUsers(users);
      signIn(su);
      return { ok: true };
    },
    [signIn],
  );

  const login = useCallback(
    async (email: string, password: string): Promise<AuthResult> => {
      const lock = readLock();
      if (lock.until > Date.now()) return { ok: false, error: "locked" };
      const key = email.trim().toLowerCase();
      const users = loadUsers();
      const su = users[key];
      if (!su) return { ok: false, error: "no_user" };
      const hash = await hashPassword(password, su.salt);
      if (hash !== su.hash) {
        const fails = lock.fails + 1;
        writeLock({ fails, until: fails >= MAX_FAILS ? Date.now() + LOCK_SECONDS * 1000 : 0 });
        if (fails >= MAX_FAILS) setLockRemaining(LOCK_SECONDS);
        return { ok: false, error: fails >= MAX_FAILS ? "locked" : "bad_password" };
      }
      signIn(su);
      return { ok: true };
    },
    [signIn],
  );

  const logout = useCallback(() => {
    localStorage.removeItem(USER_KEY);
    setUser(null);
  }, []);

  const consumeQuota = useCallback(() => {
    setUsedToday((n) => {
      const next = Math.min(DAILY_FREE_QUOTA, n + 1);
      if (user) writeQuota(user.email, next);
      return next;
    });
  }, [user]);

  const value = useMemo<AuthValue>(
    () => ({
      user,
      usedToday,
      limit: DAILY_FREE_QUOTA,
      remaining: Math.max(0, DAILY_FREE_QUOTA - usedToday),
      outOfQuota: usedToday >= DAILY_FREE_QUOTA,
      register,
      login,
      logout,
      consumeQuota,
      lockRemaining,
    }),
    [user, usedToday, register, login, logout, consumeQuota, lockRemaining],
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
  push: (item: ObfuscationItemInput) => void;
  clear: () => void;
}

export type ObfuscationItemInput = Omit<HistoryItem, "id" | "timestamp" | "seed"> & { seed: string };

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

  const persist = (next: HistoryItem[]) => {
    setItems(next);
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
    } catch {
      // 超出存储配额时保留前 2 条再试一次
      try {
        localStorage.setItem(HISTORY_KEY, JSON.stringify(next.slice(0, 2)));
      } catch {
        /* ignore */
      }
    }
  };

  const push = useCallback(
    (item: ObfuscationItemInput) => {
      const full: HistoryItem = { ...item, id: Date.now(), timestamp: Date.now() };
      setItems((prev) => {
        const next = [full, ...prev].slice(0, HISTORY_MAX);
        try {
          localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
        } catch {
          try {
            localStorage.setItem(HISTORY_KEY, JSON.stringify(next.slice(0, 2)));
          } catch {
            /* ignore */
          }
        }
        return next;
      });
    },
    [],
  );

  const clear = useCallback(() => persist([]), []);

  return (
    <HistoryContext.Provider value={{ items, push, clear }}>{children}</HistoryContext.Provider>
  );
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
