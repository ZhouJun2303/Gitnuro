<script lang="ts">
  import { getShell } from "./context";
  import "./MergePanel.css";
  import { onMount } from "svelte";
  import MergePane from "./MergePane.svelte";
  import type { ConflictParty } from "../status";
  import { hunkIncludes, loadTextMerge } from "./dialogs.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  const shell = getShell();
  let fileLeft = $state(false);
  let fileRight = $state(false);
  let openFor = "";
  let panel = $state<HTMLElement | null>(null);
  let activeConflict = $state(0);
  let leftPane = $state<MergePane>();
  let rightPane = $state<MergePane>();
  let resultPane = $state<MergePane>();
  let scrollFrame = 0;
  const mirrored = new WeakMap<HTMLElement, { top: number; left: number }>();

  function closePanel() { shell.closeTextMerge(); }

  onMount(() => {
    let disposed = false;
    let off: (() => void) | undefined;
    if (shell.inApp()) void getCurrentWindow().onCloseRequested((event) => {
      if (shell.mergeDirty || shell.mergeApplying) { event.preventDefault(); closePanel(); }
    }).then((value) => { if (disposed) value(); else off = value; });
    return () => { disposed = true; off?.(); cancelAnimationFrame(scrollFrame); };
  });

  $effect(() => {
    if (!shell.textMerge) {
      openFor = "";
      return;
    }
    if (shell.mergeFile === openFor) return;
    openFor = shell.mergeFile;
    fileLeft = false;
    fileRight = false;
    activeConflict = 0;
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
  const leftParty = $derived(view?.theirs ?? null);
  const rightParty = $derived(view?.ours ?? null);
  const unresolved = $derived(Math.max(0, conflictTotal - conflictResolved));
  const canResolve = $derived(shell.mergeKind === "file" ? fileSide !== "" : !shell.mergeLarge && unresolved === 0);

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
    if (!shell.mergeEdited) {
      run();
      return;
    }
    shell.ask(shell.tr("dialog.mergeOverwriteEdits"), shell.tr("dialog.mergeOverwriteEditsBody"), () => {
      shell.mergeEdited = false;
      shell.mergeEditing = false;
      run();
    }, [], { danger: true });
  }

  function onLeft(event: Event) {
    if (shell.mergeKind === "file") {
      shell.mergeDirty = true;
      fileLeft = !fileLeft;
      if (fileLeft) fileRight = false;
      return;
    }
    const next = !leftOn;
    // The confirm may be cancelled, so hold the browser's toggle until the state moves.
    if (shell.mergeEdited && event.currentTarget instanceof HTMLInputElement) event.currentTarget.checked = leftOn;
    withDirtyGuard(() => shell.takeMergeSide("theirs", next));
  }

  function onRight(event: Event) {
    if (shell.mergeKind === "file") {
      shell.mergeDirty = true;
      fileRight = !fileRight;
      if (fileRight) fileLeft = false;
      return;
    }
    const next = !rightOn;
    if (shell.mergeEdited && event.currentTarget instanceof HTMLInputElement) event.currentTarget.checked = rightOn;
    withDirtyGuard(() => shell.takeMergeSide("ours", next));
  }

  function stateText(state: ConflictParty["state"]) {
    if (state === "added") return shell.tr("chrome.sideAdded");
    if (state === "modified") return shell.tr("chrome.sideModified");
    if (state === "deleted") return shell.tr("chrome.sideDeleted");
    return "";
  }

  function isoAt(at: number) {
    return at ? new Date(at * 1000).toISOString() : undefined;
  }

  function syncScroll(source: "left" | "right", event: Event) {
    const target = event.currentTarget;
    if (!(target instanceof HTMLElement)) return;
    const expected = mirrored.get(target);
    if (expected && Math.abs(target.scrollTop - expected.top) <= 1 && Math.abs(target.scrollLeft - expected.left) <= 1) {
      mirrored.delete(target);
      return;
    }
    mirrored.delete(target);
    cancelAnimationFrame(scrollFrame);
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = 0;
      const other = (source === "left" ? rightPane : leftPane)?.element();
      if (!other) return;
      other.scrollTop = target.scrollTop;
      other.scrollLeft = target.scrollLeft;
      mirrored.set(other, { top: other.scrollTop, left: other.scrollLeft });
    });
  }

  async function revealHunk(index: number) {
    const at = hunkIndexes.indexOf(index);
    if (at >= 0) activeConflict = at;
    await Promise.all([leftPane?.reveal(index), rightPane?.reveal(index), resultPane?.reveal(index)]);
  }

  function chooseHunk(index: number, side: "theirs" | "ours") {
    withDirtyGuard(() => { shell.toggleMergeSide(index, side); void revealHunk(index); });
  }

  function jumpConflict(delta: number) {
    if (conflictTotal === 0) return;
    const next = (activeConflict + delta + conflictTotal) % conflictTotal;
    const index = hunkIndexes[next];
    if (index == null) return;
    void revealHunk(index);
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
    if (!canResolve || shell.busy) return;
    if (shell.mergeKind === "file") {
      if (fileSide === "ours" || fileSide === "theirs") await shell.applyFileMerge(fileSide);
    } else {
      const text = shell.mergeEditing || shell.mergeEdited ? shell.mergeResult : shell.joinedPieces(shell.mergePieces);
      await shell.applyTextMerge(text);
    }
  }

</script>

{#if shell.textMerge}
  <div class="merge-standalone" role="presentation">{@render mergeSurface()}</div>
{:else}
  <div class="merge-loading" role="status">
    <p>{shell.mergeError || shell.actionError || shell.loadError || shell.tr(shell.mergeLoading ? "chrome.reading" : "chrome.noChanges")}</p>
    <button class="text-button" type="button" onclick={closePanel}>{shell.tr("chrome.close")}</button>
  </div>
{/if}

{#snippet mergeSurface()}
    <div class="merge-panel standalone" data-layer="text-merge" bind:this={panel} role="dialog" aria-modal="false" aria-label={shell.tr("dialog.mergeTitle")} tabindex="-1" onkeydown={(event) => { if (event.key === "Escape" && !shell.confirmAsk) { event.preventDefault(); event.stopPropagation(); closePanel(); return; } trapTab(event); }}>
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
        <button class="merge-close" type="button" disabled={shell.mergeApplying} aria-label={shell.tr("chrome.close")} title={shell.tr("chrome.close")} onclick={() => closePanel()}>×</button>
      </header>
      <div class="merge-head">
        <div class="merge-parties">
          <div class="merge-party">
            <label class="merge-party-pick">
              <input type="checkbox" checked={leftOn} disabled={shell.mergeLarge || shell.busy} aria-label={shell.tr("dialog.mergeKeepSide", { name: shell.mergeLeft })} onclick={onLeft} />
              <span>{shell.tr("dialog.mergeKeepSide", { name: shell.mergeLeft })}</span>
              {#if leftParty && leftParty.state !== "unchanged"}<span class="merge-party-state">{stateText(leftParty.state)}</span>{/if}
            </label>
            {#if leftParty}
              <div class="merge-party-info">
                {#if leftParty.commit}
                  <span class="merge-party-subject" title={leftParty.commit.subject}>{leftParty.commit.subject}</span>
                  <span class="merge-party-meta">
                    <code>{leftParty.commit.short}</code>
                    <span title={leftParty.commit.email}>{leftParty.commit.author}</span>
                    {#if leftParty.commit.at}<time datetime={isoAt(leftParty.commit.at)}>{shell.forkWhen(leftParty.commit.at)}</time>{/if}
                  </span>
                {:else}
                  <span class="merge-party-meta">{formatBytes(leftParty.bytes)}</span>
                {/if}
              </div>
            {/if}
          </div>
          <div class="merge-party right">
            <label class="merge-party-pick">
              <input type="checkbox" checked={rightOn} disabled={shell.mergeLarge || shell.busy} aria-label={shell.tr("dialog.mergeKeepSide", { name: shell.mergeRight })} onclick={onRight} />
              <span>{shell.tr("dialog.mergeKeepSide", { name: shell.mergeRight })}</span>
              {#if rightParty && rightParty.state !== "unchanged"}<span class="merge-party-state">{stateText(rightParty.state)}</span>{/if}
            </label>
            {#if rightParty}
              <div class="merge-party-info">
                {#if rightParty.commit}
                  <span class="merge-party-subject" title={rightParty.commit.subject}>{rightParty.commit.subject}</span>
                  <span class="merge-party-meta">
                    <code>{rightParty.commit.short}</code>
                    <span title={rightParty.commit.email}>{rightParty.commit.author}</span>
                    {#if rightParty.commit.at}<time datetime={isoAt(rightParty.commit.at)}>{shell.forkWhen(rightParty.commit.at)}</time>{/if}
                  </span>
                {:else}
                  <span class="merge-party-meta">{formatBytes(rightParty.bytes)}</span>
                {/if}
              </div>
            {/if}
          </div>
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
      {:else if shell.mergeLarge}
        <div class="merge-stage merge-large">
          <p>{shell.tr("dialog.mergeLargeHint")}</p>
          <button class="pane-chip" type="button" disabled={shell.busy || shell.mergeLoading} onclick={() => void loadTextMerge(shell)}>{shell.tr("dialog.loadMerge")}</button>
        </div>
      {:else}
        <div class="merge-top">
          <MergePane side="theirs" bind:this={leftPane} onpick={chooseHunk} onscroll={(event) => syncScroll("left", event)} />
          <MergePane side="ours" bind:this={rightPane} onpick={chooseHunk} onscroll={(event) => syncScroll("right", event)} />
        </div>
        <div class="merge-result-pane">
          <div class="merge-result-head">
            <span>{shell.tr("dialog.result")}</span>
            {#if shell.mergeEditing}<span class="merge-result-mode">{shell.tr("dialog.editResult")}</span>{/if}
          </div>
          {#if shell.mergeEditing}
            <textarea class="merge-result-edit" aria-label={shell.tr("dialog.result")} spellcheck="false" bind:value={shell.mergeResult} disabled={shell.busy} oninput={() => { shell.mergeEdited = true; shell.mergeDirty = true; }}></textarea>
          {:else}
            <MergePane side="result" bind:this={resultPane} onpick={chooseHunk} />
          {/if}
        </div>
      {/if}
      {#if shell.mergeError}<p class="merge-error" role="alert">{shell.mergeError}</p>{/if}
      <footer class="merge-panel-foot">
        {#if shell.mergeKind === "text" && !shell.mergeLarge}
          {#if unresolved > 0}
            <span class="merge-edit-hint merge-unresolved" role="status">{shell.tr("dialog.mergeUnresolved", { count: String(unresolved) })}</span>
          {:else}
            <span class="merge-edit-hint">{shell.tr("dialog.editingAllowed")}</span>
          {/if}
        {/if}
        {#if shell.mergeKind === "text" && !shell.mergeLarge}
          <button
            class="text-button merge-edit-entry"
            disabled={shell.busy}
            type="button"
            aria-pressed={shell.mergeEditing}
            onclick={() => {
              if (!shell.mergeEditing && !shell.mergeEdited) shell.mergeResult = shell.joinedPieces(shell.mergePieces);
              shell.mergeEditing = !shell.mergeEditing;
              if (shell.mergeEditing) queueMicrotask(() => panel?.querySelector<HTMLTextAreaElement>(".merge-result-edit")?.focus());
            }}
          >{shell.mergeEditing ? shell.tr("dialog.previewResult") : shell.tr("dialog.editResult")}</button>
        {/if}
        <button class="text-button" type="button" disabled={shell.mergeApplying} onclick={() => closePanel()}>{shell.tr("chrome.cancel")}</button>
        <button class="commit" type="button" disabled={shell.busy || !canResolve} title={!canResolve ? shell.tr(shell.mergeKind === "file" ? "dialog.mergePickFile" : "dialog.mergeUnresolved", shell.mergeKind === "file" ? {} : { count: String(unresolved) }) : undefined} onclick={() => void resolve()}>{shell.tr("dialog.mergeResolve")}</button>
      </footer>
    </div>
{/snippet}

{#snippet fileCard(party: ConflictParty | undefined)}
  <div class="merge-file-slot">
    <div class="merge-file-card">
      {#if party?.state === "deleted"}
        <p class="merge-file-gone">{shell.tr("chrome.sideDeleted")}</p>
      {:else}
        {@render mark(fileKind(shell.mergeFile), 56)}
        {#if party}<span class="merge-file-size">{formatBytes(party.bytes)}</span>{/if}
      {/if}
      {#if party}
        <div class="merge-file-title">
          <strong class="merge-file-side">{party.label}</strong>
          {#if party.state !== "unchanged"}<span class="merge-file-state">{stateText(party.state)}</span>{/if}
        </div>
        {#if party.commit}
          <p class="merge-file-subject" title={party.commit.subject}>{party.commit.subject}</p>
          <dl class="merge-file-meta">
            <div><dt>{shell.tr("chrome.commit")}</dt><dd><code>{party.commit.short}</code></dd></div>
            <div><dt>{shell.tr("chrome.author")}</dt><dd title={party.commit.email}>{party.commit.author}</dd></div>
            <div><dt>{shell.tr("dialog.mergeTime")}</dt><dd>{shell.forkWhen(party.commit.at)}</dd></div>
          </dl>
        {/if}
      {/if}
    </div>
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
