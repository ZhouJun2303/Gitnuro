<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "../perf";
  import { getShell } from "./context";

  type FormatKind = { kind: string; extensions: string[] };
  type FormatGroup = { tool: "unity" | "common"; scope: "any" | "unityAssets"; kinds: FormatKind[] };
  type FormatCatalog = { executables: string[]; groups: FormatGroup[] };

  const shell = getShell();
  let catalog = $state<FormatCatalog | null>(null);
  let error = $state("");

  async function load() {
    error = "";
    try {
      catalog = await invoke<FormatCatalog>("revision_copy_formats");
    } catch (cause) {
      error = shell.message(cause);
    }
  }

  onMount(() => { void load(); });
</script>

<section class="pref-card">
  <h3 class="pref-card-title">{shell.tr("dialog.previewExecutables")}</h3>
  <div class="pref-card-body preview-catalog-exes">
    {#if catalog}
      {#if catalog.executables.length}
        <span>{catalog.executables.join(", ")}</span>
      {:else}
        <span>{shell.tr("dialog.previewExeNone")}</span>
      {/if}
    {:else if !error}
      <span>{shell.tr("dialog.previewFormatsLoading")}</span>
    {/if}
  </div>
</section>

<section class="pref-card">
  <h3 class="pref-card-title">{shell.tr("dialog.previewFormats")}</h3>
  <div class="pref-card-body preview-catalog">
    {#if error}
      <div role="alert">{error} <button type="button" class="text-button" onclick={() => void load()}>{shell.tr("chrome.refresh")}</button></div>
    {:else if catalog}
      {#each catalog.groups as group (`${group.tool}-${group.scope}`)}
        <div class="preview-format-group" data-tool={group.tool} data-scope={group.scope}>
          <h4>{group.tool === "unity" ? "Unity" : shell.tr("dialog.previewToolCommon")} <span>{shell.tr(group.scope === "unityAssets" ? "dialog.previewScopeUnityAssets" : "dialog.previewScopeAny")}</span></h4>
          {#each group.kinds as kind (kind.kind)}
            <div class="preview-format-row">
              <span class="preview-kind">{kind.kind}</span>
              <span class="preview-extensions">{kind.extensions.join("  ")}</span>
            </div>
          {/each}
        </div>
      {/each}
      <p class="preview-catalog-note">{shell.tr("dialog.previewOtherTools")}</p>
    {:else}
      <span role="status">{shell.tr("dialog.previewFormatsLoading")}</span>
    {/if}
  </div>
</section>

<style>
  .preview-catalog-exes { min-height: 36px; justify-content: center; font-size: 12px; color: var(--text-secondary); }
  .preview-catalog { gap: 14px; padding-top: 12px; font-size: 12px; }
  .preview-format-group + .preview-format-group { border-top: 1px solid var(--line); padding-top: 12px; }
  .preview-format-group h4 { margin: 0 0 7px; font-size: 12px; font-weight: 600; }
  .preview-format-group h4 span { margin-left: 8px; font-weight: 400; color: var(--text-secondary); }
  .preview-format-row { display: grid; grid-template-columns: 100px minmax(0, 1fr); gap: 12px; padding: 5px 0; align-items: start; }
  .preview-kind { color: var(--text-secondary); }
  .preview-extensions { font-family: var(--mono); overflow-wrap: anywhere; line-height: 1.5; }
  .preview-catalog-note { margin: 0; color: var(--text-secondary); }
  @media (max-width: 600px) {
    .preview-format-row { grid-template-columns: 1fr; gap: 2px; }
  }
</style>
