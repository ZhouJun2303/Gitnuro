import type { DiffLine } from "./status";

export function windowSlice<T>(items: T[], scrollTop: number, rowHeight: number, limit = 80) {
  if (items.length <= limit) return { start: 0, rows: items };
  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - 8);
  return { start, rows: items.slice(start, Math.min(items.length, start + 48)) };
}

export type SplitRow = {
  left: string;
  right: string;
  leftKind: string;
  rightKind: string;
};

/** Pair removed and added lines inside each hunk for a split diff. */
export function splitDiff(lines: DiffLine[]): SplitRow[] {
  const rows: SplitRow[] = [];
  let pending: DiffLine[] = [];
  const flush = () => {
    const removed = pending.filter((line) => line.kind === "delete");
    const added = pending.filter((line) => line.kind === "add");
    const count = Math.max(removed.length, added.length);
    for (let index = 0; index < count; index += 1) {
      rows.push({
        left: removed[index]?.text.slice(1) ?? "",
        right: added[index]?.text.slice(1) ?? "",
        leftKind: removed[index] ? "delete" : "",
        rightKind: added[index] ? "add" : "",
      });
    }
    pending = [];
  };
  for (const line of lines) {
    if (line.kind === "add" || line.kind === "delete") {
      pending.push(line);
      continue;
    }
    flush();
    if (line.kind === "context") {
      const text = line.text.startsWith(" ") ? line.text.slice(1) : line.text;
      rows.push({ left: text, right: text, leftKind: "", rightKind: "" });
    } else {
      rows.push({ left: line.text, right: line.text, leftKind: "meta", rightKind: "meta" });
    }
  }
  flush();
  return rows;
}

export function formatWhen(at: number, pattern: string, hour24: boolean) {
  if (!at) return "";
  const date = new Date(at * 1000);
  if (!pattern.trim()) return date.toLocaleString();
  const hours = date.getHours();
  const map: Record<string, string> = {
    yyyy: String(date.getFullYear()),
    MM: pad(date.getMonth() + 1),
    dd: pad(date.getDate()),
    HH: pad(hour24 ? hours : hours % 12 || 12),
    mm: pad(date.getMinutes()),
    ss: pad(date.getSeconds()),
  };
  return pattern.replace(/yyyy|MM|dd|HH|mm|ss/g, (token) => map[token] ?? token);
}

function pad(value: number) {
  return String(value).padStart(2, "0");
}

export function hiddenName(name: string, pattern: string) {
  return pattern
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean)
    .some((token) => name === token || name.startsWith(token.endsWith("/") ? token : `${token}/`));
}
