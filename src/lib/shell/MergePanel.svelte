<script lang="ts">
  import { getShell } from "./context";
  import "./MergePanel.css";
  import type { ConflictParty } from "../status";
  import type { MergePiece } from "./types";

  const shell = getShell();
  let fileLeft = $state(false);
  let fileRight = $state(false);
  let openFor = "";
  let panel = $state<HTMLElement | null>(null);

  $effect(() => {
    if (!shell.textMerge) {
      openFor = "";
      return;
    }
    if (shell.mergeFile === openFor) return;
    openFor = shell.mergeFile;
    fileLeft = false;
    fileRight = false;
  });

  $effect(() => {
    if (shell.textMerge && panel) panel.focus();
  });

  const textLeft = $derived(shell.mergePieces.every((piece) => piece.kind !== "hunk" || piece.pick === "theirs"));
  const textRight = $derived(shell.mergePieces.every((piece) => piece.kind !== "hunk" || piece.pick === "ours"));
  const leftOn = $derived(shell.mergeKind === "file" ? fileLeft : textLeft || !textRight);
  const rightOn = $derived(shell.mergeKind === "file" ? fileRight : textRight || !textLeft);
  const fileSide = $derived<"ours" | "theirs" | "">(fileLeft ? "theirs" : fileRight ? "ours" : "");
  const view = $derived(shell.activeConflict);

  function onLeft(event: Event) {
    event.preventDefault();
    if (shell.mergeKind === "file") {
      fileLeft = !fileLeft;
      if (fileLeft) fileRight = false;
      return;
    }
    shell.takeMergeSide("theirs");
  }

  function onRight(event: Event) {
    event.preventDefault();
    if (shell.mergeKind === "file") {
      fileRight = !fileRight;
      if (fileRight) fileLeft = false;
      return;
    }
    shell.takeMergeSide("ours");
  }

  function chosen(piece: MergePiece) {
    if (piece.kind !== "hunk") return "";
    if (piece.pick === "theirs") return piece.theirs;
    if (piece.pick === "both") return [piece.ours, piece.theirs].filter((part) => part.length > 0).join("\n");
    return piece.ours;
  }

  function formatBytes(bytes: number) {
    const exact = Math.max(0, Math.trunc(bytes)).toLocaleString("en-US");
    if (bytes < 1024) return `${exact} B`;
    const shown = (Math.floor((bytes / 1024) * 10) / 10).toFixed(1);
    return `${shown} KB (${exact} B)`;
  }

  function fileKind(path: string) {
    const name = path.split(/[\\/]/).pop() ?? "";
    const ext = name.includes(".") ? name.slice(name.lastIndexOf(".") + 1).toLowerCase() : "";
    if (ext === "xls" || ext === "xlsx" || ext === "xlsm" || ext === "csv" || ext === "ods") return "sheet";
    if (ext === "png" || ext === "jpg" || ext === "jpeg" || ext === "gif" || ext === "webp" || ext === "bmp" || ext === "svg" || ext === "ico") return "image";
    return "doc";
  }

  function show(text: string) {
    return text.trim() ? text : shell.tr("dialog.mergeEmpty");
  }

  async function resolve() {
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
    <div class="merge-panel" bind:this={panel} role="dialog" aria-modal="true" aria-label={shell.tr("dialog.mergeTitle")} tabindex="-1" onkeydown={(event) => { if (event.key === "Escape") { event.preventDefault(); shell.closeTextMerge(); } }}>
      <header class="merge-panel-title">
        <strong>{shell.tr("dialog.mergeTitle")}</strong>
        <span class="merge-title-file">
          {@render mark(fileKind(shell.mergeFile), 16)}
          <span>{shell.fileName(shell.mergeFile)}</span>
        </span>
        <button class="merge-close" type="button" aria-label={shell.tr("chrome.close")} onclick={() => shell.closeTextMerge()}>×</button>
      </header>
      <div class="merge-head">
        <label class="merge-party">
          <input type="checkbox" checked={leftOn} aria-label={shell.mergeLeft} onclick={onLeft} />
          <span>{shell.mergeLeft}</span>
        </label>
        {#if shell.mergeKind === "text"}<span class="merge-result-label">{shell.tr("dialog.result")}</span>{:else}<span></span>{/if}
        <label class="merge-party">
          <input type="checkbox" checked={rightOn} aria-label={shell.mergeRight} onclick={onRight} />
          <span>{shell.mergeRight}</span>
        </label>
      </div>
      {#if shell.mergeKind === "file"}
        <div class="merge-stage files">
          {@render fileCard(view?.theirs)}
          <div class="merge-file-slot"></div>
          {@render fileCard(view?.ours)}
        </div>
      {:else}
        <div class="merge-stage">
          {#each shell.mergePieces as piece, index (index)}
            {#if piece.kind === "text"}
              <pre class="merge-cell context">{piece.text}</pre>
              <pre class="merge-cell context result">{piece.text}</pre>
              <pre class="merge-cell context">{piece.text}</pre>
            {:else}
              <button class="merge-cell hunk" type="button" class:on={piece.pick === "theirs"} onclick={() => shell.pickHunk(index, "theirs")}>
                <pre class:empty={!piece.theirs.trim()}>{show(piece.theirs)}</pre>
              </button>
              <pre class="merge-cell hunk result pick-{piece.pick}" class:empty={!chosen(piece).trim()}>{show(chosen(piece))}</pre>
              <button class="merge-cell hunk" type="button" class:on={piece.pick === "ours"} onclick={() => shell.pickHunk(index, "ours")}>
                <pre class:empty={!piece.ours.trim()}>{show(piece.ours)}</pre>
              </button>
            {/if}
          {/each}
        </div>
      {/if}
      <footer class="merge-panel-foot">
        <button class="commit" type="button" disabled={shell.mergeKind === "file" ? !fileSide || (shell.mode === "live" && shell.busy) : shell.mode === "live" && shell.busy} onclick={() => void resolve()}>{shell.tr("dialog.mergeResolve")}</button>
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
