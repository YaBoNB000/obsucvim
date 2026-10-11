/**
 * 注册邮箱验证码（客户端演示实现）。
 * - 6 位数字，10 分钟有效，最多 5 次尝试
 * - 60 秒重发冷却（RESEND_COOLDOWN_S）
 * - 待验证状态存 sessionStorage（关标签页即弃）
 * 接真实后端时，把 requestCode/verifyCode 换成 API 调用即可。
 */
const PENDING_KEY = "obsucvim.verify";
const CODE_TTL_MS = 10 * 60 * 1000;
const MAX_FAILS = 5;

export const RESEND_COOLDOWN_S = 60;

export interface PendingVerification {
  email: string;
  code: string;
  expires: number;
  sentAt: number;
  fails: number;
}

function load(): PendingVerification | null {
  try {
    const raw = sessionStorage.getItem(PENDING_KEY);
    const parsed = raw ? (JSON.parse(raw) as PendingVerification) : null;
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch {
    return null;
  }
}

function save(p: PendingVerification) {
  try {
    sessionStorage.setItem(PENDING_KEY, JSON.stringify(p));
  } catch {
    /* ignore */
  }
}

export function clearPending() {
  try {
    sessionStorage.removeItem(PENDING_KEY);
  } catch {
    /* ignore */
  }
}

function randomCode(): string {
  const buf = new Uint32Array(1);
  if (globalThis.crypto?.getRandomValues) globalThis.crypto.getRandomValues(buf);
  else buf[0] = Math.floor(Math.random() * 0x100000000);
  return String(buf[0] % 1_000_000).padStart(6, "0");
}

/** 生成并保存新验证码（同时刷新冷却计时） */
export function requestCode(email: string): string {
  const code = randomCode();
  save({
    email: email.trim().toLowerCase(),
    code,
    expires: Date.now() + CODE_TTL_MS,
    sentAt: Date.now(),
    fails: 0,
  });
  return code;
}

/** 当前重发冷却剩余秒数 */
export function resendCooldownRemaining(): number {
  const p = load();
  if (!p) return 0;
  return Math.max(0, Math.ceil((p.sentAt + RESEND_COOLDOWN_S * 1000 - Date.now()) / 1000));
}

export type VerifyFailReason = "none" | "expired" | "wrong" | "exhausted";

export function verifyCode(email: string, input: string): { ok: true } | { ok: false; reason: VerifyFailReason } {
  const p = load();
  const key = email.trim().toLowerCase();
  if (!p || p.email !== key) return { ok: false, reason: "none" };
  if (Date.now() > p.expires) {
    clearPending();
    return { ok: false, reason: "expired" };
  }
  if (input.trim() !== p.code) {
    p.fails += 1;
    if (p.fails >= MAX_FAILS) {
      clearPending();
      return { ok: false, reason: "exhausted" };
    }
    save(p);
    return { ok: false, reason: "wrong" };
  }
  clearPending();
  return { ok: true };
}
