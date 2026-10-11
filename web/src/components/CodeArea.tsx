import { useEffect, useMemo, useRef } from "react";
import SimpleEditor from "react-simple-code-editor";
import { highlightLua, highlight, type CodeLang } from "../lib/prism";
import { cn } from "../utils/cn";

const LINE_HEIGHT = 22;
const PAD = 16;

const BASE_STYLE: React.CSSProperties = {
  fontFamily: "var(--font-mono)",
  fontSize: 13,
  lineHeight: `${LINE_HEIGHT}px`,
  outline: "none",
  // 不软换行 + 保留空格：长行横向滚动，行号才能与代码严格对齐
  whiteSpace: "pre",
  wordBreak: "normal",
  overflowWrap: "normal",
  tabSize: 2,
  // 文本域与高亮层必须拿到同一个「像素」高度，
  // 否则长代码时 textarea（height:100% 在 auto 父级下会塌成默认行数）
  // 覆盖不到下半部分，点击下方无法定位光标。
  minHeight: "var(--editor-h, 100%)",
};

/* ----------------------------- 可编辑区 ----------------------------- */
export function LuaEditor({
  value,
  onChange,
  height = 420,
  placeholder,
}: {
  value: string;
  onChange: (v: string) => void;
  height?: number;
  placeholder?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const gutter = useRef<HTMLDivElement>(null);
  const lineCount = useMemo(() => value.split("\n").length, [value]);
  // 可视高度 + 内容高度取较大值，保证短代码铺满、长代码可滚动
  const contentHeight = Math.max(height, lineCount * LINE_HEIGHT + PAD * 2 + 2);

  // 行号槽跟随内容滚动
  useEffect(() => {
    const s = ref.current;
    const g = gutter.current;
    if (!s || !g) return;
    const onScroll = () => {
      g.style.transform = `translateY(${-s.scrollTop}px)`;
    };
    onScroll();
    s.addEventListener("scroll", onScroll);
    return () => s.removeEventListener("scroll", onScroll);
  }, [contentHeight]);

  return (
    <div className="relative flex overflow-hidden" style={{ height }}>
      {/* 行号槽 */}
      <div
        className="tabular line-gutter code-hl hidden shrink-0 overflow-hidden pl-3 text-right text-[12px] sm:block"
        style={{ lineHeight: `${LINE_HEIGHT}px`, paddingTop: PAD, paddingBottom: PAD }}
      >
        <div ref={gutter}>
          {Array.from({ length: Math.max(lineCount, 1) }).map((_, i) => (
            <div key={i}>{i + 1}</div>
          ))}
        </div>
      </div>
      <div ref={ref} className="relative min-w-0 flex-1 overflow-auto">
        <SimpleEditor
          value={value}
          onValueChange={onChange}
          highlight={highlightLua}
          padding={PAD}
          textareaId="lua-input"
          placeholder={placeholder}
          className="code-hl"
          style={{ ...BASE_STYLE, ["--editor-h" as string]: `${contentHeight}px` }}
        />
      </div>
    </div>
  );
}

/* ----------------------------- 只读展示区 ----------------------------- */
export function CodeViewer({
  code,
  lang = "lua",
  height = 420,
  showGutter = true,
  className,
}: {
  code: string;
  lang?: CodeLang;
  height?: number | string;
  showGutter?: boolean;
  className?: string;
}) {
  const scroller = useRef<HTMLDivElement>(null);
  const gutter = useRef<HTMLDivElement>(null);
  const lines = useMemo(() => code.split("\n").length, [code]);
  const html = useMemo(() => highlight(code, lang), [code, lang]);

  // 行号与代码体同步滚动
  useEffect(() => {
    const s = scroller.current;
    const g = gutter.current;
    if (!s || !g) return;
    const onScroll = () => {
      g.style.transform = `translateY(${-s.scrollTop}px)`;
    };
    s.addEventListener("scroll", onScroll);
    return () => s.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <div className={cn("relative flex overflow-hidden", className)} style={{ height }}>
      {showGutter && (
        <div
          className="hidden w-[52px] shrink-0 overflow-hidden border-r border-aqua-300/[0.08] bg-black/20 pr-3 pl-3 sm:block"
          style={{ paddingTop: 16 }}
        >
          <div ref={gutter} className="tabular text-right font-mono text-[12px] leading-[22px] text-ink-800">
            {Array.from({ length: Math.max(lines, 1) }).map((_, i) => (
              <div key={i} className="text-[#2f4f58]">
                {i + 1}
              </div>
            ))}
          </div>
        </div>
      )}
      <div ref={scroller} className="flex-1 overflow-auto p-4">
        <pre
          className="code-hl w-max min-w-full text-[13px] leading-[22px] whitespace-pre"
          dangerouslySetInnerHTML={{ __html: html }}
        />
      </div>
    </div>
  );
}

/* ----------------------------- 小片段高亮 ----------------------------- */
export function InlineCode({
  code,
  lang = "bash",
  label,
}: {
  code: string;
  lang?: CodeLang;
  label?: string;
}) {
  const html = useMemo(() => highlight(code.trim(), lang), [code, lang]);
  return (
    <div className="glass-soft relative overflow-hidden rounded-2xl border border-white/[0.07]">
      {label && (
        <div className="flex items-center gap-2 border-b border-white/[0.06] px-4 py-2">
          <span className="h-1.5 w-1.5 rounded-full bg-aqua-300 shadow-[0_0_10px_rgba(34,226,213,0.9)]" />
          <span className="font-mono text-[11px] tracking-wide text-slate-400/80">{label}</span>
        </div>
      )}
      <pre
        className="code-hl overflow-x-auto p-4 text-[12.5px] leading-[21px]"
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </div>
  );
}
