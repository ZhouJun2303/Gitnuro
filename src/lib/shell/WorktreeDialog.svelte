<script lang="ts">
  import { untrack } from "svelte";
  import { invoke } from "../perf";
  import { getShell } from "./context";
  import {
    beginWorktree, mutateWorktree, nextWorktreeBranch, openWorktree, safeFolder, suggestedWorktreePath,
    viewWorktreeHistory, worktreeMarks, worktreeMenu, worktreeName, worktreeRefs,
    type WorktreeDetails, type WorktreeDialogState, type WorktreePrunePreview,
  } from "./worktrees";

  let { view }: { view: WorktreeDialogState } = $props();
  const shell = getShell();
  const initial = untrack(() => view);
  const sourceRepo = untrack(() => shell.repoPath());
  const initialRows = untrack(() => shell.refs?.worktrees ?? []);
  const immediatelyOccupied = initialRows.some((row) => row.branch === initial.branch);
  const suggestion = untrack(() => nextWorktreeBranch(shell,
    initial.branch || (initial.source?.startsWith("refs/") ? initial.source : shell.snapshot?.branch) || "worktree"));
  let mode = $state<"new" | "existing" | "detached">(
    initial.branch && !immediatelyOccupied ? "existing" : initial.source && !initial.source.startsWith("refs/") ? "detached" : "new",
  );
  let branch = $state(initial.branch && !immediatelyOccupied ? initial.branch : suggestion);
  let start = $state(initial.source ?? "HEAD");
  let path = $state("");
  let openAfter = $state(true);
  let selected = $state(initial.path ?? initialRows.find((row) => shell.viewing(row.path))?.path ?? initialRows[0]?.path ?? "");
  let reason = $state("");
  let busy = $state(false);
  let force = $state(false);
  let backupBranch = $state(suggestion);
  let details = $state<WorktreeDetails | null>(null);
  let reviewError = $state("");
  let loading = $state(false);
  let prune = $state.raw<WorktreePrunePreview | null>(null);
  let refresh = $state(0);
  const rows = $derived(shell.refs?.worktrees ?? []);
  const target = $derived(rows.find((row) => shell.jobKey(row.path) === shell.jobKey(selected)) ??
    (view.kind === "manage" ? rows.find((row) => shell.viewing(row.path)) ?? rows[0] : undefined));
  const assigned = $derived(rows.find((row) => row.branch === branch.trim()));
  const branchExists = $derived(shell.refs?.branches.some((row) => row.name === branch.trim()) ?? false);
  const pathDefault = $derived(view.kind === "add"
    ? suggestedWorktreePath(shell, mode === "detached" ? "detached-" + safeFolder(start.slice(0, 12)) : branch.trim() || "worktree") : "");
  const destination = $derived(path.trim() ? path : pathDefault);
  const refs = $derived(worktreeRefs(shell));
  const startMatches = $derived(refs.filter((ref) => ref.label.includes(start === "HEAD" ? "" : start) || ref.value.includes(start)).slice(0, 40));
  const branchMatches = $derived((shell.refs?.branches ?? []).filter((row) =>
    row.name.includes(branch) || !branchExists).slice(0, 40));
  const recentCommits = $derived(shell.commitsLive.slice(0, 8));
  const needsForce = $derived(!!details && (details.changeCount > 0 || details.hasSubmodules));
  const inside = () => shell.dialog === "worktree" && shell.worktreeDialog === initial && shell.repoPath() === sourceRepo;

  $effect(() => {
    const row = target, repo = shell.repoPath(), kind = view.kind;
    refresh;
    shell.contextRevision;
    if (kind !== "prune" && !((kind === "manage" || kind === "remove") && row)) return;
    if (!repo) return;
    let current = true;
    loading = true;
    details = null;
    prune = null;
    reviewError = "";
    if (kind === "prune") {
      void invoke<WorktreePrunePreview>("worktree_prune_preview", { path: repo }).then((value) => {
        if (current) prune = value;
      }).catch((error) => { if (current) reviewError = shell.message(error); })
        .finally(() => { if (current) loading = false; });
    } else if (row) {
      void invoke<WorktreeDetails>("worktree_details", { path: repo, target: row.path }).then((value) => {
        if (current) details = value;
      }).catch((error) => { if (current) reviewError = shell.message(error); })
        .finally(() => { if (current) loading = false; });
    }
    return () => { current = false; };
  });

  function changeMode(event: Event) {
    mode = (event.currentTarget as HTMLSelectElement).value as typeof mode;
    if (mode === "new" && (!branch.trim() || branchExists)) branch = suggestion;
    if (mode === "existing" && !branchExists) {
      branch = initial.branch ?? (shell.refs?.branches ?? []).find((row) => !rows.some((item) => item.branch === row.name))?.name ?? "";
    }
  }

  async function browse() {
    const picked = await shell.browseFolder(destination || sourceRepo || "");
    if (picked && inside()) path = picked;
  }

  async function refreshManager() {
    try {
      await shell.loadContext(undefined, { history: false, flow: false });
      if (inside()) refresh += 1;
    } catch (error) {
      if (inside()) shell.actionError = shell.message(error);
    }
  }

  async function write(request: Record<string, unknown>, switchFrom?: string) {
    if (busy || shell.writeBlocked) return;
    busy = true;
    try {
      const outcome = await mutateWorktree(shell, request, switchFrom);
      if (outcome === "completed" && inside()) {
        if (request.action === "createBranch" || view.kind === "manage") refresh += 1;
        else if (view.kind === "add") shell.dialog = null;
        else beginWorktree(shell, { kind: "manage", path: view.kind === "remove" ? undefined : selected });
      } else if (outcome === "failed" && inside() && (view.kind === "remove" || view.kind === "prune")) {
        force = false;
        refresh += 1;
      }
    } finally {
      busy = false;
    }
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy || shell.writeBlocked) return;
    if (view.kind === "add" && destination && (mode === "detached" ||
      (branch.trim() && (mode === "new" ? !branchExists : branchExists && !assigned)))) {
      void write({ action: "addWorktree", path: destination, branch: branch.trim(), mode, start: mode === "existing" ? null : start.trim() || "HEAD", openAfter });
    } else if (view.kind === "remove" && target && details && !loading && !reviewError && !target.main && target.locked == null && !details.protectsCommit && (!needsForce || force)) {
      void write({ action: "removeWorktree", path: target.path, force, expected: details.stamp }, target.path);
    } else if (view.kind === "move" && target && !target.main && !target.missing && target.locked == null && destination) {
      void write({ action: "moveWorktree", path: target.path, to: destination }, target.path);
    } else if (view.kind === "lock" && target && !target.main && target.locked == null) {
      void write({ action: "setWorktreeLock", path: target.path, locked: true, reason: reason.trim() || null });
    } else if (view.kind === "repair" && destination) {
      void write({ action: "repairWorktree", path: destination });
    } else if (view.kind === "prune" && prune?.output && !prune.protectedPaths.length && !loading && !reviewError) {
      void write({ action: "pruneWorktrees", expected: prune.stamp });
    }
  }

  function changeLabel(code: string) {
    if (code === "??") return shell.tr("worktree.untracked");
    if (code === "!!") return shell.tr("worktree.ignored");
    if (code.includes("U") || code === "AA" || code === "DD") return shell.tr("worktree.conflicts");
    return [code[0] !== " " ? shell.tr("worktree.staged") : "", code[1] !== " " ? shell.tr("worktree.unstaged") : ""].filter(Boolean).join(" · ");
  }
</script>

{#snippet pathField(label: string)}
  <label>{label}
    <span class="checkout-path">
      <input aria-label={label} placeholder={pathDefault || shell.tr("worktree.pathHint")} bind:value={path} />
      {#if shell.inApp()}<button class="text-button" type="button" onclick={browse}>{shell.tr("dialog.browse")}</button>{/if}
    </span>
  </label>
{/snippet}

{#snippet checkoutStatus()}
  {#if loading}<p class="empty" role="status">{shell.tr("chrome.reading")}</p>
  {:else if reviewError}<p class="dialog-error" role="alert">{reviewError}</p>
  {:else if details && target && !target.bare && !target.missing}
    {#if details.changeCount === 0}
      <p class="empty">{shell.tr("worktree.clean")}</p>
    {:else}
      <ul class="checkout-changes" aria-label={shell.tr("chrome.localChanges")}>
        {#each details.changes as change}
          <li><span>{change.path}</span><small>{changeLabel(change.code)}</small></li>
        {/each}
        {#if details.changeCount > details.changes.length}
          <li>{shell.tr("worktree.moreChanges", { count: String(details.changeCount - details.changes.length) })}</li>
        {/if}
      </ul>
    {/if}
  {/if}
{/snippet}

<form onsubmit={submit} class="worktree-form">
  <h2>{shell.tr("worktree." + view.kind)}</h2>
  {#if view.kind === "manage"}
    <div class="checkout-manager">
      <div class="checkout-list" role="group" aria-label={shell.tr("worktree.select")}>
        {#each rows as row (row.path)}
          <button class="checkout-list-row" class:active={row.path === target?.path} aria-pressed={row.path === target?.path} type="button"
            title={row.path} onclick={() => { selected = row.path; }} ondblclick={() => void openWorktree(shell, row)}
            oncontextmenu={(event) => shell.openMenu(event, worktreeMenu(shell, row))}>
            <strong>{worktreeName(shell, row)}</strong>
            <small>{row.path}</small>
            {#if worktreeMarks(shell, row).length}<small>{worktreeMarks(shell, row).join(" · ")}</small>{/if}
          </button>
        {/each}
      </div>
      {#if target}
        <div class="checkout-detail">
          <strong>{worktreeName(shell, target)}</strong>
          <small class="checkout-full-path">{target.path}</small>
          {#if target.head}<code>{target.head.slice(0, 12)}</code>{/if}
          {#if target.locked}<small>{target.locked}</small>{/if}
          {#if target.prunable}<small>{target.prunable}</small>{/if}
          {@render checkoutStatus()}
          {#if details?.protectsCommit}
            <p class="empty">{shell.tr("worktree.saveFirst")}</p>
            <label>{shell.tr("dialog.branch")}<input bind:value={backupBranch} /></label>
            <button class="text-button" type="button" disabled={busy || !backupBranch.trim()}
              onclick={() => void write({ action: "createBranch", name: backupBranch.trim(), start: target?.head })}>{shell.tr("worktree.saveBranch")}</button>
          {/if}
          <div class="checkout-actions">
            <button class="text-button" type="button" disabled={busy || target.missing} onclick={() => { if (target) { shell.dialog = null; void openWorktree(shell, target); } }}>{shell.tr("worktree.open")}</button>
            <button class="text-button" type="button" disabled={busy || !target.head} onclick={() => { if (target) viewWorktreeHistory(shell, target); }}>{shell.tr("worktree.history")}</button>
            <button class="text-button" type="button" disabled={busy || loading} onclick={() => void refreshManager()}>{shell.tr("chrome.refresh")}</button>
            {#if !target.main}
              <button class="text-button" type="button" disabled={busy} onclick={() => {
                if (!target) return;
                if (target.locked != null) void write({ action: "setWorktreeLock", path: target.path, locked: false });
                else beginWorktree(shell, { kind: "lock", path: target.path });
              }}>{shell.tr(target.locked != null ? "worktree.unlock" : "worktree.lock")}</button>
              {#if target.missing}
                <button class="text-button" type="button" disabled={busy}
                  onclick={() => beginWorktree(shell, { kind: "repair", path: target?.path })}>{shell.tr("worktree.repair")}</button>
              {:else}
                <button class="text-button" type="button" disabled={target.locked != null || busy}
                  onclick={() => beginWorktree(shell, { kind: "move", path: target?.path })}>{shell.tr("worktree.move")}</button>
              {/if}
              <button class="text-button" type="button" disabled={target.locked != null || busy}
                onclick={() => beginWorktree(shell, { kind: "remove", path: target?.path })}>{shell.tr("worktree.remove")}</button>
            {/if}
          </div>
        </div>
      {/if}
    </div>
    <div class="composer-row actions">
      <button class="text-button" type="button" disabled={busy} onclick={() => beginWorktree(shell, { kind: "prune", path: selected })}>{shell.tr("worktree.prune")}</button>
      <button class="text-button" type="button" disabled={busy} onclick={() => void write({ action: "repairWorktree", path: null })}>{shell.tr("worktree.repairAll")}</button>
      <button class="commit" type="button" disabled={busy} onclick={() => beginWorktree(shell)}>{shell.tr("worktree.add")}</button>
    </div>
  {:else if view.kind === "add"}
    <label>{shell.tr("worktree.checkoutMode")}
      <select value={mode} onchange={changeMode}>
        <option value="new">{shell.tr("worktree.new")}</option>
        <option value="existing">{shell.tr("worktree.existing")}</option>
        <option value="detached">{shell.tr("worktree.detached")}</option>
      </select>
    </label>
    {#if mode !== "detached"}
      <label>{shell.tr("dialog.branch")}<input bind:value={branch} list={mode === "existing" ? "worktree-branches" : undefined} required /></label>
      {#if mode === "existing"}
        <datalist id="worktree-branches">{#each branchMatches as item (item.name)}<option value={item.name}></option>{/each}</datalist>
        {#if assigned}
          <p class="empty">{shell.tr("worktree.occupied", { path: assigned.path })}</p>
          {#if !assigned.missing}<button class="text-button" type="button" onclick={() => { if (assigned) { shell.dialog = null; void openWorktree(shell, assigned); } }}>{shell.tr("worktree.open")}</button>{/if}
        {:else if !branchExists}<p class="empty">{shell.tr("worktree.chooseExisting")}</p>{/if}
      {:else if branchExists}<p class="empty">{shell.tr("worktree.branchExists")}</p>{/if}
    {/if}
    {#if mode !== "existing"}
      <label>{shell.tr("worktree.start")}<input bind:value={start} list="worktree-start" placeholder="HEAD" /></label>
      <datalist id="worktree-start">
        <option value="HEAD"></option>
        {#each startMatches as ref (ref.value)}<option value={ref.value}>{ref.label}</option>{/each}
        {#each recentCommits as commit (commit.id)}<option value={commit.id}>{commit.summary}</option>{/each}
      </datalist>
    {/if}
    {@render pathField(shell.tr("worktree.path"))}
    <label class="check"><input type="checkbox" bind:checked={openAfter} /> {shell.tr("worktree.openAfter")}</label>
    <div class="composer-row actions">
      <button class="text-button" type="button" disabled={busy} onclick={() => { shell.dialog = null; }}>{shell.tr("dialog.cancel")}</button>
      <button class="commit" type="submit" disabled={busy || shell.writeBlocked || !destination ||
        (mode !== "detached" && (!branch.trim() || (mode === "new" ? branchExists : !branchExists || !!assigned)))}>{shell.tr("dialog.create")}</button>
    </div>
  {:else}
    {#if target && view.kind !== "prune"}
      <p class="checkout-full-path">{target.path}</p>
    {/if}
    {#if view.kind === "remove"}
      <p class="empty">{shell.tr(target?.missing ? "worktree.removeMissing" : "worktree.removeBody")}</p>
      {#if target && shell.viewing(target.path)}<p class="empty">{shell.tr("worktree.switchFirst")}</p>{/if}
      {@render checkoutStatus()}
      {#if details?.protectsCommit}
        <p class="empty">{shell.tr("worktree.saveFirst")}</p>
        <label>{shell.tr("dialog.branch")}<input bind:value={backupBranch} /></label>
        <button class="text-button checkout-save" type="button" disabled={busy || !backupBranch.trim()}
          onclick={() => void write({ action: "createBranch", name: backupBranch.trim(), start: target?.head })}>{shell.tr("worktree.saveBranch")}</button>
      {/if}
      {#if details?.hasSubmodules}<p class="empty">{shell.tr("worktree.hasSubmodules")}</p>{/if}
      {#if needsForce && !details?.protectsCommit}
        <label class="check"><input type="checkbox" bind:checked={force} /> {shell.tr("worktree.force")}</label>
        {#if force}<p class="empty">{shell.tr("worktree.forceHint")}</p>{/if}
      {/if}
    {:else if view.kind === "move"}
      {#if target && shell.viewing(target.path)}<p class="empty">{shell.tr("worktree.switchFirst")}</p>{/if}
      {@render pathField(shell.tr("worktree.path"))}
      <p class="empty">{shell.tr("worktree.moveHint")}</p>
    {:else if view.kind === "lock"}
      <label>{shell.tr("worktree.reason")}<input bind:value={reason} /></label>
    {:else if view.kind === "repair"}
      {@render pathField(shell.tr("worktree.repairPath"))}
    {:else if view.kind === "prune"}
      {#if loading || reviewError}{@render checkoutStatus()}
      {:else if prune?.output}
        <p class="empty">{shell.tr("worktree.pruneBody")}</p>
        <pre class="checkout-prune">{prune.output}</pre>
        {#if prune.protectedPaths.length}
          <p class="empty">{shell.tr("worktree.saveFirst")}</p>
          <ul class="checkout-changes">
            {#each prune.protectedPaths as protectedPath}
              <li><button type="button" class="text-button" onclick={() => beginWorktree(shell, { kind: "manage", path: protectedPath })}>{protectedPath}</button></li>
            {/each}
          </ul>
        {/if}
      {:else}<p class="empty">{shell.tr("worktree.pruneEmpty")}</p>{/if}
    {/if}
    <div class="composer-row actions">
      <button class="text-button" type="button" disabled={busy} onclick={() => beginWorktree(shell, { kind: "manage", path: selected })}>{shell.tr("dialog.cancel")}</button>
      <button class="commit" class:danger={view.kind === "remove" || view.kind === "prune"} type="submit"
        disabled={busy || shell.writeBlocked || loading || !!reviewError ||
          (view.kind === "remove" && (!target || target.main || target.locked != null || !details || details.protectsCommit || (needsForce && !force))) ||
          (view.kind === "lock" && (!target || target.main || target.locked != null)) ||
          (view.kind === "move" && (!target || target.main || target.missing || target.locked != null)) ||
          ((view.kind === "move" || view.kind === "repair") && !destination) ||
          (view.kind === "prune" && (!prune?.output || prune.protectedPaths.length > 0))}>{shell.tr(view.kind === "prune" ? "worktree.confirmPrune" : view.kind === "remove" && force ? "worktree.force" : "worktree." + view.kind)}</button>
    </div>
  {/if}
  {#if busy}<p class="empty" role="status">{shell.tr("worktree.pending")}</p>{/if}
</form>

<style>
  .checkout-path { display: flex; gap: 8px; align-items: center; }
  .checkout-path input { flex: 1; min-width: 0; }
  .checkout-manager { display: grid; grid-template-columns: minmax(150px, 2fr) minmax(180px, 3fr); gap: 14px; }
  .checkout-list, .checkout-changes, .checkout-prune { max-height: min(38vh, 340px); overflow: auto; }
  .checkout-list { display: grid; align-content: start; gap: 4px; }
  .checkout-list-row { display: grid; gap: 3px; text-align: left; border: 1px solid transparent; border-radius: 8px; padding: 7px 9px; color: inherit; background: transparent; }
  .checkout-list-row:hover, .checkout-list-row.active { background: var(--hover); border-color: var(--border); }
  .checkout-list-row small, .checkout-list-row strong { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .checkout-list-row small, .checkout-detail small { color: var(--text-secondary); }
  .checkout-detail { display: grid; align-content: start; gap: 6px; min-width: 0; }
  .checkout-full-path { overflow-wrap: anywhere; }
  .checkout-actions { display: flex; flex-wrap: wrap; gap: 5px; }
  .checkout-changes { list-style: none; margin: 4px 0; padding: 0; }
  .checkout-changes li { display: flex; justify-content: space-between; gap: 10px; padding: 4px 0; border-bottom: 1px solid var(--border); }
  .checkout-changes span { overflow-wrap: anywhere; }
  .checkout-changes small { flex-shrink: 0; color: var(--text-secondary); }
  .checkout-prune { white-space: pre-wrap; overflow-wrap: anywhere; }
  @media (max-width: 570px) {
    .checkout-manager { grid-template-columns: 1fr; }
    .checkout-list { max-height: 22vh; }
  }
</style>
