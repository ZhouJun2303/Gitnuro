<script lang="ts">
  import { getShell } from "./context";

  const shell = getShell();
  const modes = [
    { value: "soft", name: "Soft", hint: "dialog.resetSoftHint", shortcut: "S" },
    { value: "mixed", name: "Mixed", hint: "dialog.resetMixedHint", shortcut: "M" },
    { value: "hard", name: "Hard", hint: "dialog.resetHardHint", shortcut: "H" },
  ] as const;
  const selected = $derived(modes.find((mode) => mode.value === shell.resetMode) ?? modes[0]);
  const branch = $derived(shell.currentBranch?.name ?? shell.snapshot?.branch ?? "HEAD");
  let open = $state(false);
  let active = $state(0);
  let picker: HTMLDivElement;
  let trigger: HTMLButtonElement;

  function choose(index: number) {
    shell.resetMode = modes[index].value;
    open = false;
    trigger.focus();
  }

  function toggle() {
    active = modes.indexOf(selected);
    open = !open;
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === "Tab") {
      open = false;
      return;
    }
    if (event.key === "Escape") {
      if (!open) return;
      open = false;
    } else if (event.key === "Enter" || event.key === " ") {
      if (open) choose(active);
      else toggle();
    } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      if (!open) {
        active = modes.indexOf(selected);
        open = true;
      } else active = (active + (event.key === "ArrowDown" ? 1 : -1) + modes.length) % modes.length;
    } else if (event.key === "Home" || event.key === "End") {
      open = true;
      active = event.key === "Home" ? 0 : modes.length - 1;
    } else if (!event.ctrlKey && !event.metaKey && !event.altKey) {
      const index = modes.findIndex((mode) => mode.shortcut.toLowerCase() === event.key.toLowerCase());
      if (index < 0) return;
      choose(index);
    } else return;
    event.preventDefault();
    event.stopPropagation();
  }
</script>

<svelte:window onpointerdown={(event) => { if (event.target instanceof Node && !picker?.contains(event.target)) open = false; }} />

<div class="reset-panel">
  <div class="reset-heading">
    <span class="reset-icon" aria-hidden="true">
      <svg class="line-icon" viewBox="0 0 24 24"><path d="M3 10a9 9 0 1 1 2.6 8.4M3 4v6h6"/><path d="M12 7v5l3 2"/></svg>
    </span>
    <div class="reset-copy">
      <h2>{shell.tr("dialog.resetBranch")}</h2>
      <p>{shell.tr("dialog.resetBranchBody", { branch })}</p>
    </div>
  </div>

  <div class="reset-fields">
    <span class="reset-label" id="reset-branch-label">{shell.tr("dialog.branch")}:</span>
    <div class="reset-value" aria-labelledby="reset-branch-label">
      <svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M6 8v8M18 8a6 6 0 0 1-6 6H6"/><circle cx="6" cy="5" r="3"/><circle cx="6" cy="19" r="3"/><circle cx="18" cy="5" r="3"/></svg>
      <span title={branch}>{branch}</span>
    </div>
    <span class="reset-label" id="reset-target-label">{shell.tr("dialog.resetMoveTo")}:</span>
    <div class="reset-value" aria-labelledby="reset-target-label" title={`${shell.draft} ${shell.draftExtra}`}>
      <svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 2v5m0 10v5"/><circle cx="12" cy="12" r="5"/></svg>
      <code>{shell.draft.slice(0, 7)}</code>
      <span>{shell.draftExtra}</span>
    </div>
    <span class="reset-label" id="reset-type-label">{shell.tr("dialog.resetType")}:</span>
    <div class="reset-picker" bind:this={picker}>
      <button
        class="reset-trigger"
        type="button"
        role="combobox"
        aria-labelledby="reset-type-label"
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-controls={open ? "reset-mode-options" : undefined}
        aria-activedescendant={open ? `reset-mode-${modes[active].value}` : undefined}
        disabled={shell.writing}
        bind:this={trigger}
        onclick={toggle}
        onkeydown={onKey}
      >
        <span class="reset-dot {selected.value}"></span>
        <span>{selected.name}</span>
        <span class="reset-hint">{shell.tr(selected.hint)}</span>
        <svg class="line-icon reset-caret" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>
      </button>
      {#if open}
        <div class="reset-options" id="reset-mode-options" role="listbox" aria-labelledby="reset-type-label">
          {#each modes as mode, index (mode.value)}
            <button
              class="reset-option"
              class:active={active === index}
              id={`reset-mode-${mode.value}`}
              type="button"
              role="option"
              aria-selected={shell.resetMode === mode.value}
              tabindex="-1"
              onpointermove={() => { active = index; }}
              onpointerdown={(event) => event.preventDefault()}
              onclick={() => choose(index)}
            >
              <span class="reset-dot {mode.value}"></span>
              <span>{mode.name}</span>
              <span class="reset-hint">{shell.tr(mode.hint)}</span>
              <span class="reset-shortcut" aria-hidden="true">{mode.shortcut}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <div class="composer-row actions">
    <button class="text-button" type="button" disabled={shell.busy} onclick={() => { shell.dialog = null; }}>{shell.tr("dialog.cancel")}</button>
    <button class="commit" class:reset-danger={shell.resetMode === "hard"} type="submit" disabled={shell.writing || !!shell.progress || !shell.draft}>{shell.tr("menu.reset")}</button>
  </div>
</div>

<style>
  .reset-panel { display: flex; flex-direction: column; gap: 18px; min-width: 0; }
  .reset-heading { display: flex; align-items: center; gap: 16px; min-width: 0; padding-right: 20px; }
  .reset-icon { display: grid; place-items: center; flex: none; width: 52px; height: 52px; border-radius: 50%; background: color-mix(in srgb, var(--accent) 12%, transparent); color: var(--accent); }
  .reset-icon svg { width: 29px; height: 29px; }
  .reset-copy { min-width: 0; }
  .reset-panel .reset-heading h2 { position: static; padding: 0; background: transparent; font-size: 18px; line-height: 1.4; }
  .reset-copy p { margin-top: 4px; color: var(--text-secondary); font-size: 13px; }
  .reset-fields { display: grid; grid-template-columns: max-content minmax(0, 1fr); align-items: center; gap: 12px 10px; padding-left: 68px; }
  .reset-label { text-align: right; font-size: 13px; }
  .reset-value { display: flex; align-items: center; gap: 8px; min-width: 0; user-select: text; }
  .reset-value svg { flex: none; width: 15px; height: 15px; color: var(--text-secondary); }
  .reset-value code { flex: none; font-family: var(--mono); font-size: 12px; }
  .reset-value span { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .reset-picker { position: relative; min-width: 0; }
  .reset-trigger, .reset-option { display: grid; grid-template-columns: 12px 48px minmax(0, 1fr) 14px; align-items: center; gap: 8px; width: 100%; min-height: 32px; padding: 6px 9px; border: 1px solid transparent; border-radius: var(--radius-sm); color: var(--text); text-align: left; font: inherit; font-size: 12px; cursor: pointer; }
  .reset-trigger { border-color: var(--control-edge); background: var(--canvas); }
  .reset-trigger:hover:not(:disabled) { border-color: var(--text-secondary); }
  .reset-trigger:focus-visible { outline: none; border-color: var(--accent); box-shadow: var(--focus-ring); }
  .reset-dot { width: 12px; height: 12px; border-radius: 50%; }
  .reset-dot.soft { background: var(--added); }
  .reset-dot.mixed { background: var(--modified); }
  .reset-dot.hard { background: var(--deleted); }
  .reset-hint { line-height: 1.5; }
  .reset-caret { width: 14px; height: 14px; color: var(--text-secondary); }
  .reset-options { position: absolute; z-index: 4; top: calc(100% + 4px); left: 0; right: 0; padding: 4px; border: 1px solid var(--control-edge); border-radius: var(--radius); background: var(--elevated); box-shadow: var(--shadow-menu); }
  .reset-option { background: transparent; }
  .reset-option.active { background: var(--selection); box-shadow: inset 3px 0 var(--accent); }
  .reset-shortcut { color: var(--text-secondary); text-align: center; }
  .reset-panel .reset-danger { background: var(--deleted); color: var(--on-deleted); }
  @media (max-width: 560px) {
    .reset-fields { padding-left: 0; }
    .reset-heading { gap: 12px; }
    .reset-icon { width: 40px; height: 40px; }
    .reset-panel .reset-heading h2 { font-size: 16px; }
    .reset-trigger, .reset-option { grid-template-columns: 10px 38px minmax(0, 1fr) 12px; gap: 6px; }
    .reset-dot { width: 10px; height: 10px; }
  }
</style>
