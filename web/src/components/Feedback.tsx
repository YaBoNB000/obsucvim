import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { cn } from "../utils/cn";
import { Icon } from "./ui";

/* ------------------------------------------------------------------ *
 * Toast：右下角滑入，自动消失。只用来报告“刚刚发生了什么”，
 * 不做常驻错误提示（常驻错误仍放在页面里）。
 * ------------------------------------------------------------------ */

type ToastKind = "success" | "error" | "info";

interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  detail?: string;
  leaving?: boolean;
}

interface ToastApi {
  push: (toast: { kind?: ToastKind; title: string; detail?: string }) => void;
}

const ToastContext = createContext<ToastApi | null>(null);

const TOAST_ICON: Record<ToastKind, Parameters<typeof Icon>[0]["name"]> = {
  success: "check",
  error: "sparkle",
  info: "shield",
};

const TOAST_STYLE: Record<ToastKind, string> = {
  success: "border-emerald-300/30 bg-emerald-400/[0.09] text-emerald-100",
  error: "border-rose-400/30 bg-rose-500/[0.10] text-rose-100",
  info: "border-aqua-300/30 bg-aqua-400/[0.08] text-aqua-100",
};

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const seq = useRef(0);
  const timers = useRef<number[]>([]);

  useEffect(() => () => timers.current.forEach((t) => window.clearTimeout(t)), []);

  const push = useCallback((toast: { kind?: ToastKind; title: string; detail?: string }) => {
    const id = ++seq.current;
    setToasts((prev) => [...prev.slice(-3), { id, kind: toast.kind ?? "info", title: toast.title, detail: toast.detail }]);
    const hide = window.setTimeout(() => {
      setToasts((prev) => prev.map((t) => (t.id === id ? { ...t, leaving: true } : t)));
      const drop = window.setTimeout(() => {
        setToasts((prev) => prev.filter((t) => t.id !== id));
      }, 200);
      timers.current.push(drop);
    }, 3600);
    timers.current.push(hide);
  }, []);

  const api = useMemo(() => ({ push }), [push]);

  return (
    <ToastContext.Provider value={api}>
      {children}
      <div className="pointer-events-none fixed inset-x-4 bottom-5 z-[60] flex flex-col items-end gap-2 sm:inset-x-auto sm:right-6">
        {toasts.map((toast) => (
          <div
            key={toast.id}
            role="status"
            aria-live="polite"
            className={cn(
              "pointer-events-auto flex w-full max-w-[340px] items-start gap-2.5 rounded-2xl border px-3.5 py-3 text-[12px] shadow-[0_24px_60px_-30px_rgba(0,0,0,0.9)] backdrop-blur-md",
              TOAST_STYLE[toast.kind],
              toast.leaving ? "anim-toast-out" : "anim-toast",
            )}
          >
            <Icon name={TOAST_ICON[toast.kind]} className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            <div className="min-w-0">
              <p className="font-medium leading-snug">{toast.title}</p>
              {toast.detail && <p className="mt-0.5 text-[11px] leading-relaxed opacity-70">{toast.detail}</p>}
            </div>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast(): ToastApi {
  const ctx = useContext(ToastContext);
  // 没有 Provider 时退化成空实现，组件仍可用（测试/独立渲染场景）。
  return ctx ?? { push: () => {} };
}

/* ------------------------------------------------------------------ *
 * 骨架屏
 * ------------------------------------------------------------------ */

export function Skeleton({ className, style }: { className?: string; style?: React.CSSProperties }) {
  return <div aria-hidden style={style} className={cn("anim-skeleton rounded-lg", className)} />;
}

export function SkeletonRows({ rows = 4, className }: { rows?: number; className?: string }) {
  return (
    <div className={cn("grid gap-2", className)}>
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="flex items-center gap-3 px-2 py-2">
          <Skeleton className="h-3" style={{ width: `${56 + ((i * 13) % 40)}px` }} />
          <Skeleton className="h-3 flex-1" />
          <Skeleton className="h-3 w-14" />
          <Skeleton className="h-6 w-14 rounded-lg" />
        </div>
      ))}
    </div>
  );
}

/* ------------------------------------------------------------------ *
 * 数字滚动
 * ------------------------------------------------------------------ */

export function AnimatedNumber({
  value,
  duration = 650,
  format = (n) => String(Math.round(n)),
}: {
  value: number;
  duration?: number;
  format?: (value: number) => string;
}) {
  const [shown, setShown] = useState(value);
  const from = useRef(value);
  const raf = useRef<number | null>(null);

  useEffect(() => {
    const reduce = typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    if (reduce || !Number.isFinite(value)) {
      setShown(value);
      from.current = value;
      return;
    }
    const start = performance.now();
    const origin = from.current;
    const delta = value - origin;
    const tick = (now: number) => {
      const t = Math.min(1, (now - start) / duration);
      const eased = 1 - Math.pow(1 - t, 3);
      setShown(origin + delta * eased);
      if (t < 1) raf.current = requestAnimationFrame(tick);
      else from.current = value;
    };
    raf.current = requestAnimationFrame(tick);
    return () => {
      if (raf.current !== null) cancelAnimationFrame(raf.current);
      from.current = value;
    };
  }, [value, duration]);

  return <span className="tabular-nums">{format(shown)}</span>;
}

/* ------------------------------------------------------------------ *
 * 发布进度步进器
 * ------------------------------------------------------------------ */

export interface ProgressStep {
  key: string;
  label: string;
}

export function StepProgress({
  steps,
  activeKey,
  progress,
  failed = false,
  className,
}: {
  steps: ProgressStep[];
  activeKey: string;
  progress: number;
  failed?: boolean;
  className?: string;
}) {
  const activeIndex = Math.max(0, steps.findIndex((step) => step.key === activeKey));
  return (
    <div className={cn("grid gap-2.5", className)}>
      <div className="flex items-center justify-between text-[10.5px]">
        <span className={failed ? "text-rose-200" : "text-aqua-100"}>
          {steps[activeIndex]?.label ?? ""}
        </span>
        <span className="font-mono text-slate-500">{Math.round(progress)}%</span>
      </div>
      <div className="relative h-1.5 overflow-hidden rounded-full bg-white/[0.06]">
        <div
          className={cn(
            "anim-bar absolute inset-y-0 left-0 rounded-full",
            failed ? "bg-rose-400/70" : "bg-gradient-to-r from-aqua-400 to-emerald-300",
          )}
          style={{ width: `${Math.max(3, Math.min(100, progress))}%` }}
        />
      </div>
      <ol className="flex flex-wrap items-center gap-x-3 gap-y-1.5">
        {steps.map((step, index) => {
          const done = !failed && index < activeIndex;
          const active = index === activeIndex;
          return (
            <li key={step.key} className="flex items-center gap-1.5 text-[10.5px]">
              <span
                className={cn(
                  "flex h-4 w-4 items-center justify-center rounded-full border text-[8px] transition-colors",
                  done && "border-emerald-300/40 bg-emerald-400/15 text-emerald-200",
                  active && !failed && "anim-step-pulse border-aqua-300/50 bg-aqua-400/20 text-aqua-100",
                  active && failed && "border-rose-400/50 bg-rose-500/15 text-rose-200",
                  !done && !active && "border-white/[0.09] bg-white/[0.02] text-slate-600",
                )}
              >
                {done ? <Icon name="check" className="h-2.5 w-2.5 anim-step-done" /> : index + 1}
              </span>
              <span className={cn(done ? "text-emerald-200/80" : active ? "text-slate-200" : "text-slate-500")}>
                {step.label}
              </span>
            </li>
          );
        })}
      </ol>
    </div>
  );
}
