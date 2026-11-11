<script lang="ts">
  let { label, kind, compact = false }: { label: string; kind: string; compact?: boolean } = $props();
</script>

<span class="ref {kind}" class:compact title={label}>
  {#if kind === "tag"}
    <svg class="ref-icon line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 12.2V5h7.2L20 13.8 13.8 20z"/><circle cx="8" cy="8.2" r="1.1"/></svg>
  {/if}
  <span class="ref-text">{label}</span>
</span>
