import { appliedNames, PRESETS, type ObfuscationOptions, type PresetId, type VmMode, type DispatchStyle } from "../lib/engine";
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
  const setVm = <K extends keyof ObfuscationOptions["vm"]>(
    key: K,
    value: ObfuscationOptions["vm"][K],
  ) => setOptions({ ...options, vm: { ...options.vm, [key]: value } });

  const applyPreset = (p: PresetId) => {
    const { label: _l, hint: _h, vmMode, ...rest } = PRESETS[p];
    setOptions({
      ...options,
      ...rest,
      preset: p,
      vm: { ...options.vm, mode: vmMode ?? options.vm.mode },
    });
  };

  const enabled = appliedNames(options);

  return (
    <div className="flex flex-col gap-5">
      <div>
        <h2 className="text-[13.5px] font-semibold text-slate-100">{t("ws.config")}</h2>
        <p className="mt-1 text-[11px] leading-relaxed text-slate-400/70">{PRESETS[options.preset].hint}</p>
      </div>

      <Group title={t("ws.preset")} icon={<Icon name="bolt" className="h-4 w-4" />}>
        <div className="glass-soft flex items-center gap-1 rounded-xl border border-white/[0.08] p-1">
          {(
            [
              { value: "light", label: t("preset.light") },
              { value: "balanced", label: t("preset.balanced") },
              { value: "extreme", label: t("preset.extreme") },
            ] as { value: PresetId; label: string }[]
          ).map((p) => (
            <button
              key={p.value}
              onClick={() => applyPreset(p.value)}
              className={cn(
                "flex-1 cursor-pointer rounded-lg px-2 py-1.5 text-[12px] font-medium transition-all duration-300",
                options.preset === p.value
                  ? "bg-aqua-400/20 text-aqua-100 shadow-[0_0_24px_-10px_rgba(34,226,213,0.95)_inset]"
                  : "text-slate-400 hover:text-aqua-100",
              )}
            >
              {p.label}
            </button>
          ))}
        </div>
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

      <Group title={t("ws.groupVm")} icon={<Icon name="cpu" className="h-4 w-4" />}>
        <div className="grid grid-cols-3 gap-1.5 rounded-xl border border-aqua-300/20 bg-aqua-400/[0.05] p-1">
          {(
            [
              { value: "off", label: t("ws.vmOff") },
              { value: "function", label: t("ws.vmFunction") },
              { value: "script", label: t("ws.vmScript") },
            ] as { value: VmMode; label: string }[]
          ).map((m) => (
            <button
              key={m.value}
              onClick={() => setVm("mode", m.value)}
              className={cn(
                "cursor-pointer rounded-lg px-2 py-1.5 text-[11.5px] transition-all duration-300",
                options.vm.mode === m.value
                  ? "bg-aqua-400/20 text-aqua-50 shadow-[0_0_24px_-12px_rgba(34,226,213,1)_inset]"
                  : "text-slate-400 hover:text-aqua-100",
              )}
            >
              {m.label}
            </button>
          ))}
        </div>
        <p className="px-1 text-[11px] leading-relaxed text-slate-400/65">{t("ws.vmModeHint")}</p>

        {options.vm.mode !== "off" && (
          <>
            <label className="block px-1">
              <span className="mb-1.5 flex items-center justify-between text-[11px] text-slate-400/70">
                {t("ws.dispatch")}
                <span className="font-mono text-[10.5px] text-aqua-200/80">{options.vm.dispatchStyle}</span>
              </span>
              <div className="grid grid-cols-3 gap-1.5">
                {(
                  [
                    { value: "switch", label: t("ws.dispatchSwitch") },
                    { value: "table", label: t("ws.dispatchTable") },
                    { value: "goto", label: t("ws.dispatchGoto") },
                  ] as { value: DispatchStyle; label: string }[]
                ).map((d) => (
                  <button
                    key={d.value}
                    onClick={() => setVm("dispatchStyle", d.value)}
                    className={cn(
                      "cursor-pointer rounded-lg border px-2 py-1.5 text-[11px] transition-all duration-300",
                      options.vm.dispatchStyle === d.value
                        ? "border-aqua-300/40 bg-aqua-400/[0.12] text-aqua-100"
                        : "border-white/[0.07] bg-white/[0.02] text-slate-400 hover:border-aqua-300/25 hover:text-aqua-100",
                    )}
                  >
                    {d.label}
                  </button>
                ))}
              </div>
            </label>
            <Toggle
              label={t("ws.opcodeShuffle")}
              hint={t("ws.opcodeShuffleHint")}
              checked={options.vm.opcodeShuffle}
              onChange={(v) => setVm("opcodeShuffle", v)}
            />
            <Toggle
              label={t("ws.bytecodeEnc")}
              hint={t("ws.bytecodeEncHint")}
              checked={options.vm.bytecodeEncryption}
              onChange={(v) => setVm("bytecodeEncryption", v)}
            />
            <Toggle
              label={t("ws.registerMachine")}
              hint={t("ws.registerMachineHint")}
              checked={options.vm.registerMachine}
              onChange={(v) => setVm("registerMachine", v)}
            />
          </>
        )}
      </Group>

      <Group title={t("ws.groupTransform")} icon={<Icon name="lock" className="h-4 w-4" />}>
        <Toggle label={t("ws.rename")} hint={t("ws.renameHint")} checked={options.renameIdentifiers} onChange={(v) => set("renameIdentifiers", v)} />
        <Toggle label={t("ws.strings")} hint={t("ws.stringsHint")} checked={options.stringEncryption} onChange={(v) => set("stringEncryption", v)} />
        <Toggle label={t("ws.flow")} hint={t("ws.flowHint")} checked={options.controlFlowFlattening} onChange={(v) => set("controlFlowFlattening", v)} />
        <Toggle label={t("ws.pool")} hint={t("ws.poolHint")} checked={options.constantPool} onChange={(v) => set("constantPool", v)} />
        <Toggle label={t("ws.numbers")} hint={t("ws.numbersHint")} checked={options.numbersToExpressions} onChange={(v) => set("numbersToExpressions", v)} />
      </Group>

      <Group title={t("ws.groupRuntime")} icon={<Icon name="shield" className="h-4 w-4" />}>
        <Toggle label={t("ws.antiDebug")} hint={t("ws.antiDebugHint")} checked={options.antiDebug} onChange={(v) => set("antiDebug", v)} />
        <Toggle label={t("ws.antiTamper")} hint={t("ws.antiTamperHint")} checked={options.antiTamper} onChange={(v) => set("antiTamper", v)} />
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
        <label className="block">
          <span className="mb-1 block text-[11px] text-slate-400/70">
            {t("ws.watermark")} <span className="text-slate-500">({t("common.optional")})</span>
          </span>
          <input
            value={options.watermark}
            onChange={(e) => set("watermark", e.target.value)}
            placeholder={t("ws.watermarkPh")}
            className="w-full rounded-xl border border-white/[0.09] bg-black/30 px-3 py-2 font-mono text-[12px] text-aqua-100 placeholder:text-slate-600 focus:border-aqua-300/45"
          />
        </label>
      </Group>
    </div>
  );
}

export function presetLabel(preset: PresetId, t: (k: TKey) => string) {
  return preset === "light" ? t("preset.light") : preset === "balanced" ? t("preset.balanced") : t("preset.extreme");
}
