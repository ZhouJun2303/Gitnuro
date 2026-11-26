<script lang="ts">
  import { onDestroy, tick } from "svelte";
  import { getShell } from "./context";
  import "./Sidebar.css";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { branchGroup, onVirtualScroll, windowSlice } from "../view";
  import { liveSideRows, type SideRow } from "./sideRows";
  import { beginWorktree, occupiedWorktree, openWorktree, worktreeMarks, worktreeMenu, worktreeName } from "./worktrees";
  import type { WorktreeRow } from "../status";

  const shell = getShell();
  const pageSize = 10;
  let limits = $state({ tags: pageSize, stashes: pageSize });
  const sideRows = $derived(shell.mode === "live" && shell.refs ? liveSideRows(shell, limits) : []);
  const sideWindow = $derived(windowSlice(sideRows, shell.sideTop, shell.rowSide, 60, 80));
  const onSideScroll = onVirtualScroll((top) => (shell.sideTop = top));
  let sideScroll = $state<HTMLDivElement | null>(null);
  let searchToggleEl = $state<HTMLButtonElement | null>(null);
  let searchMenuEl = $state<HTMLElement | null>(null);
  let sideRepo = "";
  let sideQuerySeen = "";
  let locateTimer = 0;

  function clearLocate() {
    clearTimeout(locateTimer);
    locateTimer = 0;
  }
  onDestroy(clearLocate);

  function locate(event: MouseEvent, name: string) {
    clearLocate();
    if (shell.pendingReference(name)) return;
    if (event.detail > 1) return;
    if (event.detail === 0) {
      void shell.locateRef(name);
      return;
    }
    const path = shell.repoPath();
    const gen = shell.openGen;
    locateTimer = window.setTimeout(() => {
      locateTimer = 0;
      if (gen === shell.openGen && shell.repoPath() === path) void shell.locateRef(name);
    }, 280);
  }

  function checkout(name: string, remote?: { remote: string; branch: string }) {
    clearLocate();
    if (shell.pendingReference(name)) return;
    void shell.quickCheckout(name, remote);
  }

  function searchItems() {
    return [...(searchMenuEl?.querySelectorAll<HTMLButtonElement>(".side-search-item") ?? [])];
  }

  function focusSearchItem(last = false) {
    void tick().then(() => {
      const items = searchItems();
      (last ? items[items.length - 1] : items[0])?.focus();
    });
  }

  function closeSearchMenu(restore: boolean) {
    shell.commitSearchOpen = false;
    if (restore) void tick().then(() => searchToggleEl?.focus());
  }

  function toggleSearchMenu(event: MouseEvent) {
    if (shell.commitSearchOpen) {
      closeSearchMenu(false);
      return;
    }
    shell.commitSearchOpen = true;
    if (event.detail === 0) focusSearchItem();
  }

  function searchToggleKeys(event: KeyboardEvent) {
    if (event.key === "Escape" && shell.commitSearchOpen) {
      event.preventDefault();
      event.stopPropagation();
      closeSearchMenu(true);
      return;
    }
    if (shell.commitSearchOpen || (event.key !== "ArrowDown" && event.key !== "ArrowUp")) return;
    event.preventDefault();
    event.stopPropagation();
    shell.commitSearchOpen = true;
    focusSearchItem(event.key === "ArrowUp");
  }

  function searchMenuKeys(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      closeSearchMenu(true);
      return;
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp" && event.key !== "Home" && event.key !== "End") return;
    const items = searchItems();
    if (items.length === 0) return;
    event.preventDefault();
    event.stopPropagation();
    const at = items.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : event.key === "ArrowDown" ? (at + 1) % items.length : (at - 1 + items.length) % items.length;
    items[next]?.focus();
  }

  function searchMenuFocusOut(event: FocusEvent) {
    const next = event.relatedTarget as Node | null;
    if (!next || !(event.currentTarget as HTMLElement).contains(next)) shell.commitSearchOpen = false;
  }

  $effect(() => {
    const path = shell.snapshot?.path ?? "";
    const query = shell.sideQuery;
    if (path === sideRepo && query === sideQuerySeen) return;
    sideRepo = path;
    sideQuerySeen = query;
    limits = { tags: pageSize, stashes: pageSize };
    shell.sideTop = 0;
    if (sideScroll) sideScroll.scrollTop = 0;
  });

  function sectionActions(row: Extract<SideRow, { kind: "section" }>) {
    const actions: { label: string; run: () => void; icon?: "add" | "gear" }[] = [];
    if (row.add === "branch") actions.push({ label: shell.tr("chrome.new"), icon: "add", run: () => { shell.draft = ""; shell.draftExtra = ""; shell.dialog = "branch"; } });
    if (row.add === "remote") actions.push({ label: shell.tr("chrome.add"), icon: "add", run: () => { shell.remoteEdit = false; shell.draft = ""; shell.draftExtra = ""; shell.dialog = "remote"; } });
    if (row.add === "submodule") actions.push({ label: shell.tr("chrome.add"), icon: "add", run: () => { shell.draft = ""; shell.draftExtra = ""; shell.dialog = "submodule"; } });
    if (row.add === "worktree") actions.push(
      { label: shell.tr("worktree.manage"), icon: "gear", run: () => beginWorktree(shell, { kind: "manage" }) },
      { label: shell.tr("worktree.add"), icon: "add", run: () => beginWorktree(shell) },
    );
    if (row.showAll) actions.push({ label: shell.tr("chrome.showAll"), run: () => shell.showAllRefs() });
    return actions;
  }

</script>

<svelte:window onclick={() => { if (shell.commitSearchOpen) shell.commitSearchOpen = false; }} />

<aside class="sidebar" style:width="{shell.sidebarWidth}px" style:--sidebar-w="{shell.sidebarWidth}px">
      {#snippet glyph(kind: string)}
        <svg class="line-icon side-glyph" class:check={kind === "check"} viewBox="0 0 24 24" aria-hidden="true">
          {#if kind === "changes"}
            <path d="M6 3.5h8l4 4V20.5H6z"/><path d="M14 3.5v4h4"/><path d="M9 13h6M9 17h4"/>
          {:else if kind === "commits"}
            <path d="M9 6h11M9 12h11M9 18h11M4 6h.01M4 12h.01M4 18h.01"/>
          {:else if kind === "branch"}
            <path d="M6 3v12"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/>
          {:else if kind === "search"}
            <circle cx="11" cy="11" r="6"/><path d="M15.5 15.5 20 20"/>
          {:else if kind === "folder"}
            <path d="M3 7.5V17.5A1.5 1.5 0 0 0 4.5 19h15a1.5 1.5 0 0 0 1.5-1.5v-8A1.5 1.5 0 0 0 19.5 8H12l-2-2H4.5A1.5 1.5 0 0 0 3 7.5z"/>
          {:else if kind === "check"}
            <path d="M5 12.5 9.2 17 19 7"/>
          {:else if kind === "remote"}
            <path d="M7 17.5h9.2a3 3 0 0 0 .4-6 4.4 4.4 0 0 0-8.4-1.6A3.1 3.1 0 0 0 7 17.5z"/>
          {:else if kind === "tag"}
            <path d="M4 12.2V5h7.2L20 13.8 13.8 20z"/><circle cx="8" cy="8.2" r="1.1"/>
          {:else if kind === "stash"}
            <path d="M4 9h16v10H4z"/><path d="M12 4v7M9 8.5 12 11.5 15 8.5"/>
          {:else if kind === "submodule"}
            <path d="M4 9h16v9H4z"/><path d="M7 9V6.5h10V9M8 13h8"/>
          {:else if kind === "worktree"}
            <rect x="3.5" y="4" width="7" height="6" rx="1"/><rect x="13.5" y="4" width="7" height="6" rx="1"/><rect x="3.5" y="14" width="7" height="6" rx="1"/><rect x="13.5" y="14" width="7" height="6" rx="1"/>
          {:else if kind === "add"}
            <path d="M12 5v14M5 12h14"/>
          {:else if kind === "gear"}
            <circle cx="12" cy="12" r="3"/><path d="M12 3.5v2.2M12 18.3V20.5M3.5 12h2.2M18.3 12h2.2M5.8 5.8l1.6 1.6M16.6 16.6l1.6 1.6M18.2 5.8l-1.6 1.6M7.4 16.6l-1.6 1.6"/>
          {:else if kind === "pin" || kind === "unpin"}
            <path d="M9 3.5h6v6.2l2.2 2.1v2.2h-4.1L12 20.5l-1.1-6.5H6.8v-2.2L9 9.7z" fill={kind === "unpin" ? "currentColor" : "none"}/>
          {:else if kind === "funnel" || kind === "funnel-plus" || kind === "funnel-off"}
            <path d="M4 5h16l-6.2 7.2V18l-3.6 2v-7.8z" fill={kind === "funnel-off" ? "currentColor" : "none"}/>
            {#if kind === "funnel-plus"}<path d="M16 17h6M19 14v6"/>{/if}
          {:else if kind === "clear"}
            <path d="m6 6 12 12M18 6 6 18"/>
          {:else if kind === "eye"}
            <path d="M2.5 12S6.2 6.5 12 6.5 21.5 12 21.5 12 17.8 17.5 12 17.5 2.5 12 2.5 12z"/><circle cx="12" cy="12" r="2.4"/>
          {/if}
          {#if kind === "unpin" || kind === "funnel-off"}
            <path d="M3 3l18 18" stroke="var(--accent)" stroke-width="5.5"/>
            <path d="M3 3l18 18" stroke-width="2.5"/>
          {/if}
        </svg>
      {/snippet}
      {#snippet chevron(open: boolean)}
        <svg class="line-icon side-chevron" viewBox="0 0 24 24" aria-hidden="true">
          {#if open}<path d="m6 9 6 6 6-6"/>{:else}<path d="m9 6 6 6-6 6"/>{/if}
        </svg>
      {/snippet}

      <div class="side-pin">
      <div class="side-brand">
        <span class="side-wordmark">AweGit</span>
        <button class="icon-btn" type="button" title={shell.tr("chrome.preferences")} aria-label={shell.tr("chrome.preferences")} onclick={() => (shell.dialog = "prefs")}>{@render glyph("gear")}</button>
      </div>
      <button class="side nav" type="button" data-section="changes" class:selected={shell.section === "changes"} onclick={() => (shell.section = "changes")}>
        {@render glyph("changes")}
        <span class="name">{shell.tr("chrome.localChanges")}</span>
        {#if shell.changeCount > 0}<span class="count">{shell.changeCount}</span>{/if}
      </button>
      <button class="side nav" type="button" data-section="history" class:selected={shell.section === "history"} onclick={() => (shell.section = "history")}>
        {@render glyph("commits")}
        <span class="name">{shell.tr("chrome.allCommits")}</span>
      </button>
      <div class="side-tools">
        <button class="icon-btn side-tool" class:on={shell.sideView === "refs"} type="button" title={shell.tr("chrome.localBranches")} aria-label={shell.tr("chrome.localBranches")} aria-pressed={shell.sideView === "refs"} onclick={() => shell.showSideRefs()}>{@render glyph("branch")}</button>
        <button class="icon-btn side-tool" class:on={shell.sideView === "search"} type="button" title={shell.tr("chrome.searchCommits")} aria-label={shell.tr("chrome.searchCommits")} aria-pressed={shell.sideView === "search"} onclick={() => shell.showSideSearch()}>{@render glyph("search")}</button>
      </div>
      {#if shell.sideView === "refs"}
      <label class="side-filter">
        {@render glyph("search")}
        <input placeholder={shell.tr("chrome.filter")} aria-label={shell.tr("chrome.filterSidebar")} bind:this={shell.sideFilter} bind:value={shell.sideQuery} onkeydown={(event) => { if (event.key === "Escape") { event.stopPropagation(); shell.sideQuery = ""; } }} />
        {#if shell.sideQuery}<button class="side-filter-clear" type="button" aria-label={shell.tr("chrome.clearFilter")} onclick={() => (shell.sideQuery = "")}>×</button>{/if}
      </label>
      {/if}
      </div>

      {#snippet branchSwitches(name: string, canHide: boolean)}
        {@const pinned = shell.pinnedHere.has(name)}
        {@const filtered = shell.branchFilterSet.has(name)}
        {@const pinLabel = pinned ? shell.tr("menu.unpin") : shell.tr("menu.pin")}
        {@const filterLabel = filtered ? shell.tr("chrome.unfilterBranchHistory") : shell.tr("chrome.filterBranchHistory")}
        <div class="side-actions branch-switches" data-branch={name}>
          <button class="icon-btn" type="button" disabled={!canHide} title={shell.tr("menu.hide")} aria-label={shell.tr("menu.hide")} onclick={(event) => { event.stopPropagation(); if (canHide) shell.hideRef(name); }}>{@render glyph("eye")}</button>
          <button class="icon-btn" class:on={pinned} type="button" title={pinLabel} aria-label={`${pinLabel}: ${name}`} aria-pressed={pinned} onclick={(event) => { event.stopPropagation(); shell.togglePin(name); }}>{@render glyph(pinned ? "unpin" : "pin")}</button>
          <button class="icon-btn history-filter-toggle" data-branch={name} class:on={filtered} type="button" title={filterLabel} aria-label={`${filterLabel}: ${name}`} aria-pressed={filtered} onclick={(event) => { event.stopPropagation(); shell.soloBranch(name); }}>{@render glyph(filtered ? "funnel-off" : "funnel-plus")}</button>
        </div>
      {/snippet}

      {#snippet sideHead(key: string, label: string, count: number | null = null, actions: { label: string; run: () => void; icon?: "add" | "gear" }[] = [], empty = count === 0, filter = false)}
        <div class="side section" class:selected={shell.sideSelected === key} data-section={key} role="group">
          <button class="file-select" type="button" aria-expanded={shell.sideOpen(key)} onclick={() => shell.pickSide(key)}>
            <span class="twist">{@render chevron(shell.sideOpen(key))}</span>
            <span class="name">{label}</span>
            {#if count !== null && count > 0}<span class="count">{count}</span>{/if}
          </button>
          <div class="section-tools">
          {#if actions.length > 0}
            <div class="side-actions">
              {#each actions as action (action.label)}
                {#if action.icon}
                  <button class="icon-btn side-add" type="button" title={action.label} aria-label={action.label} onclick={action.run}>{@render glyph(action.icon)}</button>
                {:else}
                  <button class="text-button" type="button" onclick={action.run}>{action.label}</button>
                {/if}
              {/each}
            </div>
          {/if}
          {#if filter}
            {#if !shell.allBranches}
              <button class="icon-btn section-filter section-filter-clear" type="button" title={shell.tr("chrome.clearFilter")} aria-label={shell.tr("chrome.clearFilter")} onclick={(event) => { event.stopPropagation(); shell.clearHistoryFilter(); }}>{@render glyph("clear")}</button>
            {/if}
            <button class="icon-btn section-filter" class:on={!shell.allBranches} type="button" title={shell.allBranches ? shell.tr("chrome.checkedOnly") : shell.tr("chrome.allBranches")} aria-pressed={!shell.allBranches} onclick={(event) => { event.stopPropagation(); shell.toggleHistoryScope(); }}>{@render glyph("funnel")}</button>
          {/if}
          </div>
        </div>
      {/snippet}

      {#snippet sideRow(row: SideRow)}
        {#if row.kind === "section"}
          {@render sideHead(row.id, row.label, row.count, sectionActions(row), row.empty, row.filter)}
        {:else if row.kind === "more"}
          <button class="side nested" type="button" aria-label={shell.tr("chrome.showMoreSection", { section: shell.tr(`chrome.${row.section}`) })} onclick={() => (limits[row.section] += pageSize)}>
            <span class="twist"></span>
            <span class="name">{shell.tr("chrome.showMore")}</span>
          </button>
        {:else if row.kind === "folder"}
          <button class="side nested" class:deep={row.deep} type="button" aria-expanded={shell.folderOpen(row.folderKey)} onclick={() => shell.toggleFolder(row.folderKey)}>
            <span class="twist">{@render chevron(shell.folderOpen(row.folderKey))}</span>
            {@render glyph("folder")}
            <span class="name">{row.label}</span>
          </button>
        {:else if row.kind === "local"}
          {@const remoteState = shell.branchRemoteState(row.name)}
          {@const checkedOut = occupiedWorktree(shell, row.name)}
          {@const syncTitle = [row.ahead > 0 ? shell.tr("chrome.syncAhead", { count: String(row.ahead) }) : "", row.behind > 0 ? shell.tr("chrome.syncBehind", { count: String(row.behind) }) : ""].filter(Boolean).join(" · ") + (row.upstream ? ` · ${row.upstream}` : "")}
          <div
            class="side nested"
            class:deep={row.deep}
            class:current={row.current}
            class:selected={shell.sideSelected === `local:${row.name}`}
            role="group"
            oncontextmenu={(event) => shell.openMenu(event, [
              ...(!row.current ? [
                { label: shell.tr(checkedOut ? "worktree.open" : "menu.checkout"), disabled: checkedOut ? checkedOut.missing : !!shell.progress, run: () => shell.requestCheckout(row.name) },
                { label: shell.tr("menu.compareBranches"), disabled: !shell.compareWith || shell.compareWith === row.name, run: () => { shell.drops = [shell.compareWith, row.name]; void shell.compareDrops(); } },
                { label: shell.tr("menu.pushAs"), run: () => { shell.draft = row.name; shell.draftExtra = shell.refs?.remotes[0]?.name ?? "origin"; shell.draftUser = row.name; shell.dialog = "pushAs"; } },
                { label: shell.tr("menu.merge"), disabled: !!shell.progress, run: () => { shell.draft = row.name; shell.draftExtra = shell.settings.mergeNoFf ? "no-ff" : "ff"; shell.dialog = "merge"; } },
                { label: shell.tr("menu.rebaseOnto"), disabled: !!shell.progress, run: () => shell.mutate({ action: "rebase", onto: row.name, autostash: true }) },
                { label: shell.tr("menu.delete"), run: () => shell.removeBranch(row.name) },
                { label: shell.tr("menu.forceDelete"), run: () => shell.ask(shell.tr("dialog.forceDeleteBranch"), shell.tr("dialog.forceDeleteBranchBody"), shell.bindMutation({ action: "deleteBranch", name: row.name, force: true }), [], { subject: row.name, danger: true, confirmLabel: shell.tr("menu.forceDelete") }) },
              ] : []),
              { label: shell.tr("worktree.add"), run: () => beginWorktree(shell, { kind: "add", source: "refs/heads/" + row.name, branch: row.name }) },
              { label: shell.pinnedHere.has(row.name) ? shell.tr("menu.unpin") : shell.tr("menu.pin"), run: () => shell.togglePin(row.name) },
              { label: shell.tr("menu.createPr"), disabled: !shell.pullRequestUrl(row.name), run: () => void openUrl(shell.pullRequestUrl(row.name)) },
              { label: shell.tr("menu.rename"), run: () => { shell.draft = row.name; shell.draftExtra = row.name; shell.dialog = "rename"; } },
              { label: shell.tr("menu.setUpstream"), run: () => { const branch = shell.refs?.branches.find((item) => item.name === row.name); shell.draft = row.name; shell.draftExtra = branch?.upstream ?? ""; shell.dialog = "upstream"; } },
              { label: shell.tr("menu.copyName"), run: () => shell.copyText(row.name) },
              { label: shell.tr("menu.hide"), run: () => shell.hideRef(row.name) },
              { label: shell.tr("menu.showOnly"), run: () => shell.showOnly(row.name) },
              ...shell.commandItems("branch"),
            ])}
          >
            <button class="file-select" type="button" title={remoteState === "missing" ? `${row.name} · ${shell.tr("chrome.localOnly")}` : row.name} onclick={(event) => { clearLocate(); shell.sideSelected = `local:${row.name}`; if (event.ctrlKey || event.metaKey) shell.compareWith = row.name; else locate(event, row.name); }} ondblclick={() => !row.current && checkout(row.name)}>
              <span class="twist"></span>
              {@render glyph(row.current ? "check" : checkedOut ? "worktree" : "branch")}
              {#if remoteState === "missing"}
                <span class="track-dot missing" title={shell.tr("chrome.localOnly")}></span>
              {:else if remoteState === "mirrored"}
                <span class="track-dot mirrored" title={shell.tr("chrome.trackedRemote")}></span>
              {/if}
              <span class="name">{shell.pinnedHere.has(row.name) ? "• " : ""}{row.group ? row.name.slice(row.group.length + 1) : row.name}</span>
              {#if shell.pendingReference(row.name)}<span class="reference-pending" role="status" aria-label={shell.tr("chrome.taskQueued")}></span>{/if}
              {#if row.ahead > 0 || row.behind > 0}
                <span class="sync" title={syncTitle}>
                  {#if row.ahead > 0}<span class="ahead up">{row.ahead}</span>{/if}
                  {#if row.behind > 0}<span class="ahead down">{row.behind}</span>{/if}
                </span>
              {/if}
            </button>
            {@render branchSwitches(row.name, !row.current && shell.mode === "live")}
          </div>
        {:else if row.kind === "remote"}
          <div class="side nested" class:selected={shell.sideSelected === `remote:${row.name}`} role="group" oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("chrome.fetch"), run: () => shell.mutate({ action: "fetch", remote: row.name, prune: shell.settings.fetchPrune, tags: shell.settings.fetchTags }) },
            { label: shell.tr("dialog.url"), run: () => { shell.remoteEdit = true; shell.draft = row.name; shell.draftExtra = row.url; shell.dialog = "remote"; } },
            { label: shell.tr("menu.remove"), run: () => shell.mutate({ action: "removeRemote", name: row.name }) },
          ])}>
            <button class="file-select" type="button" aria-expanded={row.empty ? undefined : shell.sideOpen(`remote:${row.name}`)} title={row.url} onclick={() => (row.empty ? (shell.sideSelected = `remote:${row.name}`) : shell.pickSide(`remote:${row.name}`))}>
              <span class="twist">{#if !row.empty}{@render chevron(shell.sideOpen(`remote:${row.name}`))}{/if}</span>
              {@render glyph("remote")}
              <span class="name">{row.name}</span>
              {#if shell.pendingReference(row.name)}<span class="reference-pending" role="status" aria-label={shell.tr("chrome.taskQueued")}></span>{/if}
            </button>
          </div>
        {:else if row.kind === "remoteHead"}
          <div class="side nested deep quiet"><span class="twist"></span>{@render glyph("branch")}<span class="name">HEAD → {row.head}</span></div>
        {:else if row.kind === "remoteBranch"}
          <div
            class="side nested deep"
            class:deeper={row.deeper}
            class:selected={shell.sideSelected === `remote:${row.remote}:${row.name}`}
            role="group"
            oncontextmenu={(event) => shell.openMenu(event, [
              { label: shell.tr("menu.checkout"), run: () => shell.mutate({ action: "checkoutRemote", remote: row.remote, branch: row.name }) },
              { label: shell.tr("worktree.add"), run: () => beginWorktree(shell, { kind: "add", source: "refs/remotes/" + row.remote + "/" + row.name }) },
              { label: shell.tr("menu.pullInto"), run: () => shell.mutate({ action: "pullRef", remote: row.remote, branch: row.name, rebase: shell.settings.pullRebase, autostash: true }) },
              { label: shell.tr("menu.pushHere"), run: () => shell.mutate({ action: "pushRef", remote: row.remote, branch: row.name, forceWithLease: shell.settings.forceWithLease, killerPush: shell.settings.killerPush, rebase: shell.settings.pullRebase }) },
              { label: shell.tr("menu.deleteRemote"), run: () => shell.ask(shell.tr("menu.deleteRemote"), shell.tr("dialog.deleteRemoteBody"), shell.bindMutation({ action: "deleteRemoteBranch", remote: row.remote, branch: row.name }), [], { subject: `${row.remote}/${row.name}`, danger: true, confirmLabel: shell.tr("menu.deleteRemote") }) },
              { label: shell.tr("menu.copyName"), run: () => shell.copyText(`${row.remote}/${row.name}`) },
            ])}
          >
            <button class="file-select" type="button" title={`${row.remote}/${row.name}`} onclick={(event) => { shell.sideSelected = `remote:${row.remote}:${row.name}`; locate(event, `${row.remote}/${row.name}`); }} ondblclick={() => checkout(`${row.remote}/${row.name}`, { remote: row.remote, branch: row.name })}>
              <span class="twist"></span>
              {@render glyph("branch")}
              <span class="name">{row.label}</span>
              {#if shell.pendingReference(row.name) || shell.pendingReference(`${row.remote}/${row.name}`)}<span class="reference-pending" role="status" aria-label={shell.tr("chrome.taskQueued")}></span>{/if}
            </button>
            {@render branchSwitches(`${row.remote}/${row.name}`, shell.mode === "live")}
          </div>
        {:else if row.kind === "tag"}
          <div class="side nested" class:deep={row.deep} class:selected={shell.sideSelected === `tag:${row.name}`} role="group" oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("worktree.add"), run: () => beginWorktree(shell, { kind: "add", source: "refs/tags/" + row.name }) },
            { label: shell.tr("menu.checkout"), disabled: shell.mode !== "live", run: () => shell.mutate({ action: "checkout", name: row.name }) },
            { label: shell.tr("menu.delete"), disabled: shell.mode !== "live", run: () => shell.ask(shell.tr("menu.delete"), shell.tr("dialog.deleteTagBody"), shell.bindMutation({ action: "deleteTag", name: row.name }), [], { subject: row.name, danger: true }) },
            { label: shell.tr("menu.deleteRemoteTag"), disabled: shell.mode !== "live" || !shell.refs?.remotes[0], run: () => { const remote = shell.refs?.remotes[0]?.name ?? ""; shell.ask(shell.tr("menu.deleteRemoteTag"), shell.tr("dialog.deleteRemoteTagBody"), shell.bindMutation({ action: "deleteRemoteTag", remote, name: row.name }), [], { subject: `${remote} ${row.name}`, danger: true }); } },
            { label: shell.tr("menu.copyName"), run: () => shell.copyText(row.name) },
          ])}>
            <button class="file-select" type="button" title={row.name} onclick={(event) => { shell.sideSelected = `tag:${row.name}`; locate(event, row.name); }} ondblclick={() => checkout(row.name)}><span class="twist"></span>{@render glyph("tag")}<span class="name">{row.label}</span>{#if shell.pendingReference(row.name)}<span class="reference-pending" role="status" aria-label={shell.tr("chrome.taskQueued")}></span>{/if}</button>
          </div>
        {:else if row.kind === "stash"}
          <div class="side nested" class:selected={shell.sideSelected === `stash:${row.name}`} role="group" oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.apply"), disabled: shell.mode !== "live", run: () => shell.confirmApplyStash(row.name, row.summary) },
            { label: shell.tr("menu.pop"), disabled: shell.mode !== "live", run: () => shell.confirmPopStash(row.name) },
            { label: shell.tr("menu.drop"), disabled: shell.mode !== "live", run: () => { const drop = shell.bindMutation({ action: "stashDrop", name: row.id }); shell.ask(shell.tr("menu.drop"), shell.tr("dialog.dropStashBody"), () => void drop(), [], { subject: row.summary, danger: true }); } },
            { label: shell.tr("menu.renameStash"), disabled: shell.mode !== "live", run: () => { shell.draft = row.summary; shell.draftExtra = row.id ?? row.name; shell.dialog = "stashRename"; } },
            { label: shell.tr("menu.stashPatch"), disabled: shell.mode !== "live", run: () => void shell.mutate({ action: "stashPatch", name: row.id }) },
            { label: shell.tr("menu.stashBranch"), disabled: shell.mode !== "live", run: () => { shell.draft = ""; shell.draftExtra = row.id ?? row.name; shell.dialog = "stashBranch"; } },
          ])}>
            <button class="file-select" type="button" title={row.name} onclick={() => { shell.sideSelected = `stash:${row.name}`; void shell.locateStash(row); }}><span class="twist"></span>{@render glyph("stash")}<span class="name">{row.summary}</span>{#if shell.pendingReference(row.name) || (row.id && shell.pendingReference(row.id))}<span class="reference-pending" role="status" aria-label={shell.tr("chrome.taskQueued")}></span>{/if}</button>
          </div>
        {:else if row.kind === "submodule"}
          <div class="side nested" class:selected={shell.sideSelected === `submodule:${row.path}`} role="group" oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.initialize"), disabled: shell.mode !== "live" || row.ready, run: () => shell.mutate({ action: "submoduleInit", path: row.path }) },
            { label: shell.tr("menu.openFile"), disabled: !row.ready, run: () => shell.openRepo(shell.fullPath(row.path)) },
            { label: shell.tr("menu.sync"), disabled: shell.mode !== "live", run: () => shell.mutate({ action: "submoduleSync", path: row.path }) },
            { label: shell.tr("menu.update"), disabled: shell.mode !== "live", run: () => shell.mutate({ action: "submoduleUpdatePath", path: row.path }) },
            { label: shell.tr("menu.delete"), disabled: shell.mode !== "live", run: () => shell.ask(shell.tr("menu.delete"), shell.tr("dialog.removeSubmoduleBody"), shell.bindMutation({ action: "submoduleRemove", path: row.path }), [], { subject: row.path, danger: true }) },
          ])}>
            <button class="file-select" type="button" title={row.path} onclick={() => (shell.sideSelected = `submodule:${row.path}`)} ondblclick={() => !shell.pendingReference(row.path) && row.ready && shell.openRepo(shell.fullPath(row.path))}><span class="twist"></span>{@render glyph("submodule")}<span class="name">{row.path}</span><span class="submodule-dot {row.tone}" aria-hidden="true"></span>{#if shell.pendingReference(row.path)}<span class="reference-pending" role="status" aria-label={shell.tr("chrome.taskQueued")}></span>{/if}</button>
          </div>
        {:else if row.kind === "submoduleNote"}
          <div class="side nested quiet submodule-note {row.tone}" role="note" title={row.title}>
            <span class="submodule-dot {row.tone}" aria-hidden="true"></span>
            <span class="name">{row.note}</span>
          </div>
        {:else if row.kind === "worktree"}
          {@render worktreeRow(row.worktree)}
        {/if}
      {/snippet}

      {#snippet worktreeRow(row: WorktreeRow)}
        {@const current = shell.viewing(row.path)}
        {@const marks = worktreeMarks(shell, row)}
        <div class="side nested" class:current class:selected={shell.sideSelected === "worktree:" + row.path}
          role="group" oncontextmenu={(event) => shell.openMenu(event, worktreeMenu(shell, row))}>
          <button class="file-select" type="button" data-worktree-path={row.path} aria-current={current ? "true" : undefined}
            title={[row.path, ...marks, row.locked, row.prunable].filter(Boolean).join(" · ")}
            onclick={(event) => {
              shell.sideSelected = "worktree:" + row.path;
              if (event.detail === 0) void openWorktree(shell, row);
            }} ondblclick={() => void openWorktree(shell, row)}>
            <span class="twist"></span>{@render glyph(current ? "check" : "worktree")}
            <span class="name">{worktreeName(shell, row)}</span>
            {#if marks.length}<span class="worktree-note">{marks.at(-1)}</span>{/if}
            {#if shell.pendingReference(row.path)}<span class="reference-pending" role="status" aria-label={shell.tr("chrome.taskQueued")}></span>{/if}
          </button>
        </div>
      {/snippet}

      {#if shell.sideView === "refs"}
      <div class="side-scroll" bind:this={sideScroll} onscroll={onSideScroll}>
      {#if shell.mode === "live" && shell.refs}
        {#if sideRows.length > 60}
          <div class="commit-window" style:height="{sideRows.length * shell.rowSide}px">
            {#each sideWindow.rows as row, index (row.key)}
              <div class="virtual-row" style:top="{(sideWindow.start + index) * shell.rowSide}px">
                {@render sideRow(row)}
              </div>
            {/each}
          </div>
        {:else}
          {#each sideRows as row (row.key)}
            {@render sideRow(row)}
          {/each}
        {/if}
      {:else}
        {@render sideHead("branches", shell.tr("chrome.localBranches"), null, [], false, true)}
        {#if shell.sideOpen("branches") && shell.matchesQuery(shell.branchLabel, shell.sideQuery)}
        <div
          class="side nested current"
          class:selected={shell.sideSelected === `local:${shell.branchLabel}`}
          role="group"
          oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.checkout"), disabled: true },
            { label: shell.tr("menu.merge"), disabled: !!shell.progress, run: () => { shell.draft = shell.branchLabel; shell.draftExtra = "ff"; shell.dialog = "merge"; } },
            { label: shell.tr("menu.rebaseOnto"), disabled: true },
            { label: shell.tr("menu.rename"), run: () => { shell.draft = shell.branchLabel; shell.draftExtra = shell.branchLabel; shell.dialog = "rename"; } },
            { label: shell.tr("menu.delete"), run: () => shell.ask(shell.tr("dialog.deleteBranch"), shell.tr("dialog.deleteBranchBody"), () => {}, [], { subject: shell.branchLabel, danger: true }) },
            { label: shell.tr("menu.setUpstream"), run: () => { shell.draft = shell.branchLabel; shell.draftExtra = "origin/main"; shell.dialog = "upstream"; } },
            { label: shell.tr("menu.copyName"), run: () => shell.copyText(shell.branchLabel) },
            { label: shell.tr("menu.hide"), disabled: true },
            { label: shell.tr("menu.showOnly"), disabled: true },
          ])}
        >
          <button class="file-select" type="button" onclick={() => (shell.sideSelected = `local:${shell.branchLabel}`)}>
            <span class="twist"></span>
            {@render glyph("check")}
            <span class="name">{shell.branchLabel}</span>
            {#if shell.mode === "sample"}<span class="sync"><span class="ahead up">5</span></span>{/if}
          </button>
          {@render branchSwitches(shell.branchLabel, false)}
        </div>
        {/if}
        {@render sideHead("remotes", shell.tr("chrome.remotes"))}
        {#if shell.sideOpen("remotes") && (shell.matchesQuery("origin", shell.sideQuery) || shell.matchesQuery("main", shell.sideQuery))}
        <div class="side nested" class:selected={shell.sideSelected === "remote:origin"}>
          <button class="file-select" type="button" aria-expanded={shell.sideOpen("remote:origin")} onclick={() => shell.pickSide("remote:origin")}>
            <span class="twist">{@render chevron(shell.sideOpen("remote:origin"))}</span>
            {@render glyph("remote")}
            <span class="name">origin</span>
          </button>
        </div>
        {#if shell.sideOpen("remote:origin")}
        <div class="side nested deep quiet"><span class="twist"></span>{@render glyph("branch")}<span class="name">HEAD → main</span></div>
        <div
          class="side nested deep"
          class:selected={shell.sideSelected === "remote:origin:main"}
          role="group"
          oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.checkout"), disabled: true },
            { label: shell.tr("menu.pullInto"), disabled: true },
            { label: shell.tr("menu.pushHere"), disabled: true },
            { label: shell.tr("menu.deleteRemote"), run: () => shell.ask(shell.tr("menu.deleteRemote"), shell.tr("dialog.deleteRemoteBody"), () => {}, [], { subject: "origin/main", danger: true, confirmLabel: shell.tr("menu.deleteRemote") }) },
            { label: shell.tr("menu.copyName"), run: () => shell.copyText("origin/main") },
          ])}
        >
          <button class="file-select" type="button" onclick={() => (shell.sideSelected = "remote:origin:main")}>
            <span class="twist"></span>
            {@render glyph("branch")}
            <span class="name">main</span>
          </button>
          {@render branchSwitches("origin/main", false)}
        </div>
        {/if}
        {/if}
      {/if}
      {#if !(shell.mode === "live" && shell.refs)}
      {@render sideHead("tags", shell.tr("chrome.tags"), shell.visibleTags.length)}
      {#if shell.sideOpen("tags")}
        {#each shell.visibleTags as tag, index (tag.name)}
          {@const group = branchGroup(tag.name)}
          {@const tagFolder = group ? `tag:${group}` : ""}
          {#if group && group !== branchGroup(shell.visibleTags[index - 1]?.name ?? "")}
            <button class="side nested" type="button" aria-expanded={shell.folderOpen(tagFolder)} onclick={() => shell.toggleFolder(tagFolder)}>
              <span class="twist">{@render chevron(shell.folderOpen(tagFolder))}</span>
              {@render glyph("folder")}
              <span class="name">{group}</span>
            </button>
          {/if}
          {#if shell.folderOpen(tagFolder)}
          <div class="side nested" class:deep={!!group} class:selected={shell.sideSelected === `tag:${tag.name}`} role="group" oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.checkout"), disabled: shell.mode !== "live", run: () => shell.mutate({ action: "checkout", name: tag.name }) },
            { label: shell.tr("menu.delete"), disabled: shell.mode !== "live", run: () => shell.ask(shell.tr("menu.delete"), shell.tr("dialog.deleteTagBody"), shell.bindMutation({ action: "deleteTag", name: tag.name }), [], { subject: tag.name, danger: true }) },
            { label: shell.tr("menu.deleteRemoteTag"), disabled: shell.mode !== "live" || !shell.refs?.remotes[0], run: () => { const remote = shell.refs?.remotes[0]?.name ?? ""; shell.ask(shell.tr("menu.deleteRemoteTag"), shell.tr("dialog.deleteRemoteTagBody"), shell.bindMutation({ action: "deleteRemoteTag", remote, name: tag.name }), [], { subject: `${remote} ${tag.name}`, danger: true }); } },
            { label: shell.tr("menu.copyName"), run: () => shell.copyText(tag.name) },
          ])}>
            <button class="file-select" type="button" title={tag.name} onclick={(event) => { shell.sideSelected = `tag:${tag.name}`; locate(event, tag.name); }} ondblclick={() => checkout(tag.name)}><span class="twist"></span>{@render glyph("tag")}<span class="name">{group ? tag.name.slice(group.length + 1) : tag.name}</span></button>
          </div>
          {/if}
        {/each}
      {/if}
      {@render sideHead("stashes", shell.tr("chrome.stashes"), shell.refs?.stashes.length ?? (shell.mode === "sample" ? 1 : 0))}
      {#if shell.sideOpen("stashes")}
        {#each (shell.refs?.stashes ?? (shell.mode === "sample" ? [{ name: "stash@{0}", summary: "WIP before the palette" }] : [])) as stash (stash.name)}
          {#if shell.matchesQuery(stash.summary, shell.sideQuery) || shell.matchesQuery(stash.name, shell.sideQuery)}
          <div class="side nested" class:selected={shell.sideSelected === `stash:${stash.name}`} role="group" oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.apply"), disabled: shell.mode !== "live", run: () => shell.confirmApplyStash(stash.name, stash.summary) },
            { label: shell.tr("menu.pop"), disabled: shell.mode !== "live", run: () => shell.confirmPopStash(stash.name) },
            { label: shell.tr("menu.drop"), disabled: shell.mode !== "live", run: () => { const drop = shell.bindMutation({ action: "stashDrop", name: "id" in stash ? stash.id : stash.name }); shell.ask(shell.tr("menu.drop"), shell.tr("dialog.dropStashBody"), () => void drop(), [], { subject: stash.summary, danger: true }); } },
            { label: shell.tr("menu.renameStash"), disabled: shell.mode !== "live", run: () => { shell.draft = stash.summary; shell.draftExtra = ("id" in stash ? stash.id : undefined) ?? stash.name; shell.dialog = "stashRename"; } },
            { label: shell.tr("menu.stashPatch"), disabled: shell.mode !== "live", run: () => void shell.mutate({ action: "stashPatch", name: "id" in stash ? stash.id : stash.name }) },
            { label: shell.tr("menu.stashBranch"), disabled: shell.mode !== "live", run: () => { shell.draft = ""; shell.draftExtra = ("id" in stash ? stash.id : undefined) ?? stash.name; shell.dialog = "stashBranch"; } },
          ])}>
            <button class="file-select" type="button" title={stash.name} onclick={() => { shell.sideSelected = `stash:${stash.name}`; void shell.locateStash(stash); }}><span class="twist"></span>{@render glyph("stash")}<span class="name">{stash.summary}</span></button>
          </div>
          {/if}
        {/each}
      {/if}
      {@render sideHead("submodules", shell.tr("chrome.submodules"), shell.refs?.submodules.length ?? 0, [
        { label: shell.tr("chrome.add"), icon: "add", run: () => { shell.draft = ""; shell.draftExtra = ""; shell.dialog = "submodule"; } },
      ])}
      {#if shell.sideOpen("submodules")}
        {#each shell.refs?.submodules ?? [] as row (row.path)}
          {#if shell.matchesQuery(row.path, shell.sideQuery)}
          <div class="side nested" class:selected={shell.sideSelected === `submodule:${row.path}`} role="group" oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.initialize"), disabled: shell.mode !== "live" || row.ready, run: () => shell.mutate({ action: "submoduleInit", path: row.path }) },
            { label: shell.tr("menu.openFile"), disabled: !row.ready, run: () => shell.openRepo(shell.fullPath(row.path)) },
            { label: shell.tr("menu.sync"), disabled: shell.mode !== "live", run: () => shell.mutate({ action: "submoduleSync", path: row.path }) },
            { label: shell.tr("menu.update"), disabled: shell.mode !== "live", run: () => shell.mutate({ action: "submoduleUpdatePath", path: row.path }) },
            { label: shell.tr("menu.delete"), disabled: shell.mode !== "live", run: () => shell.ask(shell.tr("menu.delete"), shell.tr("dialog.removeSubmoduleBody"), shell.bindMutation({ action: "submoduleRemove", path: row.path }), [], { subject: row.path, danger: true }) },
          ])}>
            <button class="file-select" type="button" title={row.path} onclick={() => (shell.sideSelected = `submodule:${row.path}`)} ondblclick={() => !shell.pendingReference(row.path) && row.ready && shell.openRepo(shell.fullPath(row.path))}><span class="twist"></span>{@render glyph("submodule")}<span class="name">{row.path}</span>{#if shell.pendingReference(row.path)}<span class="reference-pending" role="status" aria-label={shell.tr("chrome.taskQueued")}></span>{/if}</button>
          </div>
          {/if}
        {/each}
      {/if}
      {#if shell.settings.showWorktrees}
      {@render sideHead("worktrees", shell.tr("chrome.worktrees"), shell.refs?.worktrees.length ?? (shell.mode === "sample" ? 1 : 0), [])}
      {#if shell.sideOpen("worktrees")}
        {#each (shell.refs?.worktrees ?? []) as row (row.path)}
          {#if shell.matchesQuery(row.path, shell.sideQuery) || shell.matchesQuery(row.branch ?? "", shell.sideQuery)}
            {@render worktreeRow(row)}
          {/if}
        {:else}
          {#if shell.mode === "sample"}<div class="side nested"><span class="twist"></span>{@render glyph("worktree")}<span class="name">main</span></div>{/if}
        {/each}
      {/if}
      {/if}
      {/if}
      </div>
      {:else}
      <div class="side-search">
        <h2>{shell.tr("chrome.searchCommits")}</h2>
        <label class="side-filter">
          {@render glyph("search")}
          <input
            data-layer="search"
            placeholder={shell.commitSearchField === "author" ? shell.tr("chrome.author") : shell.commitSearchField === "path" ? shell.tr("dialog.path") : shell.commitSearchField === "diff" ? shell.tr("chrome.diffContent") : shell.tr("chrome.commitMessage")}
            aria-label={shell.tr("chrome.searchCommits")}
            bind:this={shell.commitSearchEl}
            bind:value={shell.commitSearch}
            oninput={() => shell.queueCommitSearch()}
            onkeydown={(event) => { if (event.key === "Escape") { event.stopPropagation(); shell.commitSearchOpen = false; if (shell.commitSearch) { shell.commitSearch = ""; shell.queueCommitSearch(); } } }}
          />
          {#if shell.commitSearch}<button class="side-filter-clear" type="button" aria-label={shell.tr("chrome.clearFilter")} onclick={() => { shell.commitSearch = ""; shell.queueCommitSearch(); }}>×</button>{/if}
        </label>
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="side-search-pick" onclick={(event) => event.stopPropagation()} onfocusout={searchMenuFocusOut}>
          <button class="side-search-toggle" id="commit-search-field" bind:this={searchToggleEl} type="button" aria-haspopup="menu" aria-expanded={shell.commitSearchOpen} onclick={toggleSearchMenu} onkeydown={searchToggleKeys}>
            <span>{shell.commitSearchField === "author" ? shell.tr("chrome.author") : shell.commitSearchField === "path" ? shell.tr("dialog.path") : shell.commitSearchField === "diff" ? shell.tr("chrome.diffContent") : shell.tr("chrome.commitMessage")}</span>
            <svg class="line-icon side-chevron" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>
          </button>
          {#if shell.commitSearchOpen}
            <div class="side-search-menu" bind:this={searchMenuEl} role="menu" aria-label={shell.tr("chrome.searchOptions")} tabindex="-1" onkeydown={searchMenuKeys}>
              <button class="side-search-item" type="button" role="menuitemradio" aria-checked={shell.commitSearchField === "message"} onclick={() => shell.pickCommitSearch("message")}><span class="side-search-check">{#if shell.commitSearchField === "message"}{@render glyph("check")}{/if}</span>{shell.tr("chrome.commitMessage")}</button>
              <button class="side-search-item" type="button" role="menuitemradio" aria-checked={shell.commitSearchField === "author"} onclick={() => shell.pickCommitSearch("author")}><span class="side-search-check">{#if shell.commitSearchField === "author"}{@render glyph("check")}{/if}</span>{shell.tr("chrome.author")}</button>
              <button class="side-search-item" type="button" role="menuitemradio" aria-checked={shell.commitSearchField === "path"} onclick={() => shell.pickCommitSearch("path")}><span class="side-search-check">{#if shell.commitSearchField === "path"}{@render glyph("check")}{/if}</span>{shell.tr("dialog.path")}</button>
              <button class="side-search-item" type="button" role="menuitemradio" aria-checked={shell.commitSearchField === "diff"} onclick={() => shell.pickCommitSearch("diff")}><span class="side-search-check">{#if shell.commitSearchField === "diff"}{@render glyph("check")}{/if}</span>{shell.tr("chrome.diffContent")}</button>
              <div class="side-search-sep"></div>
              <div class="side-search-label">{shell.tr("chrome.searchIn")}</div>
              <button class="side-search-item" type="button" role="menuitemradio" aria-checked={shell.commitSearchScope === "repository"} onclick={() => shell.pickCommitScope("repository")}><span class="side-search-check">{#if shell.commitSearchScope === "repository"}{@render glyph("check")}{/if}</span>{shell.tr("chrome.repository")}</button>
              <button class="side-search-item" type="button" role="menuitemradio" aria-checked={shell.commitSearchScope === "branch"} onclick={() => shell.pickCommitScope("branch")}><span class="side-search-check">{#if shell.commitSearchScope === "branch"}{@render glyph("check")}{/if}</span>{shell.tr("chrome.currentBranch")}</button>
            </div>
          {/if}
        </div>
      </div>
      {/if}
    </aside>
