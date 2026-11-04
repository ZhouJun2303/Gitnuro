<script lang="ts">
  import Shell from "$lib/Shell.svelte";
  import LabHome from "$lib/lab/LabHome.svelte";

  const desktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  const scene = desktop || (typeof window !== "undefined" && new URLSearchParams(window.location.search).has("scene"));
</script>

{#if scene}
  <Shell />
{:else}
  <LabHome />
{/if}
