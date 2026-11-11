import { tick } from "svelte";
import { numberedDiff, splitDiff, wrapPrefix } from "../view";
import type { DiffLine } from "../status";
import type { ShellState } from "./state.svelte";

type Reading = { file: string; restore: () => Promise<void>; cancel: () => void };
const readings = new WeakMap<ShellState, Reading[]>();

/** Capture only rendered rows, once per preference change, never during scrolling. */
export function rememberDiffReading(shell: ShellState) {
  for (const previous of readings.get(shell) ?? []) previous.cancel();
  const saved: Reading[] = [];
  readings.set(shell, saved);
  const repo = shell.repoPath(), commit = shell.selectedCommit;
  const used = new Set<HTMLElement>();
  function capture(body: HTMLElement | null, file: string, lines: DiffLine[], kind: "main" | "history" | "detail") {
    if (!body || !lines.length) return;
    const scroller = kind === "detail" ? body.closest<HTMLElement>(".detail-scroll") ?? body : body;
    if (used.has(scroller)) return;
    const bounds = scroller.getBoundingClientRect();
    const first = Array.from(body.querySelectorAll<HTMLElement>("[data-line]")).find((node) => {
      const rect = node.getBoundingClientRect();
      return rect.bottom > bounds.top + 1 && rect.top < bounds.bottom;
    });
    if (!first) return;
    const index = Number(first.dataset.line), anchor = numberedDiff(lines)[index];
    if (!anchor) return;
    used.add(scroller);
    const offset = first.getBoundingClientRect().top - bounds.top;
    let cancelled = false;
    const cancel = () => {
      cancelled = true;
      for (const event of ["wheel", "pointerdown", "keydown"]) scroller.removeEventListener(event, cancel);
    };
    for (const event of ["wheel", "pointerdown", "keydown"]) scroller.addEventListener(event, cancel, { passive: true });
    saved.push({ file, cancel, restore: async () => {
      await tick();
      if (cancelled || shell.repoPath() !== repo || shell.selectedCommit !== commit) { cancel(); return; }
      const currentBody = kind === "main" ? shell.diffEl : kind === "history" ? document.querySelector<HTMLElement>(".history-diff")
        : Array.from(document.querySelectorAll<HTMLElement>(".detail .diff-body[data-file]")).find((node) => node.dataset.file === file);
      const current = kind === "main" ? (shell.selectedPath === file ? shell.diff : null)
        : kind === "history" ? (shell.historyFile === file ? shell.historyDiff : null) : shell.patchOf(file);
      if (!currentBody || !current?.lines.length) return;
      const nextScroller = kind === "detail" ? currentBody.closest<HTMLElement>(".detail-scroll") ?? currentBody : currentBody;
      const numbered = numberedDiff(current.lines);
      let at = Math.min(index, numbered.length - 1), distance = Infinity;
      for (let i = 0; i < numbered.length; i++) {
        const row = numbered[i];
        const delta = anchor.newNo && row.newNo ? Math.abs(Number(row.newNo) - Number(anchor.newNo))
          : anchor.oldNo && row.oldNo ? Math.abs(Number(row.oldNo) - Number(anchor.oldNo)) : Infinity;
        if (delta < distance) { at = i; distance = delta; }
        if (delta === 0) break;
      }
      if (!Number.isFinite(distance)) {
        const exact = numbered.findIndex((row) => row.kind === anchor.kind && row.text === anchor.text);
        if (exact >= 0) at = exact;
      }
      const split = shell.settings.diffStyle === "split";
      const splitRows = split ? splitDiff(current.lines) : null;
      const rows = splitRows ?? current.lines;
      const rowIndex = splitRows ? Math.max(0, splitRows.findIndex((row) => row.lineIndex === at || row.leftIndex === at || row.rightIndex === at)) : at;
      const units = shell.settings.diffWordWrap ? wrapPrefix(rows)[rowIndex] ?? rowIndex : rowIndex;
      const origin = currentBody.getBoundingClientRect().top - nextScroller.getBoundingClientRect().top + nextScroller.scrollTop;
      const top = (currentBody === nextScroller ? 0 : origin) + units * shell.diffRowHeight - offset;
      nextScroller.scrollTop = Math.max(0, top);
      if (kind === "main") shell.diffTop = nextScroller.scrollTop;
      if (kind === "history") shell.historyDiffTop = nextScroller.scrollTop;
      nextScroller.dispatchEvent(new Event("scroll"));
      cancel();
    } });
  }
  if (shell.section === "changes") capture(shell.diffEl, shell.selectedPath ?? "", shell.diff?.lines ?? [], "main");
  capture(document.querySelector<HTMLElement>(".history-diff"), shell.historyFile ?? "", shell.historyDiff?.lines ?? [], "history");
  for (const body of document.querySelectorAll<HTMLElement>(".detail .diff-body[data-file]")) {
    const file = body.dataset.file!;
    capture(body, file, shell.patchOf(file)?.lines ?? [], "detail");
  }
}

export function restoreDiffReading(shell: ShellState, file?: string) {
  for (const reading of readings.get(shell) ?? []) if (!file || reading.file === file) void reading.restore();
}
