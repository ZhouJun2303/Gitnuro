<script lang="ts">
  import { browser } from "$app/environment";
  import Shell from "$lib/Shell.svelte";

  const desktop = browser && "__TAURI_INTERNALS__" in window;
  const showShell = desktop || (browser && new URLSearchParams(window.location.search).has("scene"));
  const flow = browser && new URLSearchParams(window.location.search).get("view") === "flow";
</script>

{#if flow}
  {#await import("$lib/FlowWindow.svelte") then { default: FlowWindow }}
    <FlowWindow />
  {/await}
{:else if showShell}
  <Shell />
{:else}
  {#await import("$lib/lab/LabHome.svelte") then { default: LabHome }}
    <LabHome />
  {/await}
{/if}
