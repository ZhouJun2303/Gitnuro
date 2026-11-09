<script lang="ts">
  import { fileMarkKind } from "../fileType";

  let { path, size = 48 }: { path: string; size?: number } = $props();
  const kind = $derived(fileMarkKind(path));
</script>

<svg class="file-mark" viewBox="0 0 48 56" width={size} height={size * 56 / 48} aria-hidden="true">
  <path fill="none" stroke="currentColor" stroke-width="1.6" d="M10 4h18l10 10v34a4 4 0 0 1-4 4H10a4 4 0 0 1-4-4V8a4 4 0 0 1 4-4z" />
  <path fill="none" stroke="currentColor" stroke-width="1.6" d="M28 4v10h10" />
  {#if kind === "sheet"}
    <rect x="8" y="28" width="20" height="18" rx="3" fill="#217346" />
    <path d="M13 32.5 23 41.5M23 32.5 13 41.5" fill="none" stroke="#fff" stroke-width="1.8" stroke-linecap="round" />
  {:else if kind === "slides"}
    <rect x="8" y="28" width="20" height="18" rx="3" fill="#c43e1c" />
    <rect x="12" y="32" width="12" height="7" rx="1.2" fill="#fff" opacity=".92" />
    <path d="M16 43h8" stroke="#fff" stroke-width="1.6" stroke-linecap="round" />
  {:else if kind === "doc"}
    <rect x="8" y="28" width="20" height="18" rx="3" fill="#2b579a" />
    <path d="M12 32.5h12M12 36.5h12M12 40.5h8" stroke="#fff" stroke-width="1.5" stroke-linecap="round" />
  {:else if kind === "pdf"}
    <rect x="8" y="28" width="20" height="18" rx="3" fill="#c0392b" />
    <path d="M12.5 33h11M12.5 37h11M12.5 41h6.5" stroke="#fff" stroke-width="1.5" stroke-linecap="round" />
  {:else if kind === "archive"}
    <rect x="8" y="28" width="20" height="18" rx="3" fill="#b7791f" />
    <path d="M18 28v3M20 31v3M18 34v3M20 37v3M18 40v3" stroke="#fff" stroke-width="1.2" />
    <rect x="16.5" y="42" width="5" height="4" rx="1" fill="#fff" opacity=".92" />
  {:else if kind === "image"}
    <path fill="none" stroke="currentColor" stroke-width="1.4" d="M14 40l6-7 4 4 3-3 7 6" />
    <circle cx="18" cy="24" r="2" fill="currentColor" />
  {:else if kind === "media"}
    <circle cx="18" cy="37.5" r="7.5" fill="#0f766e" />
    <path d="M16.2 33.8v7.4l5.6-3.7z" fill="#fff" />
  {:else if kind === "binary"}
    <rect x="8" y="28" width="20" height="18" rx="3" fill="currentColor" opacity=".16" />
    <path fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" d="M14.5 33.5 11.5 37l3 3.5M21.5 33.5l3 3.5-3 3.5" />
  {:else}
    <path fill="none" stroke="currentColor" stroke-width="1.4" d="M14 24h16M14 30h16M14 36h10" />
  {/if}
</svg>

<style>
  .file-mark {
    color: var(--text-secondary);
    flex: none;
  }
</style>
