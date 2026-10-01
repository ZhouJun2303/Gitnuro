import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import csharp from "highlight.js/lib/languages/csharp";
import dart from "highlight.js/lib/languages/dart";
import go from "highlight.js/lib/languages/go";
import java from "highlight.js/lib/languages/java";
import kotlin from "highlight.js/lib/languages/kotlin";
import lua from "highlight.js/lib/languages/lua";
import objectivec from "highlight.js/lib/languages/objectivec";
import php from "highlight.js/lib/languages/php";
import python from "highlight.js/lib/languages/python";
import ruby from "highlight.js/lib/languages/ruby";
import rust from "highlight.js/lib/languages/rust";
import scala from "highlight.js/lib/languages/scala";
import sql from "highlight.js/lib/languages/sql";
import swift from "highlight.js/lib/languages/swift";
import typescript from "highlight.js/lib/languages/typescript";

export type Token = { text: string; cls: string };

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("c", c);
hljs.registerLanguage("cpp", cpp);
hljs.registerLanguage("csharp", csharp);
hljs.registerLanguage("dart", dart);
hljs.registerLanguage("go", go);
hljs.registerLanguage("java", java);
hljs.registerLanguage("kotlin", kotlin);
hljs.registerLanguage("lua", lua);
hljs.registerLanguage("objectivec", objectivec);
hljs.registerLanguage("php", php);
hljs.registerLanguage("python", python);
hljs.registerLanguage("ruby", ruby);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("scala", scala);
hljs.registerLanguage("sql", sql);
hljs.registerLanguage("swift", swift);
hljs.registerLanguage("typescript", typescript);

const BY_EXT: Record<string, string> = {
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  c: "c",
  h: "c",
  cpp: "cpp",
  cc: "cpp",
  hpp: "cpp",
  cs: "csharp",
  dart: "dart",
  go: "go",
  java: "java",
  kt: "kotlin",
  kts: "kotlin",
  lua: "lua",
  m: "objectivec",
  mm: "objectivec",
  php: "php",
  py: "python",
  rb: "ruby",
  rs: "rust",
  scala: "scala",
  sc: "scala",
  sql: "sql",
  swift: "swift",
  ts: "typescript",
  tsx: "typescript",
  js: "typescript",
  jsx: "typescript",
  svelte: "typescript",
};

function language(path: string) {
  const name = path.split(/[\\/]/).pop()?.toLowerCase() ?? "";
  const ext = name.includes(".") ? (name.split(".").pop() ?? "") : name;
  return BY_EXT[ext] ?? "";
}

function cls(name: string) {
  if (name.includes("comment")) return "tok-comment";
  if (name.includes("string") || name.includes("regexp")) return "tok-string";
  if (name.includes("number")) return "tok-number";
  if (name.includes("keyword") || name.includes("built_in") || name.includes("type") || name.includes("title")) return "tok-word";
  return "";
}

function decode(text: string) {
  return text.replaceAll("&lt;", "<").replaceAll("&gt;", ">").replaceAll("&quot;", '"').replaceAll("&#x27;", "'").replaceAll("&amp;", "&");
}

function tokensFromHtml(html: string): Token[] {
  const tokens: Token[] = [];
  const stack: string[] = [];
  const pattern = /<span class="([^"]*)">|<\/span>|[^<]+/g;
  for (const match of html.matchAll(pattern)) {
    const part = match[0];
    if (part.startsWith("<span")) {
      stack.push(cls(match[1] ?? ""));
    } else if (part === "</span>") {
      stack.pop();
    } else if (part.length > 0) {
      tokens.push({ text: decode(part), cls: stack[stack.length - 1] ?? "" });
    }
  }
  return tokens;
}

/** Color one visible line. Unknown file types stay plain text. */
export function highlight(text: string, path: string): Token[] {
  const lang = language(path);
  if (!lang || text.length === 0) return [{ text, cls: "" }];
  try {
    const html = hljs.highlight(text, { language: lang, ignoreIllegals: true }).value;
    const tokens = tokensFromHtml(html);
    return tokens.length > 0 ? tokens : [{ text, cls: "" }];
  } catch {
    return [{ text, cls: "" }];
  }
}
