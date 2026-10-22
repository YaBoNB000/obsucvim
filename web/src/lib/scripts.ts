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
  heartbeat?: boolean;
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
  | "publish_busy"
  | "job_not_found"
  | "job_expired"
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
  PUBLISH_BUSY: "publish_busy",
  JOB_NOT_FOUND: "job_not_found",
  JOB_EXPIRED: "job_expired",
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

/* ---------------- 异步发布作业 ---------------- */

export type PublishStage = "queued" | "injecting" | "obfuscating" | "storing" | "completed" | "failed";

export interface PublishJob {
  jobId: string;
  status: "processing" | "completed" | "failed";
  stage: PublishStage;
  progress: number;
  elapsedMs: number;
  result?: PipelineRelease;
  error?: string;
  detail?: string;
}

interface JobEnvelope {
  ok?: boolean;
  error?: string;
  jobId?: string;
  status?: PublishJob["status"];
  stage?: PublishStage;
  progress?: number;
  elapsedMs?: number;
  result?: PipelineRelease;
  detail?: string;
}

/** 提交一个发布作业；服务端立刻返回 jobId，混淆在后台继续。 */
export async function startPublish(
  scriptId: string,
  version: string,
  source: string,
): Promise<Result<{ jobId: string; stage: PublishStage; progress: number }>> {
  try {
    const res = await fetch("/api/publish", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ scriptId, version, source }),
    });
    const payload = (await res.json().catch(() => ({}))) as JobEnvelope;
    if (!res.ok || !payload.ok || !payload.jobId) {
      return { ok: false, error: ERROR_MAP[payload.error ?? ""] ?? "unknown" };
    }
    return {
      ok: true,
      data: {
        jobId: payload.jobId,
        stage: payload.stage ?? "queued",
        progress: payload.progress ?? 5,
      },
    };
  } catch {
    return { ok: false, error: "network" };
  }
}

/** 轮询作业状态。作业不存在（服务重启）时返回 job_not_found，调用方应提示重试。 */
export async function pollPublish(jobId: string): Promise<Result<PublishJob>> {
  try {
    const res = await fetch(`/api/publish/${encodeURIComponent(jobId)}`, { credentials: "same-origin" });
    const payload = (await res.json().catch(() => ({}))) as JobEnvelope;
    if (!res.ok || !payload.ok) {
      return { ok: false, error: ERROR_MAP[payload.error ?? ""] ?? "unknown" };
    }
    return {
      ok: true,
      data: {
        jobId,
        status: payload.status ?? "processing",
        stage: payload.stage ?? "queued",
        progress: payload.progress ?? 0,
        elapsedMs: payload.elapsedMs ?? 0,
        result: payload.result,
        error: payload.error,
        detail: payload.detail,
      },
    };
  } catch {
    return { ok: false, error: "network" };
  }
}

/** 一次性发布（提交 + 轮询到结束），保留了旧调用方的语义。 */
export async function publishFromSource(
  scriptId: string,
  version: string,
  source: string,
  onProgress?: (job: PublishJob) => void,
): Promise<Result<PipelineRelease>> {
  const started = await startPublish(scriptId, version, source);
  if (!started.ok || !started.data) return { ok: false, error: started.error ?? "unknown" };
  const { jobId } = started.data;
  const deadline = Date.now() + 120_000;
  let delay = 250;
  while (Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, delay));
    delay = Math.min(1200, Math.round(delay * 1.35));
    const poll = await pollPublish(jobId);
    if (!poll.ok || !poll.data) return { ok: false, error: poll.error ?? "unknown" };
    onProgress?.(poll.data);
    if (poll.data.status === "completed" && poll.data.result) {
      return { ok: true, data: poll.data.result };
    }
    if (poll.data.status === "failed") {
      return { ok: false, error: ERROR_MAP[poll.data.error ?? ""] ?? "engine_fail" };
    }
  }
  return { ok: false, error: "engine_timeout" };
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

/** JSON export (metadata only) — used for migrations and backups. */
export function exportLicensesJson(folderId: string) {
  window.location.href = `/api/auth/licenses/export.json?folderId=${encodeURIComponent(folderId)}`;
}

export interface ImportedLicense {
  licenseKey: string;
  license: LicenseRecord;
}

/** 导入只生成新卡密；不会恢复任何旧卡密（服务端也没存明文）。 */
export async function importLicenses(
  folderId: string,
  entries: { label?: string; keyDays?: number | null; expiresAt?: string | null }[],
): Promise<Result<ImportedLicense[]>> {
  interface ImportPayload {
    ok?: boolean;
    error?: string;
    licenses?: ImportedLicense[];
  }
  try {
    const res = await fetch("/api/auth/licenses/import", {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ folderId, entries }),
    });
    const payload = (await res.json().catch(() => ({}))) as ImportPayload;
    if (!res.ok || !payload.ok || !Array.isArray(payload.licenses)) {
      return { ok: false, error: ERROR_MAP[payload.error ?? ""] ?? "unknown" };
    }
    return { ok: true, data: payload.licenses };
  } catch {
    return { ok: false, error: "network" };
  }
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
  patch: { ffa?: boolean; silent?: boolean; heartbeat?: boolean; active?: boolean; name?: string },
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
