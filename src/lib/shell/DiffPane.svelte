<script lang="ts">
  import { tick, untrack } from "svelte";
  import { getShell } from "./context";
  import "./DiffPane.css";
  import { invoke } from "../perf";
  import { sampleConflictText, sampleImages } from "../sample";
  import { onVirtualScroll, type SplitRow } from "../view";
  import WrappedRows from "../WrappedRows.svelte";
  import type { MergePiece, MergePick, Row, Side } from "./types";
  import { applyConflictChoice, clearPixel, loadBlameRange, mergeToolLabel, mergeToolMissing, openConflictTool } from "./diff.svelte";
  import BinaryDiff from "./BinaryDiff.svelte";
  import ImageComparison from "./ImageComparison.svelte";
  import { formatBytes } from "../format";
  import type { CommitRow, ConflictParty, ConflictSides, DiffLine } from "../status";

  const shell = getShell();
  const splitScroller = (node: HTMLElement, columns?: { left: number; right: number }) => shell.splitScroller(node, columns);
  const onDiffScroll = onVirtualScroll((top) => {
    shell.diffTop = top;
    if (hoveredHunk != null && pointerInside) void hoverUnderPointer();
  });
  const onBlameScroll = onVirtualScroll((top) => { shell.blameTop = top; void loadBlameRange(shell); });
  let pickOurs = $state(true);
  let pickTheirs = $state(true);
  let pickedPath = "";
  let hoveredHunk = $state<number | null>(null);
  let hoveredLine = $state<number | null>(null);
  let activeHunk = $state<number | null>(null);
  let pointerX = 0;
  let pointerY = 0;
  let pointerInside = false;

  $effect(() => {
    const path = `${shell.repoPath() ?? ""}\0${shell.selectedPath ?? ""}`;
    if (path === pickedPath) return;
    pickedPath = path;
    pickOurs = true;
    pickTheirs = true;
    hoveredHunk = null;
    hoveredLine = null;
    activeHunk = null;
    shell.imageMode = "side";
    shell.imagePos = 50;
  });

  $effect(() => {
    shell.repoPath();
    shell.focusPath;
    shell.section;
    shell.selectedPath;
    shell.selectedSide;
    shell.selectedCommit;
    shell.historyFile;
    shell.imageOld;
    shell.imageNew;
    shell.preview;
    untrack(() => {
      clearPixel(shell);
      shell.imageMode = "side";
    });
    return () => clearPixel(shell);
  });

  function pickImageMode(mode: typeof shell.imageMode, before: string | null, after: string | null) {
    clearPixel(shell);
    shell.imageMode = mode;
    if (mode === "pixel" && before && after) void shell.paintPixel(before, after);
  }

  const selectedSubmodule = $derived.by(() => {
    const path = shell.selectedPath;
    if (shell.mode !== "live" || shell.section !== "changes" || !path) return null;
    const row = shell.refs?.submodules.find((item) => item.path === path);
    if (row) return row;
    // A refs read can arrive after the diff read. Gitlink text still identifies the path.
    return shell.diff?.lines.some((line) => line.text.startsWith("-Subproject commit ") || line.text.startsWith("+Subproject commit "))
      ? { path, id: "", recorded: "", branch: "", configuredBranch: "", ready: false }
      : null;
  });
  const submoduleCommits = $derived.by(() => {
    if (!selectedSubmodule) return { oldId: "", newId: "" };
    let oldId = "";
    let newId = "";
    for (const line of shell.diff?.lines ?? []) {
      if (line.text.startsWith("-Subproject commit ")) oldId = line.text.slice("-Subproject commit ".length).trim();
      if (line.text.startsWith("+Subproject commit ")) newId = line.text.slice("+Subproject commit ".length).trim();
    }
    return { oldId, newId };
  });
  let submoduleDetailsKey = $state("");
  let submoduleDetailsToken = 0;
  let submoduleHistory = $state<CommitRow[]>([]);
  let submoduleDetailsLoading = $state(false);
  let submoduleDetailsError = $state("");
  let submoduleSelectedCommitId = $state("");
  const submoduleName = $derived(selectedSubmodule ? shell.folderName(selectedSubmodule.path) : "");
  const submoduleSelectedCommit = $derived(submoduleHistory.find((commit) => commit.id === submoduleSelectedCommitId) ?? submoduleHistory[0] ?? null);
  const submoduleUncommittedCount = $derived.by(() => {
    const path = selectedSubmodule?.path;
    if (!path) return 0;
    const parentRows = [...(shell.snapshot?.staged ?? []), ...(shell.snapshot?.unstaged ?? [])];
    return parentRows.some((file) => file.path === path) ? 1 : 0;
  });

  $effect(() => {
    const submodule = selectedSubmodule;
    const path = submodule?.ready ? shell.fullPath(submodule.path) : "";
    const oldId = submoduleCommits.oldId;
    const newId = submoduleCommits.newId;
    if (!path || !newId) {
      submoduleDetailsKey = "";
      submoduleHistory = [];
      submoduleDetailsLoading = false;
      submoduleDetailsError = "";
      submoduleSelectedCommitId = "";
      return;
    }
    const key = `${path}\0${oldId}\0${newId}`;
    if (key === submoduleDetailsKey) return;
    submoduleDetailsKey = key;
    submoduleDetailsToken += 1;
    const token = submoduleDetailsToken;
    submoduleHistory = [];
    submoduleDetailsLoading = true;
    submoduleDetailsError = "";
    submoduleSelectedCommitId = "";

    const revs = oldId && oldId !== newId ? [newId, `^${oldId}`] : [newId];
    void (async () => {
      let history: CommitRow[] = [];
      let failed = false;
      try {
        history = await invoke<CommitRow[]>("commit_log", { path, limit: 20, all: false, order: "date", revs });
      } catch {
        failed = true;
      }
      if (history.length === 0 && oldId && oldId !== newId) {
        try {
          history = await invoke<CommitRow[]>("commit_log", { path, limit: 20, all: false, order: "date", revs: [newId] });
          failed = false;
        } catch {
          failed = true;
        }
      }
      if (token !== submoduleDetailsToken) return;
      submoduleHistory = history;
      submoduleSelectedCommitId = history[0]?.id ?? "";
      submoduleDetailsError = failed && history.length === 0 ? shell.tr("chrome.submoduleDetailsUnavailable") : "";
      submoduleDetailsLoading = false;
    })();
  });
  const hunkList = $derived(shell.mode === "live" && shell.section === "changes" && !selectedSubmodule ? shell.hunkRanges() : []);
  let wrapped = $state<{ scrollTo: (index: number) => Promise<void> }>();
  const conflictSelection = $derived(shell.section === "changes" ? shell.pickedConflicts() : []);
  const activeAt = $derived(hunkList.findIndex((range) => range.index === activeHunk));
  const conflictPreview = $derived.by<ConflictSides | null>(() => {
    if (shell.settings.showConflictPreview !== true) return null;
    if (shell.mode === "sample") return shell.selectedPath ? (sampleConflictText[shell.selectedPath] ?? null) : null;
    return shell.conflictPreview;
  });
  const conflictPreviewPieces = $derived.by<MergePiece[]>(() => {
    const sides = conflictPreview;
    if (!sides) return [];
    const pieces = shell.splitConflicts(sides.working);
    if (pieces.some((piece) => piece.kind === "hunk")) return pieces;
    if (sides.ours || sides.theirs) return [{ kind: "hunk", ours: sides.ours, theirs: sides.theirs, pick: "none" }];
    return pieces;
  });
  const conflictPreviewHunks = $derived(
    conflictPreviewPieces
      .map((piece, index) => (piece.kind === "hunk" ? index : -1))
      .filter((index) => index >= 0),
  );
  let conflictPreviewPicks = $state<Record<number, MergePick>>({});
  const conflictPreviewPickedPieces = $derived(
    conflictPreviewPieces.map((piece, index) => {
      const pick = conflictPreviewPicks[index];
      return piece.kind === "hunk" && pick ? { ...piece, pick } : piece;
    }),
  );
  const conflictPreviewUnresolved = $derived(
    conflictPreviewHunks.filter((index) => !conflictPreviewPicks[index] || conflictPreviewPicks[index] === "none").length,
  );
  let activeConflictPreview = $state(0);
  let conflictPreviewPath = "";

  $effect(() => {
    const preview = conflictPreview;
    const fingerprint = preview?.version ?? (preview ? `${preview.working.length}:${preview.working.slice(0, 24)}:${preview.working.slice(-24)}` : "");
    const path = `${shell.repoPath() ?? ""}\0${shell.selectedPath ?? ""}\0${fingerprint}`;
    if (path === conflictPreviewPath) return;
    conflictPreviewPath = path;
    activeConflictPreview = 0;
    conflictPreviewPicks = {};
  });

  $effect(() => {
    if (activeConflictPreview >= conflictPreviewHunks.length) activeConflictPreview = Math.max(0, conflictPreviewHunks.length - 1);
  });

  $effect(() => {
    if (activeHunk != null && !hunkList.some((range) => range.index === activeHunk)) activeHunk = null;
  });
  const litHunk = $derived(hoveredHunk ?? activeHunk);
  const litRange = $derived(hunkList.find((range) => range.index === litHunk) ?? null);
  const litStart = $derived(litRange?.start ?? -1);
  const litEnd = $derived(litRange == null ? -1 : litRange.end - 1);
  const litOn = (at: number | null) => at != null && at >= litStart && at <= litEnd;
  const hoveredRange = $derived(hoveredHunk == null ? null : hunkList.find((range) => range.index === hoveredHunk) ?? null);
  const hoveredSelected = $derived(
    hoveredRange
      ? shell.diffSelected.filter((index) => index >= hoveredRange.start && index < hoveredRange.end)
      : [],
  );
  const hoveredLineOps = $derived.by(() => {
    if (hoveredSelected.length === 0) return [] as { line: DiffLine; index: number }[];
    return hoveredSelected
      .map((index) => ({ line: shell.diffLines[index], index }))
      .filter((entry): entry is { line: DiffLine; index: number } => !!entry.line && (entry.line.kind === "add" || entry.line.kind === "delete") && entry.line.stageAt != null);
  });
  const hoveredDiscardOps = $derived(hoveredSelected.length > 0 ? shell.discardableEntries(hoveredSelected) : []);
  const useLineOps = $derived(hoveredLineOps.length > 0 || hoveredDiscardOps.length > 0);
  const rowHeight = $derived(shell.settings.diffStyle === "split" ? shell.splitRowHeight : shell.diffRowHeight);
  /** The hunk's first rendered row decides the spot; wrapped rows are not a fixed height. */
  const hunkActionTop = $derived.by(() => {
    const body = shell.diffEl;
    const range = hoveredRange;
    if (!body || !range) return 4;
    // Re-measure as the scrollport moves or wrapping and font sizes change.
    void shell.diffTop;
    void shell.settings.diffWordWrap;
    void shell.effectiveDiffFontSize;
    void shell.effectiveUiScale;
    const bodyTop = body.getBoundingClientRect().top;
    const anchor =
      body.querySelector<HTMLElement>(`[data-line="${range.start}"]`) ??
      (hoveredLine == null ? null : body.querySelector<HTMLElement>(`[data-line="${hoveredLine}"]`));
    if (!anchor) return Math.max(4, Math.min(range.start * rowHeight + 8 - shell.diffTop, shell.diffHeight - 30));
    return Math.min(Math.max(anchor.getBoundingClientRect().top - bodyTop, 4), Math.max(4, shell.diffHeight - 30));
  });

  const sideText = (party: ConflictParty) => {
    if (party.state === "added") return shell.tr("chrome.sideAdded");
    if (party.state === "deleted") return shell.tr("chrome.sideDeleted");
    if (party.state === "modified") return shell.tr("chrome.sideModified");
    return "";
  };

  function hoverDiffLine(event: MouseEvent) {
    pointerX = event.clientX;
    pointerY = event.clientY;
    pointerInside = true;
    const hit = event.target instanceof Element ? event.target.closest<HTMLElement>("[data-line]") : null;
    const line = Number(hit?.dataset.line);
    if (!Number.isInteger(line)) {
      if (!(event.target instanceof Element) || !event.target.closest(".diff-hunk-actions")) hoveredHunk = null;
      return;
    }
    hoveredLine = line;
    hoveredHunk = shell.hunkOf(line);
  }

  function clearHunkHover(event: MouseEvent) {
    pointerInside = false;
    if (event.relatedTarget instanceof Element && event.relatedTarget.closest(".diff-hunk-actions")) return;
    hoveredHunk = null;
    hoveredLine = null;
  }

  function leaveHunkActions() {
    pointerInside = false;
    hoveredHunk = null;
    hoveredLine = null;
  }

  async function hoverUnderPointer() {
    await tick();
    let overBar = false;
    for (const node of document.elementsFromPoint(pointerX, pointerY)) {
      if (!(node instanceof HTMLElement)) continue;
      if (node.closest(".diff-hunk-actions")) {
        overBar = true;
        continue;
      }
      const at = Number(node.dataset.line);
      if (!Number.isInteger(at)) continue;
      hoveredLine = at;
      hoveredHunk = shell.hunkOf(at);
      return;
    }
    if (!overBar) {
      hoveredHunk = null;
      hoveredLine = null;
    }
  }

  function goMark(line: number) {
    const hunk = shell.hunkOf(line);
    if (hunk != null) {
      activeHunk = hunk;
      hoveredHunk = hunk;
      scrollToHunk(hunk);
      return;
    }
    const node = shell.diffEl;
    if (!node) return;
    if (shell.settings.diffStyle === "split") {
      const row = shell.splitRows.findIndex((item) => {
        const left = shell.splitLineAt(item, "left");
        const right = shell.splitLineAt(item, "right");
        return left === line || right === line;
      });
      if (shell.settings.diffWordWrap && wrapped) { void wrapped.scrollTo(Math.max(0, row)); return; }
      node.scrollTop = Math.max(0, row * shell.splitRowHeight);
    } else {
      if (shell.settings.diffWordWrap && wrapped) { void wrapped.scrollTo(line); return; }
      node.scrollTop = Math.max(0, line * shell.diffRowHeight);
    }
    shell.diffTop = node.scrollTop;
  }

  function scrollToHunk(index: number) {
    const node = shell.diffEl;
    const range = hunkList.find((item) => item.index === index);
    if (!node || !range) return;
    if (shell.settings.diffStyle === "split") {
      const row = shell.splitRows.findIndex((item) => {
        const left = shell.splitLineAt(item, "left");
        const right = shell.splitLineAt(item, "right");
        return (left != null && left >= range.start && left < range.end) || (right != null && right >= range.start && right < range.end);
      });
      if (shell.settings.diffWordWrap && wrapped) { void wrapped.scrollTo(Math.max(0, row)); return; }
      node.scrollTop = Math.max(0, row * shell.splitRowHeight - 8);
    } else {
      if (shell.settings.diffWordWrap && wrapped) { void wrapped.scrollTo(range.start); return; }
      node.scrollTop = Math.max(0, range.start * shell.diffRowHeight - 8);
    }
    shell.diffTop = node.scrollTop;
  }

  function selectHunk(index: number) {
    activeHunk = index;
    hoveredHunk = index;
    scrollToHunk(index);
  }

  function stepHunk(delta: number) {
    if (hunkList.length === 0) return;
    const at = activeAt < 0 ? (delta > 0 ? -1 : 0) : activeAt;
    const next = (at + delta + hunkList.length) % hunkList.length;
    const range = hunkList[next];
    if (range) selectHunk(range.index);
  }

  function revealConflictPreview(index: number) {
    if (conflictPreviewHunks.length === 0) return;
    const next = (index + conflictPreviewHunks.length) % conflictPreviewHunks.length;
    activeConflictPreview = next;
    const piece = conflictPreviewHunks[next];
    if (piece == null) return;
    queueMicrotask(() => document.querySelector<HTMLElement>(`[data-conflict-index="${piece}"]`)?.scrollIntoView({ block: "center" }));
  }

  function stepConflictPreview(delta: number) {
    revealConflictPreview(activeConflictPreview + delta);
  }

  function openConflictPreviewMerge() {
    const file = shell.selectedPath;
    if (file) void shell.openTextMerge(file);
  }

  function pickConflictPreview(index: number, pick: MergePick) {
    conflictPreviewPicks = { ...conflictPreviewPicks, [index]: pick };
    activeConflictPreview = Math.max(0, conflictPreviewHunks.indexOf(index));
  }

  function applyConflictPreview() {
    const file = shell.selectedPath;
    if (!file || shell.mode !== "live" || shell.writeBlocked || conflictPreviewUnresolved > 0) return;
    void shell.mutate({ action: "resolve", file, side: "text", text: shell.joinedPieces(conflictPreviewPickedPieces) });
  }

  function stageActive() {
    const file = shell.selectedPath;
    if (!file || shell.writeBlocked || shell.pathPending(file)) return;
    if (selectedSubmodule) {
      void shell.runChange(shell.selectedSide === "staged" ? "unstage_path" : "stage_path", { file });
      return;
    }
    if (activeHunk == null) return;
    void shell.mutate({ action: "stageHunk", file, index: activeHunk, unstage: shell.selectedSide === "staged", ignoreSpace: shell.settings.ignoreSpace });
  }

  function discardActive() {
    const file = shell.selectedPath;
    if (!file || shell.writeBlocked || shell.pathPending(file) || activeHunk == null || shell.selectedSide === "staged") return;
    const index = activeHunk;
    const discard = shell.bindMutation({ action: "discardHunk", file, index, ignoreSpace: shell.settings.ignoreSpace });
    shell.ask(shell.tr("menu.discardHunk"), shell.tr("dialog.discardBody"), () => void discard(), [], { subject: file, danger: true });
  }

  function stageHovered() {
    const file = shell.selectedPath;
    if (!file || shell.writeBlocked || shell.pathPending(file)) return;
    if (selectedSubmodule) {
      void shell.runChange(shell.selectedSide === "staged" ? "unstage_path" : "stage_path", { file });
      hoveredHunk = null;
      return;
    }
    const index = hoveredHunk;
    const lineMode = useLineOps;
    const lineOps = hoveredLineOps;
    hoveredHunk = null;
    if (lineMode) {
      if (lineOps.length === 0) return;
      void shell.applyLineOps("stageLine", lineOps);
      return;
    }
    if (index == null) return;
    void shell.mutate({ action: "stageHunk", file, index, unstage: shell.selectedSide === "staged", ignoreSpace: shell.settings.ignoreSpace });
  }

  function discardHovered() {
    const file = shell.selectedPath;
    if (!file || shell.writeBlocked || shell.selectedSide === "staged") return;
    const index = hoveredHunk;
    const lineMode = useLineOps;
    const discardOps = hoveredDiscardOps;
    hoveredHunk = null;
    if (lineMode) {
      if (discardOps.length === 0) return;
      if (discardOps.length === 1) {
        void shell.applyLineOps("discardLine", discardOps);
        return;
      }
      const discard = shell.bindLineOps("discardLine", discardOps);
      shell.ask(shell.tr("menu.discardLines"), shell.tr("dialog.discardLinesBody", { count: String(discardOps.length) }), () => void discard(), [], { danger: true });
      return;
    }
    if (index == null) return;
    const discard = shell.bindMutation({ action: "discardHunk", file, index, ignoreSpace: shell.settings.ignoreSpace });
    shell.ask(shell.tr("menu.discardHunk"), shell.tr("dialog.discardBody"), () => void discard(), [], { subject: file, danger: true });
  }

  function stageLabel() {
    if (useLineOps) {
      const many = hoveredLineOps.length > 1;
      return shell.selectedSide === "staged"
        ? shell.tr(many ? "menu.unstageLines" : "menu.unstageLine")
        : shell.tr(many ? "menu.stageLines" : "menu.stageLine");
    }
    return shell.selectedSide === "staged" ? shell.tr("menu.unstageHunk") : shell.tr("menu.stage");
  }

  function submoduleActionLabel() {
    return shell.selectedSide === "staged" ? "menu.unstage" : "menu.stage";
  }

  function discardLabel() {
    if (useLineOps) return shell.tr(hoveredDiscardOps.length > 1 ? "menu.discardLines" : "menu.discardLine");
    return shell.tr("menu.discard");
  }
</script>

<section class="diff flex-fill">
          {#if shell.blameLines}
            <header class="diff-head" data-layer="blame">
              <span>{shell.tr("chrome.blameOf", { name: shell.selectedPath ?? "" })}</span>
              <button class="text-button" type="button" onclick={() => (shell.blameLines = null)}>{shell.tr("chrome.close")}</button>
            </header>
            <div class="diff-body" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} onscroll={onBlameScroll}>
              <div class="commit-window" style:height="{(shell.blameLines?.length ?? 0) * shell.diffRowHeight}px">
                {#each shell.blameWindow.rows as line, index (`${line.line}-${line.id}`)}
                  {@const pending = !line.truncated && (!line.id || /^0+$/.test(line.id))}
                  <button class="virtual-row blame-line" class:blame-pending={pending} type="button" style:top="{(shell.blameWindow.start + index) * shell.diffRowHeight}px" title={line.truncated ? shell.tr("chrome.truncated") : !line.id ? shell.tr("chrome.reading") : pending ? shell.tr("dialog.notCommitted") : `${line.author} · ${line.summary}`} onclick={() => { if (pending || line.truncated) return; shell.section = "history"; void shell.selectCommit(line.id); }}>
                    <span class="blame-gutter">{line.truncated ? "…" : line.line}</span>
                    <span class="blame-who">{line.truncated ? "" : !line.id ? shell.tr("chrome.reading") : pending ? shell.tr("dialog.notCommitted") : line.author}</span>
                    <span class="blame-code">{line.truncated ? shell.tr("chrome.truncated") : line.text}</span>
                  </button>
                {/each}
              </div>
            </div>
          {:else if (shell.mode === "live" || shell.mode === "sample") && shell.section === "changes" && shell.selectedPath && shell.selectedConflict}
            <header class="diff-head">
              <span>{shell.selectedPath}</span>
            </header>
            {#if shell.activeConflict}
              {@const view = shell.activeConflict}
              {@const tool = shell.mode === "sample" ? (view.binary ? "ExcelMergeFork" : "") : mergeToolLabel(shell)}
              {@const only = pickTheirs !== pickOurs ? (pickTheirs ? view.theirs.label : view.ours.label) : ""}
              {@const batch = conflictSelection.length > 1}
              <div class="conflict-pane">
                <div class="conflict-title">
                  <svg class="conflict-warn" viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M8 1.4 14.8 13.4H1.2L8 1.4z"/><path d="M8 6.1v3.3" stroke="#fff" stroke-width="1.4" stroke-linecap="round"/><circle cx="8" cy="11.4" r=".75" fill="#fff"/></svg>
                  <strong>{shell.tr("chrome.mergeConflict")}</strong>
                </div>
                <p class="conflict-hint">{shell.tr("chrome.mergeConflictHint")}</p>
                <div class="conflict-file">
                  <svg viewBox="0 0 16 16" aria-hidden="true"><path fill="none" stroke="currentColor" stroke-width="1.2" d="M3.2 1.5h6L12.8 5v9.5H3.2z"/><path fill="none" stroke="currentColor" stroke-width="1.2" d="M9.1 1.6V5.1h3.6"/></svg>
                  <span title={shell.selectedPath}>{shell.fileName(shell.selectedPath)}</span>
                </div>
                {#if batch}
                  <p class="conflict-selection" role="status">{shell.tr("chrome.conflictSelected", { count: String(conflictSelection.length) })}</p>
                {/if}
                <div class="conflict-cluster">
                <div class="conflict-row" class:pick-one={pickTheirs !== pickOurs}>
                  <div class="conflict-end left">{@render conflictCard(view.theirs, "theirs")}</div>
                  <div class="conflict-bridge">
                    <input id="conflict-theirs" type="checkbox" bind:checked={pickTheirs} aria-label={shell.tr("dialog.mergeKeepSide", { name: view.theirs.label })} title={shell.tr("dialog.mergeKeepSide", { name: view.theirs.label })} />
                    <span class="conflict-link" aria-hidden="true"></span>
                    <input id="conflict-ours" type="checkbox" bind:checked={pickOurs} aria-label={shell.tr("dialog.mergeKeepSide", { name: view.ours.label })} title={shell.tr("dialog.mergeKeepSide", { name: view.ours.label })} />
                  </div>
                  <div class="conflict-end right">{@render conflictCard(view.ours, "ours")}</div>
                </div>
                <div class="conflict-actions">
                  <button class="conflict-merge" type="button" disabled={(!pickOurs && !pickTheirs) || (shell.mode === "live" && shell.writeBlocked)} onclick={() => { if (batch && only && shell.mode === "live") void shell.resolvePicked(pickOurs ? "ours" : "theirs", conflictSelection); else void applyConflictChoice(shell, pickOurs, pickTheirs); }}>{only ? (batch ? shell.tr("menu.keepSideCount", { name: only, count: String(conflictSelection.length) }) : shell.tr("chrome.chooseSide", { name: only })) : pickOurs && pickTheirs ? shell.tr("chrome.mergeBoth") : shell.tr("chrome.mergeAction")}</button>
                  {#if tool && pickOurs && pickTheirs}
                    <span class="conflict-tool-group">
                      <button class="conflict-tool" type="button" disabled={shell.mode === "live" && shell.writeBlocked} onclick={() => { if (shell.mode === "live") void openConflictTool(shell); }}>
                        {shell.tr("chrome.mergeInTool", { name: tool })}
                      </button>
                      <button class="conflict-tool conflict-caret" type="button" disabled={shell.mode === "live" && shell.writeBlocked} aria-label={shell.tr("menu.externalMerge")} title={shell.tr("menu.externalMerge")} onclick={(event) => shell.openExternalToolMenu(event, "merge", shell.selectedPath ?? "")}>
                        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4.5 6.25 8 9.75l3.5-3.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
                      </button>
                    </span>
                  {/if}
                  {#if shell.mode === "live" && tool && mergeToolMissing(shell, shell.selectedPath)}
                    <p class="conflict-note">{shell.tr("chrome.toolPathMissing", { name: tool })}</p>
                  {/if}
                </div>
                </div>
                {#if !view.binary && conflictPreview && conflictPreviewPieces.length > 0}
                  <section class="conflict-preview" aria-label={shell.tr("chrome.conflictPreview")}>
                    <div class="conflict-preview-head">
                      <div>
                        <strong>{shell.tr("chrome.conflictPreview")}</strong>
                        <span class="conflict-preview-hint">{shell.tr("chrome.conflictPreviewHint")}</span>
                      </div>
                      {#if conflictPreviewHunks.length > 0}
                        <div class="conflict-preview-nav" role="group" aria-label={shell.tr("chrome.conflictBlocks")}>
                          <span>{shell.tr("chrome.conflictBlock", { current: String(activeConflictPreview + 1), count: String(conflictPreviewHunks.length) })}</span>
                          <button type="button" aria-label={shell.tr("menu.prevHunk")} title={shell.tr("menu.prevHunk")} onclick={() => stepConflictPreview(-1)}>‹</button>
                          <button type="button" aria-label={shell.tr("menu.nextHunk")} title={shell.tr("menu.nextHunk")} onclick={() => stepConflictPreview(1)}>›</button>
                        </div>
                      {/if}
                    </div>
                    <div class="conflict-preview-body">
                      {#each conflictPreviewPieces as piece, index (index)}
                        {#if piece.kind === "text"}
                          {#if piece.text.length > 0}
                            <pre class="conflict-preview-context">{piece.text}</pre>
                          {/if}
                        {:else}
                          {@const hunkAt = conflictPreviewHunks.indexOf(index)}
                          <article class="conflict-preview-block" class:active={hunkAt === activeConflictPreview} data-conflict-index={index}>
                            <header class="conflict-preview-block-head">
                              <span class="conflict-preview-number">{shell.tr("chrome.conflictBlock", { current: String(hunkAt + 1), count: String(conflictPreviewHunks.length) })}</span>
                              <span class="conflict-preview-label">{view.theirs.label} ↔ {view.ours.label}</span>
                              <button class="text-button" type="button" onclick={openConflictPreviewMerge}>{shell.tr("chrome.conflictOpenMerge")}</button>
                            </header>
                            <div class="conflict-preview-columns">
                              <div class="conflict-preview-side theirs" class:selected={conflictPreviewPicks[index] === "theirs" || conflictPreviewPicks[index] === "both"}>
                                <div class="conflict-preview-side-head">
                                  <span>{view.theirs.label}</span>
                                  <button type="button" onclick={() => pickConflictPreview(index, "theirs")}>{shell.tr("dialog.useTheirs")}</button>
                                </div>
                                <pre>{piece.theirs || shell.tr("dialog.mergeEmpty")}</pre>
                              </div>
                              <div class="conflict-preview-side ours" class:selected={conflictPreviewPicks[index] === "ours" || conflictPreviewPicks[index] === "both"}>
                                <div class="conflict-preview-side-head">
                                  <span>{view.ours.label}</span>
                                  <button type="button" onclick={() => pickConflictPreview(index, "ours")}>{shell.tr("dialog.useOurs")}</button>
                                </div>
                                <pre>{piece.ours || shell.tr("dialog.mergeEmpty")}</pre>
                              </div>
                            </div>
                            <div class="conflict-preview-block-actions">
                              <button type="button" class:active={conflictPreviewPicks[index] === "both"} onclick={() => pickConflictPreview(index, "both")}>{shell.tr("dialog.useBoth")}</button>
                              <span>{conflictPreviewPicks[index] && conflictPreviewPicks[index] !== "none" ? shell.tr("chrome.conflictChoiceSet") : shell.tr("chrome.conflictChoicePending")}</span>
                            </div>
                          </article>
                        {/if}
                      {/each}
                    </div>
                    <div class="conflict-preview-foot">
                      <p class="conflict-preview-readonly">{shell.tr("chrome.conflictPreviewReadOnly")}</p>
                      <div class="conflict-preview-actions">
                        <button class="text-button" type="button" onclick={openConflictPreviewMerge}>{shell.tr("chrome.conflictOpenMerge")}</button>
                        {#if shell.mode === "live"}
                          <button class="conflict-merge" type="button" disabled={shell.writeBlocked || conflictPreviewUnresolved > 0} onclick={applyConflictPreview}>{shell.tr("chrome.conflictApplyBlocks", { count: String(conflictPreviewHunks.length) })}</button>
                        {/if}
                      </div>
                    </div>
                  </section>
                {/if}
              </div>
            {:else if shell.diffError}
              <p class="diff-empty">{shell.diffError}</p>
            {:else}
              <p class="diff-empty">{shell.tr("chrome.reading")}</p>
            {/if}
          {:else if shell.mode === "live" && shell.section === "changes" && shell.selectedPath}
            {@const submodule = selectedSubmodule}
            <header class="diff-head">
              <span title={shell.selectedPath}>{shell.selectedPath}</span>
              {#if !submodule}
                <button class="pane-chip" type="button" disabled={shell.writeBlocked} title={shell.tr("menu.externalDiff")} onclick={(event) => shell.openExternalToolMenu(event, "diff", shell.selectedPath ?? "")}>{shell.tr("chrome.externalDiff")}</button>
              {/if}
              {#if submodule}
                <button class="pane-chip" data-submodule-stage type="button" disabled={shell.stageDisabled || shell.pathPending(shell.selectedPath)} title={shell.tr(submoduleActionLabel())} onclick={stageActive}>{shell.tr(submoduleActionLabel())}</button>
              {:else if hunkList.length > 0}
                <div class="hunk-tools" role="group" aria-label={shell.tr("chrome.hunkActions")}>
                  <button class="hunk-step" type="button" disabled={shell.writeBlocked} aria-label={shell.tr("menu.prevHunk")} title={shell.tr("menu.prevHunk")} onclick={() => stepHunk(-1)}>‹</button>
                  <span class="hunk-pos" aria-live="polite">{activeAt >= 0 ? activeAt + 1 : "–"}/{hunkList.length}</span>
                  <button class="hunk-step" type="button" disabled={shell.writeBlocked} aria-label={shell.tr("menu.nextHunk")} title={shell.tr("menu.nextHunk")} onclick={() => stepHunk(1)}>›</button>
                  <button class="pane-chip" type="button" disabled={shell.writeBlocked || activeHunk == null} title={shell.tr(shell.selectedSide === "staged" ? "menu.unstageHunk" : "menu.stageHunk")} onclick={stageActive}>{shell.tr(shell.selectedSide === "staged" ? "menu.unstageHunk" : "menu.stageHunk")}</button>
                  {#if shell.selectedSide === "unstaged"}
                    <button class="pane-chip danger" type="button" disabled={shell.writeBlocked || activeHunk == null} title={shell.tr("menu.discardHunk")} onclick={discardActive}>{shell.tr("menu.discardHunk")}</button>
                  {/if}
                </div>
              {/if}
              {#if !submodule && shell.discardableSelected.length > 1}
                <button class="pane-chip danger" type="button" disabled={shell.writeBlocked || !!shell.selectedPath && shell.pathPending(shell.selectedPath)} onclick={() => { const discard = shell.bindLineOps("discardLine", shell.discardableSelected); shell.ask(shell.tr("menu.discardLines"), shell.tr("dialog.discardLinesBody", { count: String(shell.discardableSelected.length) }), () => void discard(), [], { danger: true }); }}>{shell.tr("menu.discardLines")}</button>
              {/if}
            </header>
            {#if !submodule}
              {@render diffToolbar()}
            {/if}
            {#if shell.diff && shell.diff.lines.length > 0 && !submodule}
              <div class="minimap" role="presentation">
                {#each shell.minimapMarks as mark (`m-${mark.index}`)}
                  <button class="mark {mark.kind}" type="button" aria-label={`${shell.tr("menu.nextHunk")} ${mark.index + 1}`} title={`${shell.tr("menu.nextHunk")} ${mark.index + 1}`} style:left="{mark.top}%" onclick={() => goMark(mark.index)}></button>
                {/each}
              </div>
            {/if}
            {#if shell.pendingStage(shell.selectedPath) !== undefined}
              <p class="diff-empty" data-stage-pending aria-live="polite">{shell.tr(shell.pendingStage(shell.selectedPath) ? "chrome.unstagePending" : "chrome.stagePending")}</p>
            {:else if submodule}
              <div class="submodule-diff" data-submodule-diff>
                <div class="submodule-content">
                  <div class="submodule-scroll">
                  <p class="submodule-diff-path" title={submodule.path}>{submodule.path}</p>
                  <div class="submodule-diff-title">
                    <svg viewBox="0 0 16 16" aria-hidden="true"><path fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" d="M3 3.5h10M3 8h10M3 12.5h6"/><circle cx="12.2" cy="12.5" r="1.3" fill="currentColor"/></svg>
                    <strong>{shell.tr("chrome.submoduleChangedTitle", { name: submoduleName })}</strong>
                  </div>
                  {#if submoduleUncommittedCount > 0}
                    <p class="submodule-uncommitted" data-submodule-uncommitted>
                      <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="5.6" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M8 4.9v3.4l2.1 1.3" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
                      {shell.tr(submoduleUncommittedCount === 1 ? "chrome.submoduleUncommittedOne" : "chrome.submoduleUncommittedMany", { count: String(submoduleUncommittedCount) })}
                    </p>
                  {/if}
                  {#if !submodule.ready}
                    <p class="submodule-diff-hint">{shell.tr("chrome.submoduleNotReady")}</p>
                  {:else if submoduleDetailsError}
                    <p class="submodule-diff-hint">{submoduleDetailsError}</p>
                  {:else if submoduleDetailsLoading}
                    <p class="submodule-diff-hint">{shell.tr("chrome.submoduleHistoryLoading")}</p>
                  {:else if submoduleHistory.length === 0}
                    <p class="submodule-diff-hint">{shell.tr("chrome.submoduleHistoryEmpty")}</p>
                  {:else}
                    <div class="submodule-history" aria-label={shell.tr("chrome.submoduleHistory")}>
                      {#each submoduleHistory as commit (commit.id)}
                        <button class="submodule-commit" class:selected={commit.id === submoduleSelectedCommit?.id} data-submodule-commit type="button" aria-pressed={commit.id === submoduleSelectedCommit?.id} onclick={() => (submoduleSelectedCommitId = commit.id)}>
                          <span class="submodule-commit-avatar" style:background={shell.avatarColor(commit.author)}>{(commit.author || "?").slice(0, 1).toUpperCase()}</span>
                          <span class="submodule-commit-copy">
                            <strong class="submodule-commit-subject" title={commit.summary}>{commit.summary}</strong>
                            <span class="submodule-commit-author" title={commit.author}>{commit.author}</span>
                          </span>
                          <span class="submodule-commit-meta">
                            <code title={commit.id}>{commit.shortId}</code>
                            <time datetime={commit.at ? new Date(commit.at * 1000).toISOString() : undefined}>{commit.at ? shell.forkWhen(commit.at) : commit.when}</time>
                          </span>
                        </button>
                      {/each}
                    </div>
                    {#if submoduleSelectedCommit}
                      <div class="submodule-commit-detail" data-submodule-commit-detail>
                        <span class="submodule-commit-marker" aria-hidden="true"></span>
                        <strong title={submoduleSelectedCommit.summary}>{submoduleSelectedCommit.summary}</strong>
                        <span class="submodule-commit-detail-meta">
                          <span>{submoduleSelectedCommit.author}</span>
                          <button class="submodule-sha" type="button" title={submoduleSelectedCommit.id} onclick={() => shell.copyText(submoduleSelectedCommit.id)}>{submoduleSelectedCommit.shortId}</button>
                          <time>{submoduleSelectedCommit.at ? shell.forkWhen(submoduleSelectedCommit.at) : submoduleSelectedCommit.when}</time>
                        </span>
                      </div>
                    {/if}
                  {/if}
                  </div>
                  <div class="submodule-dock">
                  <div class="submodule-range">
                    <div data-submodule-old><span>{shell.tr("chrome.submoduleOld")}</span><code title={submoduleCommits.oldId}>{submoduleCommits.oldId || "—"}</code></div>
                    <span class="submodule-range-arrow" aria-hidden="true">→</span>
                    <div data-submodule-new><span>{shell.tr("chrome.submoduleNew")}</span><code title={submoduleCommits.newId}>{submoduleCommits.newId || "—"}</code></div>
                  </div>
                  <div class="submodule-footer">
                    <span class="submodule-footer-hint">{shell.tr(submodule.ready ? "chrome.submoduleChangeHint" : "chrome.submoduleNotReady")}</span>
                    <button class="pane-chip submodule-open" data-submodule-open type="button" disabled={!submodule.ready} title={shell.tr("chrome.openSubmodule")} onclick={() => void shell.openRepo(shell.fullPath(submodule.path))}>{shell.tr("chrome.openSubmodule")}</button>
                  </div>
                  </div>
                </div>
              </div>
            {:else if shell.diffError}
              <p class="diff-empty">{shell.diffError}</p>
            {:else if !shell.diff}
              <p class="diff-empty">{shell.tr("chrome.reading")}</p>
            {:else if shell.preview?.mime === "application/pdf"}
              <embed class="pdf-view" title={shell.selectedPath} src={shell.preview.dataUrl} type="application/pdf" />
            {:else if shell.diff.binary}
              {#if shell.imageOld || shell.imageNew}
                {@render imageCompare(shell.imageOld, shell.imageNew, shell.diff.oldBytes, shell.diff.newBytes)}
              {:else}
                <BinaryDiff path={shell.selectedPath ?? ""} oldBytes={shell.diff.oldBytes} newBytes={shell.diff.newBytes} />
              {/if}
            {:else if shell.diff.loadable && shell.diff.lines.length === 0}
              <div class="diff-empty diff-load">
                <p>{shell.tr("chrome.largeFileHint")}</p>
                {#if shell.diff.oldBytes != null || shell.diff.newBytes != null}
                  <p class="diff-sizes">
                    {#if shell.diff.oldBytes != null}<span>{shell.tr("chrome.binaryOld")} {formatBytes(shell.diff.oldBytes, shell.activeLocale)}</span>{/if}
                    {#if shell.diff.oldBytes != null && shell.diff.newBytes != null}<span aria-hidden="true">→</span>{/if}
                    {#if shell.diff.newBytes != null}<span>{shell.tr("chrome.binaryNew")} {formatBytes(shell.diff.newBytes, shell.activeLocale)}</span>{/if}
                  </p>
                {/if}
                <button class="pane-chip" type="button" disabled={shell.writeBlocked} onclick={() => void shell.loadDiff(true)}>{shell.tr("chrome.loadDiff")}</button>
              </div>
            {:else if shell.diff.lines.length === 0}
              <p class="diff-empty">{shell.tr("chrome.noChanges")}</p>
            {:else if shell.settings.diffStyle === "split"}
              <div class="diff-viewport">
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="diff-body split pick-{shell.splitPick}" class:virtual={!shell.settings.diffWordWrap} class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} style:--split-row="{shell.splitRowHeight}px" role="presentation" bind:this={shell.diffEl} bind:clientHeight={shell.diffHeight} onmousedown={shell.pickSplitSide.bind(shell)} onscroll={onDiffScroll} onmousemove={hoverDiffLine} onmouseleave={clearHunkHover} use:splitScroller={shell.splitColumns}>
                {#if shell.settings.diffWordWrap}
                  <WrappedRows items={shell.splitRows} rowHeight={shell.splitRowHeight} split bind:this={wrapped}>
                    {#snippet children(row, index)}{@render liveSplitRow(row, index)}{/snippet}
                  </WrappedRows>
                {:else}
                {#if shell.splitWindow.beforePx > 0}<div class="split-gap" style:height="{shell.splitWindow.beforePx}px"></div>{/if}
                {#each shell.splitWindow.rows as row, offset (shell.splitWindow.start + offset)}
                  {@render liveSplitRow(row, shell.splitWindow.start + offset)}
                {/each}
                {#if shell.splitWindow.afterPx > 0}<div class="split-gap" style:height="{shell.splitWindow.afterPx}px"></div>{/if}
                {/if}
                {#if shell.diff.truncated}<span class="meta split-meta">{shell.tr("chrome.truncated")}</span>{/if}
                {@render splitBar()}
              </div>
              {@render hunkActions()}
              </div>
            {:else}
              <div class="diff-viewport">
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="diff-body" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} bind:this={shell.diffEl} bind:clientHeight={shell.diffHeight} onscroll={onDiffScroll} onmousemove={hoverDiffLine} onmouseleave={clearHunkHover}>
                {#if shell.settings.diffWordWrap}
                  <WrappedRows items={shell.diffLines} rowHeight={shell.diffRowHeight} bind:this={wrapped}>
                    {#snippet children(line, at)}{@render liveUnifiedRow(line, at)}{/snippet}
                  </WrappedRows>
                {:else}
                <div class="commit-window" style:height="{shell.diffWindow.height}px">
                  {#each shell.diffWindow.rows as line, index (`d${shell.diffWindow.start + index}`)}
                    {@render liveUnifiedRow(line, shell.diffWindow.start + index)}
                  {/each}
                </div>
                {/if}
                {#if shell.diff.truncated}<span class="meta">{shell.tr("chrome.truncated")}</span>{/if}
              </div>
              {@render hunkActions()}
              </div>
            {/if}
          {:else if shell.section === "changes" && shell.selectedPath && shell.selectedDiff.length > 0}
            <header class="diff-head">
              <span>{shell.selectedPath}</span>
            </header>
            {@render diffToolbar()}
            {#if shell.settings.diffStyle === "split"}
              <div class="diff-body split pick-{shell.splitPick}" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} role="presentation" onmousedown={shell.pickSplitSide.bind(shell)} use:splitScroller={shell.splitColumnsOf(shell.sampleSplit)}>
                {#each shell.sampleSplit as row, index (index)}
                  {#if row.meta}
                    <div class="split-meta" data-line={row.lineIndex}>{row.left}</div>
                  {:else}
                    <div class="split-row">
                      {@render splitCell(row, "left", shell.selectedPath ?? "", `sl${index}`)}
                      {@render splitCell(row, "right", shell.selectedPath ?? "", `sr${index}`)}
                    </div>
                  {/if}
                {/each}
                {@render splitBar()}
              </div>
            {:else}
              <pre class="diff-body" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined}>{#each shell.selectedDiff as line, index (index)}<span class:add={line.startsWith("+") && !line.startsWith("+++")} class:del={line.startsWith("-") && !line.startsWith("---")} class:hunk={line.startsWith("@@")}>{line + "\n"}</span>{/each}</pre>
            {/if}
          {:else if shell.selectedPath === "art/mark.png"}
            {@render imageCompare(sampleImages.before, sampleImages.after)}
          {:else}
            <p class="diff-empty">{shell.tr("chrome.selectFile")}</p>
          {/if}
        </section>
{#snippet liveSplitRow(row: SplitRow, index: number)}
  {@const leftAt = shell.splitLineAt(row, "left")}
  {@const rightAt = shell.splitLineAt(row, "right")}
  {@const rowLit = litOn(leftAt) || litOn(rightAt)}
  {#if row.meta}
    <div class="split-meta" class:hunk-hover={rowLit} class:hunk-top={rowLit && (leftAt === litStart || rightAt === litStart)} class:hunk-bottom={rowLit && (leftAt === litEnd || rightAt === litEnd)} data-line={row.lineIndex}>{row.left}</div>
  {:else}
    <div class="split-row" class:hunk-hover={rowLit} class:hunk-top={rowLit && (leftAt === litStart || rightAt === litStart)} class:hunk-bottom={rowLit && (leftAt === litEnd || rightAt === litEnd)}>
      {@render splitCell(row, "left", shell.selectedPath ?? "", `l${index}`, leftAt)}
      {@render splitCell(row, "right", shell.selectedPath ?? "", `r${index}`, rightAt)}
    </div>
  {/if}
{/snippet}

{#snippet liveUnifiedRow(line: DiffLine, at: number)}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span class="virtual-row {line.kind}" class:add={line.kind === "add"} class:del={line.kind === "delete"} class:hunk={line.kind === "hunk"} class:meta={line.kind === "meta"} class:picked={shell.diffPicked.has(at)} class:hunk-hover={litOn(at)} class:hunk-top={at === litStart} class:hunk-bottom={at === litEnd} data-line={at} style:top="{at * shell.diffRowHeight}px" onclick={(event) => shell.clickDiffLine(at, event)} oncontextmenu={(event) => shell.openMenu(event, shell.diffLineMenu(at))}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span class="gutter" onmousedown={(event) => shell.dragDiffLine(at, event)}>{shell.shownDiffLines[at]?.oldNo ?? ""}</span>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span class="gutter" onmousedown={(event) => shell.dragDiffLine(at, event)}>{shell.shownDiffLines[at]?.newNo ?? ""}</span>
    <span class="diff-line-text">{#each shell.paint(line.text, shell.selectedPath ?? "") as token, tokenIndex (`d${at}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
  </span>
{/snippet}

{#snippet splitCell(row: SplitRow, side: "left" | "right", file: string, key: string, lineIndex: number | null = null)}
    {@const kind = side === "left" ? row.leftKind : row.rightKind}
    {@const mark = side === "left" ? "delete" : "add"}
    {@const text = side === "left" ? row.left : row.right}
    {@const other = side === "left" ? row.right : row.left}
    {@const paired = Boolean(row.left && row.right && row.left !== row.right && (row.leftKind === "delete" || row.rightKind === "add"))}
    {@const picked = lineIndex != null && shell.diffPicked.has(lineIndex)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span class="split-no" class:del={kind === "delete"} class:add={kind === "add"} class:meta={kind === "meta"} class:picked data-line={lineIndex ?? undefined} onmousedown={(event) => lineIndex != null && shell.dragDiffLine(lineIndex, event)} oncontextmenu={(event) => lineIndex != null && shell.openMenu(event, shell.diffLineMenu(lineIndex))}>{(side === "left" ? row.leftNo : row.rightNo) ?? ""}</span>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span class="split-code" data-side={side} data-line={lineIndex ?? undefined} class:del={kind === "delete"} class:add={kind === "add"} class:meta={kind === "meta"} class:picked onclick={(event) => lineIndex != null && shell.clickDiffLine(lineIndex, event)} oncontextmenu={(event) => lineIndex != null && shell.openMenu(event, shell.diffLineMenu(lineIndex))}><span class="split-text">{#if shell.settings.showDiffMarks && kind === mark}{side === "left" ? "-" : "+"}{/if}{#each shell.markedLine(text, paired ? other : "", file) as token, tokenIndex (`${key}-${tokenIndex}`)}<span class={token.cls} class:intra={token.hot}>{token.text}</span>{/each}</span></span>
  {/snippet}

{#snippet splitBar(actions = false)}
    <span class="split-bar-pad"></span>
    <div class="split-bar" data-bar="left"><div></div></div>
    <span class="split-bar-pad"></span>
    <div class="split-bar" data-bar="right"><div></div></div>
    {#if actions}<span class="split-bar-pad"></span>{/if}
  {/snippet}

{#snippet hunkActions()}
  {#if hoveredRange && shell.mode === "live"}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="diff-hunk-actions" style:top="{hunkActionTop}px" onmouseenter={() => { if (hoveredRange) hoveredHunk = hoveredRange.index; }} onmouseleave={leaveHunkActions}>
      {#if !useLineOps || hoveredLineOps.length > 0}
        <button type="button" disabled={shell.writeBlocked} onclick={(event) => { event.stopPropagation(); stageHovered(); }}>{stageLabel()}</button>
      {/if}
      {#if shell.selectedSide === "unstaged" && (!useLineOps || hoveredDiscardOps.length > 0)}
        <button class="danger" type="button" disabled={shell.writeBlocked} onclick={(event) => { event.stopPropagation(); discardHovered(); }}>{discardLabel()}</button>
      {/if}
    </div>
  {/if}
{/snippet}

{#snippet conflictCard(party: ConflictParty, side: "ours" | "theirs")}
  {@const on = side === "ours" ? pickOurs : pickTheirs}
  <label class="conflict-card" class:on for="conflict-{side}">
    <svg class="branch" viewBox="0 0 16 16" aria-hidden="true"><path fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" d="M4 2.2v7.2a2.2 2.2 0 1 0 2.2 2.2"/><circle cx="4" cy="12.2" r="1.35"/><circle cx="12" cy="3.6" r="1.35"/><path fill="none" stroke="currentColor" stroke-width="1.3" d="M12 5a4.2 4.2 0 0 1-6 2.6"/></svg>
    <span class="name" title={party.label}>{party.label}</span>
    {#if party.commit?.short}<code class="conflict-commit" title={party.commit.subject}>{party.commit.short}</code>{/if}
    {#if sideText(party)}<span class="conflict-pill {party.state}">{sideText(party)}</span>{/if}
  </label>
{/snippet}

{#snippet imageCompare(before: string | null, after: string | null, oldBytes: number | null | undefined = null, newBytes: number | null | undefined = null)}
  <ImageComparison {before} {after} {oldBytes} {newBytes} mode={shell.imageMode} bind:position={shell.imagePos} pixelUrl={shell.pixelUrl} pixelStatus={shell.pixelStatus} onmodechange={(mode) => pickImageMode(mode, before, after)} />
{/snippet}

{#snippet diffToolbar()}
  <div class="diff-toolbar" data-diff-toolbar role="toolbar" aria-label={shell.tr("chrome.diffToolbar")}>
    <div class="diff-toolbar-group diff-toolbar-view" role="group" aria-label={shell.tr("chrome.diffView")}>
      <button class:active={shell.settings.diffStyle === "split"} type="button" title={shell.tr("dialog.diffSplit")} aria-label={shell.tr("dialog.diffSplit")} onclick={() => shell.setDiffStyle("split")}>↔</button>
      <button class:active={shell.settings.diffStyle !== "split"} type="button" title={shell.tr("dialog.diffUnified")} aria-label={shell.tr("dialog.diffUnified")} onclick={() => shell.setDiffStyle("unified")}>≡</button>
    </div>
    <div class="diff-toolbar-group diff-toolbar-context" role="group" aria-label={shell.tr("dialog.diffContext")}>
      {#if !shell.settings.showEntireFile}
        <span class="diff-toolbar-label">{shell.tr("dialog.diffContext")}</span>
        <button type="button" aria-label={shell.tr("chrome.diffContextDecrease")} title={shell.tr("chrome.diffContextDecrease")} onclick={() => shell.setDiffContext(shell.settings.diffContext - 1)}>−</button>
        <span class="diff-toolbar-value">{shell.settings.diffContext}</span>
        <button type="button" aria-label={shell.tr("chrome.diffContextIncrease")} title={shell.tr("chrome.diffContextIncrease")} onclick={() => shell.setDiffContext(shell.settings.diffContext + 1)}>+</button>
      {/if}
      <button class:active={shell.settings.showEntireFile} type="button" title={shell.tr("chrome.diffShowWhole")} aria-pressed={shell.settings.showEntireFile} onclick={() => shell.toggleShowEntireFile()}>{shell.tr("chrome.diffShowWhole")}</button>
    </div>
    <button class:active={shell.settings.diffWordWrap} type="button" title={shell.tr("dialog.wordWrap")} aria-pressed={shell.settings.diffWordWrap} onclick={() => shell.toggleDiffWordWrap()}>{shell.tr("dialog.wordWrap")}</button>
    <button class:active={shell.settings.ignoreSpace} type="button" title={shell.tr("dialog.ignoreSpace")} aria-pressed={shell.settings.ignoreSpace} onclick={() => shell.toggleIgnoreSpace()}>{shell.tr("dialog.ignoreSpace")}</button>
  </div>
{/snippet}
