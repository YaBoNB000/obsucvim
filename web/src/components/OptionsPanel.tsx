import { appliedNames, PRESETS, PRESET_IDS, type ObfuscationOptions, type PresetId } from "../lib/engine";
import { Icon, Toggle } from "./ui";
import { useI18n, type TKey } from "../lib/i18n";
import { cn } from "../utils/cn";

function Group({ title, icon, children }: { title: string; icon: React.ReactNode; children: React.ReactNode }) {
  return (
    <div>
      <div className="mb-2.5 flex items-center gap-2">
        <span className="text-aqua-300">{icon}</span>
        <span className="text-[10.5px] font-semibold tracking-[0.2em] text-slate-300/80 uppercase">{title}</span>
        <span className="h-px flex-1 bg-gradient-to-r from-aqua-300/25 to-transparent" />
      </div>
      <div className="grid gap-2">{children}</div>
    </div>
  );
}

export function OptionsPanel({
  options,
  setOptions,
}: {
  options: ObfuscationOptions;
  setOptions: (o: ObfuscationOptions) => void;
}) {
  const { t } = useI18n();
  const set = <K extends keyof ObfuscationOptions>(key: K, value: ObfuscationOptions[K]) =>
    setOptions({ ...options, [key]: value });

  const applyPreset = (p: PresetId) => {
    const { label: _l, hint: _h, ...rest } = PRESETS[p];
    setOptions({ ...options, ...rest, preset: p });
  };

  const enabled = appliedNames(options);
  const isV15 = options.preset === "v15";

  return (
    <div className="flex flex-col gap-5">
      <div>
        <h2 className="text-[13.5px] font-semibold text-slate-100">{t("ws.config")}</h2>
        <p className="mt-1 text-[11px] leading-relaxed text-slate-400/70">{PRESETS[options.preset].hint}</p>
      </div>

      <Group title={t("ws.preset")} icon={<Icon name="bolt" className="h-4 w-4" />}>
        <div className="glass-soft grid grid-cols-3 gap-1 rounded-xl border border-white/[0.08] p-1">
          {PRESET_IDS.map((p) => (
            <button
              key={p}
              onClick={() => applyPreset(p)}
              className={cn(
                "relative cursor-pointer rounded-lg px-2 py-1.5 text-[12px] font-medium transition-all duration-300",
                options.preset === p
                  ? "bg-aqua-400/20 text-aqua-100 shadow-[0_0_24px_-10px_rgba(34,226,213,0.95)_inset]"
                  : "text-slate-400 hover:text-aqua-100",
              )}
            >
              {t(`preset.${p}` as TKey)}
              {p === "v15" && (
                <span className="ml-1 rounded border border-aqua-300/30 px-1 py-px font-mono text-[8.5px] tracking-wide text-aqua-200/80">
                  {t("preset.default")}
                </span>
              )}
            </button>
          ))}
        </div>
        <p className="px-0.5 text-[10.5px] leading-relaxed text-slate-500">{t("ws.presetLegacyHint")}</p>
        <div className="flex flex-wrap gap-1.5 px-0.5">
          {enabled.length === 0 ? (
            <span className="text-[11px] text-slate-500">{t("ws.appliedCount", { n: 0 })}</span>
          ) : (
            enabled.map((a) => (
              <span
                key={a}
                className="rounded-lg border border-aqua-300/20 bg-aqua-400/[0.07] px-2 py-0.5 font-mono text-[10px] text-aqua-100/90"
              >
                {a}
              </span>
            ))
          )}
        </div>
      </Group>

      <Group title={t("ws.groupV15")} icon={<Icon name="shield" className="h-4 w-4" />}>
        {!isV15 && <p className="px-1 text-[10.5px] leading-relaxed text-amber-200/70">{t("ws.v15Only")}</p>}
        <Toggle
          label={t("ws.bindExecutor")}
          hint={t("ws.bindExecutorHint")}
          checked={options.bindExecutor}
          locked={!isV15}
          onChange={(v) => set("bindExecutor", v)}
        />
        <Toggle
          label={t("ws.envCheck")}
          hint={t("ws.envCheckHint")}
          checked={options.envCheck}
          locked={!isV15}
          onChange={(v) => set("envCheck", v)}
        />
        <Toggle
          label={t("ws.bindEnv")}
          hint={t("ws.bindEnvHint")}
          checked={options.bindEnv}
          locked={!isV15}
          onChange={(v) => set("bindEnv", v)}
        />
      </Group>

      <Group title={t("ws.groupTransform")} icon={<Icon name="lock" className="h-4 w-4" />}>
        <Toggle label={t("ws.mangle")} hint={t("ws.mangleHint")} checked={options.mangle} onChange={(v) => set("mangle", v)} />
        <Toggle label={t("ws.strings")} hint={t("ws.stringsHint")} checked={options.strings} onChange={(v) => set("strings", v)} />
        <Toggle label={t("ws.flatten")} hint={t("ws.flattenHint")} checked={options.flatten} onChange={(v) => set("flatten", v)} />
        <Toggle label={t("ws.junk")} hint={t("ws.junkHint")} checked={options.junk} onChange={(v) => set("junk", v)} />
        <Toggle label={t("ws.numbers")} hint={t("ws.numbersHint")} checked={options.numbers} onChange={(v) => set("numbers", v)} />
        <Toggle label={t("ws.body")} hint={t("ws.bodyHint")} checked={options.body} onChange={(v) => set("body", v)} />
        <Toggle label={t("ws.minify")} hint={t("ws.minifyHint")} checked={options.minify} onChange={(v) => set("minify", v)} />
      </Group>

      <Group title={t("ws.groupRuntime")} icon={<Icon name="cpu" className="h-4 w-4" />}>
        <Toggle label={t("ws.antidbg")} hint={t("ws.antidbgHint")} checked={options.antidbg} onChange={(v) => set("antidbg", v)} />
        <Toggle label={t("ws.guard")} hint={t("ws.guardHint")} checked={options.guard} onChange={(v) => set("guard", v)} />
        <Toggle
          label={t("ws.vmSeal")}
          hint={t("ws.vmSealHint")}
          checked={options.vmSealStrings}
          locked={options.preset !== "vm" && options.preset !== "max"}
          onChange={(v) => set("vmSealStrings", v)}
        />
      </Group>

      <Group title={t("ws.groupMeta")} icon={<Icon name="layers" className="h-4 w-4" />}>
        <label className="block">
          <span className="mb-1 block text-[11px] text-slate-400/70">
            {t("ws.seed")} <span className="text-slate-500">({t("common.optional")})</span>
          </span>
          <input
            value={options.seed}
            onChange={(e) => set("seed", e.target.value)}
            placeholder={t("ws.seedPh")}
            className="w-full rounded-xl border border-white/[0.09] bg-black/30 px-3 py-2 font-mono text-[12px] text-aqua-100 placeholder:text-slate-600 focus:border-aqua-300/45"
          />
        </label>
        <div className="rounded-xl border border-white/[0.07] bg-white/[0.02] px-3 py-2.5">
          <div className="flex items-center gap-2 text-[11px] text-slate-400/80">
            <Icon name="lock" className="h-3.5 w-3.5 text-aqua-300/70" />
            {t("ws.watermarkNote")}
          </div>
          <p className="mt-1.5 font-mono text-[10.5px] leading-relaxed text-aqua-200/60">-- This file was obfuscated by Obsucvim.</p>
        </div>
      </Group>
    </div>
  );
}

export function presetLabel(preset: PresetId, t: (k: TKey) => string) {
  return t(`preset.${preset}` as TKey);
}
