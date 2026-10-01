export type Token = { text: string; cls: string };

const WORDS: Record<string, string[]> = {
  rs: ["fn", "let", "mut", "pub", "struct", "enum", "impl", "use", "mod", "match", "if", "else", "return", "for", "while", "async", "await", "self"],
  kt: ["fun", "val", "var", "class", "object", "interface", "if", "else", "when", "return", "for", "while", "package", "import"],
  ts: ["const", "let", "function", "return", "if", "else", "import", "export", "class", "type", "interface", "new", "await"],
  js: ["const", "let", "function", "return", "if", "else", "import", "export", "class", "new", "await"],
  py: ["def", "class", "return", "if", "elif", "else", "import", "from", "for", "while", "None", "True", "False"],
  go: ["func", "package", "import", "return", "if", "else", "for", "var", "const", "type", "struct"],
  c: ["if", "else", "return", "for", "while", "struct", "int", "void", "const", "static"],
  java: ["class", "interface", "return", "if", "else", "new", "public", "private", "static", "void"],
  css: ["color", "background", "display", "flex", "margin", "padding"],
};

function family(path: string) {
  const name = path.split(/[\\/]/).pop()?.toLowerCase() ?? "";
  const ext = name.includes(".") ? name.split(".").pop() ?? "" : name;
  if (ext === "tsx" || ext === "jsx" || ext === "svelte") return "ts";
  if (ext === "kt" || ext === "kts") return "kt";
  if (ext === "py") return "py";
  if (ext === "rs") return "rs";
  if (ext === "go") return "go";
  if (ext === "java") return "java";
  if (ext === "css") return "css";
  if (ext === "c" || ext === "h" || ext === "cpp" || ext === "hpp") return "c";
  if (ext === "sh" || ext === "bash") return "sh";
  if (ext === "json" || ext === "toml" || ext === "yml" || ext === "yaml") return "data";
  return "ts";
}

function commentStart(kind: string) {
  if (kind === "py" || kind === "sh" || kind === "toml" || kind === "data") return "#";
  if (kind === "css") return null;
  return "//";
}

/** A small original tokenizer. It is not a port of the old highlighters. */
export function highlight(text: string, path: string): Token[] {
  const kind = family(path);
  const words = new Set(WORDS[kind] ?? WORDS.ts);
  const comment = commentStart(kind);
  const tokens: Token[] = [];
  let index = 0;
  const push = (cls: string, end: number) => {
    if (end > index) tokens.push({ text: text.slice(index, end), cls });
    index = end;
  };
  while (index < text.length) {
    const rest = text.slice(index);
    if (comment && rest.startsWith(comment)) {
      tokens.push({ text: rest, cls: "tok-comment" });
      break;
    }
    const char = text[index];
    if (char === '"' || char === "'" || char === "`") {
      let end = index + 1;
      while (end < text.length && text[end] !== char) {
        if (text[end] === "\\") end += 1;
        end += 1;
      }
      push("tok-string", Math.min(text.length, end + 1));
      continue;
    }
    if (char >= "0" && char <= "9") {
      let end = index + 1;
      while (end < text.length && /[0-9a-fx.]/.test(text[end])) end += 1;
      push("tok-number", end);
      continue;
    }
    if (/[A-Za-z_]/.test(char)) {
      let end = index + 1;
      while (end < text.length && /[A-Za-z0-9_]/.test(text[end])) end += 1;
      const word = text.slice(index, end);
      tokens.push({ text: word, cls: words.has(word) ? "tok-word" : "" });
      index = end;
      continue;
    }
    push("", index + 1);
  }
  return tokens;
}
