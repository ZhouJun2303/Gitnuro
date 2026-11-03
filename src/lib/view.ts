import type { DiffLine } from "./status";

export const laneColors = ["#e0a106", "#3b82f6", "#16a34a", "#7c3aed", "#e11d48", "#0d9488", "#db2777", "#64748b"];

export type GraphCell = {
  color: number;
  role: string;
  /** Columns from this cell to the commit that opened the lane. Negative means that commit sits to the left. */
  forkDelta?: number;
  /** Columns from this commit to the lane its first parent already occupies. Negative means that lane sits to the left. */
  mergeDelta?: number;
} | null;

export type GraphMark = { color: number; d: string };

export type GraphDot = { color: number; x: number };

/** Strokes for one commit row. The viewBox is `width` by 28, and lines run one pixel past each edge so rows meet. */
export function rowGraph(row: GraphCell[]): { width: number; lines: GraphMark[]; curves: GraphMark[]; nodes: GraphDot[] } {
  const lines: GraphMark[] = [];
  const curves: GraphMark[] = [];
  const nodes: GraphDot[] = [];
  row.forEach((cell, index) => {
    if (!cell) return;
    const x = index * 12 + 6;
    const roles = cell.role.split(" ");
    const node = roles.includes("node");
    const up = roles.includes("line-up");
    const down = roles.includes("line-down");
    const through = roles.includes("line");
    if (through || (node && up && down)) lines.push({ color: cell.color, d: `M ${x} -1 L ${x} 29` });
    else if (node && up) lines.push({ color: cell.color, d: `M ${x} -1 L ${x} 14` });
    else if (node && down && cell.forkDelta === undefined) lines.push({ color: cell.color, d: `M ${x} 14 L ${x} 29` });
    if (cell.forkDelta !== undefined) {
      const sx = x + cell.forkDelta * 12;
      curves.push({ color: cell.color, d: `M ${sx} 14 C ${sx} 22, ${x} 22, ${x} 29` });
    }
    if (cell.mergeDelta) {
      const tx = x + cell.mergeDelta * 12;
      curves.push({ color: cell.color, d: `M ${x} 14 C ${x} 22, ${tx} 22, ${tx} 29` });
    }
    if (node) nodes.push({ color: cell.color, x });
  });
  return { width: Math.max(row.length, 1) * 12, lines, curves, nodes };
}

export type GraphBuild = {
  rows: GraphCell[][];
  width: number;
  columns: Array<string | null>;
};

export type GraphResume = {
  columns: Array<string | null>;
  count: number;
  rows: GraphCell[][];
};

/** One row of the commit graph. Columns line up across the list. Pass the previous build when `commits` only grew at the end. */
export function commitGraph(commits: { id: string; parents: string[] }[], resume?: GraphResume): GraphBuild {
  const columns = resume ? resume.columns.slice() : [];
  const rows = resume ? resume.rows.slice() : [];
  const laneOf = new Map<string, number>();
  for (let index = 0; index < columns.length; index++) {
    const id = columns[index];
    if (id && !laneOf.has(id)) laneOf.set(id, index);
  }
  const assign = (index: number, id: string | null) => {
    const previous = columns[index];
    columns[index] = id;
    if (previous && laneOf.get(previous) === index) {
      let nextAt = -1;
      for (let cursor = 0; cursor < columns.length; cursor++) {
        if (columns[cursor] === previous) {
          nextAt = cursor;
          break;
        }
      }
      if (nextAt < 0) laneOf.delete(previous);
      else laneOf.set(previous, nextAt);
    }
    if (id) {
      const known = laneOf.get(id);
      if (known === undefined || index < known) laneOf.set(id, index);
    }
  };

  for (let i = resume?.count ?? 0; i < commits.length; i++) {
    const commit = commits[i];
    let lane = laneOf.get(commit.id);
    const hasUp = lane !== undefined;
    if (lane === undefined) {
      lane = columns.findIndex((id) => !id);
      if (lane < 0) {
        lane = columns.length;
        columns.push(commit.id);
        laneOf.set(commit.id, lane);
      } else {
        assign(lane, commit.id);
      }
    }

    const firstParent = commit.parents[0];
    const hasDown = Boolean(firstParent);

    const extraParents = commit.parents.slice(1);
    const forkLanes: number[] = [];
    for (const extra of extraParents) {
      let free = columns.findIndex((id) => !id);
      if (free < 0) {
        free = columns.length;
        columns.push(extra);
        if (!laneOf.has(extra)) laneOf.set(extra, free);
      } else {
        assign(free, extra);
      }
      forkLanes.push(free);
    }

    let mergeTargetLane: number | null = null;
    if (firstParent) {
      let existingLane = laneOf.get(firstParent);
      if (existingLane === lane) {
        const found = columns.findIndex((id, idx) => idx !== lane && id === firstParent);
        existingLane = found >= 0 ? found : undefined;
      }
      if (existingLane !== undefined && existingLane !== lane) mergeTargetLane = existingLane;
    }

    const rowCells: GraphCell[] = [];
    for (let index = 0; index < columns.length; index++) {
      const id = columns[index];
      if (index === lane) {
        const classes = ["node"];
        if (hasUp) classes.push("line-up");
        if (hasDown && mergeTargetLane === null) classes.push("line-down");
        const cell: GraphCell = {
          color: index % laneColors.length,
          role: classes.join(" "),
        };
        if (mergeTargetLane !== null) cell.mergeDelta = mergeTargetLane - lane;
        rowCells.push(cell);
      } else if (forkLanes.includes(index)) {
        rowCells.push({
          color: index % laneColors.length,
          role: "line-down",
          forkDelta: lane - index,
        });
      } else if (id) {
        rowCells.push({
          color: index % laneColors.length,
          role: "line",
        });
      } else {
        rowCells.push(null);
      }
    }

    rows.push(rowCells);

    if (mergeTargetLane !== null) assign(lane, null);
    else assign(lane, firstParent ?? null);
  }

  const width = rows.reduce((max, row) => Math.max(max, row.length), 1);
  return { rows, width, columns };
}

/** Empty lanes are filled when a row is drawn, so earlier rows stay aligned with later ones. */
export function padGraphRow(row: GraphCell[] | undefined, width: number): GraphCell[] {
  const next = row ? row.slice() : [];
  while (next.length < width) next.push(null);
  return next;
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

/** Coalesce scroll events so a virtual list writes state at most once per frame. */
export function onVirtualScroll(apply: (top: number) => void) {
  let frame = 0;
  return (event: Event) => {
    const node = event.currentTarget as HTMLElement;
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      apply(node.scrollTop);
    });
  };
}

export type SplitRow = {
  left: string;
  right: string;
  leftKind: string;
  rightKind: string;
  leftNo?: number;
  rightNo?: number;
  leftLine?: DiffLine;
  rightLine?: DiffLine;
  /** Index of this cell in the original unified diff. */
  leftIndex?: number;
  rightIndex?: number;
  lineIndex?: number;
  hunkIndex?: number;
};

/** Pair removed and added lines inside each hunk for a split diff. */
export function splitDiff(lines: DiffLine[]): SplitRow[] {
  const rows: SplitRow[] = [];
  let pending: { line: DiffLine; index: number }[] = [];
  let hunkIndex = -1;
  let oldNo = 0;
  let newNo = 0;
  const flush = () => {
    const removed = pending.filter((item) => item.line.kind === "delete");
    const added = pending.filter((item) => item.line.kind === "add");
    const count = Math.max(removed.length, added.length);
    for (let index = 0; index < count; index += 1) {
      const left = removed[index];
      const right = added[index];
      rows.push({
        left: left?.line.text.slice(1) ?? "",
        right: right?.line.text.slice(1) ?? "",
        leftKind: left ? "delete" : "",
        rightKind: right ? "add" : "",
        leftNo: left ? oldNo++ : undefined,
        rightNo: right ? newNo++ : undefined,
        leftLine: left?.line,
        rightLine: right?.line,
        leftIndex: left?.index,
        rightIndex: right?.index,
      });
    }
    pending = [];
  };
  lines.forEach((line, index) => {
    if (line.kind === "add" || line.kind === "delete") {
      pending.push({ line, index });
      return;
    }
    flush();
    if (line.kind === "hunk") {
      hunkIndex += 1;
      const start = line.text.match(/@@ -(\d+)(?:,\d+)? \+(\d+)/);
      if (start) {
        oldNo = Number(start[1]);
        newNo = Number(start[2]);
      }
    }
    if (line.kind === "context") {
      const text = line.text.startsWith(" ") ? line.text.slice(1) : line.text;
      rows.push({ left: text, right: text, leftKind: "", rightKind: "", leftNo: oldNo++, rightNo: newNo++, lineIndex: index });
    } else {
      rows.push({
        left: line.text,
        right: line.text,
        leftKind: "meta",
        rightKind: "meta",
        lineIndex: index,
        hunkIndex: line.kind === "hunk" ? hunkIndex : undefined,
      });
    }
  });
  flush();
  return rows;
}

function tokenize(text: string) {
  return text.match(/[A-Za-z0-9_]+|[\u3400-\u9fff]|\s+|[^\s]/g) ?? [];
}

/** Character ranges in `text` that are not shared with `other`, at word granularity. */
export function changedRanges(other: string, text: string): Array<[number, number]> {
  if (!other || !text || other === text) return [];
  const left = tokenize(other);
  const right = tokenize(text);
  if (left.length === 0 || right.length === 0 || left.length > 96 || right.length > 96) return [];
  if (left.join("") !== other || right.join("") !== text) return [];
  const keep = lcsKeep(left, right);
  const ranges: Array<[number, number]> = [];
  let cursor = 0;
  let start = -1;
  for (let index = 0; index < right.length; index += 1) {
    const token = right[index] ?? "";
    if (!keep[index]) {
      if (start < 0) start = cursor;
    } else if (start >= 0) {
      ranges.push([start, cursor]);
      start = -1;
    }
    cursor += token.length;
  }
  if (start >= 0) ranges.push([start, cursor]);
  return ranges.filter(([from, to]) => text.slice(from, to).trim().length > 0);
}

function lcsKeep(left: string[], right: string[]) {
  const rows = left.length;
  const cols = right.length;
  const score: number[][] = Array.from({ length: rows + 1 }, () => new Array<number>(cols + 1).fill(0));
  for (let row = rows - 1; row >= 0; row -= 1) {
    for (let col = cols - 1; col >= 0; col -= 1) {
      score[row][col] = left[row] === right[col] ? (score[row + 1]?.[col + 1] ?? 0) + 1 : Math.max(score[row + 1]?.[col] ?? 0, score[row]?.[col + 1] ?? 0);
    }
  }
  const keep = new Array<boolean>(cols).fill(false);
  let row = 0;
  let col = 0;
  while (row < rows && col < cols) {
    if (left[row] === right[col]) {
      keep[col] = true;
      row += 1;
      col += 1;
    } else if ((score[row + 1]?.[col] ?? 0) >= (score[row]?.[col + 1] ?? 0)) {
      row += 1;
    } else {
      col += 1;
    }
  }
  return keep;
}

/** Split syntax tokens so changed characters can take a stronger background. */
export function markTokens<T extends { text: string }>(tokens: T[], ranges: Array<[number, number]>): Array<T & { hot: boolean }> {
  if (ranges.length === 0) return tokens.map((token) => ({ ...token, hot: false }));
  const out: Array<T & { hot: boolean }> = [];
  let cursor = 0;
  for (const token of tokens) {
    const start = cursor;
    const end = cursor + token.text.length;
    cursor = end;
    let pos = start;
    for (const [from, to] of ranges) {
      if (to <= pos || from >= end) continue;
      if (from > pos) {
        out.push({ ...token, text: token.text.slice(pos - start, from - start), hot: false });
        pos = from;
      }
      const cut = Math.min(to, end);
      out.push({ ...token, text: token.text.slice(pos - start, cut - start), hot: true });
      pos = cut;
      if (pos >= end) break;
    }
    if (pos < end) out.push({ ...token, text: token.text.slice(pos - start), hot: false });
  }
  return out.filter((token) => token.text.length > 0);
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

/** Folders (names with a first path segment) stay together, ahead of bare names. */
export function orderRefNames(names: readonly string[], pinned?: ReadonlySet<string>) {
  const folders: string[] = [];
  const loose: string[] = [];
  for (const name of names) (branchGroup(name) ? folders : loose).push(name);
  const byName = (a: string, b: string) => a.localeCompare(b);
  folders.sort((a, b) => {
    const group = branchGroup(a).localeCompare(branchGroup(b));
    return group || byName(a, b);
  });
  loose.sort((a, b) => {
    const pin = Number(pinned?.has(b) ?? false) - Number(pinned?.has(a) ?? false);
    return pin || byName(a, b);
  });
  return [...folders, ...loose];
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
