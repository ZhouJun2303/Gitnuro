<script lang="ts">
  import { invoke } from "../perf";
  import type { FileDiff, FilePreview } from "../status";
  import { getShell } from "./context";
  import BinaryDiff from "./BinaryDiff.svelte";

  const shell = getShell();
  let { file, diff, compact = false }: { file: string; diff: FileDiff; compact?: boolean } = $props();
  let before = $state<FilePreview | null>(null);
  let after = $state<FilePreview | null>(null);
  let loading = $state(true);
  let failed = $state(false);

  $effect(() => {
    const path = shell.repoPath();
    const id = shell.selectedCommit;
    const pair = shell.comparePair;
    const change = shell.commitFiles.find((item) => item.path === file);
    const oldFile = change?.previousPath || file;
    let cancelled = false;
    before = after = null;
    failed = false;
    loading = true;
    // The backend caps each preview at 8 MB. Keep only the visible revision pair.
    const read = async (rev: string, name: string | null, bytes: number | null | undefined) => {
      if (!name || bytes == null || bytes > 8_000_000) return null;
      const result = await invoke<FilePreview | null>("revision_preview", { path, rev, file: name }).catch(() => null);
      return result?.mime.startsWith("image/") ? result : null;
    };
    if (id && file && diff) void Promise.all([
      read(pair?.[0] ?? `${id}^`, oldFile, diff.oldBytes),
      read(pair?.[1] ?? id, file, diff.newBytes),
    ]).then(([old, next]) => {
      if (cancelled) return;
      before = old;
      after = next;
      loading = false;
    });
    else loading = false;
    return () => { cancelled = true; };
  });
</script>

{#if loading}
  <p class="diff-empty" role="status">{shell.tr("chrome.reading")}</p>
{:else if (before || after) && !failed}
  <div class="revision-images">
    {#each [{ label: "dialog.imageBefore", preview: before }, { label: "dialog.imageAfter", preview: after }] as side (side.label)}
      <figure>
        <figcaption>{shell.tr(side.label)}</figcaption>
        <div class="revision-image-box">
          {#if side.preview}
            <img src={side.preview.dataUrl} alt={shell.tr(side.label)} decoding="async" onerror={() => (failed = true)} />
          {:else}<span class="meta">—</span>{/if}
        </div>
      </figure>
    {/each}
  </div>
{:else}
  <BinaryDiff path={file} oldBytes={diff.oldBytes} newBytes={diff.newBytes} {compact} />
{/if}

<style>
  .revision-images { display: flex; flex-wrap: wrap; gap: 16px; padding: 20px; min-height: 0; max-height: 100%; overflow: auto; }
  figure { flex: 1 1 220px; min-width: 0; margin: 0; }
  figcaption { margin-bottom: 8px; color: var(--text-secondary); font-size: 12px; }
  .revision-image-box {
    min-height: 180px; display: grid; place-items: center; padding: 12px; border: 1px solid var(--line); border-radius: var(--radius-sm);
    background: repeating-conic-gradient(var(--image-checker-a) 0% 25%, var(--image-checker-b) 0% 50%) 50% / 16px 16px;
  }
  img { display: block; max-width: 100%; max-height: 65vh; object-fit: contain; }
</style>
