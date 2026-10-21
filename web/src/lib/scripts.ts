/**
 * 脚本托管 / 卡密 API 客户端。
 * 会话由 HttpOnly Cookie 承载，这里只发同源请求，不接触任何令牌。
 */

export interface ScriptProject {
  scriptId: string;
  name: string;
  description: string;
  createdAt: number;
  active: boolean;
}

export interface LicenseRecord {
  licenseId: string;
  scriptId: string;
  label: string;
  createdAt: number;
  expiresAt: number | null;
  revokedAt: number | null;
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
  | "script_limit"
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
}

const ERROR_MAP: Record<string, ApiError> = {
  NOT_FOUND: "not_found",
  SCRIPT_NOT_FOUND: "not_found",
  BAD_INPUT: "bad_input",
  BAD_VERSION: "bad_version",
  BAD_EXPIRY: "bad_expiry",
  SCRIPT_LIMIT: "script_limit",
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
): Promise<Result<ScriptProject>> {
  const res = await request<Envelope>("/api/auth/scripts", {
    method: "POST",
    body: { name, description },
  });
  return res.ok && res.data?.script ? { ok: true, data: res.data.script } : { ok: false, error: res.error ?? "unknown" };
}

export async function listLicenses(scriptId: string): Promise<Result<LicenseRecord[]>> {
  const res = await request<Envelope>(`/api/auth/licenses?scriptId=${encodeURIComponent(scriptId)}`);
  return res.ok ? { ok: true, data: res.data?.licenses ?? [] } : { ok: false, error: res.error };
}

export async function issueLicense(
  scriptId: string,
  label: string,
  expiresAt: string | null,
): Promise<Result<{ license: LicenseRecord; licenseKey: string }>> {
  const res = await request<Envelope>("/api/auth/licenses", {
    method: "POST",
    body: { scriptId, label, expiresAt },
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
