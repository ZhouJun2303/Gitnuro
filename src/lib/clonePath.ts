/** A clone source: a remote URL, or a single local path. */
export function cloneUrl(raw: string): string {
  const remote = remoteFromText(raw);
  if (remote) return remote;
  const text = raw.trim();
  if (!text || /\s/.test(text) || text.startsWith("-")) return "";
  if (text.includes("://") || text.includes("\\") || text.includes("/")) return text;
  return "";
}

/** Pull a remote URL out of clipboard text or a `git clone` command. */
export function remoteFromText(raw: string): string {
  const line = raw.replace(/^\uFEFF/, "").split(/\r?\n/).map((item) => item.trim()).find((item) => item.length > 0) ?? "";
  if (!line) return "";
  const body = line.replace(/^git\s+clone\s+/i, "").trim();
  const parts = body.match(/"[^"]*"|'[^']*'|\S+/g) ?? [];
  for (const part of parts) {
    const token = part.replace(/^['"]|['"]$/g, "");
    if (isRemote(token)) return token;
  }
  return "";
}

/** Use the clipboard only when it is a URL, optionally wrapped in `git clone` and flags. */
export function clipboardRemote(text: string): string {
  const trimmed = text.replace(/^\uFEFF/, "").trim();
  if (!trimmed || trimmed.length > 2000) return "";
  const remote = remoteFromText(trimmed);
  if (!remote) return "";
  const rest = trimmed.replace(remote, "").replace(/^git\s+clone\s+/i, "").trim();
  if (!rest) return remote;
  const tokens = rest.split(/\s+/);
  return tokens.every((token) => token.startsWith("-") || /^\d+$/.test(token)) ? remote : "";
}

export function repoNameFromUrl(url: string): string {
  const text = url.trim();
  if (!text) return "";
  if (/^(https?|ssh|git|file):\/\//i.test(text)) {
    try {
      const parsed = new URL(text);
      const parts = parsed.pathname.split("/").filter(Boolean).map((part) => {
        try {
          return decodeURIComponent(part);
        } catch {
          return part;
        }
      });
      const gitAt = parts.lastIndexOf("_git");
      if (gitAt >= 0 && parts[gitAt + 1]) return safeFolderName(parts[gitAt + 1].replace(/\.git$/i, ""));
      const noiseAt = parts.findIndex((part, index) => index > 0 && ["-", "tree", "blob", "commit", "commits", "pull", "pulls", "issues", "compare", "src"].includes(part));
      const kept = noiseAt > 0 ? parts.slice(0, noiseAt) : parts;
      const last = kept.at(-1) ?? "";
      return safeFolderName(last.replace(/\.git$/i, ""));
    } catch {
      /* plain path parsing below */
    }
  }
  let value = text.replace(/\\/g, "/").replace(/[?#].*$/, "").replace(/\/+$/, "").replace(/\.git$/i, "");
  const cut = Math.max(value.lastIndexOf("/"), value.lastIndexOf(":"));
  const name = cut >= 0 ? value.slice(cut + 1) : value;
  return safeFolderName(name);
}

/** A git-valid remote name taken from the URL, so the add-remote dialog never asks for one. */
export function remoteNameFromUrl(url: string): string {
  const base = repoNameFromUrl(remoteFromText(url) || url)
    .replace(/\s+/g, "-")
    .replace(/[~^:?*[\]\\{}@]/g, "")
    .replace(/\.{2,}/g, ".")
    .replace(/^[./-]+|[./]+$/g, "")
    .replace(/\.lock$/i, "");
  return base || "origin";
}

export function safeFolderName(name: string): string {
  const cleaned = name.replace(/[<>:"/\\|?*\u0000-\u001f]/g, "").replace(/[. ]+$/g, "").trim();
  if (!cleaned || cleaned === "." || cleaned === ".." || /^(con|prn|aux|nul|com\d|lpt\d)$/i.test(cleaned)) return "";
  return cleaned;
}

export function joinPath(root: string, name: string): string {
  const base = root.trim().replace(/[\\/]+$/, "");
  if (!base || !name) return "";
  const sep = base.includes("\\") ? "\\" : "/";
  return `${base}${sep}${name}`;
}

/** Destination is always `<root>/<repo name>`, never the source folder itself. */
export function cloneTarget(root: string, folder: string, url: string): string {
  const name = safeFolderName(folder.trim() || repoNameFromUrl(remoteFromText(url) || url));
  return joinPath(root, name);
}

function isRemote(value: string): boolean {
  if (!value || value.startsWith("-")) return false;
  if (/^(https?|ssh|git|file):\/\//i.test(value)) return true;
  return /^[\w.+-]+@[\w.-]+:\S+$/.test(value);
}
