import { useCallback, useEffect, useMemo, useState } from "react";
import { Button, GlassCard, Icon, Pill, Stat } from "../components/ui";
import { useI18n, type TKey } from "../lib/i18n";
import {
  createScript,
  issueLicense,
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
  script_limit: "scripts.errScriptLimit",
  license_limit: "scripts.errLicenseLimit",
  release_limit: "scripts.errReleaseLimit",
  release_too_large: "scripts.errReleaseTooLarge",
  bad_version: "scripts.errBadVersion",
  network: "scripts.errNetwork",
  unknown: "scripts.errUnknown",
};

function when(ts: number | null) {
  if (!ts) return "—";
  return new Date(ts).toLocaleString();
}

export function Scripts() {
  const { t } = useI18n();
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
  const [releases, setReleases] = useState<ReleaseRecord[]>([]);
  const [issuedKey, setIssuedKey] = useState<{ key: string; label: string } | null>(null);
  const [label, setLabel] = useState("");
  const [expiry, setExpiry] = useState("");
  const [version, setVersion] = useState("v1");
  const [artifact, setArtifact] = useState("");
  const [copied, setCopied] = useState(false);

  const current = useMemo(
    () => projects.find((p) => p.scriptId === selected) ?? null,
    [projects, selected],
  );

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

  const refreshDetail = useCallback(async (scriptId: string) => {
    const [lic, rel] = await Promise.all([listLicenses(scriptId), listReleases(scriptId)]);
    if (lic.ok && lic.data) setLicenses(lic.data);
    if (rel.ok && rel.data) setReleases(rel.data);
  }, []);

  useEffect(() => {
    void refreshProjects();
  }, [refreshProjects]);

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
    const res = await createScript(newName.trim(), newDesc.trim());
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

  const handleIssue = async () => {
    if (!selected || busy) return;
    setBusy(true);
    setError(null);
    const res = await issueLicense(selected, label.trim(), expiry ? new Date(expiry).toISOString() : null);
    setBusy(false);
    if (!res.ok || !res.data) {
      setError(ERROR_KEY[res.error ?? "unknown"]);
      return;
    }
    setIssuedKey({ key: res.data.licenseKey, label: res.data.license.label });
    setLicenses((prev) => [res.data!.license, ...prev]);
    setLabel("");
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
    setArtifact("");
  };

  const copyKey = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    } catch {
      setCopied(false);
    }
  };

  const activeLicenses = licenses.filter((l) => !l.revokedAt).length;

  return (
    <div className="grid items-start gap-5 xl:grid-cols-[320px_minmax(0,1fr)]">
      {/* ---------------- 项目列表 ---------------- */}
      <div className="grid content-start gap-4">
        <GlassCard className="p-5">
          <div className="mb-4 flex items-center gap-2.5">
            <Icon name="layers" className="h-4 w-4 text-aqua-300" />
            <h2 className="text-[13.5px] font-semibold text-slate-100">{t("scripts.projects")}</h2>
            <span className="ml-auto font-mono text-[11px] text-slate-500">{projects.length}</span>
          </div>

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
            <div className="glass-soft rounded-2xl border border-white/[0.07] px-4 py-6 text-center text-[12px] text-slate-500">
              {t("scripts.loading")}
            </div>
          )}
          {!loading && projects.length === 0 && (
            <div className="glass-soft rounded-2xl border border-dashed border-white/[0.09] px-4 py-8 text-center">
              <p className="text-[12.5px] text-slate-400">{t("scripts.empty")}</p>
              <p className="mt-1 text-[11px] text-slate-500">{t("scripts.emptyHint")}</p>
            </div>
          )}
          {projects.map((p) => {
            const active = p.scriptId === selected;
            return (
              <button
                key={p.scriptId}
                onClick={() => setSelected(p.scriptId)}
                className={
                  "group w-full cursor-pointer rounded-2xl border px-4 py-3 text-left transition-all duration-300 " +
                  (active
                    ? "border-aqua-300/30 bg-aqua-400/[0.09] text-aqua-50 shadow-[0_0_30px_-18px_rgba(34,226,213,0.9)_inset]"
                    : "border-white/[0.07] bg-white/[0.02] text-slate-300 hover:border-aqua-300/20 hover:bg-white/[0.04]")
                }
              >
                <span className="flex items-center gap-2">
                  <span className={"h-1.5 w-1.5 shrink-0 rounded-full " + (p.active ? "bg-emerald-300" : "bg-slate-600")} />
                  <span className="truncate text-[13px] font-medium">{p.name}</span>
                </span>
                <span className="mt-1 block truncate font-mono text-[10.5px] text-slate-500">{p.scriptId}</span>
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
                <Stat label={t("scripts.licenses")} value={String(licenses.length)} />
                <Stat label={t("scripts.active")} value={String(activeLicenses)} />
                <Stat label={t("scripts.releases")} value={String(releases.length)} />
              </div>
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
                  <div className="grid gap-2.5 sm:grid-cols-[1fr_180px_auto]">
                    <input
                      className={INPUT}
                      value={label}
                      onChange={(e) => setLabel(e.target.value)}
                      placeholder={t("scripts.labelPh")}
                      maxLength={80}
                    />
                    <input
                      className={INPUT}
                      type="date"
                      value={expiry}
                      onChange={(e) => setExpiry(e.target.value)}
                      title={t("scripts.expiry")}
                    />
                    <Button className="px-4 py-2.5" disabled={busy} onClick={handleIssue}>
                      {busy ? t("scripts.working") : t("scripts.issueAction")}
                    </Button>
                  </div>
                  <p className="mt-2 text-[10.5px] text-slate-500">{t("scripts.expiryHint")}</p>

                  {issuedKey && (
                    <div className="mt-3 rounded-2xl border border-amber-300/25 bg-amber-400/[0.07] p-3.5">
                      <p className="text-[11px] font-semibold text-amber-100">{t("scripts.keyOnce")}</p>
                      <div className="mt-2 flex items-center gap-2">
                        <code className="min-w-0 flex-1 truncate rounded-lg bg-black/40 px-2.5 py-2 font-mono text-[11.5px] text-amber-100">
                          {issuedKey.key}
                        </code>
                        <Button variant="outline" className="px-2.5 py-1.5" onClick={() => copyKey(issuedKey.key)}>
                          <Icon name={copied ? "check" : "copy"} className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                      <p className="mt-1.5 text-[10.5px] text-amber-100/70">{t("scripts.keyOnceHint")}</p>
                    </div>
                  )}
                </GlassCard>

                <GlassCard className="overflow-hidden p-0">
                  <div className="flex items-center gap-2.5 border-b border-white/[0.06] px-5 py-3.5">
                    <Icon name="shield" className="h-4 w-4 text-aqua-300" />
                    <h3 className="text-[13px] font-semibold text-slate-100">{t("scripts.licenses")}</h3>
                    <span className="ml-auto font-mono text-[11px] text-slate-500">{licenses.length}</span>
                  </div>
                  {licenses.length === 0 ? (
                    <p className="px-5 py-8 text-center text-[12px] text-slate-500">{t("scripts.noLicenses")}</p>
                  ) : (
                    <div className="overflow-x-auto">
                      <table className="w-full min-w-[620px] text-left text-[11.5px]">
                        <thead>
                          <tr className="text-[10px] tracking-[0.14em] text-slate-500 uppercase">
                            <th className="px-5 py-2.5 font-medium">{t("scripts.colLabel")}</th>
                            <th className="px-3 py-2.5 font-medium">{t("scripts.colState")}</th>
                            <th className="px-3 py-2.5 font-medium">{t("scripts.colDevice")}</th>
                            <th className="px-3 py-2.5 font-medium">{t("scripts.colLastUsed")}</th>
                            <th className="px-5 py-2.5 text-right font-medium">{t("scripts.colActions")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {licenses.map((l) => {
                            const expired = l.expiresAt !== null && l.expiresAt <= Date.now();
                            const state = l.revokedAt
                              ? { key: "scripts.stateRevoked" as TKey, tone: "text-rose-300" }
                              : expired
                                ? { key: "scripts.stateExpired" as TKey, tone: "text-amber-300" }
                                : { key: "scripts.stateActive" as TKey, tone: "text-emerald-300" };
                            return (
                              <tr key={l.licenseId} className="border-t border-white/[0.05]">
                                <td className="px-5 py-2.5">
                                  <span className="block truncate text-slate-200">{l.label || "—"}</span>
                                  <span className="block font-mono text-[10px] text-slate-500">{l.licenseId}</span>
                                </td>
                                <td className={"px-3 py-2.5 " + state.tone}>{t(state.key)}</td>
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
                </GlassCard>
              </>
            ) : (
              <>
                <GlassCard className="p-5">
                  <div className="mb-3 flex items-center gap-2.5">
                    <Icon name="upload" className="h-4 w-4 text-aqua-300" />
                    <h3 className="text-[13px] font-semibold text-slate-100">{t("scripts.publish")}</h3>
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
                      <p className="self-center text-[10.5px] leading-relaxed text-slate-500">{t("scripts.publishHint")}</p>
                    </div>
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
                  </div>
                </GlassCard>

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
  );
}
