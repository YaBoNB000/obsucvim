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

interface AuthValue {
  user: User | null;
  remaining: number;
  usedToday: number;
  limit: number;
  login: (email: string, password: string) => void;
  logout: () => void;
  consumeQuota: () => void;
  outOfQuota: boolean;
}

const AuthContext = createContext<AuthValue | null>(null);
const USER_KEY = "obsucvim.user";
const QUOTA_KEY = "obsucvim.quota";

const today = () => new Date().toISOString().slice(0, 10);

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
      const raw = localStorage.getItem(QUOTA_KEY);
      const parsed = raw ? (JSON.parse(raw) as { date: string; used: number }) : null;
      return parsed && parsed.date === today() ? parsed.used : 0;
    } catch {
      return 0;
    }
  });

  // 日期变更即重置额度
  useEffect(() => {
    const s = JSON.stringify({ date: today(), used: usedToday });
    localStorage.setItem(QUOTA_KEY, s);
  }, [usedToday]);

  useEffect(() => {
    const timer = setInterval(() => {
      try {
        const raw = localStorage.getItem(QUOTA_KEY);
        const parsed = raw ? (JSON.parse(raw) as { date: string }) : null;
        if (parsed && parsed.date !== today()) setUsedToday(0);
      } catch {
        /* ignore */
      }
    }, 60_000);
    return () => clearInterval(timer);
  }, []);

  const login = useCallback((email: string, _password: string) => {
    const u: User = { email, nickname: email.split("@")[0], plan: "free" };
    localStorage.setItem(USER_KEY, JSON.stringify(u));
    setUser(u);
  }, []);

  const logout = useCallback(() => {
    localStorage.removeItem(USER_KEY);
    setUser(null);
  }, []);

  const consumeQuota = useCallback(() => setUsedToday((n) => Math.min(DAILY_FREE_QUOTA, n + 1)), []);

  const value = useMemo<AuthValue>(
    () => ({
      user,
      usedToday,
      limit: DAILY_FREE_QUOTA,
      remaining: Math.max(0, DAILY_FREE_QUOTA - usedToday),
      outOfQuota: usedToday >= DAILY_FREE_QUOTA,
      login,
      logout,
      consumeQuota,
    }),
    [user, usedToday, login, logout, consumeQuota],
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
  preset: ObfuscationOptions["preset"];
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

export function loadDefaults(fallback: ObfuscationOptions): ObfuscationOptions {
  try {
    const raw = localStorage.getItem(DEFAULTS_KEY);
    return raw ? ({ ...fallback, ...(JSON.parse(raw) as ObfuscationOptions) } as ObfuscationOptions) : fallback;
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
