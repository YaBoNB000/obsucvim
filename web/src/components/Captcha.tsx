import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { useI18n } from "../lib/i18n";
import type { CaptchaChallenge } from "../lib/store";

export function Captcha({
  challenge,
  onAnswerChange,
  disabled = false,
  className,
}: {
  challenge: CaptchaChallenge | null;
  onAnswerChange: (answer: string) => void;
  disabled?: boolean;
  className?: string;
}) {
  const { t } = useI18n();
  const boardRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ pointerId: number; startX: number; startOffset: number; scale: number } | null>(null);
  const offsetRef = useRef(0);
  const [offset, setOffset] = useState(0);
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    offsetRef.current = 0;
    setOffset(0);
    setDragging(false);
    dragRef.current = null;
    onAnswerChange("");
  }, [challenge?.captchaId, onAnswerChange]);

  const updateOffset = (value: number) => {
    const max = challenge?.maxOffset ?? 0;
    const next = Math.min(max, Math.max(0, Math.round(value)));
    offsetRef.current = next;
    setOffset(next);
    return next;
  };

  const beginDrag = (event: ReactPointerEvent<HTMLImageElement>) => {
    if (!challenge || disabled || !boardRef.current) return;
    event.preventDefault();
    const scale = boardRef.current.getBoundingClientRect().width / challenge.width;
    dragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startOffset: offsetRef.current,
      scale: scale || 1,
    };
    setDragging(true);
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const movePiece = (event: ReactPointerEvent<HTMLImageElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    event.preventDefault();
    updateOffset(drag.startOffset + (event.clientX - drag.startX) / drag.scale);
  };

  const endDrag = (event: ReactPointerEvent<HTMLImageElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    dragRef.current = null;
    setDragging(false);
    onAnswerChange(String(offsetRef.current));
  };

  if (!challenge) {
    return (
      <div className={`flex h-24 w-full max-w-[320px] items-center justify-center rounded-xl border border-white/10 bg-black/25 text-[11px] text-slate-500 ${className ?? ""}`}>
        {t("login.captchaLoading")}
      </div>
    );
  }

  return (
    <div className={`w-full max-w-[320px] ${className ?? ""}`}>
      <div
        ref={boardRef}
        className="relative overflow-hidden rounded-xl border border-white/[0.14] bg-[#04121a] shadow-[0_8px_24px_-16px_rgba(34,226,213,0.8)]"
        style={{ aspectRatio: `${challenge.width} / ${challenge.height}` }}
      >
        <img
          src={challenge.backgroundUrl}
          alt={t("login.captchaImageAlt")}
          draggable={false}
          className="pointer-events-none absolute inset-0 h-full w-full select-none"
        />
        <img
          src={challenge.pieceUrl}
          alt=""
          draggable={false}
          onDragStart={(event) => event.preventDefault()}
          onPointerDown={beginDrag}
          onPointerMove={movePiece}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
          className={`absolute z-10 select-none drop-shadow-[0_1px_5px_rgba(103,232,249,0.9)] ${disabled ? "cursor-not-allowed opacity-60" : dragging ? "cursor-grabbing" : "cursor-grab"} touch-none`}
          style={{
            left: `${(offset / challenge.width) * 100}%`,
            top: `${(challenge.pieceY / challenge.height) * 100}%`,
            width: `${(challenge.pieceSize / challenge.width) * 100}%`,
            height: `${(challenge.pieceSize / challenge.height) * 100}%`,
          }}
        />
      </div>

      <div className="mt-2 flex items-center gap-3 rounded-xl border border-white/[0.08] bg-black/25 px-3 py-2">
        <span className="shrink-0 text-[10px] text-slate-500">0</span>
        <input
          type="range"
          min={0}
          max={challenge.maxOffset}
          step={1}
          value={offset}
          disabled={disabled}
          aria-label={t("login.captchaSlider")}
          aria-valuetext={`${offset} / ${challenge.maxOffset}`}
          onChange={(event) => onAnswerChange(String(updateOffset(Number(event.target.value))))}
          className="h-2 w-full cursor-pointer touch-pan-y accent-[#67e8f9] disabled:cursor-not-allowed disabled:opacity-50"
        />
        <span className="shrink-0 text-[10px] text-slate-500">{challenge.maxOffset}</span>
      </div>
      <p className="mt-1.5 text-[10.5px] text-slate-500" aria-live="polite">
        {offset > 0 ? t("login.captchaMoved") : t("login.captchaPh")}
      </p>
    </div>
  );
}
