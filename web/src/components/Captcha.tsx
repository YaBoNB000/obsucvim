import { useCallback, useEffect, useRef } from "react";

/* 服务端人机验证的渲染端：显示算式题面（答案只在服务端），
 * 用户输入计算结果。畸变绘制防 OCR；点击换题。 */
const COLORS = ["#67e8f9", "#5eead4", "#a5f3fc", "#7dd3fc", "#86efac"];

export function Captcha({
  question,
  onRefresh,
  className,
}: {
  question: string | null;
  onRefresh: () => void;
  className?: string;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const draw = useCallback((text: string) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const w = canvas.width;
    const h = canvas.height;
    const bg = ctx.createLinearGradient(0, 0, w, h);
    bg.addColorStop(0, "#04121a");
    bg.addColorStop(1, "#06222c");
    ctx.fillStyle = bg;
    ctx.fillRect(0, 0, w, h);
    for (let i = 0; i < 5; i++) {
      ctx.strokeStyle = `rgba(103,232,249,${0.08 + Math.random() * 0.12})`;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(Math.random() * w, Math.random() * h);
      ctx.bezierCurveTo(
        Math.random() * w, Math.random() * h,
        Math.random() * w, Math.random() * h,
        Math.random() * w, Math.random() * h,
      );
      ctx.stroke();
    }
    const cell = w / (text.length + 1);
    for (let i = 0; i < text.length; i++) {
      const x = cell * (i + 1) - 4;
      const y = h / 2 + (Math.random() * 8 - 4);
      ctx.save();
      ctx.translate(x, y);
      ctx.rotate((Math.random() - 0.5) * 0.45);
      ctx.font = `bold ${18 + Math.floor(Math.random() * 4)}px ui-monospace, monospace`;
      ctx.fillStyle = COLORS[i % COLORS.length];
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(text[i], 0, 0);
      ctx.restore();
    }
    for (let i = 0; i < 40; i++) {
      ctx.fillStyle = `rgba(165,243,252,${0.05 + Math.random() * 0.16})`;
      ctx.fillRect(Math.random() * w, Math.random() * h, 1.6, 1.6);
    }
  }, []);

  useEffect(() => {
    draw(question ?? "···");
  }, [question, draw]);

  return (
    <canvas
      ref={canvasRef}
      width={132}
      height={44}
      onClick={onRefresh}
      className={`shrink-0 cursor-pointer rounded-xl border border-white/[0.12] bg-black/40 ${className ?? ""}`}
      title="captcha"
    />
  );
}
