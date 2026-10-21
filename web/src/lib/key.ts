/**
 * 卡密自助查询 / 自助重置设备（公开端点，凭卡密本身鉴权）。
 * 不需要登录，调用方只需持有自己的卡密。
 */

export type KeyState = "active" | "expired" | "revoked";

export interface KeyStatus {
  state: KeyState;
  note: string;
  scriptName: string;
  folderName: string;
  expiresAt: number | null;
  durationDays: number | null;
  secondsLeft: number | null;
  deviceBound: boolean;
  firstActivatedAt: number | null;
  lastUsedAt: number | null;
  totalExecutions: number;
  selfReset: boolean;
  resetCooldownDays: number;
  resetAvailableIn: number;
}

export type KeyError =
  | "invalid"
  | "rate_limited"
  | "self_reset_disabled"
  | "reset_cooldown"
  | "bad_input"
  | "network"
  | "unknown";

const ERROR_MAP: Record<string, KeyError> = {
  LICENSE_INVALID: "invalid",
  RATE_LIMITED: "rate_limited",
  SELF_RESET_DISABLED: "self_reset_disabled",
  RESET_COOLDOWN: "reset_cooldown",
  BAD_REQUEST: "bad_input",
  BAD_JSON: "bad_input",
};

export interface KeyResult<T> {
  ok: boolean;
  data?: T;
  error?: KeyError;
  retryAfter?: number;
}

async function post<T>(path: string, key: string): Promise<KeyResult<T>> {
  try {
    const res = await fetch(path, {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ key }),
    });
    const payload = (await res.json().catch(() => ({}))) as { ok?: boolean; error?: string; retryAfter?: number };
    if (!res.ok || !payload.ok) {
      return {
        ok: false,
        error: ERROR_MAP[payload.error ?? ""] ?? "unknown",
        retryAfter: payload.retryAfter,
      };
    }
    return { ok: true, data: payload as unknown as T };
  } catch {
    return { ok: false, error: "network" };
  }
}

export async function checkKey(key: string): Promise<KeyResult<KeyStatus>> {
  return post<KeyStatus>("/api/key/status", key);
}

export async function selfResetHwid(key: string): Promise<KeyResult<{ resetCooldownDays: number }>> {
  return post<{ resetCooldownDays: number }>("/api/key/reset-hwid", key);
}
