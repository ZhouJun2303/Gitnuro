<script lang="ts">
  import { getShell } from "./context";
  import FileMark from "./FileMark.svelte";
  import { fileExtension } from "../fileType";
  import { formatBytes, formatInt } from "../format";
  import "./BinaryDiff.css";

  let {
    path,
    oldBytes = null,
    newBytes = null,
    compact = false,
  }: {
    path: string;
    oldBytes?: number | null;
    newBytes?: number | null;
    compact?: boolean;
  } = $props();

  const shell = getShell();
  const extension = $derived(fileExtension(path));
  const locale = $derived(shell.activeLocale);

  function sideLabel(side: "old" | "new") {
    return shell.tr(side === "old" ? "chrome.binaryOld" : "chrome.binaryNew");
  }

  function absentLabel(side: "old" | "new") {
    return shell.tr(side === "old" ? "chrome.binaryAdded" : "chrome.binaryDeleted");
  }
</script>

{#if compact}
  <div class="binary-summary">
    <FileMark {path} size={18} />
    <span class="binary-summary-title">{shell.tr("chrome.binary")}</span>
    {#if oldBytes == null && newBytes == null}
      <span class="binary-summary-size">—</span>
    {:else if oldBytes == null}
      <span class="binary-summary-size">{shell.tr("chrome.binaryAdded")} · {formatBytes(newBytes ?? 0, locale)}</span>
    {:else if newBytes == null}
      <span class="binary-summary-size">{shell.tr("chrome.binaryDeleted")} · {formatBytes(oldBytes, locale)}</span>
    {:else}
      <span class="binary-summary-size">{sideLabel("old")} {formatBytes(oldBytes, locale)} → {sideLabel("new")} {formatBytes(newBytes, locale)}</span>
    {/if}
  </div>
{:else}
  <div class="binary-diff">
    <div class="binary-cards">
      {@render sideCard("old", oldBytes)}
      {@render sideCard("new", newBytes)}
    </div>
    <p class="binary-hint">{shell.tr("chrome.binaryHint")}</p>
  </div>
{/if}

{#snippet sideCard(side: "old" | "new", bytes: number | null)}
  <figure class="binary-card" class:absent={bytes == null}>
    <figcaption class="binary-side">{sideLabel(side)}</figcaption>
    {#if bytes == null}
      <span class="binary-dash" aria-hidden="true">—</span>
      <span class="binary-extension">{absentLabel(side)}</span>
    {:else}
      <FileMark {path} size={56} />
      <span class="binary-extension">{extension ? `.${extension}` : "—"}</span>
      <span class="binary-size">
        {formatBytes(bytes, locale)}
        {#if bytes >= 1024}<span class="binary-exact">({formatInt(bytes, locale)} B)</span>{/if}
      </span>
    {/if}
  </figure>
{/snippet}
