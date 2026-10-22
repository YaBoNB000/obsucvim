/**
 * 脚本托管 / 卡密 API 客户端。
 * 会话由 HttpOnly Cookie 承载，这里只发同源请求，不接触任何令牌。
 */

export interface Folder {
  folderId: string;
  name: string;
  createdAt: number;
  logWebhook: boolean;
  alertWebhook: boolean;
}

export interface ScriptProject {
  scriptId: string;
  folderId: string | null;
  name: string;
  description: string;
  createdAt: number;
  active: boolean;
  ffa?: boolean;
  silent?: boolean;
}

export interface LicenseRecord {
  licenseId: string;
  scriptId: string;
  label: string;
  createdAt: number;
  expiresAt: number | null;
  revokedAt: number | null;
  durationDays: number | null;
  activated: boolean;
  status?: "active" | "expired" | "revoked" | "banned";
  banned?: boolean;
  banReason?: string;
  banExpireAt?: number | null;
  totalResets?: number;
  lastResetAt?: number | null;
  hwidBound: boolean;
  firstActivatedAt: number | null;
  lastUsedAt: number | null;
}

export interface ReleaseRecord {
  scriptId: string;
  version: string;
  sha256: string;
  size: number;
  createdAt: number;
}

export type ApiError =
  | "not_found"
  | "bad_input"
  | "bad_expiry"
  | "bad_key_days"
  | "bad_source"
  | "quota"
  | "engine_busy"
  | "engine_timeout"
  | "engine_fail"
  | "script_limit"
  | "folder_limit"
  | "folder_not_found"
  | "bad_count"
  | "bad_webhook"
  | "bad_ban_days"
  | "license_limit"
  | "release_limit"
  | "release_too_large"
  | "bad_version"
  | "network"
  | "unknown";

interface Envelope {
  ok?: boolean;
  error?: string;
  scripts?: ScriptProject[];
  script?: ScriptProject;
  licenses?: LicenseRecord[];
  license?: LicenseRecord;
  licenseKey?: string;
  releases?: ReleaseRecord[];
  release?: ReleaseRecord;
  folders?: Folder[];
  folder?: Folder;
  count?: number;
  total?: number;
  from?: number;
  until?: number;
}

const ERROR_MAP: Record<string, ApiError> = {
  NOT_FOUND: "not_found",
  SCRIPT_NOT_FOUND: "not_found",
  BAD_INPUT: "bad_input",
  BAD_VERSION: "bad_version",
  BAD_EXPIRY: "bad_expiry",
  BAD_KEY_DAYS: "bad_key_days",
  BAD_SOURCE: "bad_source",
  QUOTA_EXCEEDED: "quota",
  ENGINE_BUSY: "engine_busy",
  ENGINE_TIMEOUT: "engine_timeout",
  ENGINE_FAIL: "engine_fail",
  SCRIPT_LIMIT: "script_limit",
  FOLDER_LIMIT: "folder_limit",
  FOLDER_NOT_FOUND: "folder_not_found",
  BAD_COUNT: "bad_count",
  BAD_WEBHOOK: "bad_webhook",
  BAD_BAN_DAYS: "bad_ban_days",
  LICENSE_LIMIT: "license_limit",
  RELEASE_LIMIT: "release_limit",
  RELEASE_TOO_LARGE: "release_too_large",
};

export interface Result<T> {
  ok: boolean;
  data?: T;
  error?: ApiError;
}

async function request<T>(
  path: string,
  init: { method?: string; body?: unknown } = {},
): Promise<Result<T>> {
  try {
    const res = await fetch(path, {
      method: init.method ?? "GET",
      credentials: "same-origin",
      ...(init.body === undefined
        ? {}
        : { headers: { "content-type": "application/json" }, body: JSON.stringify(init.body) }),
    });
    const payload = (await res.json().catch(() => ({}))) as Envelope;
    if (!res.ok || !payload.ok) {
      return { ok: false, error: ERROR_MAP[payload.error ?? ""] ?? "unknown" };
    }
    return { ok: true, data: payload as T };
  } catch {
    return { ok: false, error: "network" };
  }
}

export async function listScripts(): Promise<Result<ScriptProject[]>> {
  const res = await request<Envelope>("/api/auth/scripts");
  return res.ok ? { ok: true, data: res.data?.scripts ?? [] } : { ok: false, error: res.error };
}

export async function createScript(
  name: string,
  description: string,
  folderId?: string,
): Promise<Result<ScriptProject>> {
  const res = await request<Envelope>("/api/auth/scripts", {
    method: "POST",
    body: { name, description, ...(folderId ? { folderId } : {}) },
  });
  return res.ok && res.data?.script ? { ok: true, data: res.data.script } : { ok: false, error: res.error ?? "unknown" };
}

export interface LicensePage {
  licenses: LicenseRecord[];
  total: number;
  from: number;
  until: number;
}

export async function listLicenses(
  folderId: string,
  opts: { from?: number; until?: number; search?: string; status?: string } = {},
): Promise<Result<LicensePage>> {
  const params = new URLSearchParams({ folderId });
  if (opts.from !== undefined) params.set("from", String(opts.from));
  if (opts.until !== undefined) params.set("until", String(opts.until));
  if (opts.search) params.set("search", opts.search);
  if (opts.status) params.set("status", opts.status);
  const res = await request<Envelope>(`/api/auth/licenses?${params.toString()}`);
  if (!res.ok) return { ok: false, error: res.error };
  return {
    ok: true,
    data: {
      licenses: res.data?.licenses ?? [],
      total: res.data?.total ?? 0,
      from: res.data?.from ?? 0,
      until: res.data?.until ?? 0,
    },
  };
}

export async function issueLicense(
  scriptId: string,
  label: string,
  expiry: { expiresAt: string | null; keyDays?: number | null },
): Promise<Result<{ license: LicenseRecord; licenseKey: string }>> {
  const res = await request<Envelope>("/api/auth/licenses", {
    method: "POST",
    body: {
      scriptId,
      label,
      expiresAt: expiry.keyDays ? null : expiry.expiresAt,
      ...(expiry.keyDays ? { keyDays: expiry.keyDays } : {}),
    },
  });
  if (!res.ok || !res.data?.license || !res.data.licenseKey) {
    return { ok: false, error: res.error ?? "unknown" };
  }
  return { ok: true, data: { license: res.data.license, licenseKey: res.data.licenseKey } };
}

export async function revokeLicense(licenseId: string): Promise<Result<LicenseRecord>> {
  const res = await request<Envelope>("/api/auth/licenses/revoke", {
    method: "POST",
    body: { licenseId },
  });
  return res.ok && res.data?.license ? { ok: true, data: res.data.license } : { ok: false, error: res.error ?? "unknown" };
}

export async function resetLicenseHwid(licenseId: string): Promise<Result<LicenseRecord>> {
  const res = await request<Envelope>("/api/auth/licenses/reset-hwid", {
    method: "POST",
    body: { licenseId },
  });
  return res.ok && res.data?.license ? { ok: true, data: res.data.license } : { ok: false, error: res.error ?? "unknown" };
}

export async function listReleases(scriptId: string): Promise<Result<ReleaseRecord[]>> {
  const res = await request<Envelope>(`/api/auth/releases?scriptId=${encodeURIComponent(scriptId)}`);
  return res.ok ? { ok: true, data: res.data?.releases ?? [] } : { ok: false, error: res.error };
}

export async function publishRelease(
  scriptId: string,
  version: string,
  code: string,
): Promise<Result<ReleaseRecord>> {
  const res = await request<Envelope>("/api/auth/releases", {
    method: "POST",
    body: { scriptId, version, code },
  });
  return res.ok && res.data?.release ? { ok: true, data: res.data.release } : { ok: false, error: res.error ?? "unknown" };
}

export interface PipelineRelease extends ReleaseRecord {
  sourceBytes: number;
  guardedBytes: number;
}

/** Upload source -> server injects the guard -> obfuscates -> stores the release. */
export async function publishFromSource(
  scriptId: string,
  version: string,
  source: string,
): Promise<Result<PipelineRelease>> {
  try {
    const res = await fetch("/api/publish", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ scriptId, version, source }),
    });
    const payload = (await res.json().catch(() => ({}))) as Envelope & {
      release?: PipelineRelease;
    };
    if (!res.ok || !payload.ok || !payload.release) {
      return { ok: false, error: ERROR_MAP[payload.error ?? ""] ?? "unknown" };
    }
    return { ok: true, data: payload.release };
  } catch {
    return { ok: false, error: "network" };
  }
}

export async function listFolders(): Promise<Result<Folder[]>> {
  const res = await request<Envelope>("/api/auth/folders");
  return res.ok ? { ok: true, data: res.data?.folders ?? [] } : { ok: false, error: res.error };
}

export async function createFolder(name: string): Promise<Result<Folder>> {
  const res = await request<Envelope>("/api/auth/folders", { method: "POST", body: { name } });
  return res.ok && res.data?.folder ? { ok: true, data: res.data.folder } : { ok: false, error: res.error ?? "unknown" };
}

export async function updateFolder(
  folderId: string,
  patch: { name?: string; logWebhook?: string; alertWebhook?: string },
): Promise<Result<Folder>> {
  const res = await request<Envelope>("/api/auth/folders/update", {
    method: "POST",
    body: { folderId, ...patch },
  });
  return res.ok && res.data?.folder ? { ok: true, data: res.data.folder } : { ok: false, error: res.error ?? "unknown" };
}

/** Batch issue: one call, up to 100 keys. */
export async function issueLicenseBatch(
  folderId: string,
  opts: { label: string; count: number; keyDays: number | null; expiresAt: string | null },
): Promise<Result<{ license: LicenseRecord; licenseKey: string }[]>> {
  interface BatchPayload {
    ok?: boolean;
    error?: string;
    count?: number;
    licenses?: { licenseKey: string; license: LicenseRecord }[];
    licenseKey?: string;
    license?: LicenseRecord;
  }
  try {
    const res = await fetch("/api/auth/licenses", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        folderId,
        label: opts.label,
        count: opts.count,
        keyDays: opts.keyDays,
        expiresAt: opts.keyDays ? null : opts.expiresAt,
      }),
    });
    const payload = (await res.json().catch(() => ({}))) as BatchPayload;
    if (!res.ok || !payload.ok) {
      return { ok: false, error: ERROR_MAP[payload.error ?? ""] ?? "unknown" };
    }
    if (Array.isArray(payload.licenses)) return { ok: true, data: payload.licenses };
    if (payload.license && payload.licenseKey) {
      return { ok: true, data: [{ license: payload.license, licenseKey: payload.licenseKey }] };
    }
    return { ok: false, error: "unknown" };
  } catch {
    return { ok: false, error: "network" };
  }
}

/** CSV export never contains raw keys, only metadata. */
export function exportLicensesCsv(folderId: string) {
  window.location.href = `/api/auth/licenses/export?folderId=${encodeURIComponent(folderId)}`;
}

export async function banLicense(
  licenseId: string,
  opts: { reason: string; banDays: number | null },
): Promise<Result<{ license: LicenseRecord; unbanToken: string }>> {
  interface BanPayload { ok?: boolean; error?: string; unbanToken?: string; license?: LicenseRecord }
  try {
    const res = await fetch("/api/auth/licenses/ban", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ licenseId, reason: opts.reason, banDays: opts.banDays }),
    });
    const payload = (await res.json().catch(() => ({}))) as BanPayload;
    if (!res.ok || !payload.ok || !payload.license || !payload.unbanToken) {
      return { ok: false, error: ERROR_MAP[payload.error ?? ""] ?? "unknown" };
    }
    return { ok: true, data: { license: payload.license, unbanToken: payload.unbanToken } };
  } catch {
    return { ok: false, error: "network" };
  }
}

export async function unbanLicense(licenseId: string): Promise<Result<LicenseRecord>> {
  const res = await request<Envelope>("/api/auth/licenses/unban", { method: "POST", body: { licenseId } });
  return res.ok && res.data?.license
    ? { ok: true, data: res.data.license }
    : { ok: false, error: res.error ?? "unknown" };
}

export async function updateScript(
  scriptId: string,
  patch: { ffa?: boolean; silent?: boolean; active?: boolean; name?: string },
): Promise<Result<ScriptProject>> {
  const res = await request<Envelope>("/api/auth/scripts/update", {
    method: "POST",
    body: { scriptId, ...patch },
  });
  return res.ok && res.data?.script
    ? { ok: true, data: res.data.script }
    : { ok: false, error: res.error ?? "unknown" };
}

/** The URL never changes, so users can paste it once and forget it. */
export function loaderUrl(scriptId: string): string {
  return `${window.location.origin}/files/${scriptId}.lua`;
}

export function loadstringSnippet(scriptId: string): string {
  return `loadstring(game:HttpGet("${loaderUrl(scriptId)}"))()`;
}

/** Ready-to-send snippet: sets the key first, then runs the stable loader. */
export function keySnippet(scriptId: string): string {
  return `getgenv().key = "在这里填入卡密"\n${loadstringSnippet(scriptId)}`;
}
