<script lang="ts">
  import { setShell } from "./shell/context";
  import Chrome from "./shell/Chrome.svelte";
  import Welcome from "./shell/Welcome.svelte";
  import Sidebar from "./shell/Sidebar.svelte";
  import ChangesView from "./shell/ChangesView.svelte";
  import HistoryView from "./shell/HistoryView.svelte";
  import FileHistoryPane from "./shell/FileHistoryPane.svelte";
  import BlobPane from "./shell/BlobPane.svelte";
  import DialogHost from "./shell/DialogHost.svelte";
  import MergePanel from "./shell/MergePanel.svelte";
  import "./shell/common.css";
  import { ShellState } from "./shell/state.svelte";
  import { invoke } from "./perf";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { diffs, sampleImages, sampleReflog } from "./sample";
  import SplitHandle from "./SplitHandle.svelte";
  import PerfPanel from "./PerfPanel.svelte";
  import { cloneTarget, cloneUrl } from "./clonePath";
  import { fileAccessReport } from "./fileAccess";
  import { untrack } from "svelte";
  import { shortcutLabel } from "./keys";
  import { branchGroup, gravatarUrl, laneColors, splitDiff, type SplitRow } from "./view";
  import { type UpdateNotice } from "./status";
  import type { MenuItem, MenuItems, Row, SearchKey, Side, TreeEntry } from "./shell/types";

  const shell = new ShellState();
  setShell(shell);
  const focusOnMount = (node: HTMLInputElement) => shell.focusOnMount(node);
  const focusWhen = (node: HTMLInputElement, on: boolean) => shell.focusWhen(node, on);
  const fitMenu = (node: HTMLElement, spot: { x: number; y: number; anchorTop?: number }) => shell.fitMenu(node, spot);
  const splitScroller = (node: HTMLElement, columns?: { left: number; right: number }) => shell.splitScroller(node, columns);

  let submenuAnchor: HTMLElement | null = null;
  const focusTraps: HTMLElement[] = [];

  $effect(() => {
    if (!shell.dialog && !shell.textMerge) return;
    // Close other layers when a dialog opens, but do not track them: a menu
    // opened from inside the dialog must stay open.
    untrack(() => {
      if (shell.commandLogOpen) shell.commandLogOpen = false;
      if (shell.repoPickerOpen) shell.repoPickerOpen = false;
      if (shell.menu) shell.dismissMenu(false);
      if (shell.launchOpen) shell.closeLaunch();
      if (shell.forgeOpen) shell.forgeOpen = null;
    });
  });

  function openSubmenu(item: MenuItem, anchor: HTMLElement, focus = false) {
    if (!item.children) return;
    submenuAnchor = anchor;
    const rect = anchor.getBoundingClientRect();
    shell.submenu = { x: rect.right - 4, y: rect.top, items: item.children };
    if (focus) requestAnimationFrame(() => document.querySelector<HTMLButtonElement>('[data-layer="submenu"] button:not(:disabled)')?.focus());
  }

  function menuKeys(node: HTMLElement, autofocus = true) {
    const items = () => [...node.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];
    if (autofocus) queueMicrotask(() => items()[0]?.focus());
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Home" || event.key === "End") {
        const list = items();
        if (list.length === 0) return;
        event.preventDefault();
        const at = list.indexOf(document.activeElement as HTMLButtonElement);
        const next = event.key === "Home" ? 0 : event.key === "End" ? list.length - 1 : event.key === "ArrowDown" ? (at + 1) % list.length : (at - 1 + list.length) % list.length;
        list[next]?.focus();
        return;
      }
      if (event.key === "ArrowRight") {
        const active = document.activeElement as HTMLButtonElement | null;
        if (active?.getAttribute("aria-haspopup") === "menu") {
          event.preventDefault();
          active.click();
        }
        return;
      }
      if (event.key === "ArrowLeft" && node.dataset.layer === "submenu") {
        event.preventDefault();
        shell.submenu = null;
        requestAnimationFrame(() => submenuAnchor?.isConnected && submenuAnchor.focus());
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        if (node.dataset.layer === "submenu") {
          shell.submenu = null;
          requestAnimationFrame(() => submenuAnchor?.isConnected && submenuAnchor.focus());
          return;
        }
        shell.dismissMenu();
      }
    };
    node.addEventListener("keydown", onKey);
    return { destroy() { node.removeEventListener("keydown", onKey); } };
  }

  function overlayFocus(node: HTMLElement, initial: { close: () => void; focus?: string }) {
    let options = initial;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const focusables = () => [...node.querySelectorAll<HTMLElement>('a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])')].filter((item) => item.offsetParent !== null);
    const focusFirst = () => {
      const preferred = options.focus ? node.querySelector<HTMLElement>(options.focus) : null;
      (preferred ?? node).focus();
    };
    node.setAttribute("tabindex", node.getAttribute("tabindex") ?? "-1");
    queueMicrotask(focusFirst);
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        options.close();
        return;
      }
      if (event.key !== "Tab") return;
      const list = focusables();
      if (list.length === 0) {
        event.preventDefault();
        node.focus();
        return;
      }
      const first = list[0];
      const last = list[list.length - 1];
      if (event.shiftKey && (document.activeElement === first || !node.contains(document.activeElement))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (document.activeElement === last || !node.contains(document.activeElement))) {
        event.preventDefault();
        first.focus();
      }
    };
    const onFocusIn = (event: FocusEvent) => {
      if (focusTraps[focusTraps.length - 1] !== node) return;
      if (!node.contains(event.target as Node)) focusFirst();
    };
    focusTraps.push(node);
    node.addEventListener("keydown", onKey);
    document.addEventListener("focusin", onFocusIn);
    return {
      update(next: { close: () => void; focus?: string }) { options = next; },
      destroy() {
        const at = focusTraps.indexOf(node);
        if (at >= 0) focusTraps.splice(at, 1);
        node.removeEventListener("keydown", onKey);
        document.removeEventListener("focusin", onFocusIn);
        if (focusTraps[focusTraps.length - 1]?.isConnected) focusTraps[focusTraps.length - 1].focus();
        else if (previous?.isConnected) previous.focus();
      },
    };
  }
</script>

<style>
.menu {
    position: fixed;
    z-index: var(--z-menu, 30);
    min-width: 220px;
    max-height: min(70vh, 480px);
    overflow: auto;
    padding: 4px;
    background: var(--elevated);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
  }

.menu button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    width: 100%;
    min-height: 28px;
    flex: none;
    padding: 4px 8px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    border-radius: var(--radius-sm);
    transition: background 120ms ease, color 120ms ease;
  }

.menu button:disabled { opacity: 0.4; }

.menu hr { border: 0; border-top: 1px solid var(--line); margin: 4px 6px; }

.shortcut { color: var(--text-secondary); font-size: 11px; }

.menu-main { display: flex; align-items: center; gap: 8px; min-width: 0; }

.menu-check { width: 14px; flex: none; text-align: center; }

.menu button, .menu-dismiss { font: inherit; }

.scrim.menu-dismiss { position: fixed; inset: 0; z-index: 29; background: transparent; border: 0; backdrop-filter: none; animation: none; }

.scrim.confirm-scrim { z-index: var(--z-confirm, 60); }

.confirm-files {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--canvas);
  }

.confirm-files ol {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-height: 0;
    max-height: 40vh;
    margin: 0;
    padding: 0;
    list-style: none;
    overflow: auto;
  }

.confirm-files li {
    display: flex;
    min-width: 0;
    overflow: hidden;
    font-size: 12px;
    line-height: 18px;
    white-space: nowrap;
  }

.confirm-file-dir {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-secondary);
  }

.confirm-file-name {
    flex: 0 0 auto;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
  }

.confirm-count {
    color: var(--text-secondary);
    font-size: 12px;
  }

.dialog.confirm {
    width: min(460px, calc(100vw - 32px));
    gap: 14px;
    padding: 24px 24px 18px;
  }

.confirm-head {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    min-width: 0;
  }

.confirm-icon {
    width: 30px;
    height: 30px;
    flex: none;
    display: grid;
    place-items: center;
    margin-top: 1px;
    border-radius: calc(var(--radius) + 2px);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
  }

.confirm-icon.danger {
    background: color-mix(in srgb, var(--deleted) 13%, transparent);
    color: var(--deleted);
  }

.confirm-icon svg {
    width: 17px;
    height: 17px;
  }

.confirm-copy {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    flex: 1;
  }

.dialog.confirm h2 {
    position: static;
    padding: 0;
    background: transparent;
    color: var(--text);
    font-size: 16px;
    font-weight: 650;
    line-height: 1.3;
    letter-spacing: -0.015em;
  }

.confirm-title-text {
    display: block;
    min-width: 0;
    overflow-wrap: anywhere;
  }

.confirm-subject {
    min-width: 0;
    max-width: 100%;
    max-height: 9.5em;
    padding: 9px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--canvas);
    color: var(--text);
    font-family: var(--mono, ui-monospace, SFMono-Regular, Menlo, Consolas, monospace);
    font-size: 12px;
    line-height: 1.55;
    white-space: pre-wrap;
    overflow: auto;
    overflow-wrap: anywhere;
    user-select: text;
  }

.dialog.confirm.danger .confirm-subject {
    border-color: color-mix(in srgb, var(--deleted) 24%, transparent);
    background: color-mix(in srgb, var(--deleted) 5%, var(--canvas));
  }

.dialog.confirm .confirm-body {
    margin: 0;
    font-size: 13px;
    line-height: 1.55;
    color: var(--text-secondary);
  }

.dialog.confirm .confirm-detail {
    padding: 9px 12px;
    border-left: 2px solid color-mix(in srgb, var(--accent) 38%, transparent);
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
    background: var(--field);
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 1.55;
    max-height: 20vh;
    overflow: auto;
    white-space: pre-wrap;
  }

.dialog.confirm.danger .confirm-detail {
    border-left-color: color-mix(in srgb, var(--deleted) 45%, transparent);
  }

.confirm-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 2px;
    padding-top: 16px;
    border-top: 1px solid var(--line);
  }

.confirm-actions .text-button,
  .confirm-actions .commit {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 32px;
    margin-left: 0;
  }

.confirm-actions .text-button {
    min-width: 78px;
    padding: 0 14px;
    border: 1px solid var(--line);
    background: var(--elevated);
  }

.confirm-actions .text-button:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--text-secondary) 35%, transparent);
    background: var(--hover);
  }

.confirm-actions .commit {
    min-width: 88px;
    padding: 0 16px;
  }

.dialog .commit.danger {
    background: var(--deleted);
    color: var(--on-deleted);
  }

.dialog .commit.danger:focus-visible {
    outline-color: var(--deleted);
  }

.shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--canvas);
    color: var(--text);
    font-size: var(--ui-scale, 13px);
    user-select: none;
    overflow: hidden;
  }

.repo-picker {
    position: fixed;
    z-index: var(--z-popover, 31);
    width: 292px;
    display: flex;
    flex-direction: column;
    background: var(--elevated);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow);
    overflow: hidden;
  }

.repo-picker:focus,
.repo-picker:focus-visible,
.command-log:focus,
.command-log:focus-visible,
.palette:focus,
.palette:focus-visible {
    outline: none;
  }

.repo-picker h2 {
    margin: 0;
    padding: 14px 14px 6px;
    font-size: 16px;
    font-weight: 600;
    line-height: 1.3;
    flex: none;
  }

.repo-picker-scroll {
    overflow: auto;
    min-height: 0;
    flex: 1;
    padding: 0 6px 8px;
  }

.repo-section,
  .repo-row {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-height: 28px;
    height: auto;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
  }

.repo-section {
    padding: 4px 6px;
    font-weight: 600;
  }

.repo-row {
    padding: 4px 8px 4px 22px;
    border-radius: var(--radius-sm);
  }

.repo-row.folder { padding-left: 6px; }

.repo-row.nested { padding-left: 40px; }

.repo-section:hover,
  .repo-row:hover { background: var(--hover); }

.repo-row.current,
  .repo-row.current:hover {
    background: var(--accent);
    color: var(--on-accent);
  }

.repo-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

.repo-glyph {
    width: 16px;
    height: 16px;
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.3;
    stroke-linejoin: round;
    color: var(--text-secondary);
  }

.repo-glyph.folder {
    fill: currentColor;
    stroke: none;
    color: var(--icon-folder);
  }

.repo-row.current .repo-glyph { color: var(--on-accent); }

.repo-chevron {
    width: 12px;
    height: 12px;
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
    color: var(--text-secondary);
    transition: transform 120ms ease;
  }

.repo-chevron.open { transform: rotate(90deg); }

.repo-dot {
    width: 8px;
    height: 8px;
    margin-left: auto;
    border: 1.5px solid var(--deleted);
    border-radius: 50%;
    flex: none;
  }

.repo-row.current .repo-dot { border-color: var(--on-accent); }

.repo-empty {
    margin: 0;
    padding: 2px 8px 8px 22px;
    color: var(--text-secondary);
  }

.command-log {
    position: fixed;
    z-index: var(--z-popover, 31);
    display: grid;
    grid-template-columns: minmax(240px, 320px) minmax(0, 1fr);
    background: var(--elevated);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow);
    overflow: hidden;
  }

.command-list,
  .command-detail {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

.command-detail { border-left: 1px solid var(--line); }

.command-filters {
    display: flex;
    flex-wrap: wrap;
    gap: 0 16px;
    padding: 4px 14px 0;
    border-bottom: 1px solid var(--line);
    flex: none;
  }

.command-filters button {
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--text-secondary);
    font: inherit;
    padding: 8px 0;
    white-space: nowrap;
  }

.command-filters button.on {
    color: var(--accent);
    border-bottom-color: var(--accent);
    font-weight: 650;
  }

.command-filters button.alert {
    color: var(--deleted);
  }

.command-filters button.alert.on {
    color: var(--deleted);
    border-bottom-color: var(--deleted);
  }

.command-filters button em {
    margin-left: 4px;
    font-style: normal;
    font-weight: 650;
  }

.notice-actions {
    flex: none;
    padding: 10px 14px 14px;
    border-top: 1px solid var(--line);
  }

.command-item.failed:not(.current) .title { color: var(--deleted); }

.command-items,
  .command-detail pre {
    overflow: auto;
    min-height: 0;
  }

.command-items { flex: 1; padding: 6px; }

.command-item {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 2px 10px;
    width: 100%;
    padding: 8px 10px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
  }

.command-item .title {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

.command-item .time { color: var(--text-secondary); font-size: 12px; }

.command-item .summary {
    grid-column: 1 / -1;
    color: var(--text-secondary);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

.command-item.notice .summary {
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    white-space: normal;
    line-height: 1.4;
  }

.command-item:hover { background: var(--hover); }

.command-item.current,
  .command-item.current:hover { background: var(--accent); color: var(--on-accent); }

.command-item.current .time,
  .command-item.current .summary { color: color-mix(in srgb, var(--on-accent) 78%, transparent); }

.command-detail header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 40px 8px 16px;
    flex: none;
  }

.command-detail header strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

.command-detail header span {
    color: var(--text-secondary);
    font-size: 12px;
    white-space: nowrap;
  }

.command-detail pre {
    margin: 0;
    padding: 0 16px 16px;
    flex: 1;
    font-family: var(--mono);
    font-size: 12.5px;
    line-height: 1.55;
    white-space: pre-wrap;
    user-select: text;
  }

.command-detail pre strong { font-weight: 700; }

.file-access-notice {
    display: block;
    margin: 10px 0;
    padding: 14px;
    border: 1px solid color-mix(in srgb, var(--deleted) 40%, transparent);
    border-left: 4px solid var(--deleted);
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--deleted) 7%, var(--elevated));
    color: var(--text);
    font-size: 14px;
    line-height: 1.65;
    overflow-wrap: anywhere;
  }

.file-access-heading { font-size: 16px; font-weight: 800; color: var(--deleted); }
.file-access-important { font-weight: 700; }

.palette-scrim {
    position: fixed;
    inset: 0;
    z-index: var(--z-palette, 40);
    background: rgba(24, 24, 27, 0.28);
    display: flex;
    align-items: center;
    justify-content: center;
  }

.palette {
    width: 520px;
    max-width: calc(100vw - 32px);
    border-radius: var(--radius-lg);
    background: var(--elevated);
    border: 1px solid var(--line);
    box-shadow: var(--shadow);
    overflow: hidden;
  }

.palette-input {
    width: 100%;
    height: 44px;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 15px;
    padding: 0 4px;
    outline: none;
  }

.palette-query:focus-within {
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 45%, transparent);
  }

.palette-query {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    border-bottom: 1px solid var(--line);
  }

.palette-chip {
    flex: none;
    height: 24px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    background: var(--field);
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 24px;
  }

.palette-list {
    max-height: 360px;
    overflow: auto;
    padding: 4px;
    border-top: 1px solid var(--line);
  }

.palette-list button {
    width: 100%;
    height: 36px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    border-radius: var(--radius);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 10px;
    text-align: left;
  }

.palette-list button.on { background: var(--selection); }

.palette-hint {
    height: 32px;
    display: flex;
    align-items: center;
    padding: 0 16px;
    border-top: 1px solid var(--line);
    color: var(--text-secondary);
    font-size: 12px;
  }

.body {
    flex: 1;
    min-height: 0;
    min-width: 0;
    display: flex;
    overflow: hidden;
    position: relative;
  }
</style>

<PerfPanel />

<div class="shell" class:vertical-tabs={shell.settings.verticalTabs} class:diff-wrap={shell.settings.diffWordWrap} style:--diff-font="{shell.settings.diffFontSize || 13}px" style:--guide="{shell.settings.pageGuide || 72}" data-lab-scene={shell.labSceneId || undefined} data-lab-error={shell.labPage && !shell.labSceneId && shell.loadError ? shell.loadError : undefined}>
  {#if shell.standalone === "merge"}
    <MergePanel />
  {:else if shell.standalone === "history"}
    <FileHistoryPane />
  {:else if shell.standalone === "blob"}
    <BlobPane />
  {:else}
  <Chrome />

  {#if !shell.snapshot && shell.mode !== "sample"}
    <Welcome />
  {:else}
  <div class="body">
    {#if !shell.sidebarCollapsed}
    <Sidebar />
    {/if}

    <SplitHandle
      direction="vertical"
      bind:size={shell.sidebarWidth}
      min={180}
      max={480}
      defaultSize={240}
      collapsible
      label={shell.tr("chrome.resizeSidebar")}
      bind:collapsed={shell.sidebarCollapsed}
      onchange={shell.handleLayoutChange.bind(shell)}
    />

    {#if shell.section === "changes"}
      <ChangesView />
    {:else}
      <HistoryView />
    {/if}

    

    

    

    

    

    

    

    
  </div>
  {/if}
  {/if}

  

  
  
  
  
  
  
  
  
  
  
  
  
  

  {#snippet repoChoice(path: string, nested: boolean)}
    <button class="repo-row" class:nested class:current={shell.currentRepo(path)} type="button" title={path} onclick={() => shell.pickRepo(path)}>
      <svg class="repo-glyph" viewBox="0 0 16 16" aria-hidden="true"><path d="M4 1.75h5.2L12.25 4.8V14.25H4z"/><path d="M9.1 1.75V5h3.15"/></svg>
      <span class="repo-name">{shell.tabText(path)}</span>
      {#if shell.currentRepo(path) && shell.changeCount > 0}<span class="repo-dot" aria-hidden="true"></span>{/if}
    </button>
  {/snippet}
  {#snippet repoChevron(open: boolean)}
    <svg class="repo-chevron" class:open viewBox="0 0 16 16" aria-hidden="true"><path d="M6 4.5 10 8l-4 3.5"/></svg>
  {/snippet}

  {#snippet menuList(items: MenuItems, nested = false)}
    {@const list = typeof items === "function" ? items() : items}
    {#each list as item, index (`${item.sep ? "sep" : item.label}-${index}`)}
      {#if item.sep}
        <hr />
      {:else}
        <button type="button" role={item.checked !== undefined ? "menuitemradio" : "menuitem"} disabled={item.disabled} aria-checked={item.checked !== undefined ? item.checked : undefined} aria-haspopup={item.children ? "menu" : undefined} aria-expanded={item.children ? shell.submenu?.items === item.children : undefined} onmouseenter={(event) => { if (item.children) { openSubmenu(item, event.currentTarget as HTMLElement); return; } if (!nested) shell.submenu = null; }} onclick={(event) => { if (item.disabled) return; if (item.children) { openSubmenu(item, event.currentTarget as HTMLElement, true); return; } if (!item.run) return; shell.dismissMenu(); void item.run(); }}>
          <span class="menu-main">
            {#if item.checked !== undefined}<span class="menu-check" aria-hidden="true">{item.checked ? "✓" : ""}</span>{/if}
            <span>{item.label}</span>
          </span>
          {#if item.shortcut}<span class="shortcut">{item.shortcut}</span>{/if}
          {#if item.children}<span class="shortcut">›</span>{/if}
        </button>
      {/if}
    {/each}
  {/snippet}

  {#snippet commandOutput(output: string, command = "")}
    {@const report = fileAccessReport(output)}
    <pre>{#if command}<strong>$ {command}</strong>{/if}{#if output}{command ? "\n\n" : ""}{#if report}<span class="file-access-notice" role="note">{#each report.lines as line, index}<span class:file-access-heading={index === 0 || line === "How to release access:"} class:file-access-important={/^(File:|Cause:|In use by:)/.test(line)}>{line}{index < report.lines.length - 1 ? "\n" : ""}</span>{/each}</span>{report.detail ? `\n${report.detail}` : ""}{:else}{output}{/if}{/if}</pre>
  {/snippet}

  {#if shell.commandLogOpen}
    {@const picked = shell.pickedCommand()}
    <div class="command-log" data-layer="commands" style:left="{shell.commandPos.x}px" style:top="{shell.commandPos.y}px" style:width="{shell.commandPos.width}px" style:height="{shell.commandPos.maxHeight}px" role="dialog" aria-modal="true" aria-label={shell.tr("chrome.commandHistory")} tabindex="-1" use:overlayFocus={{ close: () => (shell.commandLogOpen = false), focus: ".command-item.current" }}>
      <button class="dialog-close" type="button" aria-label={shell.tr("chrome.close")} title={shell.tr("chrome.close")} onclick={() => (shell.commandLogOpen = false)}>×</button>
      <div class="command-list">
        <div class="command-filters">
          <button type="button" class:on={shell.commandFilter === "all"} onclick={() => shell.showCommandFilter("all")}>{shell.tr("chrome.commandAll")}</button>
          <button type="button" class:on={shell.commandFilter === "user"} onclick={() => shell.showCommandFilter("user")}>{shell.tr("chrome.commandUser")}</button>
          <button type="button" class:on={shell.commandFilter === "background"} onclick={() => shell.showCommandFilter("background")}>{shell.tr("chrome.commandBackground")}</button>
          <button type="button" class:on={shell.commandFilter === "errors"} class:alert={shell.recentErrors.length > 0} onclick={() => shell.showCommandFilter("errors")}>{shell.tr("chrome.commandErrors")}{#if shell.recentErrors.length > 0}<em>{shell.recentErrors.length}</em>{/if}</button>
          <button type="button" class:on={shell.commandFilter === "notices"} onclick={() => shell.showCommandFilter("notices")}>{shell.tr("chrome.commandNotices")}{#if shell.statusNotices.length > 0}<em>{shell.statusNotices.length}</em>{/if}</button>
        </div>
        <div class="command-items">
          {#each shell.visibleCommands() as item (item.id)}
            <button class="command-item" class:current={item.id === shell.commandPick} class:failed={!item.ok} class:notice={!!item.noticeId} type="button" onclick={() => (shell.commandPick = item.id)}>
              <span class="title">{item.title}</span>
              <span class="time">{shell.commandClock(item.at)}</span>
              <span class="summary" title={item.summary}>{item.summary}</span>
            </button>
          {:else}
            <p class="repo-empty">{shell.tr("chrome.commandEmpty")}</p>
          {/each}
        </div>
      </div>
      <div class="command-detail">
        {#if picked?.noticeId}
          {@const notice = shell.pickedNotice()}
          <header data-layer={notice?.dataLayer}>
            <strong>{picked.title}</strong>
            <span>{shell.tr("chrome.commandNotices")}</span>
          </header>
          {#if picked.output}{@render commandOutput(picked.output)}{/if}
          {#if notice && notice.actions.length > 0}
            <div class="composer-row notice-actions">
              {#each notice.actions as action (action.id)}
                <button class="text-button" type="button" disabled={action.disabled} onclick={() => shell.runStatusNoticeAction(notice.id, action.id)}>{action.label}</button>
              {/each}
            </div>
          {/if}
        {:else if picked}
          <header>
            <strong>{picked.title}</strong>
            <span>{picked.ok ? shell.tr("chrome.commandSucceeded") : shell.tr("chrome.commandFailed")} {shell.commandStamp(picked.at)}</span>
            {#if !picked.ok}<button class="text-button" type="button" onclick={() => shell.copyNoted(picked.output)}>{shell.errorCopied ? shell.tr("chrome.copied") : shell.tr("chrome.copyError")}</button>{/if}
          </header>
          {@render commandOutput(picked.output, picked.command)}
        {:else}
          <p class="repo-empty">{shell.tr("chrome.commandEmpty")}</p>
        {/if}
      </div>
    </div>
    <button class="scrim menu-dismiss" type="button" aria-label={shell.tr("chrome.close")} onclick={() => (shell.commandLogOpen = false)}></button>
  {/if}
  {#if shell.repoPickerOpen}
    <div class="repo-picker" data-layer="repos" style:left="{shell.repoPickerPos.x}px" style:top="{shell.repoPickerPos.y}px" style:max-height="{shell.repoPickerPos.maxHeight}px" role="dialog" aria-modal="true" aria-label={shell.tr("chrome.manager")} tabindex="-1" use:overlayFocus={{ close: () => (shell.repoPickerOpen = false), focus: ".repo-row.current" }}>
      <button class="dialog-close" type="button" aria-label={shell.tr("chrome.close")} title={shell.tr("chrome.close")} onclick={() => (shell.repoPickerOpen = false)}>×</button>
      <h2>{shell.tr("chrome.manager")}</h2>
      <div class="repo-picker-scroll">
        <button class="repo-section" type="button" aria-expanded={shell.repoPickerRecentOpen} onclick={() => (shell.repoPickerRecentOpen = !shell.repoPickerRecentOpen)}>
          {@render repoChevron(shell.repoPickerRecentOpen)}
          <span>{shell.tr("chrome.recent")}</span>
        </button>
        {#if shell.repoPickerRecentOpen}
          {#each shell.recentPaths() as path (path)}
            {@render repoChoice(path, false)}
          {:else}
            <p class="repo-empty">{shell.tr("chrome.noRecentRepos")}</p>
          {/each}
        {/if}
        <button class="repo-section" type="button" aria-expanded={shell.repoPickerReposOpen} onclick={() => (shell.repoPickerReposOpen = !shell.repoPickerReposOpen)}>
          {@render repoChevron(shell.repoPickerReposOpen)}
          <span>{shell.tr("chrome.repositories")}</span>
        </button>
        {#if shell.repoPickerReposOpen}
          {#each shell.repositoryEntries() as entry (entry.kind === "folder" ? `folder:${entry.path}` : entry.path)}
            {#if entry.kind === "folder"}
              <button class="repo-row folder" type="button" aria-expanded={shell.repoPickerExpanded.includes(entry.path)} onclick={() => shell.toggleRepoFolder(entry.path)}>
                {@render repoChevron(shell.repoPickerExpanded.includes(entry.path))}
                <svg class="repo-glyph folder" viewBox="0 0 16 16" aria-hidden="true"><path d="M1.6 3.4c0-.8.7-1.5 1.5-1.5h3.1l1.2 1.4h5.5c.8 0 1.5.7 1.5 1.5v7.3c0 .8-.7 1.5-1.5 1.5H3.1c-.8 0-1.5-.7-1.5-1.5V3.4z"/></svg>
                <span class="repo-name">{entry.name}</span>
              </button>
              {#if shell.repoPickerExpanded.includes(entry.path)}
                {#each entry.repos as path (path)}
                  {@render repoChoice(path, true)}
                {/each}
              {/if}
            {:else}
              {@render repoChoice(entry.path, false)}
            {/if}
          {:else}
            <p class="repo-empty">{shell.repoScanning ? shell.tr("chrome.reading") : shell.tr("chrome.noRepositories")}</p>
          {/each}
          {#if shell.repoScanTruncated}<p class="repo-empty" data-scan-truncated>{shell.tr("chrome.scanTruncated")}</p>{/if}
        {/if}
      </div>
    </div>
    <button class="scrim menu-dismiss" type="button" aria-label={shell.tr("chrome.close")} onclick={() => (shell.repoPickerOpen = false)}></button>
  {/if}
  {#if shell.menu}
    <div class="menu" data-layer="menu" use:fitMenu={shell.menu} use:menuKeys role="menu">
      {@render menuList(shell.menu.items)}
    </div>
    {#if shell.submenu}
      <div class="menu" data-layer="submenu" use:fitMenu={shell.submenu} use:menuKeys={false} role="menu">
        {@render menuList(shell.submenu.items, true)}
      </div>
    {/if}
    <button class="scrim menu-dismiss" type="button" aria-label={shell.tr("chrome.close")} onclick={() => shell.dismissMenu()}></button>
  {/if}
  {#if shell.confirmAsk}
    {@const ask = shell.confirmAsk}
    <div class="scrim confirm-scrim" role="presentation">
      <div class="dialog sm confirm" class:danger={ask.danger} data-confirm data-layer="confirm" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" aria-describedby={[ask.body ? "confirm-body" : "", ask.detail ? "confirm-detail" : ""].filter(Boolean).join(" ") || undefined} tabindex="-1" use:overlayFocus={{ close: () => (shell.confirmAsk = null), focus: ask.danger ? ".text-button" : ".commit" }}>
        <div class="confirm-head">
          <span class="confirm-icon" class:danger={ask.danger} aria-hidden="true">
            {#if ask.danger}
              <svg class="line-icon" viewBox="0 0 24 24"><path d="M10.3 4.2 2.6 18a2 2 0 0 0 1.7 3h15.4a2 2 0 0 0 1.7-3L13.7 4.2a2 2 0 0 0-3.4 0Z"/><path d="M12 9.4v4.3"/><path d="M12 17.3h.01"/></svg>
            {:else}
              <svg class="line-icon" viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.5"/><path d="M9.6 9.3a2.5 2.5 0 1 1 3.3 2.4c-.62.24-.9.72-.9 1.34v.36"/><path d="M12 16.6h.01"/></svg>
            {/if}
          </span>
          <div class="confirm-copy">
            <h2 id="confirm-title"><span class="confirm-title-text">{ask.title}</span></h2>
            {#if ask.body}<p class="confirm-body" id="confirm-body">{ask.body}</p>{/if}
          </div>
        </div>
        {#if ask.subject}<div class="confirm-subject" data-confirm-subject title={ask.subject}>{ask.subject}</div>{/if}
        {#if ask.detail}<p class="confirm-detail" id="confirm-detail">{ask.detail}</p>{/if}
        {#if ask.files?.length}
          <div class="confirm-files" data-confirm-files>
            <div class="confirm-count">{shell.tr("dialog.fileCount", { count: String(ask.files.length) })}</div>
            <ol>
              {#each ask.files as file (file)}
                {@const cut = file.lastIndexOf("/")}
                <li title={file}><span class="confirm-file-dir">{cut > 0 ? file.slice(0, cut + 1) : ""}</span><span class="confirm-file-name">{cut > 0 ? file.slice(cut + 1) : file}</span></li>
              {/each}
            </ol>
          </div>
        {/if}
        <div class="confirm-actions">
          <button class="text-button" type="button" onclick={() => (shell.confirmAsk = null)}>{shell.tr("dialog.cancel")}</button>
          <button class="commit" class:danger={ask.danger} type="button" onclick={() => { const run = shell.confirmAsk?.run; shell.confirmAsk = null; run?.(); }}>{ask.confirmLabel ?? shell.tr("dialog.confirm")}</button>
        </div>
      </div>
    </div>
  {/if}
  {#if shell.forgeOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="scrim" role="presentation" onclick={() => (shell.forgeOpen = null)} onkeydown={() => {}}>
      <div class="dialog md" data-layer="forge" role="dialog" aria-modal="true" aria-labelledby="forge-title" tabindex="-1" use:overlayFocus={{ close: () => (shell.forgeOpen = null) }} onclick={(event) => event.stopPropagation()}>
        <button class="dialog-close" type="button" aria-label={shell.tr("chrome.close")} title={shell.tr("chrome.close")} onclick={() => (shell.forgeOpen = null)}>×</button>
        <h2 id="forge-title">{shell.forgeOpen === "notes" ? shell.tr("chrome.notifications") : shell.tr("chrome.pulls")}</h2>
        {#if shell.forgeOpen === "notes"}
          {#if shell.notices.length === 0}<p class="empty">{shell.settings.githubToken || shell.settings.gitlabToken || shell.mode !== "live" ? shell.tr("dialog.noNotifications") : shell.tr("dialog.noToken")}</p>{/if}
          {#each shell.notices as notice (notice.id)}
            <div class="composer-row"><span>{notice.title}</span><span class="grow"></span><button class="text-button" type="button" onclick={() => openUrl(notice.url)}>{shell.tr("dialog.openBrowser")}</button><button class="text-button" type="button" onclick={() => { if (shell.mode === "live") void invoke("mark_notification", { id: notice.id }).then(() => (shell.notices = shell.notices.filter((item) => item.id !== notice.id))); else shell.notices = shell.notices.filter((item) => item.id !== notice.id); }}>{shell.tr("dialog.markRead")}</button></div>
          {/each}
          {#if shell.notices.length > 0}
            <div class="composer-row">
              <button class="text-button" type="button" onclick={() => { const ids = shell.notices.map((item) => item.id); if (shell.mode === "live") void Promise.all(ids.map((id) => invoke("mark_notification", { id }))).then(() => (shell.notices = [])).catch((error) => (shell.actionError = shell.message(error))); else shell.notices = []; }}>{shell.tr("dialog.markAllRead")}</button>
            </div>
          {/if}
        {:else}
          {#if shell.pulls.length === 0}<p class="empty">{shell.settings.githubToken || shell.settings.gitlabToken || shell.mode !== "live" ? shell.tr("dialog.noPulls") : shell.tr("dialog.noToken")}</p>{/if}
          {#each shell.pulls as pull (pull.url)}
            <div class="composer-row"><span>{pull.title}</span><span class="meta">{pull.author}</span><span class="grow"></span><button class="text-button" type="button" disabled={shell.mode !== "live"} onclick={() => shell.mutate({ action: "checkout", name: pull.branch })}>{shell.tr("dialog.checkoutBranch")}</button></div>
          {/each}
        {/if}
      </div>
    </div>
  {/if}
  <DialogHost />
  {#if shell.launchOpen}
    <div class="palette-scrim" role="presentation" onclick={() => shell.closeLaunch()}>
      <div class="palette" data-layer="launch" role="dialog" aria-modal="true" tabindex="-1" aria-label={shell.tr("chrome.quickLaunch")} use:overlayFocus={{ close: () => shell.closeLaunch(), focus: ".palette-input" }} onclick={(event) => event.stopPropagation()} onkeydown={() => {}}>
        <div class="palette-query">
          {#if shell.launchMode === "files"}
            <span class="palette-chip">{shell.tr("chrome.fileHistory")}</span>
          {/if}
          <input
            class="palette-input"
            placeholder={shell.launchMode === "files" ? shell.tr("chrome.repositoryFiles") : shell.tr("chrome.quickLaunch")}
            aria-label={shell.tr("chrome.quickLaunch")}
            bind:this={shell.launchEl}
            bind:value={shell.launch}
            oninput={() => (shell.launchIndex = 0)}
            onkeydown={shell.onLaunchKey.bind(shell)}
          />
        </div>
        <div class="palette-list" bind:this={shell.launchList}>
          {#each shell.matches as item, index (`${item.group}-${item.label}`)}
            <button type="button" class:on={index === shell.launchIndex} onmouseenter={() => (shell.launchIndex = index)} onclick={() => shell.runLaunch(item)}>
              <span class="name">{item.label}</span>
              <span class="meta">{item.group}</span>
            </button>
          {:else}
            <p class="empty">{shell.tr("chrome.noPaletteMatch")}</p>
          {/each}
        </div>
        <div class="palette-hint">{shell.tr("chrome.paletteHint")}</div>
      </div>
    </div>
  {/if}
</div>


