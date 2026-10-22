import { useEffect, type ReactNode } from "react";
import { cn } from "../utils/cn";

/* ------------------------------ 玻璃卡片 ------------------------------ */
export function GlassCard({
  children,
  className,
  glow = false,
  padded = true,
  style,
}: {
  children: ReactNode;
  className?: string;
  glow?: boolean;
  padded?: boolean;
  style?: React.CSSProperties;
}) {
  return (
    <div
      style={style}
      className={cn(
        "glass specular relative overflow-hidden rounded-3xl",
        padded && "p-6",
        glow && "glow-aqua",
        className,
      )}
    >
      {children}
    </div>
  );
}

/* ------------------------------ 按钮 ------------------------------ */
export function Button({
  children,
  variant = "primary",
  className,
  ...rest
}: React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "ghost" | "outline";
}) {
  return (
    <button
      {...rest}
      className={cn(
        "relative inline-flex cursor-pointer items-center justify-center gap-2 overflow-hidden rounded-xl px-4 py-2.5 text-sm font-medium transition-all duration-300 disabled:cursor-not-allowed disabled:opacity-45",
        variant === "primary" &&
          "bg-gradient-to-b from-aqua-300 to-aqua-500 text-ink-1000 shadow-[0_10px_36px_-12px_rgba(34,226,213,0.75)] hover:from-aqua-200 hover:to-aqua-400 hover:shadow-[0_14px_44px_-10px_rgba(34,226,213,0.85)] active:scale-[0.985]",
        variant === "outline" &&
          "glass-soft border border-aqua-300/25 text-aqua-100 hover:border-aqua-300/50 hover:bg-white/[0.06]",
        variant === "ghost" && "text-slate-300/80 hover:bg-white/[0.06] hover:text-aqua-100",
        className,
      )}
    >
      {children}
    </button>
  );
}

/* ------------------------------ 开关 ------------------------------ */
export function Toggle({
  checked,
  onChange,
  label,
  hint,
  locked,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  hint?: string;
  locked?: boolean;
}) {
  return (
    <button
      type="button"
      disabled={locked}
      onClick={() => onChange(!checked)}
      className={cn(
        "group flex w-full cursor-pointer items-center justify-between gap-3 rounded-xl border px-3 py-2.5 text-left transition-all duration-300",
        checked
          ? "border-aqua-300/35 bg-aqua-400/[0.09] shadow-[0_0_28px_-14px_rgba(34,226,213,0.9)_inset]"
          : "border-white/[0.07] bg-white/[0.015] hover:border-aqua-300/20 hover:bg-white/[0.04]",
        locked && "cursor-not-allowed opacity-60",
      )}
    >
      <span className="min-w-0">
        <span
          className={cn(
            "block truncate text-[13px] font-medium transition-colors",
            checked ? "text-aqua-100" : "text-slate-300/85",
          )}
        >
          {label}
        </span>
        {hint && <span className="mt-0.5 block truncate text-[11px] text-slate-400/60">{hint}</span>}
      </span>
      <span
        className={cn(
          "relative h-[22px] w-[40px] shrink-0 rounded-full border transition-all duration-300",
          checked
            ? "border-aqua-300/50 bg-aqua-400/30"
            : "border-white/10 bg-white/[0.05]",
        )}
      >
        <span
          className={cn(
            "absolute top-[2px] h-[16px] w-[16px] rounded-full transition-all duration-300",
            checked
              ? "left-[21px] bg-aqua-200 shadow-[0_0_14px_2px_rgba(34,226,213,0.8)]"
              : "left-[2px] bg-slate-500/70",
          )}
        />
      </span>
    </button>
  );
}

/* ------------------------------ 分段控制器 ------------------------------ */
export function Segmented<T extends string>({
  value,
  onChange,
  items,
  className,
}: {
  value: T;
  onChange: (v: T) => void;
  items: { value: T; label: string }[];
  className?: string;
}) {
  return (
    <div
      className={cn(
        "glass-soft inline-flex items-center gap-1 rounded-xl border border-white/[0.08] p-1",
        className,
      )}
    >
      {items.map((it) => (
        <button
          key={it.value}
          type="button"
          onClick={() => onChange(it.value)}
          className={cn(
            "relative cursor-pointer rounded-lg px-3 py-1.5 text-xs font-medium transition-all duration-300",
            value === it.value
              ? "bg-aqua-400/20 text-aqua-100 shadow-[0_0_24px_-10px_rgba(34,226,213,0.9)_inset]"
              : "text-slate-400 hover:text-aqua-100",
          )}
        >
          {it.label}
        </button>
      ))}
    </div>
  );
}

/* ------------------------------ 标签 ------------------------------ */
export function Pill({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full border border-aqua-300/20 bg-aqua-400/[0.07] px-3 py-1 text-[11px] font-medium tracking-wide text-aqua-100/90",
        className,
      )}
    >
      {children}
    </span>
  );
}

/* ------------------------------ 统计块 ------------------------------ */
export function Stat({ label, value, unit }: { label: string; value: ReactNode; unit?: string }) {
  return (
    <div className="glass-soft rounded-2xl border border-white/[0.07] px-4 py-3">
      <div className="text-[10px] tracking-[0.18em] text-slate-400/70 uppercase">{label}</div>
      <div className="tabular mt-1 font-mono text-lg text-aqua-100">
        {value}
        {unit && <span className="ml-1 text-[11px] text-slate-400/70">{unit}</span>}
      </div>
    </div>
  );
}

/* ------------------------------ 区块标题 ------------------------------ */
export function SectionHeading({
  eyebrow,
  title,
  desc,
  center,
}: {
  eyebrow: string;
  title: ReactNode;
  desc?: string;
  center?: boolean;
}) {
  return (
    <div className={cn("max-w-2xl", center && "mx-auto text-center")}>
      <div className="mb-3 flex items-center gap-3" style={center ? { justifyContent: "center" } : undefined}>
        <span className="h-px w-8 bg-gradient-to-r from-transparent to-aqua-300/70" />
        <span className="text-[11px] font-semibold tracking-[0.32em] text-aqua-300/80 uppercase">
          {eyebrow}
        </span>
        <span className="h-px w-8 bg-gradient-to-l from-transparent to-aqua-300/70" />
      </div>
      <h2 className="text-3xl font-semibold tracking-tight text-slate-100 sm:text-[34px]">{title}</h2>
      {desc && <p className="mt-3 text-sm leading-relaxed text-slate-400/85">{desc}</p>}
    </div>
  );
}

/* ------------------------------ 图标 ------------------------------ */
export function Icon({ name, className }: { name: IconName; className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={cn("h-5 w-5", className)}
      aria-hidden
    >
      {ICONS[name]}
    </svg>
  );
}

export type IconName =
  | "shield"
  | "lock"
  | "flow"
  | "cpu"
  | "bolt"
  | "eye"
  | "copy"
  | "check"
  | "download"
  | "play"
  | "sparkle"
  | "upload"
  | "trash"
  | "clock"
  | "code"
  | "terminal"
  | "chevron"
  | "github"
  | "layers";

const ICONS: Record<IconName, ReactNode> = {
  shield: <path d="M12 3l7 3v6c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V6l7-3z" />,
  lock: (
    <>
      <rect x="4" y="10" width="16" height="10" rx="2.5" />
      <path d="M8 10V7.5a4 4 0 018 0V10" />
      <path d="M12 14v2" />
    </>
  ),
  flow: (
    <>
      <path d="M4 6h6.5l3 6H20" />
      <path d="M4 18h6.5l3-6" />
      <circle cx="20" cy="12" r="1.6" />
    </>
  ),
  cpu: (
    <>
      <rect x="6" y="6" width="12" height="12" rx="2.5" />
      <path d="M10 3v3M14 3v3M10 18v3M14 18v3M3 10h3M3 14h3M18 10h3M18 14h3" />
    </>
  ),
  bolt: <path d="M13 3L5 14h6l-1 7 8-11h-6l1-7z" />,
  eye: (
    <>
      <path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" />
      <circle cx="12" cy="12" r="3" />
    </>
  ),
  copy: (
    <>
      <rect x="9" y="9" width="11" height="11" rx="2" />
      <path d="M15 5.5A2.5 2.5 0 0012.5 3H6a3 3 0 00-3 3v6.5A2.5 2.5 0 005.5 15" />
    </>
  ),
  check: <path d="M4.5 12.5l5 5 10-11" />,
  download: (
    <>
      <path d="M12 3v12" />
      <path d="M7.5 10.5L12 15l4.5-4.5" />
      <path d="M4 20h16" />
    </>
  ),
  play: <path d="M7 4.5l12 7.5-12 7.5z" />,
  sparkle: (
    <>
      <path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8L12 3z" />
      <path d="M18.5 16.5l.7 2 2 .7-2 .7-.7 2-.7-2-2-.7 2-.7.7-2z" />
    </>
  ),
  upload: (
    <>
      <path d="M12 20V8" />
      <path d="M7.5 12.5L12 8l4.5 4.5" />
      <path d="M4 4h16" />
    </>
  ),
  trash: (
    <>
      <path d="M4 7h16" />
      <path d="M9 7V5.5A1.5 1.5 0 0110.5 4h3A1.5 1.5 0 0115 5.5V7" />
      <path d="M6.5 7l1 12.5A2 2 0 009.5 21.5h5A2 2 0 0016.5 19.5L17.5 7" />
    </>
  ),
  clock: (
    <>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7.5V12l3 2" />
    </>
  ),
  code: (
    <>
      <path d="M9 8l-4 4 4 4" />
      <path d="M15 8l4 4-4 4" />
      <path d="M13 5l-2 14" />
    </>
  ),
  terminal: (
    <>
      <rect x="3" y="4" width="18" height="16" rx="2.5" />
      <path d="M7 10l2.5 2L7 14" />
      <path d="M12.5 14.5H17" />
    </>
  ),
  chevron: <path d="M6 9.5l6 6 6-6" />,
  github: (
    <path d="M12 2.5a9.5 9.5 0 00-3 18.5c-.1-.6-.2-1.6-.1-2.3-2.4.5-3-1.1-3-1.1-.5-1.2-1.2-1.5-1.2-1.5-1-.7 0-.7 0-.7 1 0 1.6 1.1 1.6 1.1.9 1.6 2.4 1.2 3 .9.1-.7.4-1.2.7-1.5-2-.2-4-1-4-4.3 0-1 .3-1.7.9-2.3-.1-.2-.4-1.1.1-2.3 0 0 .8-.3 2.6 1a8.8 8.8 0 014.7 0c1.8-1.3 2.6-1 2.6-1 .5 1.2.2 2.1.1 2.3.6.6.9 1.4.9 2.3 0 3.3-2 4.1-4 4.3.4.4.8 1.1.8 2.2 0 1.6-.1 2.6-.1 3a9.5 9.5 0 00-3-18.5z" />
  ),
  layers: (
    <>
      <path d="M12 3.5l8 4.3-8 4.3-8-4.3 8-4.3z" />
      <path d="M4 12.5l8 4.3 8-4.3" />
      <path d="M4 16.8l8 4.2 8-4.2" />
    </>
  ),
};

/* ------------------------------- 弹窗 ------------------------------- */
export function Modal({
  open,
  onClose,
  title,
  children,
  wide,
}: {
  open: boolean;
  onClose: () => void;
  title?: string;
  children: ReactNode;
  wide?: boolean;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6" role="dialog" aria-modal="true">
      <div className="absolute inset-0 bg-black/70 backdrop-blur-sm" onClick={onClose} />
      <GlassCard
        padded={false}
        className={`relative z-10 flex max-h-[86vh] w-full flex-col overflow-hidden ${wide ? "max-w-4xl" : "max-w-lg"}`}
      >
        <div className="flex items-center gap-3 border-b border-white/[0.07] px-5 py-3.5">
          <span className="h-1.5 w-1.5 rounded-full bg-aqua-300 shadow-[0_0_10px_rgba(34,226,213,0.9)]" />
          <h3 className="min-w-0 flex-1 truncate text-[13.5px] font-semibold text-slate-100">{title}</h3>
          <button
            type="button"
            onClick={onClose}
            className="cursor-pointer rounded-lg border border-white/[0.08] bg-white/[0.03] px-2 py-1 text-[11px] text-slate-400 transition hover:border-rose-300/30 hover:text-rose-200"
          >
            ✕
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-auto">{children}</div>
      </GlassCard>
    </div>
  );
}
