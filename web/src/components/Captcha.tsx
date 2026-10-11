import { useCallback, useEffect, useImperativeHandle, useRef, useState, forwardRef } from "react";

/* 内置人机验证（无第三方外联）：canvas 畸变字符 + 噪声。
 * 通过 forwardRef 暴露 check(input) / refresh()，由登录/注册表单在提交时校验。 */
export interface CaptchaHandle {
  check: (input: string) => boolean;
  refresh: () => void;
}

const CHARS = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // 排除 0O1I

function makeCode(): string {
  let out = "";
  for (let i = 0; i < 4; i++) out += CHARS[Math.floor(Math.random() * CHARS.length)];
  return out;
}

const COLORS = ["#67e8f9", "#5eead4", "#a5f3fc", "#7dd3fc", "#86efac"];

export const Captcha = forwardRef<CaptchaHandle, { className?: string }>(function Captcha({ className }, ref) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [code, setCode] = useState<string>(makeCode);

  const draw = useCallback((text: string) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const w = canvas.width;
    const h = canvas.height;
    // 背景
    const bg = ctx.createLinearGradient(0, 0, w, h);
    bg.addColorStop(0, "#04121a");
    bg.addColorStop(1, "#06222c");
    ctx.fillStyle = bg;
    ctx.fillRect(0, 0, w, h);
    // 噪声线
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
    // 字符（旋转 + 抖动 + 干扰色块）
    const cell = w / (text.length + 1);
    for (let i = 0; i < text.length; i++) {
      const ch = text[i];
      const x = cell * (i + 1) - 6;
      const y = h / 2 + (Math.random() * 10 - 5);
      ctx.save();
      ctx.translate(x, y);
      ctx.rotate((Math.random() - 0.5) * 0.7);
      ctx.font = `bold ${20 + Math.floor(Math.random() * 5)}px ui-monospace, monospace`;
      ctx.fillStyle = COLORS[i % COLORS.length];
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(ch, 0, 0);
      ctx.restore();
    }
    // 噪点
    for (let i = 0; i < 46; i++) {
      ctx.fillStyle = `rgba(165,243,252,${0.05 + Math.random() * 0.18})`;
      ctx.fillRect(Math.random() * w, Math.random() * h, 1.6, 1.6);
    }
  }, []);

  useEffect(() => {
    draw(code);
  }, [code, draw]);

  useImperativeHandle(ref, () => ({
    check: (input: string) => input.trim().toUpperCase() === code,
    refresh: () => setCode(makeCode()),
  }));

  return (
    <canvas
      ref={canvasRef}
      width={128}
      height={44}
      onClick={() => setCode(makeCode())}
      className={`shrink-0 cursor-pointer rounded-xl border border-white/[0.12] bg-black/40 ${className ?? ""}`}
      title="captcha"
    />
  );
});
