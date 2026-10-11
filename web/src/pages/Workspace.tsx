import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  DEFAULT_OPTIONS,
  ENGINE,
  MAX_SOURCE_BYTES,
  SAMPLE_LUA,
  runObfuscation,
  type ObfuscationOptions,
  type ObfuscationResult,
} from "../lib/engine";
import { Button, GlassCard, Icon } from "../components/ui";
import { LuaEditor } from "../components/CodeArea";
import { OptionsPanel } from "../components/OptionsPanel";
import { ResultPanel } from "../components/ResultPanel";
import { useI18n } from "../lib/i18n";
import { loadDefaults, useAuth, useHistory } from "../lib/store";
import { cn } from "../utils/cn";

export function Workspace({ prefill }: { prefill?: { code: string; options: ObfuscationOptions } | null }) {
  const { t } = useI18n();
  const { outOfQuota, consumeQuota } = useAuth();
  const { push } = useHistory();

  const [code, setCode] = useState(SAMPLE_LUA);
  const [options, setOptions] = useState<ObfuscationOptions>(() => loadDefaults(DEFAULT_OPTIONS));
  const [result, setResult] = useState<ObfuscationResult | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const inputBytes = useMemo(() => new TextEncoder().encode(code).length, [code]);
  const tooLarge = inputBytes > MAX_SOURCE_BYTES;

  useEffect(() => {
    if (prefill) {
      setCode(prefill.code);
      setOptions(prefill.options);
      setResult(null);
      setError(null);
    }
  }, [prefill]);

  // 工作台是常驻挂载的（切页不卸载），这里负责回到顶部
  useEffect(() => {
    window.scrollTo({ top: 0, behavior: "smooth" });
  }, []);

  const run = useCallback(async () => {
    if (running) return;
    if (!code.trim()) {
      setError(t("ws.empty"));
      return;
    }
    if (tooLarge) {
      setError(t("ws.tooLarge"));
      return;
    }
    if (outOfQuota) {
      setError(t("ws.quotaEmpty"));
      return;
    }
    setRunning(true);
    setError(null);
    setResult(null);
    try {
      const res = await runObfuscation(code, options);
      setResult(res);
      consumeQuota();
      push({
        seed: res.stats.seeded,
        inputBytes: res.stats.inputBytes,
        outputBytes: res.stats.outputBytes,
        code: res.code,
        options,
      });
    } catch (e) {
      const msg = e instanceof Error ? e.message : "";
      if (msg.startsWith("SOURCE_TOO_LARGE")) setError(t("ws.tooLarge"));
      else if (msg === "ENGINE_TIMEOUT") setError(t("ws.timeout"));
      else setError(msg || t("ws.error"));
    } finally {
      setRunning(false);
    }
  }, [code, options, running, tooLarge, outOfQuota, consumeQuota, push, t]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        void run();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [run]);

  const readFile = async (file: File) => {
    const text = await file.text();
    setCode(text);
    setResult(null);
    setError(null);
  };

  return (
    <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1.62fr)_minmax(336px,1fr)] xl:gap-5">
      {/* ---------------- 源码 + 运行（桌面：左上） ---------------- */}
      <GlassCard padded={false} className="flex min-w-0 flex-col overflow-hidden xl:col-start-1 xl:row-start-1">
        <div className="flex flex-wrap items-center gap-3 border-b border-white/[0.07] px-4 py-3">
          <div className="flex items-center gap-1.5">
            <span className="h-2.5 w-2.5 rounded-full bg-rose-400/60" />
            <span className="h-2.5 w-2.5 rounded-full bg-amber-300/60" />
            <span className="h-2.5 w-2.5 rounded-full bg-emerald-300/60" />
          </div>
          <span className="font-mono text-[12px] text-slate-300/80">main.luau</span>
          <span
            className={cn(
              "tabular rounded-lg border px-2 py-0.5 text-[11px]",
              tooLarge
                ? "border-rose-400/40 bg-rose-500/[0.1] text-rose-200"
                : "border-white/[0.07] bg-black/25 text-slate-400/80",
            )}
          >
            {code.split("\n").length} lines · {(inputBytes / 1024).toFixed(1)} KB / 1024 KB
          </span>
          {tooLarge && <span className="text-[11px] text-rose-300">{t("ws.limit")}</span>}

          <div className="ml-auto flex items-center gap-1">
            <Button variant="ghost" className="px-2.5 py-1.5 text-[12px]" onClick={() => fileRef.current?.click()}>
              <Icon name="upload" className="h-4 w-4" />
              <span className="hidden sm:inline">{t("ws.import")}</span>
            </Button>
            <Button
              variant="ghost"
              className="px-2.5 py-1.5 text-[12px]"
              onClick={() => {
                setCode(SAMPLE_LUA);
                setResult(null);
                setError(null);
              }}
            >
              <Icon name="sparkle" className="h-4 w-4" />
              <span className="hidden sm:inline">{t("ws.sample")}</span>
            </Button>
            <Button
              variant="ghost"
              className="px-2.5 py-1.5 text-[12px]"
              onClick={() => {
                setCode("");
                setResult(null);
                setError(null);
              }}
            >
              <Icon name="trash" className="h-4 w-4" />
              <span className="hidden sm:inline">{t("common.clear")}</span>
            </Button>
            <input
              ref={fileRef}
              type="file"
              accept=".lua,.luau,.txt"
              className="hidden"
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) void readFile(f);
                e.target.value = "";
              }}
            />
          </div>
        </div>

        <div
          onDragOver={(e) => {
            e.preventDefault();
            setDragging(true);
          }}
          onDragLeave={() => setDragging(false)}
          onDrop={(e) => {
            e.preventDefault();
            setDragging(false);
            const f = e.dataTransfer.files?.[0];
            if (f) void readFile(f);
          }}
          className={cn("relative", dragging && "bg-aqua-400/[0.06]")}
        >
          {dragging && (
            <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center border-2 border-dashed border-aqua-300/60 bg-ink-1000/60 text-[12.5px] text-aqua-100">
              {t("ws.drop")}
            </div>
          )}
          <LuaEditor value={code} onChange={setCode} height={392} placeholder={t("ws.placeholder")} />
        </div>

        <div className="flex flex-wrap items-center gap-3 border-t border-white/[0.07] px-4 py-3">
          <Button onClick={run} disabled={running || !code.trim() || tooLarge || outOfQuota} className="min-w-[168px]">
            {running ? (
              <>
                <span className="h-3.5 w-3.5 animate-spin rounded-full border-2 border-ink-1000/40 border-t-ink-1000" />
                {t("ws.running")}
              </>
            ) : (
              <>
                <Icon name="play" className="h-4 w-4" />
                {t("ws.run")}
              </>
            )}
          </Button>

          <span className="hidden items-center gap-1.5 font-mono text-[11px] text-slate-500 sm:inline-flex">
            {ENGINE.name}
          </span>

          <div className="ml-auto flex items-center gap-2 text-[11px]">
            {outOfQuota ? (
              <span className="flex items-center gap-1.5 text-amber-200">
                <Icon name="bolt" className="h-3.5 w-3.5" />
                {t("quota.exhausted")}
              </span>
            ) : running ? (
              <span className="flex items-center gap-2 text-aqua-200">
                <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-aqua-300" />
                {t("ws.running")}
              </span>
            ) : result ? (
              <span className="flex items-center gap-1.5 text-emerald-300">
                <Icon name="check" className="h-3.5 w-3.5" />
                {t("ws.done")}
              </span>
            ) : (
              <span className="text-slate-500">{t("ws.idle")}</span>
            )}
          </div>
        </div>
      </GlassCard>

      {/* ---------------- 保护配置（桌面：右侧整列 / 移动端：源码下方） ---------------- */}
      <GlassCard className="min-w-0 p-5 xl:col-start-2 xl:row-span-2 xl:row-start-1">
        <OptionsPanel options={options} setOptions={setOptions} />
      </GlassCard>

      {/* ---------------- 提示条 + 结果（桌面：源码正下方） ---------------- */}
      <div className="grid min-w-0 content-start gap-4 xl:col-start-1 xl:row-start-2">
        {error && (
          <div
            className={cn(
              "flex items-start gap-3 rounded-2xl border px-4 py-3 text-[12.5px] backdrop-blur-xl",
              error === t("ws.quotaEmpty")
                ? "border-amber-300/30 bg-amber-400/[0.07] text-amber-100"
                : "border-rose-400/30 bg-rose-500/[0.08] text-rose-200",
            )}
          >
            <Icon name="shield" className="mt-0.5 h-4 w-4 shrink-0" />
            <div>
              <p className="font-medium">{error === t("ws.quotaEmpty") ? t("ws.quotaEmpty") : t("ws.error")}</p>
              <p className="mt-0.5 opacity-85">{error === t("ws.quotaEmpty") ? t("ws.quotaEmptyDesc") : error}</p>
            </div>
          </div>
        )}
        <ResultPanel
          result={result}
          options={options}
          running={running}
          hasInput={Boolean(code.trim())}
        />
      </div>
    </div>
  );
}
