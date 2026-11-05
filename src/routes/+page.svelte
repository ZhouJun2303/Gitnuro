<script lang="ts">
  import { browser } from "$app/environment";
  import Shell from "$lib/Shell.svelte";
  import LabHome from "$lib/lab/LabHome.svelte";

  const desktop = browser && "__TAURI_INTERNALS__" in window;
  const showShell = desktop || (browser && new URLSearchParams(window.location.search).has("scene"));
</script>

{#if showShell}
  <Shell />
{:else}
  <LabHome />
{/if}
