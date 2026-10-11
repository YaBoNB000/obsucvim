import Prism from "prismjs";
import "prismjs/components/prism-lua";
import "prismjs/components/prism-rust";
import "prismjs/components/prism-bash";
import "prismjs/components/prism-json";

export type CodeLang = "lua" | "rust" | "bash" | "json" | "javascript";

export default Prism;

export function highlight(code: string, lang: CodeLang): string {
  const grammar = Prism.languages[lang] ?? Prism.languages.lua;
  return Prism.highlight(code, grammar, lang);
}

export function highlightLua(code: string): string {
  return Prism.highlight(code, Prism.languages.lua, "lua");
}
