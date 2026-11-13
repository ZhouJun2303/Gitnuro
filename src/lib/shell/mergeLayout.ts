import type { MergePiece } from "./types";

export type MergeSide = "theirs" | "ours" | "result";
export type MergeBlock = { key: string; piece: number; kind: "text" | "hunk" | "pending" | "manual"; text: string; empty: boolean };

/** Bound text in a single DOM node, including a file containing just one huge hunk. */
export function mergeBlocks(pieces: MergePiece[], side: MergeSide, empty: string, pending: string, manual?: string): MergeBlock[] {
  const blocks: MergeBlock[] = [];
  function append(text: string, piece: number, kind: MergeBlock["kind"]) {
    const blank = kind === "hunk" && side !== "result" && !text.trim();
    const lines = (blank ? empty : text).split("\n");
    for (let start = 0; start < lines.length; start += 32) {
      blocks.push({ key: `${piece}:${start}`, piece, kind, empty: blank, text: lines.slice(start, start + 32).join("\n") });
    }
  }
  if (manual !== undefined) append(manual, -1, "manual");
  else pieces.forEach((piece, index) => {
    if (piece.kind === "text") append(piece.text, index, "text");
    else if (side !== "result") append(piece[side], index, "hunk");
    else if (piece.pick === "none") append(pending, index, "pending");
    else {
      const parts: string[] = [];
      if ((piece.pick === "theirs" || piece.pick === "both") && (piece.theirs || piece.theirsLines === 1)) parts.push(piece.theirs);
      if ((piece.pick === "ours" || piece.pick === "both") && (piece.ours || piece.oursLines === 1)) parts.push(piece.ours);
      append(parts.join("\n"), index, "hunk");
    }
  });
  return blocks;
}

/** A height index supports measurement and viewport lookup without scanning the file on scroll. */
export class MergeLayout {
  private tree: Float64Array;
  private heights: Float64Array;
  constructor(public blocks: MergeBlock[], width: number) {
    this.tree = new Float64Array(blocks.length + 1);
    this.heights = new Float64Array(blocks.length);
    const columns = Math.max(8, Math.floor((width - 24) / 7.2));
    blocks.forEach((block, index) => {
      const rows = block.text.split("\n").reduce((sum, line) => sum + Math.max(1, Math.ceil(line.length / columns)), 0);
      this.measure(index, rows * 17.4 + (block.kind === "pending" ? 28 : 14));
    });
  }
  offset(end: number): number {
    let sum = 0;
    for (let i = end; i > 0; i -= i & -i) sum += this.tree[i];
    return sum;
  }
  get height() { return this.offset(this.blocks.length); }
  measure(index: number, height: number) {
    if (!Number.isFinite(height) || height <= 0) return false;
    const delta = height - this.heights[index];
    if (Math.abs(delta) < 0.5) return false;
    this.heights[index] = height;
    for (let i = index + 1; i < this.tree.length; i += i & -i) this.tree[i] += delta;
    return true;
  }
  at(top: number) {
    let index = 0, sum = 0;
    for (let step = 2 ** Math.floor(Math.log2(Math.max(1, this.blocks.length))); step; step >>= 1) {
      const next = index + step;
      if (next < this.tree.length && sum + this.tree[next] <= top) { sum += this.tree[next]; index = next; }
    }
    return Math.min(index, Math.max(0, this.blocks.length - 1));
  }
}
