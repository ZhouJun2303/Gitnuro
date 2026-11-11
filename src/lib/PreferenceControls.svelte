<script lang="ts">
  import { preferenceSpec, type PreferenceKey, type PreferencePatch, type PreferenceValues } from "./preferences";
  let { values, keys, tr, onchange, disabled = [], label = "", children }: {
    values: PreferenceValues; keys: PreferenceKey[]; tr: (key: string) => string;
    onchange: (patch: PreferencePatch) => void; disabled?: PreferenceKey[]; label?: string; children?: import("svelte").Snippet;
  } = $props();
</script>

<div class="preference-toolbar" role="group" aria-label={label || tr("chrome.viewOptions")}>
  {#each keys as key (key)}
    {@const spec = preferenceSpec(key)}
    {@const locked = disabled.includes(key) || (key === "diffContext" && values.showEntireFile) || (key === "swapPanes" && values.fileLayout === "combined")}
    <label class="preference-control" class:toggle={!spec.options && spec.min === undefined} class:locked title={tr(key === "diffContext" && values.showEntireFile ? "chrome.contextWhileWhole" : key === "swapPanes" && values.fileLayout === "combined" ? "chrome.swapWhileCombined" : spec.label)}>
      {#if spec.options}
        <span>{tr(spec.shortLabel ?? spec.label)}</span>
        <select data-preference={key} value={String(values[key])} disabled={locked} onchange={(event) => onchange({ [key]: event.currentTarget.value })}>
          {#each spec.options as [value, text]}<option {value}>{tr(text)}</option>{/each}
        </select>
      {:else if spec.min !== undefined}
        <span>{tr(spec.shortLabel ?? spec.label)}</span>
        <input data-preference={key} type="number" value={Number(values[key])} min={spec.min} max={spec.max} step="1" disabled={locked} onchange={(event) => { const input = event.currentTarget; if (input.value !== "" && input.checkValidity()) onchange({ [key]: input.valueAsNumber }); else input.value = String(values[key]); }} />
      {:else}
        <input data-preference={key} type="checkbox" checked={spec.invert ? !values[key] : !!values[key]} disabled={locked} onchange={(event) => onchange({ [key]: spec.invert ? !event.currentTarget.checked : event.currentTarget.checked })} />
        <span>{tr(spec.shortLabel ?? spec.label)}</span>
      {/if}
    </label>
  {/each}
  {@render children?.()}
</div>

<style>
  .preference-toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 10px; flex: none; min-width: 0; padding: 5px 8px; border-bottom: 1px solid var(--line); background: var(--panel-head); color: var(--text-secondary); font-size: 11px; line-height: 1.4; }
  .preference-control { display: inline-flex; align-items: center; gap: 5px; min-height: 25px; min-width: 0; max-width: 100%; cursor: pointer; }
  .preference-control > span { overflow-wrap: anywhere; }
  .locked { opacity: .6; cursor: default; }
  select, input[type="number"] { color: var(--text); background: var(--canvas); border: 1px solid var(--line); border-radius: var(--radius-sm, 4px); min-height: 25px; font: inherit; padding: 2px 5px; max-width: 100%; }
  select { min-width: 0; max-width: 150px; }
  input[type="number"] { width: 63px; flex: none; }
  input[type="checkbox"] { width: 13px; height: 13px; margin: 0; flex: none; accent-color: var(--accent); }
  :is(select, input):focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .toggle:has(input:checked) { color: var(--text); }
</style>
