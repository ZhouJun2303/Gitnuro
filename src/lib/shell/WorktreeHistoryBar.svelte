<script lang="ts">
  import { untrack } from "svelte";
  import { invoke } from "../perf";
  import { getShell } from "./context";
  import { reloadWorktreeHistory, worktreeBase, worktreeName, worktreeRefs, type WorktreeComparison } from "./worktrees";

  const shell = getShell();
  const row = $derived(shell.worktreeHistoryRow);
  const initial = untrack(() => shell.worktreeHistory?.baseName || (row ? worktreeBase(shell, row) : "HEAD"));
  let base = $state(initial);
  let comparison = $state.raw<WorktreeComparison | null>(null);
  let comparing = $state(false);
  let fault = $state("");
  let generation = 0;
  const choices = $derived(worktreeRefs(shell).filter((ref) => ref.value.includes(base) || ref.label.includes(base)).slice(0, 40));
  const scope = $derived(shell.worktreeHistory);

  $effect(() => {
    const active = scope, head = row?.head, repo = shell.repoPath();
    shell.contextRevision;
    shell.refs?.labels;
    if (active?.base && head && repo) {
      void compare(active.base, head, false);
    }
  });

  async function compare(rev: string, head: string, filter = true) {
    const repo = shell.repoPath(), active = scope;
    if (!rev || !repo || !active) return;
    const token = ++generation;
    const current = () => token === generation && shell.worktreeHistory?.path === active.path && shell.viewing(repo);
    comparing = true;
    fault = "";
    try {
      const value = await invoke<WorktreeComparison>("worktree_compare", { path: repo, head, base: rev });
      if (!current()) return;
      comparison = value;
      if (filter) {
        shell.worktreeHistory = { ...active, base: rev, baseName: base.trim() };
        reloadWorktreeHistory(shell);
      }
    } catch (error) {
      if (current()) {
        comparison = null;
        fault = shell.message(error);
      }
    } finally {
      if (current()) comparing = false;
    }
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!row?.head || !base.trim()) return;
    const value = base.trim();
    const matches = worktreeRefs(shell).filter((ref) => ref.label === value);
    void compare(matches.length === 1 ? matches[0].value : value, row.head);
  }

  function resetBase() {
    if (!scope) return;
    generation += 1;
    comparison = null;
    fault = "";
    comparing = false;
    shell.worktreeHistory = { ...scope, base: "", baseName: "" };
    reloadWorktreeHistory(shell);
  }

  function compareFiles() {
    if (!comparison) return;
    shell.drops = [comparison.mergeBase ?? comparison.base, comparison.head];
    void shell.compareDrops();
  }
</script>

<div class="worktree-history-bar">
  <div class="filter-bar">
    <span title={scope?.path}>{row ? shell.tr("worktree.scope", { name: worktreeName(shell, row) }) : shell.tr("worktree.lost")}</span>
    <button class="text-button" type="button" onclick={() => shell.clearHistoryFilter()}>{shell.tr("chrome.clearFilter")}</button>
  </div>
  {#if row?.head}
    <form onsubmit={submit}>
      <label>{shell.tr("worktree.base")} <input bind:value={base} list="worktree-history-bases" placeholder={shell.tr("worktree.baseHint")} /></label>
      <datalist id="worktree-history-bases">
        <option value="HEAD"></option>
        {#each choices as ref (ref.value)}<option value={ref.value}>{ref.label}</option>{/each}
      </datalist>
      <button class="text-button" type="submit" disabled={comparing || !base.trim()}>{shell.tr("worktree.compare")}</button>
      {#if scope?.base}
        <label class="check"><input type="checkbox" checked onchange={resetBase} /> {shell.tr("worktree.owned")}</label>
      {/if}
      {#if comparison && !comparing}
        <span aria-live="polite">{shell.tr("worktree.distance", { ahead: String(comparison.ahead), behind: String(comparison.behind) })}</span>
        <button class="text-button" type="button" onclick={compareFiles}>{shell.tr("worktree.files")}</button>
      {/if}
    </form>
  {:else if row}<p class="empty">{shell.tr("worktree.noHead")}</p>{/if}
  {#if fault}<p class="empty" role="alert">{fault}</p>{/if}
</div>

<style>
  .worktree-history-bar form { display: flex; align-items: center; flex-wrap: wrap; gap: 6px 10px; padding: 4px 12px; }
  .worktree-history-bar input:not([type="checkbox"]) { max-width: 190px; }
  .worktree-history-bar label { display: flex; align-items: center; gap: 6px; }
</style>
