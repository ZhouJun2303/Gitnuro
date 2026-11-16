<script lang="ts">
  import { tick } from "svelte";
  import { getShell } from "./context";
  import { capturedShortcut, configureShortcuts, defaultShortcutLabels, shortcutLabels, shortcutOwner, type Shortcut } from "../keys";
  import "./ShortcutPage.css";

  const shell = getShell();
  let query = $state("");
  let recording = $state<{ action: Shortcut; index: number } | null>(null);
  let conflict = $state("");

  const groups: { name: string; actions: Shortcut[] }[] = [
    { name: "shortcutGroupApp", actions: ["exit", "settings", "switchLanguage", "launch", "zoomIn", "zoomOut", "performance"] },
    { name: "shortcutGroupRepo", actions: ["refresh", "pull", "push", "fetch", "quickFetch", "quickPull", "quickPush", "branch", "stash", "tag", "clone", "init", "open", "terminal", "explorer"] },
    { name: "shortcutGroupNavigate", actions: ["newTab", "closeTab", "reopen", "tabLeft", "tabRight", "back", "forward", "changes", "commits", "reveal", "filterBranch"] },
    { name: "shortcutGroupChanges", actions: ["commit", "commitPush", "stageToggle", "stageAll", "discard", "copy", "selectAll", "pastePatch", "search", "up", "down"] },
  ];
  const visible = $derived.by(() => {
    shell.settings.shortcuts;
    return groups.map((group) => ({
      ...group,
      actions: group.actions.filter((action) => `${shell.tr(`shortcuts.${action}`)} ${shortcutLabels(action, shell.settings.shortcuts).join(" ")}`.toLowerCase().includes(query.trim().toLowerCase())),
    })).filter((group) => group.actions.length > 0);
  });

  function update(action: Shortcut, labels: string[]) {
    const next = { ...shell.settings.shortcuts };
    const defaults = defaultShortcutLabels(action);
    if (labels.length === defaults.length && labels.every((value, index) => value === defaults[index])) delete next[action];
    else next[action] = labels;
    shell.settings.shortcuts = next;
    configureShortcuts(next);
    void shell.savePrefs(false);
  }

  async function begin(action: Shortcut, index: number) {
    recording = { action, index };
    conflict = "";
    await tick();
    document.querySelector<HTMLElement>(`.shortcut-row[data-shortcut="${action}"] .shortcut-key.recording`)?.focus();
  }

  function record(event: KeyboardEvent, action: Shortcut, index: number) {
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      recording = null;
      conflict = "";
      return;
    }
    const value = capturedShortcut(event);
    if (!value) return;
    const owner = shortcutOwner(value, action);
    if (owner) {
      conflict = `${shell.tr("dialog.shortcutConflict")} ${shell.tr(`shortcuts.${owner}`)} (${value})`;
      return;
    }
    const labels = shortcutLabels(action);
    if (!labels.includes(value)) {
      const next = [...labels];
      next.splice(index, index < next.length ? 1 : 0, value);
      update(action, next);
    }
    recording = null;
    conflict = "";
  }

  function resetAll() {
    recording = null;
    conflict = "";
    shell.settings.shortcuts = {};
    configureShortcuts({});
    void shell.savePrefs(false);
  }
</script>

<div class="shortcut-toolbar">
  <input type="search" bind:value={query} placeholder={shell.tr("dialog.shortcutSearch")} aria-label={shell.tr("dialog.shortcutSearch")} />
  <button type="button" class="text-button" onclick={resetAll} disabled={Object.keys(shell.settings.shortcuts).length === 0}>{shell.tr("dialog.shortcutResetAll")}</button>
</div>

{#each visible as group (group.name)}
  <section class="shortcut-group">
    <h4>{shell.tr(`dialog.${group.name}`)}</h4>
    {#each group.actions as action (action)}
      <div class="shortcut-row" data-shortcut={action}>
        <span class="shortcut-name">{shell.tr(`shortcuts.${action}`)}</span>
        <div class="shortcut-binding">
          {#each shortcutLabels(action, shell.settings.shortcuts) as value, index (`${action}-${index}`)}
            <span class="shortcut-pair">
              <button class="shortcut-key" class:recording={recording?.action === action && recording.index === index} type="button" aria-label={`${shell.tr(`shortcuts.${action}`)}: ${value}`} title={shell.tr("dialog.shortcutRecord")} onclick={() => void begin(action, index)} onkeydown={(event) => { if (recording?.action === action && recording.index === index) record(event, action, index); }}>
                {recording?.action === action && recording.index === index ? shell.tr("dialog.shortcutRecord") : value}
              </button>
              <button class="shortcut-remove" type="button" aria-label={`${shell.tr("dialog.shortcutClear")}: ${shell.tr(`shortcuts.${action}`)} ${value}`} title={shell.tr("dialog.shortcutClear")} onclick={() => update(action, shortcutLabels(action, shell.settings.shortcuts).filter((_, position) => position !== index))}>×</button>
            </span>
          {:else}
            {#if recording?.action !== action}<span class="shortcut-unbound">{shell.tr("dialog.shortcutUnbound")}</span>{/if}
          {/each}
          {#if recording?.action === action && recording.index === shortcutLabels(action, shell.settings.shortcuts).length}
            <button class="shortcut-key recording" type="button" aria-label={shell.tr("dialog.shortcutRecord")} onkeydown={(event) => record(event, action, recording?.index ?? 0)}>{shell.tr("dialog.shortcutRecord")}</button>
          {:else}
            <button class="shortcut-add" type="button" aria-label={`${shell.tr("dialog.shortcutAdd")}: ${shell.tr(`shortcuts.${action}`)}`} title={shell.tr("dialog.shortcutAdd")} onclick={() => void begin(action, shortcutLabels(action, shell.settings.shortcuts).length)}>+</button>
          {/if}
          {#if shell.settings.shortcuts[action] !== undefined}
            <button class="shortcut-reset" type="button" title={shell.tr("dialog.shortcutReset")} aria-label={`${shell.tr("dialog.shortcutReset")}: ${shell.tr(`shortcuts.${action}`)}`} onclick={() => update(action, defaultShortcutLabels(action))}>↺</button>
          {/if}
        </div>
        {#if recording?.action === action && conflict}<span class="shortcut-error" role="alert">{conflict}</span>{/if}
      </div>
    {/each}
  </section>
{:else}
  <p class="shortcut-empty">{shell.tr("dialog.shortcutNoResults")}</p>
{/each}
