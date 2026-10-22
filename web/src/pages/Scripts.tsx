import { useCallback, useEffect, useMemo, useState, type CSSProperties } from "react";
import { Button, GlassCard, Icon, Pill, Stat } from "../components/ui";
import { AnimatedNumber, Skeleton, SkeletonRows, StepProgress, useToast } from "../components/Feedback";
import { useI18n, type TKey } from "../lib/i18n";
import {
  batchLicenses,
  batchPublish,
  listMetrics,
  banLicense,
  unbanLicense,
  updateScript,
  keySnippet,
  loadstringSnippet,
  loaderUrl,
  createFolder,
  createScript,
  exportLicensesCsv,
  exportLicensesJson,
  issueLicense,
  pollPublish,
  startPublish,
  type MetricDay,
  type PublishJob,
  issueLicenseBatch,
  listFolders,
  updateFolder,
  type Folder,
  listLicenses,
  listReleases,
  listScripts,
  publishRelease,
  resetLicenseHwid,
  revokeLicense,
  type ApiError,
  type LicenseRecord,
  type ReleaseRecord,
  type ScriptProject,
} from "../lib/scripts";

const INPUT =
  "w-full rounded-xl border border-white/[0.09] bg-black/30 px-3 py-2.5 text-[13px] text-aqua-100 placeholder:text-slate-600 transition focus:border-aqua-300/45 focus:outline-none";

const ERROR_KEY: Record<ApiError, TKey> = {
  not_found: "scripts.errNotFound",
  bad_input: "scripts.errBadInput",
  bad_expiry: "scripts.errBadExpiry",
  bad_key_days: "scripts.errBadKeyDays",
  folder_limit: "scripts.errFolderLimit",
  folder_not_found: "scripts.errFolderNotFound",
  bad_count: "scripts.errBadCount",
  bad_webhook: "scripts.errBadWebhook",
  bad_ban_days: "scripts.errBadBanDays",
  bad_source: "scripts.errBadSource",
  quota: "scripts.errQuota",
  engine_busy: "scripts.errEngineBusy",
  engine_timeout: "scripts.errEngineTimeout",
  engine_fail: "scripts.errEngineFail",
  script_limit: "scripts.errScriptLimit",
  license_limit: "scripts.errLicenseLimit",
  release_limit: "scripts.errReleaseLimit",
  release_too_large: "scripts.errReleaseTooLarge",
  publish_busy: "scripts.errPublishBusy",
  job_not_found: "scripts.errJobNotFound",
  job_expired: "scripts.errJobExpired",
  batch_too_large: "scripts.errBatchTooLarge",
  webhook_limit: "scripts.errWebhookLimit",
  app_limit: "scripts.errAppLimit",
  bad_version: "scripts.errBadVersion",
  network: "scripts.errNetwork",
  unknown: "scripts.errUnknown",
};

function when(ts: number | null) {
  if (!ts) return "—";
  return new Date(ts).toLocaleString();
}

/** 一行代码 + 复制按钮，复制成功后按钮变成对勾 */
function CopyField({
  label,
  value,
  copied,
  onCopy,
}: {
  label: string;
  value: string;
  copied: boolean;
  onCopy: () => void;
}) {
  return (
    <div>
      <p className="mb-1 text-[10.5px] text-slate-500">{label}</p>
      <div className="flex items-start gap-2">
        <code className="min-w-0 flex-1 overflow-x-auto whitespace-pre rounded-xl border border-white/[0.07] bg-black/40 px-3 py-2.5 font-mono text-[11px] leading-relaxed text-aqua-100">
          {value}
        </code>
        <Button variant="outline" className="shrink-0 px-2.5 py-2" onClick={onCopy} title={label}>
          <Icon name={copied ? "check" : "copy"} className={copied ? "anim-pop h-3.5 w-3.5" : "h-3.5 w-3.5"} />
        </Button>
      </div>
    </div>
  );
}

export function Scripts() {
  const { t } = useI18n();
  const toast = useToast();
  const [folders, setFolders] = useState<Folder[]>([]);
  const [folderId, setFolderId] = useState<string | null>(null);
  const [newFolder, setNewFolder] = useState("");
  const [creatingFolder, setCreatingFolder] = useState(false);
  const [batch, setBatch] = useState("1");
  const [batchKeys, setBatchKeys] = useState<string[]>([]);
  const [logHook, setLogHook] = useState("");
  const [alertHook, setAlertHook] = useState("");
  const [savingHooks, setSavingHooks] = useState(false);
  const [projects, setProjects] = useState<ScriptProject[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [tab, setTab] = useState<"licenses" | "releases">("licenses");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<TKey | null>(null);

  const [newName, setNewName] = useState("");
  const [newDesc, setNewDesc] = useState("");
  const [creating, setCreating] = useState(false);

  const [licenses, setLicenses] = useState<LicenseRecord[]>([]);
  const [licenseTotal, setLicenseTotal] = useState(0);
  const [pageFrom, setPageFrom] = useState(0);
  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState("");
  const [banTarget, setBanTarget] = useState<LicenseRecord | null>(null);
  const [banReason, setBanReason] = useState("");
  const [banDays, setBanDays] = useState("");
  const [unbanToken, setUnbanToken] = useState<string | null>(null);
  const [releases, setReleases] = useState<ReleaseRecord[]>([]);
  const [issuedKey, setIssuedKey] = useState<{ key: string; label: string } | null>(null);
  const [label, setLabel] = useState("");
  const [expiryMode, setExpiryMode] = useState<"forever" | "days" | "date">("forever");
  const [expiry, setExpiry] = useState("");
  const [days, setDays] = useState("7");
  const [sourceMode, setSourceMode] = useState<"pipeline" | "manual">("pipeline");
  const [rawSource, setRawSource] = useState("");
  const [pipelineBusy, setPipelineBusy] = useState(false);
  const [publishJob, setPublishJob] = useState<PublishJob | null>(null);
  const [metrics, setMetrics] = useState<MetricDay[]>([]);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [publishResult, setPublishResult] = useState<{ version: string; size: number; scriptId: string } | null>(null);
  const [version, setVersion] = useState("v1");
  const [artifact, setArtifact] = useState("");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [copyFailed, setCopyFailed] = useState(false);

  const current = useMemo(
    () => projects.find((p) => p.scriptId === selected) ?? null,
    [projects, selected],
  );

  const refreshFolders = useCallback(async () => {
    const res = await listFolders();
    if (res.ok && res.data?.length) {
      setFolders(res.data);
      setFolderId((prev) => prev ?? res.data![0].folderId);
    }
  }, []);

  const refreshProjects = useCallback(async () => {
    const res = await listScripts();
    if (!res.ok || !res.data) {
      setError(ERROR_KEY[res.error ?? "unknown"]);
      setLoading(false);
      return;
    }
    const list = res.data;
    setProjects(list);
    setSelected((prev) => prev ?? list[0]?.scriptId ?? null);
    setLoading(false);
  }, []);

  const PAGE_SIZE = 20;

  const refreshDetail = useCallback(async (scriptId: string, opts?: { from?: number; search?: string; status?: string }) => {
    const script = projects.find((p) => p.scriptId === scriptId);
    const rel = await listReleases(scriptId);
    if (rel.ok && rel.data) setReleases(rel.data);
    if (!script?.folderId) {
      setLicenses([]);
      setLicenseTotal(0);
      return;
    }
    const from = opts?.from ?? 0;
    const lic = await listLicenses(script.folderId, {
      from,
      until: from + PAGE_SIZE,
      search: opts?.search ?? "",
      status: opts?.status ?? "",
    });
    if (lic.ok && lic.data) {
      setLicenses(lic.data.licenses);
      setLicenseTotal(lic.data.total);
      setPageFrom(lic.data.from);
    }
  }, [projects]);

  useEffect(() => {
    void refreshFolders();
    void refreshProjects();
    void listMetrics(7).then((res) => {
      if (res.ok && res.data) setMetrics(res.data);
    });
  }, [refreshFolders, refreshProjects]);

  useEffect(() => {
    if (!selected) return;
    setIssuedKey(null);
    setError(null);
    void refreshDetail(selected);
  }, [selected, refreshDetail]);

  const handleCreate = async () => {
    if (newName.trim().length < 2 || busy) return;
    setBusy(true);
    setError(null);
    const res = await createScript(
      newName.trim(),
      newDesc.trim(),
      folderId ?? undefined,
    );
    setBusy(false);
    if (!res.ok || !res.data) {
      setError(ERROR_KEY[res.error ?? "unknown"]);
      return;
    }
    setProjects((prev) => [res.data as ScriptProject, ...prev]);
    setSelected(res.data.scriptId);
    setNewName("");
    setNewDesc("");
    setCreating(false);
  };

  const handleCreateFolder = async () => {
    if (newFolder.trim().length < 2 || busy) return;
    setBusy(true);
    setError(null);
    const res = await createFolder(newFolder.trim());
    setBusy(false);
    if (!res.ok || !res.data) {
      setError(ERROR_KEY[res.error ?? "unknown"]);
      return;
    }
    setFolders((prev) => [...prev, res.data as Folder]);
    setFolderId(res.data.folderId);
    setNewFolder("");
    setCreatingFolder(false);
  };

  const handleSaveWebhooks = async () => {
    if (!folderId || savingHooks) return;
    setSavingHooks(true);
    setError(null);
    const res = await updateFolder(folderId, {
      ...(logHook ? { logWebhook: logHook } : {}),
      ...(alertHook ? { alertWebhook: alertHook } : {}),
    });
    setSavingHooks(false);
    if (!res.ok || !res.data) {
      setError(ERROR_KEY[res.error ?? "unknown"]);
      return;
    }
    setFolders((prev) => prev.map((f) => (f.folderId === folderId ? (res.data as Folder) : f)));
    setLogHook("");
    setAlertHook("");
  };

  const handleBatch = async () => {
    if (!folderId || busy) return;
    const count = Math.max(1, Math.min(100, Number(batch) || 1));
    setBusy(true);
    setError(null);
    setBatchKeys([]);
    const keyDays = expiryMode === "days" ? Number(days) : null;
    const expiresAt = expiryMode === "date" && expiry ? new Date(expiry).toISOString() : null;
    const res = await issueLicenseBatch(folderId, {
      label: label.trim(),
      count,
      keyDays: keyDays && Number.isInteger(keyDays) && keyDays > 0 ? keyDays : null,
      expiresAt,
    });
    setBusy(false);
    if (!res.ok || !res.data) {
      setError(ERROR_KEY[res.error ?? "unknown"]);
      return;
    }
    setBatchKeys(res.data.map((entry) => entry.licenseKey));
    setLabel("");
    toast.push({ kind: "success", title: t("scripts.batchIssued", { n: res.data.length }) });
    applyFilter(0);
  };

  const handleIssue = async () => {
    if (!selected || busy) return;
    setBusy(true);
    setError(null);
    const keyDays = expiryMode === "days" ? Number(days) : null;
    const expiresAt =
      expiryMode === "date" && expiry ? new Date(expiry).toISOString() : null;
    const res = await issueLicense(selected, label.trim(), {
      expiresAt,
      keyDays: keyDays && Number.isInteger(keyDays) && keyDays > 0 ? keyDays : null,
    });
    setBusy(false);
    if (!res.ok || !res.data) {
      setError(ERROR_KEY[res.error ?? "unknown"]);
      return;
    }
    setIssuedKey({ key: res.data.licenseKey, label: res.data.license.label });
    setLabel("");
    toast.push({ kind: "success", title: t("scripts.toastIssued") });
    applyFilter(0);
  };

  const handleRevoke = async (licenseId: string) => {
    setBusy(true);
    const res = await revokeLicense(licenseId);
    setBusy(false);
    if (res.ok && res.data) {
      setLicenses((prev) => prev.map((l) => (l.licenseId === licenseId ? res.data! : l)));
    } else {
      setError(ERROR_KEY[res.error ?? "unknown"]);
    }
  };

  const handleReset = async (licenseId: string) => {
    setBusy(true);
    const res = await resetLicenseHwid(licenseId);
    setBusy(false);
    if (res.ok && res.data) {
      setLicenses((prev) => prev.map((l) => (l.licenseId === licenseId ? res.data! : l)));
    } else {
      setError(ERROR_KEY[res.error ?? "unknown"]);
    }
  };

  const handlePublish = async () => {
    if (!selected || busy || !artifact.trim()) return;
    setBusy(true);
    setError(null);
    const res = await publishRelease(selected, version.trim(), artifact);
    setBusy(false);
    if (!res.ok || !res.data) {
      setError(ERROR_KEY[res.error ?? "unknown"]);
      return;
    }
    setReleases((prev) => [res.data as ReleaseRecord, ...prev]);
    setPublishResult({ version: res.data.version, size: res.data.size, scriptId: selected });
    setArtifact("");
  };

  /* 提交作业 -> 轮询进度。轮询失败（服务重启）时明确提示重试，而不是卡住转圈。 */
  const handlePipeline = async () => {
    if (!selected || pipelineBusy || !rawSource.trim()) return;
    const scriptId = selected;
    setPipelineBusy(true);
    setError(null);
    setPublishJob({ jobId: "", status: "processing", stage: "queued", progress: 5, elapsedMs: 0 });

    const started = await startPublish(scriptId, version.trim(), rawSource);
    if (!started.ok || !started.data) {
      setPipelineBusy(false);
      setPublishJob(null);
      setError(ERROR_KEY[started.error ?? "unknown"]);
      toast.push({ kind: "error", title: t(ERROR_KEY[started.error ?? "unknown"]) });
      return;
    }

    const deadline = Date.now() + 120_000;
    let delay = 260;
    let final: PublishJob | null = null;
    while (Date.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, delay));
      delay = Math.min(1200, Math.round(delay * 1.35));
      const poll = await pollPublish(started.data.jobId);
      if (!poll.ok || !poll.data) {
        setPipelineBusy(false);
        setPublishJob(null);
        setError(ERROR_KEY[poll.error ?? "unknown"]);
        toast.push({ kind: "error", title: t(ERROR_KEY[poll.error ?? "unknown"]) });
        return;
      }
      setPublishJob(poll.data);
      if (poll.data.status !== "processing") {
        final = poll.data;
        break;
      }
    }

    setPipelineBusy(false);
    if (!final) {
      setPublishJob(null);
      setError("scripts.errEngineTimeout");
      toast.push({ kind: "error", title: t("scripts.errEngineTimeout") });
      return;
    }
    if (final.status === "failed" || !final.result) {
      setError(ERROR_KEY[(final.error ?? "engine_fail") as keyof typeof ERROR_KEY] ?? "scripts.errUnknown");
      toast.push({
        kind: "error",
        title: t(ERROR_KEY[(final.error ?? "engine_fail") as keyof typeof ERROR_KEY] ?? "scripts.errUnknown"),
      });
      return;
    }
    setReleases((prev) => [final!.result as ReleaseRecord, ...prev]);
    setPublishResult({ version: final.result.version, size: final.result.size, scriptId });
    setRawSource("");
    toast.push({
      kind: "success",
      title: t("scripts.pipelineDone", {
        v: final.result.version,
        size: (final.result.size / 1024).toFixed(1),
      }),
      detail: t("scripts.toastPublished"),
    });
  };

  const readSourceFile = async (file: File | undefined) => {
    if (!file) return;
    try {
      const text = await file.text();
      setRawSource(text);
      setError(null);
    } catch {
      setError("scripts.errBadSource");
    }
  };

  const handleBan = async () => {
    if (!banTarget || busy) return;
    setBusy(true);
    setError(null);
    const days = banDays.trim() ? Number(banDays) : null;
    const res = await banLicense(banTarget.licenseId, {
      reason: banReason.trim(),
      banDays: days && Number.isInteger(days) ? days : null,
    });
    setBusy(false);
    if (!res.ok || !res.data) {
      setError(ERROR_KEY[res.error ?? "unknown"]);
      return;
    }
    setUnbanToken(res.data.unbanToken);
    toast.push({ kind: "success", title: t("scripts.toastBanned") });
    setLicenses((prev) => prev.map((l) => (l.licenseId === res.data!.license.licenseId ? res.data!.license : l)));
    setBanTarget(null);
    setBanReason("");
    setBanDays("");
  };

  const handleUnban = async (licenseId: string) => {
    setBusy(true);
    const res = await unbanLicense(licenseId);
    setBusy(false);
    if (res.ok && res.data) {
      setLicenses((prev) => prev.map((l) => (l.licenseId === licenseId ? res.data! : l)));
    } else {
      setError(ERROR_KEY[res.error ?? "unknown"]);
    }
  };

  const runBatch = async (action: "ban" | "unban" | "reset-hwid" | "revoke") => {
    if (busy || selectedIds.length === 0) return;
    setBusy(true);
    setError(null);
    const res = await batchLicenses(action, selectedIds, action === "ban" ? { reason: "batch ban" } : {});
    setBusy(false);
    if (!res.ok || !res.data) {
      setError(ERROR_KEY[res.error ?? "unknown"]);
      toast.push({ kind: "error", title: t(ERROR_KEY[res.error ?? "unknown"]) });
      return;
    }
    const byId = new Map(res.data.map((license) => [license.licenseId, license]));
    setLicenses((prev) => prev.map((license) => byId.get(license.licenseId) ?? license));
    setSelectedIds([]);
    toast.push({ kind: "success", title: t("scripts.batchSelected", { n: res.data.length }) });
    applyFilter(pageFrom);
  };

  const handleBatchPublish = async () => {
    if (!folderId || pipelineBusy || !rawSource.trim()) return;
    setPipelineBusy(true);
    setError(null);
    const res = await batchPublish(folderId, version.trim(), rawSource);
    if (!res.ok || !res.data) {
      setPipelineBusy(false);
      setError(ERROR_KEY[res.error ?? "unknown"]);
      toast.push({ kind: "error", title: t(ERROR_KEY[res.error ?? "unknown"]) });
      return;
    }
    let done = 0;
    for (const entry of res.data.jobs) {
      let delay = 260;
      for (let i = 0; i < 60; i++) {
        await new Promise((resolve) => setTimeout(resolve, delay));
        delay = Math.min(1200, Math.round(delay * 1.35));
        const poll = await pollPublish(entry.jobId);
        if (!poll.ok || !poll.data || poll.data.status !== "processing") break;
      }
      done += 1;
      setPublishJob({ jobId: "", status: "processing", stage: "completed", progress: (done / res.data.jobs.length) * 100, elapsedMs: 0 });
    }
    setPipelineBusy(false);
    setPublishJob(null);
    setRawSource("");
    toast.push({ kind: "success", title: t("scripts.batchIssued", { n: done }) });
    if (selected) void refreshDetail(selected);
  };

  const toggleHeartbeat = async (next: boolean) => {
    if (!selected || busy) return;
    setBusy(true);
    const res = await updateScript(selected, { heartbeat: next });
    setBusy(false);
    if (res.ok && res.data) {
      setProjects((prev) => prev.map((p) => (p.scriptId === selected ? { ...p, ...res.data! } : p)));
      toast.push({ kind: "success", title: t(next ? "scripts.heartbeatOn" : "scripts.heartbeatOff") });
    } else {
      setError(ERROR_KEY[res.error ?? "unknown"]);
      toast.push({ kind: "error", title: t(ERROR_KEY[res.error ?? "unknown"]) });
    }
  };

  const toggleFfa = async (next: boolean) => {
    if (!selected || busy) return;
    setBusy(true);
    const res = await updateScript(selected, { ffa: next });
    setBusy(false);
    if (res.ok && res.data) {
      setProjects((prev) => prev.map((p) => (p.scriptId === selected ? { ...p, ...res.data! } : p)));
    } else {
      setError(ERROR_KEY[res.error ?? "unknown"]);
    }
  };

  const applyFilter = (nextFrom: number, nextSearch = search, nextStatus = statusFilter) => {
    if (!selected) return;
    void refreshDetail(selected, { from: nextFrom, search: nextSearch, status: nextStatus });
  };

  // 剪贴板 API 在无权限的 iframe（预览环境）里会静默失败，退回 execCommand；
  // 仍失败就如实提示，而不是假装复制成功。
  const copyKey = async (text: string, id = "generic") => {
    let ok = false;
    try {
      await navigator.clipboard.writeText(text);
      ok = true;
    } catch {
      try {
        const ta = document.createElement("textarea");
        ta.value = text;
        ta.style.position = "fixed";
        ta.style.opacity = "0";
        document.body.appendChild(ta);
        ta.focus();
        ta.select();
        ok = document.execCommand("copy");
        ta.remove();
      } catch {
        ok = false;
      }
    }
    setCopiedId(ok ? id : null);
    setCopyFailed(!ok);
    window.setTimeout(() => {
      setCopiedId(null);
      setCopyFailed(false);
    }, 1800);
  };

  const activeLicenses = licenses.filter((l) => !l.revokedAt).length;

  const currentFolder = folders.find((f) => f.folderId === folderId) ?? null;

  return (
    <div className="grid gap-4">
      {/* ---------------- 项目文件夹 ---------------- */}
      <GlassCard className="p-4">
        <div className="flex flex-wrap items-center gap-2.5">
          <Icon name="layers" className="h-4 w-4 text-aqua-300" />
          <h2 className="text-[13px] font-semibold text-slate-100">{t("scripts.folders")}</h2>

          <div className="flex flex-wrap items-center gap-1">
            {folders.map((f) => (
              <button
                key={f.folderId}
                onClick={() => setFolderId(f.folderId)}
                className={
                  "cursor-pointer rounded-xl border px-2.5 py-1 text-[11.5px] transition " +
                  (f.folderId === folderId
                    ? "border-aqua-300/35 bg-aqua-400/[0.12] text-aqua-100"
                    : "border-white/[0.08] text-slate-400 hover:border-aqua-300/25 hover:text-aqua-100")
                }
              >
                {f.name}
              </button>
            ))}
          </div>

          {creatingFolder ? (
            <div className="flex items-center gap-2">
              <input
                className={INPUT + " w-40 py-1.5 text-[12px]"}
                value={newFolder}
                onChange={(e) => setNewFolder(e.target.value)}
                placeholder={t("scripts.folderNamePh")}
                maxLength={60}
                autoFocus
              />
              <Button className="px-3 py-1.5" disabled={busy || newFolder.trim().length < 2} onClick={handleCreateFolder}>
                {t("scripts.save")}
              </Button>
              <Button variant="ghost" className="px-2 py-1.5" onClick={() => setCreatingFolder(false)}>
                {t("common.cancel")}
              </Button>
            </div>
          ) : (
            <button
              onClick={() => setCreatingFolder(true)}
              className="cursor-pointer rounded-xl border border-white/10 px-2.5 py-1 text-[11px] text-slate-300 transition hover:border-aqua-300/40 hover:text-aqua-100"
            >
              + {t("scripts.newFolder")}
            </button>
          )}

          <div className="ml-auto flex items-center gap-2">
            {currentFolder && (
              <span className="hidden font-mono text-[10.5px] text-slate-500 sm:inline">
                {currentFolder.logWebhook && t("scripts.logWebhook")}
                {currentFolder.logWebhook && currentFolder.alertWebhook && " · "}
                {currentFolder.alertWebhook && t("scripts.alertWebhook")}
              </span>
            )}
            <button
              disabled={!folderId}
              onClick={() => folderId && exportLicensesCsv(folderId)}
              className="cursor-pointer rounded-xl border border-white/10 px-2.5 py-1 text-[11px] text-slate-300 transition hover:border-aqua-300/40 hover:text-aqua-100 disabled:cursor-not-allowed disabled:opacity-40"
            >
              {t("scripts.exportCsv")}
            </button>
            <button
              disabled={!folderId}
              onClick={() => folderId && exportLicensesJson(folderId)}
              className="cursor-pointer rounded-xl border border-white/10 px-2.5 py-1 text-[11px] text-slate-300 transition hover:border-aqua-300/40 hover:text-aqua-100 disabled:cursor-not-allowed disabled:opacity-40"
            >
              {t("scripts.exportJson")}
            </button>
          </div>
        </div>

        {currentFolder && (
          <div className="mt-3 grid gap-2 border-t border-white/[0.06] pt-3 sm:grid-cols-[1fr_1fr_auto]">
            <input
              className={INPUT + " py-1.5 text-[12px]"}
              value={logHook}
              onChange={(e) => setLogHook(e.target.value)}
              placeholder={`${t("scripts.logWebhook")} · ${t("scripts.webhookPh")}`}
            />
            <input
              className={INPUT + " py-1.5 text-[12px]"}
              value={alertHook}
              onChange={(e) => setAlertHook(e.target.value)}
              placeholder={`${t("scripts.alertWebhook")} · ${t("scripts.webhookPh")}`}
            />
            <Button
              variant="outline"
              className="px-3 py-1.5 text-[12px]"
              disabled={savingHooks || (!logHook && !alertHook)}
              onClick={handleSaveWebhooks}
            >
              {savingHooks ? t("scripts.working") : t("scripts.saveWebhooks")}
            </Button>
            <p className="text-[10.5px] leading-relaxed text-slate-500 sm:col-span-3">{t("scripts.webhookHint")}</p>
          </div>
        )}
      </GlassCard>

      <div className="grid items-start gap-5 xl:grid-cols-[320px_minmax(0,1fr)]">
      {/* ---------------- 项目列表 ---------------- */}
      <div className="grid content-start gap-4">
        <GlassCard className="p-5">
          <div className="mb-4 flex items-center gap-2.5">
            <Icon name="layers" className="h-4 w-4 text-aqua-300" />
            <h2 className="text-[13.5px] font-semibold text-slate-100">{t("scripts.projects")}</h2>
            <span className="ml-auto font-mono text-[11px] text-slate-500">{projects.length}</span>
          </div>

          {metrics.length > 0 && (
            <div className="mb-4 rounded-2xl border border-white/[0.07] bg-white/[0.02] p-3">
              <div className="mb-2 flex items-center gap-2">
                <Icon name="activity" className="h-3.5 w-3.5 text-aqua-300" />
                <span className="text-[10.5px] tracking-[0.16em] text-slate-400/70 uppercase">
                  {t("scripts.metricsTitle")}
                </span>
                <span className="ml-auto font-mono text-[10px] text-slate-600">{metrics[metrics.length - 1]?.date}</span>
              </div>
              <div className="grid grid-cols-4 gap-1.5 text-center">
                {[
                  ["scripts.metricValidations", metrics[metrics.length - 1]?.validations ?? 0, "text-emerald-300"],
                  ["scripts.metricRejected", metrics[metrics.length - 1]?.rejected ?? 0, "text-rose-300"],
                  ["scripts.metricExecutions", metrics[metrics.length - 1]?.executions ?? 0, "text-aqua-200"],
                  ["scripts.metricDevices", metrics[metrics.length - 1]?.devices ?? 0, "text-slate-300"],
                ].map(([key, value, tone], index) => (
                  <div key={String(key)} style={{ "--i": index } as CSSProperties} className="anim-stagger">
                    <div className={`font-mono text-[15px] ${tone as string}`}>
                      <AnimatedNumber value={Number(value)} />
                    </div>
                    <div className="mt-0.5 text-[9.5px] leading-tight text-slate-500">{t(key as TKey)}</div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {!creating ? (
            <Button variant="outline" className="w-full py-2.5" onClick={() => setCreating(true)}>
              <Icon name="sparkle" className="h-4 w-4" />
              {t("scripts.create")}
            </Button>
          ) : (
            <div className="grid gap-2.5">
              <input
                className={INPUT}
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder={t("scripts.namePh")}
                maxLength={80}
                autoFocus
              />
              <input
                className={INPUT}
                value={newDesc}
                onChange={(e) => setNewDesc(e.target.value)}
                placeholder={t("scripts.descPh")}
                maxLength={500}
              />
              <div className="flex gap-2">
                <Button className="flex-1 py-2" disabled={busy || newName.trim().length < 2} onClick={handleCreate}>
                  {busy ? t("scripts.working") : t("scripts.save")}
                </Button>
                <Button variant="ghost" className="px-3 py-2" onClick={() => setCreating(false)}>
                  {t("common.cancel")}
                </Button>
              </div>
            </div>
          )}
        </GlassCard>

        <div className="grid gap-2">
          {loading && (
            <div className="grid gap-2">
              {[0, 1, 2].map((i) => (
                <div key={i} className="glass-soft rounded-2xl border border-white/[0.07] px-4 py-3.5">
                  <Skeleton className="h-3" style={{ width: `${120 + i * 34}px` }} />
                  <Skeleton className="mt-2 h-2.5 w-24" />
                </div>
              ))}
            </div>
          )}
          {!loading && projects.length === 0 && (
            <div className="glass-soft rounded-2xl border border-dashed border-white/[0.09] px-4 py-8 text-center">
              <p className="text-[12.5px] text-slate-400">{t("scripts.empty")}</p>
              <p className="mt-1 text-[11px] text-slate-500">{t("scripts.emptyHint")}</p>
            </div>
          )}
          {projects.map((p, index) => {
            const active = p.scriptId === selected;
            return (
              <button
                key={p.scriptId}
                onClick={() => setSelected(p.scriptId)}
                style={{ "--i": index } as CSSProperties}
                className={
                  "anim-stagger anim-lift group w-full cursor-pointer rounded-2xl border px-4 py-3 text-left transition-all duration-300 " +
                  (active
                    ? "border-aqua-300/30 bg-aqua-400/[0.09] text-aqua-50 shadow-[0_0_30px_-18px_rgba(34,226,213,0.9)_inset]"
                    : "border-white/[0.07] bg-white/[0.02] text-slate-300 hover:border-aqua-300/20 hover:bg-white/[0.04]")
                }
              >
                <span className="flex items-center gap-2">
                  <span className={"h-1.5 w-1.5 shrink-0 rounded-full " + (p.active ? "bg-emerald-300" : "bg-slate-600")} />
                  <span className="truncate text-[13px] font-medium">{p.name}</span>
                </span>
                <span className="mt-1 flex items-center gap-1.5 truncate font-mono text-[10.5px] text-slate-500">
                  {p.scriptId}
                  {p.latestVersion && (
                    <span className="rounded-md border border-aqua-300/20 bg-aqua-400/[0.08] px-1 py-px text-[9.5px] text-aqua-100/90">
                      {p.latestVersion}
                    </span>
                  )}
                  {typeof p.licenseCount === "number" && (
                    <span className="text-[9.5px] text-slate-600">{p.licenseCount} keys</span>
                  )}
                </span>
              </button>
            );
          })}
        </div>

        {error && (
          <div className="flex items-start gap-2 rounded-xl border border-rose-400/25 bg-rose-500/[0.08] px-3 py-2.5 text-[11.5px] text-rose-200">
            <Icon name="sparkle" className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            {t(error)}
          </div>
        )}
      </div>

      {/* ---------------- 详情 ---------------- */}
      <div className="grid min-w-0 content-start gap-4">
        {!current ? (
          <GlassCard className="flex min-h-[280px] flex-col items-center justify-center p-8 text-center">
            <Icon name="layers" className="h-8 w-8 text-slate-600" />
            <p className="mt-3 text-[13px] text-slate-400">{t("scripts.selectHint")}</p>
          </GlassCard>
        ) : (
          <>
            <GlassCard className="p-5">
              <div className="flex flex-wrap items-start gap-3">
                <div className="min-w-0">
                  <h2 className="truncate text-[16px] font-semibold text-slate-50">{current.name}</h2>
                  {current.description && (
                    <p className="mt-1 text-[12px] leading-relaxed text-slate-400/80">{current.description}</p>
                  )}
                  <div className="mt-2 flex flex-wrap items-center gap-2">
                    <Pill className="font-mono">{current.scriptId}</Pill>
                    <span className="text-[10.5px] text-slate-500">{t("scripts.created")}: {when(current.createdAt)}</span>
                  </div>
                </div>
              </div>
              <div className="mt-4 grid grid-cols-3 gap-2.5">
                <Stat label={t("scripts.licenses")} value={<AnimatedNumber value={licenseTotal || licenses.length} />} />
                <Stat label={t("scripts.active")} value={<AnimatedNumber value={activeLicenses} />} />
                <Stat label={t("scripts.releases")} value={<AnimatedNumber value={releases.length} />} />
              </div>
            </GlassCard>

            <GlassCard className="p-5">
              <div className="mb-2 flex items-center gap-2.5">
                <Icon name="code" className="h-4 w-4 text-aqua-300" />
                <h3 className="text-[13px] font-semibold text-slate-100">{t("scripts.loaderUrl")}</h3>
                <span className="ml-auto text-[10.5px] text-slate-500">{t("scripts.loaderHint")}</span>
              </div>
              <div className="grid gap-2.5">
                <CopyField
                  label={t("scripts.loaderCode")}
                  value={loadstringSnippet(current.scriptId)}
                  copied={copiedId === "loader-code"}
                  onCopy={() => void copyKey(loadstringSnippet(current.scriptId), "loader-code")}
                />
                <CopyField
                  label={t("scripts.directUrl")}
                  value={loaderUrl(current.scriptId)}
                  copied={copiedId === "loader-url"}
                  onCopy={() => void copyKey(loaderUrl(current.scriptId), "loader-url")}
                />
              </div>
              {copyFailed && (
                <p className="mt-2 text-[10.5px] text-rose-200">{t("scripts.copyFailed")}</p>
              )}
              <label className="mt-3 flex items-start gap-2.5 rounded-xl border border-white/[0.07] bg-white/[0.02] px-3 py-2.5">
                <input
                  type="checkbox"
                  className="mt-0.5 h-3.5 w-3.5 accent-[#22e2d5]"
                  checked={Boolean(current.ffa)}
                  disabled={busy}
                  onChange={(e) => void toggleFfa(e.target.checked)}
                />
                <span className="min-w-0">
                  <span className="block text-[12px] text-slate-200">{t("scripts.ffa")}</span>
                  <span className="mt-0.5 block text-[10.5px] leading-relaxed text-slate-500">{t("scripts.ffaHint")}</span>
                </span>
              </label>
              <label className="mt-2 flex items-start gap-2.5 rounded-xl border border-white/[0.07] bg-white/[0.02] px-3 py-2.5">
                <input
                  type="checkbox"
                  className="mt-0.5 h-3.5 w-3.5 accent-[#22e2d5]"
                  checked={Boolean(current.heartbeat)}
                  disabled={busy}
                  onChange={(e) => void toggleHeartbeat(e.target.checked)}
                />
                <span className="min-w-0">
                  <span className="block text-[12px] text-slate-200">{t("scripts.heartbeat")}</span>
                  <span className="mt-0.5 block text-[10.5px] leading-relaxed text-slate-500">{t("scripts.heartbeatHint")}</span>
                </span>
              </label>
              <p className="mt-2 text-[10.5px] leading-relaxed text-slate-500">{t("scripts.silentNote")}</p>
            </GlassCard>

            <div className="glass-soft inline-flex w-fit items-center gap-1 rounded-xl border border-white/[0.08] p-1">
              {([
                ["licenses", "scripts.tabLicenses"],
                ["releases", "scripts.tabReleases"],
              ] as const).map(([id, key]) => (
                <button
                  key={id}
                  onClick={() => setTab(id)}
                  className={
                    "cursor-pointer rounded-lg px-3.5 py-1.5 text-[12px] font-medium transition-all duration-300 " +
                    (tab === id
                      ? "bg-aqua-400/20 text-aqua-100 shadow-[0_0_24px_-10px_rgba(34,226,213,0.9)_inset]"
                      : "text-slate-400 hover:text-aqua-100")
                  }
                >
                  {t(key)}
                </button>
              ))}
            </div>

            {tab === "licenses" ? (
              <>
                <GlassCard className="p-5">
                  <div className="mb-3 flex items-center gap-2.5">
                    <Icon name="lock" className="h-4 w-4 text-aqua-300" />
                    <h3 className="text-[13px] font-semibold text-slate-100">{t("scripts.issue")}</h3>
                  </div>
                  <div className="grid gap-2.5 sm:grid-cols-[1fr_auto]">
                    <input
                      className={INPUT}
                      value={label}
                      onChange={(e) => setLabel(e.target.value)}
                      placeholder={t("scripts.labelPh")}
                      maxLength={80}
                    />
                    <Button className="px-4 py-2.5" disabled={busy} onClick={handleIssue}>
                      {busy ? t("scripts.working") : t("scripts.issueAction")}
                    </Button>
                  </div>

                  <div className="mt-2.5 flex flex-wrap items-center gap-2">
                    <div className="glass-soft inline-flex items-center gap-0.5 rounded-xl border border-white/[0.08] p-0.5">
                      {([
                        ["forever", "scripts.modeForever"],
                        ["days", "scripts.modeDays"],
                        ["date", "scripts.modeDate"],
                      ] as const).map(([id, key]) => (
                        <button
                          key={id}
                          type="button"
                          onClick={() => setExpiryMode(id)}
                          className={
                            "cursor-pointer rounded-lg px-2.5 py-1 text-[11px] font-medium transition " +
                            (expiryMode === id
                              ? "bg-aqua-400/20 text-aqua-100"
                              : "text-slate-400 hover:text-aqua-100")
                          }
                        >
                          {t(key)}
                        </button>
                      ))}
                    </div>
                    {expiryMode === "days" && (
                      <input
                        className={INPUT + " w-24 py-1.5 text-[12px]"}
                        type="number"
                        min={1}
                        max={3650}
                        value={days}
                        onChange={(e) => setDays(e.target.value)}
                        placeholder={t("scripts.daysPh")}
                      />
                    )}
                    {expiryMode === "date" && (
                      <input
                        className={INPUT + " w-auto py-1.5 text-[12px]"}
                        type="date"
                        value={expiry}
                        onChange={(e) => setExpiry(e.target.value)}
                        title={t("scripts.expiry")}
                      />
                    )}
                  </div>
                  <p className="mt-2 text-[10.5px] text-slate-500">
                    {expiryMode === "days" ? t("scripts.daysHint") : t("scripts.expiryHint")}
                  </p>

                  <div className="mt-3 flex flex-wrap items-center gap-2 border-t border-white/[0.06] pt-3">
                    <span className="text-[11px] text-slate-400">{t("scripts.batchCount")}</span>
                    <input
                      className={INPUT + " w-20 py-1.5 text-[12px]"}
                      type="number"
                      min={1}
                      max={100}
                      value={batch}
                      onChange={(e) => setBatch(e.target.value)}
                    />
                    <span className="text-[10.5px] text-slate-500">{t("scripts.batchHint")}</span>
                    <Button
                      variant="outline"
                      className="ml-auto px-3 py-1.5 text-[12px]"
                      disabled={busy || !folderId}
                      onClick={handleBatch}
                    >
                      {busy ? t("scripts.working") : t("scripts.issueAction")}
                    </Button>
                  </div>

                  {batchKeys.length > 0 && (
                    <div className="mt-3 rounded-2xl border border-amber-300/25 bg-amber-400/[0.07] p-3.5">
                      <div className="flex items-center gap-2">
                        <p className="text-[11px] font-semibold text-amber-100">
                          {t("scripts.batchIssued", { n: batchKeys.length })}
                        </p>
                        <Button
                          variant="outline"
                          className="ml-auto px-2.5 py-1"
                          onClick={() => void copyKey(batchKeys.join("\n"), "batch")}
                        >
                          <Icon name={copiedId === "batch" ? "check" : "copy"} className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                      <textarea
                        readOnly
                        value={batchKeys.join("\n")}
                        className="mt-2 h-24 w-full resize-y rounded-lg bg-black/40 p-2 font-mono text-[11px] text-amber-100"
                      />
                      <p className="mt-1.5 text-[10.5px] text-amber-100/70">{t("scripts.keyOnceHint")}</p>
                    </div>
                  )}

                  {issuedKey && (
                    <div className="mt-3 rounded-2xl border border-amber-300/25 bg-amber-400/[0.07] p-3.5">
                      <p className="text-[11px] font-semibold text-amber-100">{t("scripts.keyOnce")}</p>
                      <div className="mt-2 flex items-center gap-2">
                        <code className="min-w-0 flex-1 truncate rounded-lg bg-black/40 px-2.5 py-2 font-mono text-[11.5px] text-amber-100">
                          {issuedKey.key}
                        </code>
                        <Button
                          variant="outline"
                          className="px-2.5 py-1.5"
                          onClick={() => void copyKey(issuedKey.key, "issued")}
                        >
                          <Icon name={copiedId === "issued" ? "check" : "copy"} className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                      <p className="mt-1.5 text-[10.5px] text-amber-100/70">{t("scripts.keyOnceHint")}</p>
                    </div>
                  )}
                </GlassCard>

                {selectedIds.length > 0 && (
                  <div className="anim-enter flex flex-wrap items-center gap-2 rounded-2xl border border-aqua-300/25 bg-aqua-400/[0.07] px-3.5 py-2.5">
                    <span className="text-[11.5px] text-aqua-100">
                      {t("scripts.batchSelected", { n: selectedIds.length })}
                    </span>
                    <div className="ml-auto flex flex-wrap gap-1.5">
                      {([
                        ["ban", "scripts.batchBan"],
                        ["unban", "scripts.batchUnban"],
                        ["reset-hwid", "scripts.batchReset"],
                        ["revoke", "scripts.batchRevoke"],
                      ] as const).map(([action, key]) => (
                        <button
                          key={action}
                          disabled={busy}
                          onClick={() => void runBatch(action)}
                          className="cursor-pointer rounded-lg border border-white/12 px-2 py-1 text-[10.5px] text-slate-200 transition hover:border-aqua-300/40 hover:text-aqua-100 disabled:opacity-40"
                        >
                          {t(key)}
                        </button>
                      ))}
                      <button
                        onClick={() => setSelectedIds([])}
                        className="cursor-pointer rounded-lg px-2 py-1 text-[10.5px] text-slate-400 transition hover:text-aqua-100"
                      >
                        {t("scripts.batchClear")}
                      </button>
                    </div>
                  </div>
                )}

                <GlassCard className="overflow-hidden p-0">
                  <div className="flex flex-wrap items-center gap-2.5 border-b border-white/[0.06] px-5 py-3">
                    <Icon name="shield" className="h-4 w-4 text-aqua-300" />
                    <h3 className="text-[13px] font-semibold text-slate-100">{t("scripts.licenses")}</h3>
                    <span className="font-mono text-[11px] text-slate-500">{licenseTotal}</span>
                    <div className="ml-auto flex flex-wrap items-center gap-2">
                      <input
                        className={INPUT + " w-40 py-1.5 text-[11.5px]"}
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        onKeyDown={(e) => { if (e.key === "Enter") applyFilter(0); }}
                        placeholder={t("scripts.searchPh")}
                      />
                      <select
                        className={INPUT + " w-auto py-1.5 text-[11.5px]"}
                        value={statusFilter}
                        onChange={(e) => {
                          setStatusFilter(e.target.value);
                          applyFilter(0, search, e.target.value);
                        }}
                      >
                        <option value="">{t("scripts.filterAll")}</option>
                        <option value="active">{t("scripts.stateActive")}</option>
                        <option value="expired">{t("scripts.stateExpired")}</option>
                        <option value="banned">{t("scripts.stateBanned")}</option>
                        <option value="revoked">{t("scripts.stateRevoked")}</option>
                      </select>
                      <Button variant="outline" className="px-2.5 py-1.5 text-[11.5px]" onClick={() => applyFilter(0)}>
                        {t("scripts.check")}
                      </Button>
                    </div>
                  </div>
                  {licenses.length === 0 && busy ? (
                    <SkeletonRows rows={4} className="px-3 py-3" />
                  ) : licenses.length === 0 ? (
                    <p className="px-5 py-8 text-center text-[12px] text-slate-500">{t("scripts.noLicenses")}</p>
                  ) : (
                    <div className="overflow-x-auto">
                      <table className="w-full min-w-[620px] text-left text-[11.5px]">
                        <thead>
                          <tr className="text-[10px] tracking-[0.14em] text-slate-500 uppercase">
                            <th className="w-9 py-2.5 pl-4 font-medium">
                              <input
                                type="checkbox"
                                aria-label={t("scripts.selectAll")}
                                className="h-3.5 w-3.5 accent-[#22e2d5]"
                                checked={licenses.length > 0 && licenses.every((l) => selectedIds.includes(l.licenseId))}
                                onChange={(e) => {
                                  if (e.target.checked) {
                                    setSelectedIds((prev) => [...new Set([...prev, ...licenses.map((l) => l.licenseId)])]);
                                  } else {
                                    const pageIds = new Set(licenses.map((l) => l.licenseId));
                                    setSelectedIds((prev) => prev.filter((id) => !pageIds.has(id)));
                                  }
                                }}
                              />
                            </th>
                            <th className="px-3 py-2.5 font-medium">{t("scripts.colLabel")}</th>
                            <th className="px-3 py-2.5 font-medium">{t("scripts.colState")}</th>
                            <th className="px-3 py-2.5 font-medium">{t("scripts.colDevice")}</th>
                            <th className="px-3 py-2.5 font-medium">{t("scripts.colLastUsed")}</th>
                            <th className="px-5 py-2.5 text-right font-medium">{t("scripts.colActions")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {licenses.map((l, rowIndex) => {
                            const expired = l.expiresAt !== null && l.expiresAt <= Date.now();
                            const state = l.banned
                              ? { key: "scripts.stateBanned" as TKey, tone: "text-rose-400" }
                              : l.revokedAt
                                ? { key: "scripts.stateRevoked" as TKey, tone: "text-rose-300" }
                                : expired
                                  ? { key: "scripts.stateExpired" as TKey, tone: "text-amber-300" }
                                  : { key: "scripts.stateActive" as TKey, tone: "text-emerald-300" };
                            return (
                              <tr
                                key={l.licenseId}
                                style={{ "--i": Math.min(rowIndex, 12) } as CSSProperties}
                                className="anim-stagger border-t border-white/[0.05]"
                              >
                                <td className="w-9 py-2.5 pl-4">
                                  <input
                                    type="checkbox"
                                    aria-label={l.licenseId}
                                    className="h-3.5 w-3.5 accent-[#22e2d5]"
                                    checked={selectedIds.includes(l.licenseId)}
                                    onChange={(e) =>
                                      setSelectedIds((prev) =>
                                        e.target.checked
                                          ? [...new Set([...prev, l.licenseId])]
                                          : prev.filter((id) => id !== l.licenseId),
                                      )
                                    }
                                  />
                                </td>
                                <td className="px-3 py-2.5">
                                  <span className="block truncate text-slate-200">{l.label || "—"}</span>
                                  <span className="block font-mono text-[10px] text-slate-500">{l.licenseId}</span>
                                </td>
                                <td className={"px-3 py-2.5 " + state.tone}>
                                  {t(state.key)}
                                  {l.banned && l.banReason ? (
                                    <span className="mt-0.5 block truncate text-[10px] text-rose-300/70">{l.banReason}</span>
                                  ) : null}
                                </td>
                                <td className="px-3 py-2.5 text-slate-400">
                                  {l.hwidBound ? t("scripts.bound") : t("scripts.unbound")}
                                </td>
                                <td className="px-3 py-2.5 text-slate-500">{when(l.lastUsedAt)}</td>
                                <td className="px-5 py-2.5">
                                  <div className="flex justify-end gap-1.5">
                                    <button
                                      disabled={busy || !l.hwidBound}
                                      onClick={() => handleReset(l.licenseId)}
                                      className="cursor-pointer rounded-lg border border-white/10 px-2 py-1 text-[10.5px] text-slate-300 transition hover:border-aqua-300/40 hover:text-aqua-100 disabled:cursor-not-allowed disabled:opacity-40"
                                    >
                                      {t("scripts.resetDevice")}
                                    </button>
                                    {l.banned ? (
                                      <button
                                        disabled={busy}
                                        onClick={() => handleUnban(l.licenseId)}
                                        className="cursor-pointer rounded-lg border border-white/10 px-2 py-1 text-[10.5px] text-slate-300 transition hover:border-emerald-300/40 hover:text-emerald-200 disabled:cursor-not-allowed disabled:opacity-40"
                                      >
                                        {t("scripts.unban")}
                                      </button>
                                    ) : (
                                      <button
                                        disabled={busy || Boolean(l.revokedAt)}
                                        onClick={() => { setBanTarget(l); setBanReason(""); setBanDays(""); }}
                                        className="cursor-pointer rounded-lg border border-white/10 px-2 py-1 text-[10.5px] text-slate-300 transition hover:border-rose-400/40 hover:text-rose-300 disabled:cursor-not-allowed disabled:opacity-40"
                                      >
                                        {t("scripts.ban")}
                                      </button>
                                    )}
                                    <button
                                      disabled={busy || Boolean(l.revokedAt)}
                                      onClick={() => handleRevoke(l.licenseId)}
                                      className="cursor-pointer rounded-lg border border-white/10 px-2 py-1 text-[10.5px] text-slate-300 transition hover:border-rose-400/40 hover:text-rose-300 disabled:cursor-not-allowed disabled:opacity-40"
                                    >
                                      {t("scripts.revoke")}
                                    </button>
                                  </div>
                                </td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                  )}
                  {licenseTotal > PAGE_SIZE && (
                    <div className="flex items-center gap-3 border-t border-white/[0.06] px-5 py-2.5 text-[11px] text-slate-500">
                      <span>
                        {t("scripts.pageInfo", {
                          a: pageFrom + 1,
                          b: Math.min(pageFrom + licenses.length, licenseTotal),
                          t: licenseTotal,
                        })}
                      </span>
                      <div className="ml-auto flex gap-2">
                        <Button
                          variant="outline"
                          className="px-2.5 py-1 text-[11px]"
                          disabled={pageFrom === 0}
                          onClick={() => applyFilter(Math.max(0, pageFrom - PAGE_SIZE))}
                        >
                          {t("scripts.prev")}
                        </Button>
                        <Button
                          variant="outline"
                          className="px-2.5 py-1 text-[11px]"
                          disabled={pageFrom + PAGE_SIZE >= licenseTotal}
                          onClick={() => applyFilter(pageFrom + PAGE_SIZE)}
                        >
                          {t("scripts.next")}
                        </Button>
                      </div>
                    </div>
                  )}
                </GlassCard>

                {unbanToken && (
                  <GlassCard className="border-emerald-300/25 bg-emerald-400/[0.06] p-4">
                    <p className="text-[11.5px] font-semibold text-emerald-100">{t("scripts.banDone")}</p>
                    <div className="mt-2 flex items-center gap-2">
                      <code className="min-w-0 flex-1 truncate rounded-lg bg-black/40 px-2.5 py-2 font-mono text-[11px] text-emerald-100">
                        {unbanToken}
                      </code>
                      <Button
                        variant="outline"
                        className="px-2.5 py-1.5"
                        onClick={() => void copyKey(unbanToken, "unban")}
                      >
                        <Icon name={copiedId === "unban" ? "check" : "copy"} className="h-3.5 w-3.5" />
                      </Button>
                      <Button variant="ghost" className="px-2 py-1.5" onClick={() => setUnbanToken(null)}>
                        {t("common.close")}
                      </Button>
                    </div>
                  </GlassCard>
                )}

                {banTarget && (
                  <div className="anim-backdrop fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4 backdrop-blur-sm">
                    <GlassCard className="anim-modal w-full max-w-[420px] p-5">
                      <h3 className="text-[14px] font-semibold text-slate-100">{t("scripts.banTitle")}</h3>
                      <p className="mt-1 text-[11px] text-slate-500">{banTarget.label || banTarget.licenseId}</p>
                      <div className="mt-3 grid gap-2.5">
                        <input
                          className={INPUT}
                          value={banReason}
                          onChange={(e) => setBanReason(e.target.value)}
                          placeholder={t("scripts.banReasonPh")}
                          maxLength={200}
                        />
                        <input
                          className={INPUT}
                          type="number"
                          min={1}
                          max={3650}
                          value={banDays}
                          onChange={(e) => setBanDays(e.target.value)}
                          placeholder={t("scripts.banDaysPh")}
                        />
                        <div className="flex justify-end gap-2">
                          <Button variant="ghost" className="px-3 py-2" onClick={() => setBanTarget(null)}>
                            {t("common.cancel")}
                          </Button>
                          <Button className="px-3 py-2" disabled={busy} onClick={handleBan}>
                            {busy ? t("scripts.working") : t("scripts.banConfirm")}
                          </Button>
                        </div>
                      </div>
                    </GlassCard>
                  </div>
                )}
              </>
            ) : (
              <>
                <GlassCard className="p-5">
                  <div className="mb-3 flex flex-wrap items-center gap-2.5">
                    <Icon name="upload" className="h-4 w-4 text-aqua-300" />
                    <h3 className="text-[13px] font-semibold text-slate-100">{t("scripts.publish")}</h3>
                    <div className="glass-soft ml-auto inline-flex items-center gap-0.5 rounded-xl border border-white/[0.08] p-0.5">
                      {([
                        ["pipeline", "scripts.tabPipeline"],
                        ["manual", "scripts.tabManual"],
                      ] as const).map(([id, key]) => (
                        <button
                          key={id}
                          type="button"
                          onClick={() => setSourceMode(id)}
                          className={
                            "cursor-pointer rounded-lg px-2.5 py-1 text-[11px] font-medium transition " +
                            (sourceMode === id
                              ? "bg-aqua-400/20 text-aqua-100"
                              : "text-slate-400 hover:text-aqua-100")
                          }
                        >
                          {t(key)}
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className="grid gap-2.5">
                    <div className="grid gap-2.5 sm:grid-cols-[180px_1fr]">
                      <input
                        className={INPUT}
                        value={version}
                        onChange={(e) => setVersion(e.target.value)}
                        placeholder="v1"
                        maxLength={32}
                      />
                      <p className="self-center text-[10.5px] leading-relaxed text-slate-500">
                        {sourceMode === "pipeline" ? t("scripts.pipelineHint") : t("scripts.publishHint")}
                      </p>
                    </div>

                    {sourceMode === "pipeline" ? (
                      <>
                        <textarea
                          className={INPUT + " min-h-[150px] resize-y font-mono text-[11.5px] leading-relaxed"}
                          value={rawSource}
                          onChange={(e) => setRawSource(e.target.value)}
                          placeholder={t("scripts.sourcePh")}
                          spellCheck={false}
                        />
                        <div className="flex flex-wrap items-center gap-2">
                          <label className="cursor-pointer rounded-xl border border-white/10 px-3 py-1.5 text-[11px] text-slate-300 transition hover:border-aqua-300/40 hover:text-aqua-100">
                            {t("scripts.pickFile")}
                            <input
                              type="file"
                              accept=".lua,.luau,text/plain"
                              className="hidden"
                              onChange={(e) => {
                                void readSourceFile(e.target.files?.[0]);
                                e.target.value = "";
                              }}
                            />
                          </label>
                          <span className="font-mono text-[10.5px] text-slate-500">
                            {new TextEncoder().encode(rawSource).length} / 1048576 B
                          </span>
                          <Button
                            className="ml-auto px-4 py-2"
                            disabled={pipelineBusy || !rawSource.trim() || !version.trim()}
                            onClick={handlePipeline}
                          >
                            {pipelineBusy ? t("scripts.working") : t("scripts.publishPipeline")}
                          </Button>
                        </div>
                        <div className="flex flex-wrap items-center gap-2">
                          <Button
                            variant="outline"
                            className="px-3 py-1.5 text-[11.5px]"
                            disabled={pipelineBusy || !rawSource.trim() || !version.trim() || !folderId}
                            onClick={() => void handleBatchPublish()}
                          >
                            <Icon name="layers" className="h-3.5 w-3.5" />
                            {t("scripts.batchPublish")}
                          </Button>
                          <span className="text-[10.5px] text-slate-500">{t("scripts.batchPublishHint")}</span>
                        </div>
                        {publishJob && (
                          <div className="anim-enter rounded-2xl border border-white/[0.07] bg-black/25 px-3.5 py-3">
                            <StepProgress
                              steps={[
                                { key: "queued", label: t("scripts.stageQueued") },
                                { key: "injecting", label: t("scripts.stageInjecting") },
                                { key: "obfuscating", label: t("scripts.stageObfuscating") },
                                { key: "storing", label: t("scripts.stageStoring") },
                                { key: "completed", label: t("scripts.stageCompleted") },
                              ]}
                              activeKey={publishJob.stage}
                              progress={publishJob.progress}
                              failed={publishJob.status === "failed"}
                            />
                            <p className="mt-2 text-[10.5px] text-slate-500">
                              {t("scripts.stageElapsed", { s: (publishJob.elapsedMs / 1000).toFixed(1) })}
                            </p>
                          </div>
                        )}
                      </>
                    ) : (
                      <>
                        <textarea
                          className={INPUT + " min-h-[150px] resize-y font-mono text-[11.5px] leading-relaxed"}
                          value={artifact}
                          onChange={(e) => setArtifact(e.target.value)}
                          placeholder={t("scripts.artifactPh")}
                          spellCheck={false}
                        />
                        <div className="flex items-center gap-2">
                          <span className="font-mono text-[10.5px] text-slate-500">
                            {new TextEncoder().encode(artifact).length} / 4194304 B
                          </span>
                          <Button
                            className="ml-auto px-4 py-2"
                            disabled={busy || !artifact.trim() || !version.trim()}
                            onClick={handlePublish}
                          >
                            {busy ? t("scripts.working") : t("scripts.publishAction")}
                          </Button>
                        </div>
                      </>
                    )}
                  </div>
                </GlassCard>

                {publishResult && publishResult.scriptId === current.scriptId && (
                  <GlassCard className="border-emerald-300/25 bg-emerald-400/[0.05] p-5">
                    <div className="flex flex-wrap items-center gap-2.5">
                      <span className="flex h-7 w-7 items-center justify-center rounded-full border border-emerald-300/30 bg-emerald-400/10">
                        <Icon name="check" className="h-3.5 w-3.5 text-emerald-200" />
                      </span>
                      <div className="min-w-0">
                        <h3 className="text-[13px] font-semibold text-emerald-50">
                          {t("scripts.pipelineDone", {
                            v: publishResult.version,
                            size: (publishResult.size / 1024).toFixed(1),
                          })}
                        </h3>
                        <p className="mt-0.5 text-[10.5px] text-emerald-100/70">{t("scripts.publishReady")}</p>
                      </div>
                      <Button
                        variant="ghost"
                        className="ml-auto px-2 py-1 text-[11px]"
                        onClick={() => setPublishResult(null)}
                      >
                        {t("common.close")}
                      </Button>
                    </div>

                    <div className="mt-3 grid gap-2.5">
                      <CopyField
                        label={t("scripts.loaderCode")}
                        value={loadstringSnippet(publishResult.scriptId)}
                        copied={copiedId === "pub-code"}
                        onCopy={() => void copyKey(loadstringSnippet(publishResult.scriptId), "pub-code")}
                      />
                      <CopyField
                        label={t("scripts.directUrl")}
                        value={loaderUrl(publishResult.scriptId)}
                        copied={copiedId === "pub-url"}
                        onCopy={() => void copyKey(loaderUrl(publishResult.scriptId), "pub-url")}
                      />
                    </div>

                    <div className="mt-3 flex flex-wrap items-center gap-2">
                      <Button
                        variant="outline"
                        className="px-3 py-1.5 text-[11.5px]"
                        onClick={() => void copyKey(keySnippet(publishResult.scriptId), "pub-full")}
                      >
                        <Icon
                          name={copiedId === "pub-full" ? "check" : "copy"}
                          className="mr-1.5 h-3.5 w-3.5"
                        />
                        {t("scripts.copyWithKey")}
                      </Button>
                      <a
                        className="rounded-xl border border-white/10 px-3 py-1.5 text-[11.5px] text-slate-300 transition hover:border-aqua-300/40 hover:text-aqua-100"
                        href={loaderUrl(publishResult.scriptId)}
                        target="_blank"
                        rel="noreferrer"
                      >
                        {t("scripts.openLoader")}
                      </a>
                      <span className="text-[10.5px] leading-relaxed text-slate-500">
                        {t("scripts.copyWithKeyHint")}
                      </span>
                    </div>
                    {copyFailed && (
                      <p className="mt-2 text-[10.5px] text-rose-200">{t("scripts.copyFailed")}</p>
                    )}
                  </GlassCard>
                )}

                <GlassCard className="overflow-hidden p-0">
                  <div className="flex items-center gap-2.5 border-b border-white/[0.06] px-5 py-3.5">
                    <Icon name="layers" className="h-4 w-4 text-aqua-300" />
                    <h3 className="text-[13px] font-semibold text-slate-100">{t("scripts.releases")}</h3>
                    <span className="ml-auto font-mono text-[11px] text-slate-500">{releases.length}</span>
                  </div>
                  {releases.length === 0 ? (
                    <p className="px-5 py-8 text-center text-[12px] text-slate-500">{t("scripts.noReleases")}</p>
                  ) : (
                    <div className="grid gap-1 px-3 py-2">
                      {releases
                        .slice()
                        .sort((a, b) => b.createdAt - a.createdAt)
                        .map((r) => (
                          <div
                            key={r.version}
                            className="flex flex-wrap items-center gap-3 rounded-xl px-2 py-2 transition hover:bg-white/[0.03]"
                          >
                            <span className="rounded-lg border border-aqua-300/25 bg-aqua-400/[0.08] px-2 py-0.5 font-mono text-[11px] text-aqua-100">
                              {r.version}
                            </span>
                            <span className="min-w-0 flex-1 truncate font-mono text-[10.5px] text-slate-500">
                              {r.sha256}
                            </span>
                            <span className="font-mono text-[10.5px] text-slate-500">{(r.size / 1024).toFixed(1)} KB</span>
                            <span className="text-[10.5px] text-slate-500">{when(r.createdAt)}</span>
                          </div>
                        ))}
                    </div>
                  )}
                </GlassCard>
              </>
            )}
          </>
        )}
      </div>
      </div>
    </div>
  );
}
