import { appliedNames, type ObfuscationOptions } from "../lib/engine";
import { Icon, Toggle } from "./ui";
import { useI18n } from "../lib/i18n";

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

  const enabled = appliedNames(options);

  return (
    <div className="flex flex-col gap-5">
      <div>
        <h2 className="text-[13.5px] font-semibold text-slate-100">{t("ws.config")}</h2>
      </div>

      <Group title={t("ws.preset")} icon={<Icon name="bolt" className="h-4 w-4" />}>
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

      <Group title={t("ws.groupTransform")} icon={<Icon name="lock" className="h-4 w-4" />}>
        <Toggle label={t("ws.mangle")} checked={options.mangle} onChange={(v) => set("mangle", v)} />
        <Toggle label={t("ws.strings")} checked={options.strings} onChange={(v) => set("strings", v)} />
        <Toggle label={t("ws.flatten")} checked={options.flatten} onChange={(v) => set("flatten", v)} />
        <Toggle label={t("ws.junk")} checked={options.junk} onChange={(v) => set("junk", v)} />
        <Toggle label={t("ws.numbers")} checked={options.numbers} onChange={(v) => set("numbers", v)} />
        <Toggle label={t("ws.body")} checked={options.body} onChange={(v) => set("body", v)} />
        <Toggle label={t("ws.minify")} checked={options.minify} onChange={(v) => set("minify", v)} />
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
      </Group>
    </div>
  );
}

