import { useState } from "react";
import { cn } from "../utils/cn";
import { useI18n } from "../lib/i18n";

/* ============================ 背景（青黑 + 液态玻璃） ============================ */
export function Background() {
  return (
    <div className="pointer-events-none fixed inset-0 -z-10 overflow-hidden">
      <div className="absolute inset-0 bg-[radial-gradient(120%_90%_at_50%_-10%,#08222c_0%,#041016_40%,#01060b_80%)]" />
      <div className="animate-drift absolute top-[-20%] left-[6%] h-[42rem] w-[42rem] rounded-full bg-[radial-gradient(circle,rgba(34,226,213,0.26),rgba(34,226,213,0)_62%)] blur-3xl" />
      <div className="animate-drift-slow absolute right-[-14%] bottom-[-18%] h-[38rem] w-[38rem] rounded-full bg-[radial-gradient(circle,rgba(6,189,180,0.22),rgba(6,189,180,0)_66%)] blur-3xl" />
      <div className="grid-lines absolute inset-0 opacity-70" />
      <div className="noise absolute inset-0 opacity-[0.04] mix-blend-overlay" />
    </div>
  );
}

/* ============================ 黑洞标记（SVG 兜底） ============================ */
function BlackHoleSVG({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 64 64" className={className} aria-hidden>
      <defs>
        <radialGradient id="disk" cx="50%" cy="50%" r="50%">
          <stop offset="55%" stopColor="#22e2d5" stopOpacity="0.05" />
          <stop offset="74%" stopColor="#5ff2e6" stopOpacity="0.85" />
          <stop offset="86%" stopColor="#c9fff8" stopOpacity="0.95" />
          <stop offset="100%" stopColor="#06bdb4" stopOpacity="0.15" />
        </radialGradient>
        <radialGradient id="horizon" cx="50%" cy="50%" r="50%">
          <stop offset="70%" stopColor="#000308" />
          <stop offset="100%" stopColor="#01060b" />
        </radialGradient>
      </defs>
      <circle cx="32" cy="32" r="30" fill="url(#disk)" />
      <ellipse cx="32" cy="32" rx="14" ry="4.6" fill="none" stroke="#93fbf1" strokeOpacity="0.7" strokeWidth="0.8" transform="rotate(-18 32 32)" />
      <circle cx="32" cy="32" r="11" fill="url(#horizon)" />
      <circle cx="32" cy="32" r="11.6" fill="none" stroke="#5ff2e6" strokeOpacity="0.85" strokeWidth="0.9" />
    </svg>
  );
}

/* ============================ 品牌 ============================ */
export function Logo({
  compact = false,
  className,
}: {
  compact?: boolean;
  className?: string;
}) {
  const { t } = useI18n();
  const [imgFailed, setImgFailed] = useState(false);

  return (
    <div className={cn("flex items-center gap-3", className)}>
      <span className="relative flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-2xl border border-aqua-300/25 bg-ink-1000 shadow-[0_0_30px_-10px_rgba(34,226,213,0.95)]">
        {imgFailed ? (
          <BlackHoleSVG className="h-9 w-9" />
        ) : (
          <img
            src="/blackhole.png"
            alt=""
            onError={() => setImgFailed(true)}
            className="h-full w-full scale-[1.06] object-cover"
          />
        )}
        <span className="pointer-events-none absolute inset-0 rounded-2xl border border-white/10" />
      </span>
      {!compact && (
        <span className="min-w-0 leading-none">
          <span className="block truncate text-[15px] font-semibold tracking-[0.14em] text-slate-100">
            OBS<span className="text-aqua-300">U</span>CVIM
          </span>
          <span className="mt-1 block truncate text-[9.5px] tracking-[0.24em] text-slate-500 uppercase">
            {t("brandSub")}
          </span>
        </span>
      )}
    </div>
  );
}

export { BlackHoleSVG };

/* ============================ 语言切换 ============================ */
export function LangSwitch({ size = "sm" }: { size?: "sm" | "md" }) {
  const { lang, setLang } = useI18n();
  const items: ("zh" | "en")[] = ["zh", "en"];
  return (
    <div className="glass-soft inline-flex items-center gap-0.5 rounded-xl border border-white/[0.08] p-0.5">
      {items.map((l) => (
        <button
          key={l}
          onClick={() => setLang(l)}
          className={cn(
            "cursor-pointer rounded-lg px-2.5 font-medium transition-all duration-300",
            size === "sm" ? "py-1 text-[11px]" : "py-1.5 text-[12px]",
            lang === l ? "bg-aqua-400/20 text-aqua-100" : "text-slate-400 hover:text-aqua-100",
          )}
        >
          {l === "zh" ? "中文" : "EN"}
        </button>
      ))}
    </div>
  );
}
