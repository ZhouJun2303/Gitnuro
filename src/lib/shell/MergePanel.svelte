<script lang="ts">
  import { getShell } from "./context";
  import "./MergePanel.css";
  import type { ConflictParty } from "../status";
  import type { MergePiece } from "./types";
  import { hunkIncludes } from "./dialogs.svelte";

  const shell = getShell();
  let fileLeft = $state(false);
  let fileRight = $state(false);
  let openFor = "";
  let panel = $state<HTMLElement | null>(null);
  let hoverHunk = $state<number | null>(null);
  let hoverSide = $state<"theirs" | "ours" | null>(null);
  let activeConflict = $state(0);
  let leftPane = $state<HTMLElement | null>(null);
  let rightPane = $state<HTMLElement | null>(null);
  let resultPane = $state<HTMLElement | null>(null);
  let syncing = false;
  let editingResult = $state(false);
  let resultDirty = $state(false);

  $effect(() => {
    if (!shell.textMerge) {
      openFor = "";
      return;
    }
    if (shell.mergeFile === openFor) return;
    openFor = shell.mergeFile;
    fileLeft = false;
    fileRight = false;
    hoverHunk = null;
    hoverSide = null;
    activeConflict = 0;
    editingResult = false;
    resultDirty = false;
  });

  $effect(() => {
    if (shell.textMerge && panel) panel.focus();
  });

  const hunkIndexes = $derived(
    shell.mergePieces
      .map((piece, index) => (piece.kind === "hunk" ? index : -1))
      .filter((index) => index >= 0),
  );
  const conflictTotal = $derived(hunkIndexes.length);
  const conflictResolved = $derived(
    hunkIndexes.filter((index) => {
      const piece = shell.mergePieces[index];
      return piece?.kind === "hunk" && piece.pick !== "none";
    }).length,
  );
  const textLeft = $derived(
    conflictTotal > 0 && shell.mergePieces.every((piece) => piece.kind !== "hunk" || hunkIncludes(piece.pick, "theirs")),
  );
  const textRight = $derived(
    conflictTotal > 0 && shell.mergePieces.every((piece) => piece.kind !== "hunk" || hunkIncludes(piece.pick, "ours")),
  );
  const leftOn = $derived(shell.mergeKind === "file" ? fileLeft : textLeft);
  const rightOn = $derived(shell.mergeKind === "file" ? fileRight : textRight);
  const fileSide = $derived<"ours" | "theirs" | "">(fileLeft ? "theirs" : fileRight ? "ours" : "");
  const view = $derived(shell.activeConflict);
  const unresolved = $derived(Math.max(0, conflictTotal - conflictResolved));
  const canResolve = $derived(shell.mergeKind === "file" ? fileSide !== "" : unresolved === 0);

  function trapTab(event: KeyboardEvent) {
    if (event.key !== "Tab" || !panel) return;
    const nodes = [...panel.querySelectorAll<HTMLElement>('a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])')].filter((node) => node.offsetParent !== null);
    if (nodes.length === 0) return;
    const first = nodes[0];
    const last = nodes[nodes.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function withDirtyGuard(run: () => void) {
    if (!resultDirty) {
      run();
      return;
    }
    shell.ask(shell.tr("dialog.mergeOverwriteEdits"), shell.tr("dialog.mergeOverwriteEditsBody"), () => {
      resultDirty = false;
      editingResult = false;
      run();
    });
  }

  function onLeft(event: Event) {
    event.preventDefault();
    if (shell.mergeKind === "file") {
      fileLeft = !fileLeft;
      if (fileLeft) fileRight = false;
      return;
    }
    withDirtyGuard(() => shell.takeMergeSide("theirs", !leftOn));
  }

  function onRight(event: Event) {
    event.preventDefault();
    if (shell.mergeKind === "file") {
      fileRight = !fileRight;
      if (fileRight) fileLeft = false;
      return;
    }
    withDirtyGuard(() => shell.takeMergeSide("ours", !rightOn));
  }

  function show(text: string) {
    return text.trim() ? text : shell.tr("dialog.mergeEmpty");
  }

  function sideOn(piece: Extract<MergePiece, { kind: "hunk" }>, side: "theirs" | "ours") {
    return hunkIncludes(piece.pick, side);
  }

  function syncScroll(source: "left" | "right", event: Event) {
    if (syncing) return;
    const target = event.currentTarget;
    if (!(target instanceof HTMLElement)) return;
    const other = source === "left" ? rightPane : leftPane;
    if (!other) return;
    syncing = true;
    other.scrollTop = target.scrollTop;
    other.scrollLeft = target.scrollLeft;
    queueMicrotask(() => {
      syncing = false;
    });
  }

  function jumpConflict(delta: number) {
    if (conflictTotal === 0) return;
    const next = (activeConflict + delta + conflictTotal) % conflictTotal;
    activeConflict = next;
    const index = hunkIndexes[next];
    if (index == null) return;
    leftPane?.querySelector<HTMLElement>(`[data-merge-hunk="${index}"]`)?.scrollIntoView({ block: "nearest" });
    rightPane?.querySelector<HTMLElement>(`[data-merge-hunk="${index}"]`)?.scrollIntoView({ block: "nearest" });
    resultPane?.querySelector<HTMLElement>(`[data-merge-result="${index}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function formatBytes(bytes: number) {
    const units = ["B", "KB", "MB", "GB"];
    let value = Math.max(0, bytes);
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
      value /= 1024;
      unit += 1;
    }
    const shown = new Intl.NumberFormat(shell.activeLocale === "zh" ? "zh-CN" : "en-US", {
      maximumFractionDigits: unit === 0 ? 0 : 1,
    }).format(value);
    return `${shown} ${units[unit]}`;
  }

  function fileKind(path: string) {
    const name = path.split(/[\\/]/).pop() ?? "";
    const ext = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1).toLowerCase() : "";
    if (ext === "xls" || ext === "xlsx" || ext === "xlsm" || ext === "csv" || ext === "ods") return "sheet";
    if (ext === "png" || ext === "jpg" || ext === "jpeg" || ext === "gif" || ext === "webp" || ext === "bmp" || ext === "svg" || ext === "ico") return "image";
    return "doc";
  }

  async function resolve() {
    if (!canResolve) return;
    if (shell.mergeKind === "file") {
      if (fileSide !== "ours" && fileSide !== "theirs") return;
      await shell.applyFileMerge(fileSide);
      return;
    }
    await shell.applyTextMerge();
  }
</script>

{#if shell.textMerge}
  <div class="scrim merge-scrim" role="presentation">
    <div class="merge-panel" data-layer="text-merge" bind:this={panel} role="dialog" aria-modal="true" aria-label={shell.tr("dialog.mergeTitle")} tabindex="-1" onkeydown={(event) => { if (event.key === "Escape") { event.preventDefault(); shell.closeTextMerge(); return; } trapTab(event); }}>
      <header class="merge-panel-title">
        <strong>{shell.tr("dialog.mergeTitle")}</strong>
        <span class="merge-title-file">
          {@render mark(fileKind(shell.mergeFile), 16)}
          <span>{shell.mergeFile}</span>
        </span>
        {#if shell.mergeKind === "text" && conflictTotal > 0}
          <div class="merge-nav">
            <span class="merge-count" class:done={conflictResolved === conflictTotal}>{conflictResolved}/{conflictTotal}</span>
            <button class="merge-nav-btn" type="button" aria-label={shell.tr("menu.prevHunk")} onclick={() => jumpConflict(-1)}>‹</button>
            <button class="merge-nav-btn" type="button" aria-label={shell.tr("menu.nextHunk")} onclick={() => jumpConflict(1)}>›</button>
          </div>
        {/if}
        <button class="merge-close" type="button" aria-label={shell.tr("chrome.close")} onclick={() => shell.closeTextMerge()}>×</button>
      </header>
      <div class="merge-head">
        <div class="merge-parties">
          <label class="merge-party">
            <input type="checkbox" checked={leftOn} aria-label={shell.tr("dialog.mergeKeepSide", { name: shell.mergeLeft })} onclick={onLeft} />
            <span>{shell.tr("dialog.mergeKeepSide", { name: shell.mergeLeft })}</span>
          </label>
          <label class="merge-party right">
            <input type="checkbox" checked={rightOn} aria-label={shell.tr("dialog.mergeKeepSide", { name: shell.mergeRight })} onclick={onRight} />
            <span>{shell.tr("dialog.mergeKeepSide", { name: shell.mergeRight })}</span>
          </label>
        </div>
        {#if shell.mergeKind === "text"}
          <p class="merge-overview-hint">{shell.tr("dialog.mergeOverviewHint")}</p>
        {/if}
      </div>
      {#if shell.mergeKind === "file"}
        <div class="merge-stage files">
          {@render fileCard(view?.theirs)}
          {@render fileCard(view?.ours)}
        </div>
      {:else}
        <div class="merge-top">
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="merge-side left" bind:this={leftPane} onscroll={(event) => syncScroll("left", event)}>
            {#each shell.mergePieces as piece, index (index)}
              {#if piece.kind === "text"}
                <pre class="merge-cell context">{piece.text}</pre>
              {:else}
                {@const on = sideOn(piece, "theirs")}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="merge-cell hunk"
                  class:on
                  class:hover={hoverHunk === index && hoverSide === "theirs"}
                  data-merge-hunk={index}
                  role="group"
                  onmouseenter={() => { hoverHunk = index; hoverSide = "theirs"; }}
                  onmouseleave={() => { if (hoverHunk === index && hoverSide === "theirs") { hoverHunk = null; hoverSide = null; } }}
                >
                  <pre class:empty={!piece.theirs.trim()}>{show(piece.theirs)}</pre>
                  {#if hoverHunk === index && hoverSide === "theirs"}
                    <button
                      class="merge-hover-btn"
                      class:remove={on}
                      type="button"
                      onclick={(event) => { event.stopPropagation(); withDirtyGuard(() => { shell.toggleMergeSide(index, "theirs"); activeConflict = Math.max(0, hunkIndexes.indexOf(index)); }); }}
                    >{shell.tr(on ? "dialog.removeLeft" : "dialog.selectLeft")}</button>
                  {/if}
                </div>
              {/if}
            {/each}
          </div>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="merge-side right" bind:this={rightPane} onscroll={(event) => syncScroll("right", event)}>
            {#each shell.mergePieces as piece, index (index)}
              {#if piece.kind === "text"}
                <pre class="merge-cell context">{piece.text}</pre>
              {:else}
                {@const on = sideOn(piece, "ours")}
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="merge-cell hunk"
                  class:on
                  class:hover={hoverHunk === index && hoverSide === "ours"}
                  data-merge-hunk={index}
                  role="group"
                  onmouseenter={() => { hoverHunk = index; hoverSide = "ours"; }}
                  onmouseleave={() => { if (hoverHunk === index && hoverSide === "ours") { hoverHunk = null; hoverSide = null; } }}
                >
                  <pre class:empty={!piece.ours.trim()}>{show(piece.ours)}</pre>
                  {#if hoverHunk === index && hoverSide === "ours"}
                    <button
                      class="merge-hover-btn"
                      class:remove={on}
                      type="button"
                      onclick={(event) => { event.stopPropagation(); withDirtyGuard(() => { shell.toggleMergeSide(index, "ours"); activeConflict = Math.max(0, hunkIndexes.indexOf(index)); }); }}
                    >{shell.tr(on ? "dialog.removeRight" : "dialog.selectRight")}</button>
                  {/if}
                </div>
              {/if}
            {/each}
          </div>
        </div>
        <div class="merge-result-pane">
          <div class="merge-result-head">
            <span>{shell.tr("dialog.result")}</span>
            {#if editingResult}<span class="merge-result-mode">{shell.tr("dialog.editResult")}</span>{/if}
          </div>
          {#if editingResult}
            <textarea class="merge-result-edit" aria-label={shell.tr("dialog.result")} spellcheck="false" bind:value={shell.mergeResult} oninput={() => (resultDirty = true)}></textarea>
          {:else}
            <div class="merge-result-scroll" bind:this={resultPane}>
              {#each shell.mergePieces as piece, index (index)}
                {#if piece.kind === "text"}
                  <pre class="merge-result-text">{piece.text}</pre>
                {:else if piece.pick === "none"}
                  <div class="merge-conflict-mark" data-merge-result={index}>{shell.tr("dialog.mergeConflictMark")}</div>
                {:else}
                  <pre class="merge-result-hunk pick-{piece.pick}" data-merge-result={index}>{shell.chosenHunkText(piece) || shell.tr("dialog.mergeEmpty")}</pre>
                {/if}
              {/each}
            </div>
          {/if}
        </div>
      {/if}
      <footer class="merge-panel-foot">
        {#if shell.mergeKind === "text"}
          {#if unresolved > 0}
            <span class="merge-edit-hint merge-unresolved" role="status">{shell.tr("dialog.mergeUnresolved", { count: String(unresolved) })}</span>
          {:else}
            <span class="merge-edit-hint">{shell.tr("dialog.editingAllowed")}</span>
          {/if}
        {/if}
        {#if shell.mergeKind === "text"}
          <button
            class="text-button merge-edit-entry"
            type="button"
            aria-pressed={editingResult}
            onclick={() => {
              editingResult = !editingResult;
              if (editingResult) queueMicrotask(() => panel?.querySelector<HTMLTextAreaElement>(".merge-result-edit")?.focus());
            }}
          >{editingResult ? shell.tr("dialog.previewResult") : shell.tr("dialog.editResult")}</button>
        {/if}
        <button class="commit" type="button" disabled={shell.busy || !canResolve} title={!canResolve ? shell.tr(shell.mergeKind === "file" ? "dialog.mergePickFile" : "dialog.mergeUnresolved", shell.mergeKind === "file" ? {} : { count: String(unresolved) }) : undefined} onclick={() => void resolve()}>{shell.tr("dialog.mergeResolve")}</button>
        <button class="text-button" type="button" onclick={() => shell.closeTextMerge()}>{shell.tr("chrome.cancel")}</button>
      </footer>
    </div>
  </div>
{/if}

{#snippet fileCard(party: ConflictParty | undefined)}
  <div class="merge-file-slot">
    {#if party?.state === "deleted"}
      <p class="merge-file-gone">{shell.tr("chrome.sideDeleted")}</p>
    {:else}
      <div class="merge-file-card">
        {@render mark(fileKind(shell.mergeFile), 56)}
        {#if party}<span>{formatBytes(party.bytes)}</span>{/if}
      </div>
    {/if}
  </div>
{/snippet}

{#snippet mark(kind: string, size: number)}
  <svg class="merge-mark {kind}" viewBox="0 0 48 56" width={size} height={size * 56 / 48} aria-hidden="true">
    <path fill="none" stroke="currentColor" stroke-width="1.6" d="M10 4h18l10 10v34a4 4 0 0 1-4 4H10a4 4 0 0 1-4-4V8a4 4 0 0 1 4-4z" />
    <path fill="none" stroke="currentColor" stroke-width="1.6" d="M28 4v10h10" />
    {#if kind === "sheet"}
      <rect x="8" y="28" width="20" height="18" rx="3" fill="#217346" stroke="none" />
      <path d="M13 32.5 23 41.5M23 32.5 13 41.5" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" />
    {:else if kind === "image"}
      <path fill="none" stroke="currentColor" stroke-width="1.4" d="M14 40l6-7 4 4 3-3 7 6" />
      <circle cx="18" cy="24" r="2" fill="currentColor" stroke="none" />
    {:else}
      <path fill="none" stroke="currentColor" stroke-width="1.4" d="M14 24h16M14 30h16M14 36h10" />
    {/if}
  </svg>
{/snippet}
