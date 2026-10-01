import type { DiffLine } from "./status";

export const laneColors = ["#e0a106", "#3b82f6", "#16a34a", "#7c3aed", "#e11d48", "#0d9488", "#db2777", "#64748b"];

export type GraphCell = { color: number; role: "node" | "line" } | null;

/** One row of the commit graph. Columns line up across the list. */
export function commitGraph(commits: { id: string; parents: string[] }[]): GraphCell[][] {
  const columns: Array<string | null> = [];
  const rows: GraphCell[][] = [];
  for (const commit of commits) {
    let lane = columns.findIndex((id) => id === commit.id);
    if (lane < 0) {
      lane = columns.findIndex((id) => !id);
      if (lane < 0) {
        lane = columns.length;
        columns.push(commit.id);
      } else columns[lane] = commit.id;
    }
    rows.push(
      columns.map((id, index) => {
        if (!id && index !== lane) return null;
        if (index === lane) return { color: index % laneColors.length, role: "node" };
        return id ? { color: index % laneColors.length, role: "line" } : null;
      }),
    );
    columns[lane] = commit.parents[0] ?? null;
    for (const extra of commit.parents.slice(1)) {
      const free = columns.findIndex((id) => !id);
      if (free < 0) columns.push(extra);
      else columns[free] = extra;
    }
  }
  const width = rows.reduce((max, row) => Math.max(max, row.length), 1);
  return rows.map((row) => {
    const next = row.slice();
    while (next.length < width) next.push(null);
    return next;
  });
}

export function numberedDiff(lines: DiffLine[]) {
  let oldNo = 0;
  let newNo = 0;
  return lines.map((line) => {
    const hunk = line.kind === "hunk" ? line.text.match(/@@ -(\d+)(?:,\d+)? \+(\d+)/) : null;
    if (hunk) {
      oldNo = Number(hunk[1]);
      newNo = Number(hunk[2]);
      return { ...line, oldNo: "", newNo: "" };
    }
    if (line.kind === "add") {
      const current = newNo;
      newNo += 1;
      return { ...line, oldNo: "", newNo: String(current) };
    }
    if (line.kind === "delete") {
      const current = oldNo;
      oldNo += 1;
      return { ...line, oldNo: String(current), newNo: "" };
    }
    if (line.kind === "context") {
      const left = oldNo;
      const right = newNo;
      oldNo += 1;
      newNo += 1;
      return { ...line, oldNo: String(left), newNo: String(right) };
    }
    return { ...line, oldNo: "", newNo: "" };
  });
}

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
  leftLine?: DiffLine;
  rightLine?: DiffLine;
  hunkIndex?: number;
};

/** Pair removed and added lines inside each hunk for a split diff. */
export function splitDiff(lines: DiffLine[]): SplitRow[] {
  const rows: SplitRow[] = [];
  let pending: DiffLine[] = [];
  let hunkIndex = -1;
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
        leftLine: removed[index],
        rightLine: added[index],
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
    if (line.kind === "hunk") hunkIndex += 1;
    if (line.kind === "context") {
      const text = line.text.startsWith(" ") ? line.text.slice(1) : line.text;
      rows.push({ left: text, right: text, leftKind: "", rightKind: "" });
    } else {
      rows.push({
        left: line.text,
        right: line.text,
        leftKind: "meta",
        rightKind: "meta",
        hunkIndex: line.kind === "hunk" ? hunkIndex : undefined,
      });
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
  const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  const map: Record<string, string> = {
    yyyy: String(date.getFullYear()),
    MMM: months[date.getMonth()] ?? "",
    MM: pad(date.getMonth() + 1),
    dd: pad(date.getDate()),
    HH: pad(hour24 ? hours : hours % 12 || 12),
    mm: pad(date.getMinutes()),
    ss: pad(date.getSeconds()),
  };
  return pattern.replace(/yyyy|MMM|MM|dd|HH|mm|ss/g, (token) => map[token] ?? token);
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

/** First path segment used as a sidebar group. A name without `/` stays on its own. */
export function branchGroup(name: string) {
  const slash = name.indexOf("/");
  return slash > 0 ? name.slice(0, slash) : "";
}

export function gravatarUrl(email: string) {
  const hash = md5(email.trim().toLowerCase());
  return `https://www.gravatar.com/avatar/${hash}?s=32&d=blank`;
}

function md5(value: string) {
  const bytes = new TextEncoder().encode(value);
  const words = new Uint32Array((((bytes.length + 8) >>> 6) + 1) * 16);
  for (let index = 0; index < bytes.length; index += 1) words[index >> 2] |= bytes[index] << ((index % 4) * 8);
  words[bytes.length >> 2] |= 0x80 << ((bytes.length % 4) * 8);
  words[words.length - 2] = bytes.length * 8;
  let a = 0x67452301;
  let b = 0xefcdab89;
  let c = 0x98badcfe;
  let d = 0x10325476;
  const s = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];
  const k = new Uint32Array(64);
  for (let index = 0; index < 64; index += 1) k[index] = Math.floor(Math.abs(Math.sin(index + 1)) * 2 ** 32);
  for (let offset = 0; offset < words.length; offset += 16) {
    let aa = a;
    let bb = b;
    let cc = c;
    let dd = d;
    for (let index = 0; index < 64; index += 1) {
      let f = 0;
      let g = 0;
      if (index < 16) {
        f = (bb & cc) | (~bb & dd);
        g = index;
      } else if (index < 32) {
        f = (dd & bb) | (~dd & cc);
        g = (5 * index + 1) % 16;
      } else if (index < 48) {
        f = bb ^ cc ^ dd;
        g = (3 * index + 5) % 16;
      } else {
        f = cc ^ (bb | ~dd);
        g = (7 * index) % 16;
      }
      const turn = (aa + f + k[index] + words[offset + g]) >>> 0;
      const shift = s[(index >> 4) * 4 + (index % 4)];
      const next = bb + ((turn << shift) | (turn >>> (32 - shift)));
      aa = dd;
      dd = cc;
      cc = bb;
      bb = next >>> 0;
    }
    a = (a + aa) >>> 0;
    b = (b + bb) >>> 0;
    c = (c + cc) >>> 0;
    d = (d + dd) >>> 0;
  }
  return [a, b, c, d].map((word) => [word & 255, (word >>> 8) & 255, (word >>> 16) & 255, (word >>> 24) & 255].map((byte) => byte.toString(16).padStart(2, "0")).join("")).join("");
}
