import { invoke } from "../perf";
import type { MutationResult } from "../operation-flow";
import type { WorktreeRow } from "../status";
import type { ShellState } from "./state.svelte";
import type { MenuItem } from "./types";

export type WorktreeDialogState = {
  kind: "manage" | "add" | "remove" | "lock" | "move" | "repair" | "prune";
  path?: string;
  source?: string;
  branch?: string;
};
export type WorktreeDetails = {
  changes: { code: string; path: string }[];
  changeCount: number;
  hasSubmodules: boolean;
  protectsCommit: boolean;
  stamp: string;
};
export type WorktreePrunePreview = { output: string; protectedPaths: string[]; stamp: string };
export type WorktreeComparison = { head: string; base: string; mergeBase: string | null; ahead: number; behind: number };
export type WorktreeHistory = { path: string; base: string; baseName: string };

export function worktreeName(shell: ShellState, row: WorktreeRow) {
  if (row.bare) return shell.tr("worktree.bare");
  if (row.branch) return row.branch;
  return row.detached && row.head
    ? shell.tr("worktree.detached") + " " + row.head.slice(0, 8)
    : shell.folderName(row.path);
}

export function worktreeMarks(shell: ShellState, row: WorktreeRow) {
  const marks = [];
  if (row.main && !row.bare) marks.push(shell.tr("worktree.main"));
  if (shell.viewing(row.path)) marks.push(shell.tr("worktree.current"));
  if (row.locked != null) marks.push(shell.tr("worktree.locked"));
  if (row.missing) marks.push(shell.tr("worktree.missing"));
  if (row.prunable != null) marks.push(shell.tr("worktree.prunable"));
  return marks;
}

export function beginWorktree(shell: ShellState, view: WorktreeDialogState = { kind: "add" }) {
  if (shell.mode !== "live") return;
  shell.actionError = null;
  shell.worktreeDialog = view;
  shell.dialog = "worktree";
}

export function occupiedWorktree(shell: ShellState, branch: string) {
  return shell.refs?.worktrees.find((row) => row.branch === branch && !shell.viewing(row.path));
}

export async function openWorktree(shell: ShellState, row: WorktreeRow) {
  if (shell.mode !== "live" || shell.pendingReference(row.path)) return;
  if (row.missing) {
    shell.actionError = shell.tr("worktree.unavailable", { path: row.path });
    return;
  }
  if (!shell.viewing(row.path)) await shell.openRepo(row.path);
}

export function viewWorktreeHistory(shell: ShellState, row: WorktreeRow) {
  if (!row.head) return;
  shell.locateSeq += 1;
  shell.dialog = null;
  shell.contentHits = null;
  shell.historyFilter = "";
  shell.sideView = "refs";
  shell.openHistory();
  shell.worktreeHistory = { path: row.path, base: "", baseName: "" };
  reloadWorktreeHistory(shell);
}

export function reloadWorktreeHistory(shell: ShellState) {
  shell.resetCommitView();
  shell.selectedCommit = null;
  shell.comparePair = null;
  shell.commitSelection.clear();
  shell.historyTop = 0;
  shell.commitsLive = [];
  void shell.loadContext(undefined, { history: true, flow: false }).catch((error) => {
    shell.actionError = shell.message(error);
  });
}

export function worktreeBase(shell: ShellState, row: WorktreeRow) {
  const upstream = shell.refs?.branches.find((branch) => branch.name === row.branch)?.upstream;
  if (upstream) {
    const remote = shell.refs?.remotes.some(({ name, branches }) =>
      branches.some((branch) => name + "/" + branch === upstream));
    if (remote) return "refs/remotes/" + upstream;
    if (shell.refs?.branches.some(({ name }) => name === upstream)) return "refs/heads/" + upstream;
  }
  const main = shell.refs?.worktrees.find((worktree) => worktree.main && worktree.path !== row.path);
  return main?.branch ? "refs/heads/" + main.branch : main?.head || "HEAD";
}

export function worktreeRefs(shell: ShellState) {
  const refs = shell.refs;
  if (!refs) return [];
  const locals = refs.branches.map((row) => ({ value: "refs/heads/" + row.name, label: row.name }));
  const remotes = refs.remotes.flatMap(({ name, branches }) => branches.map((branch) =>
    ({ value: "refs/remotes/" + name + "/" + branch, label: name + "/" + branch })));
  const tags = refs.tags.map((row) => ({ value: "refs/tags/" + row.name, label: row.name }));
  return [...locals, ...remotes, ...tags];
}

export function safeFolder(name: string) {
  return name.replace(/[\\/:*?"<>|\u0000-\u001f]/g, "-").replace(/[. ]+$/, "") || "worktree";
}

export function nextWorktreeBranch(shell: ShellState, hint: string) {
  const leaf = hint.split("/").at(-1) || "worktree";
  const prefix = "wt-" + leaf.replace(/[^-\p{L}\p{N}_.]+/gu, "-").replace(/[.]+$/, "");
  const names = new Set(shell.refs?.branches.map((branch) => branch.name));
  let branch = prefix;
  for (let index = 2; names.has(branch); index += 1) branch = prefix + "-" + index;
  return branch;
}

export function suggestedWorktreePath(shell: ShellState, label: string) {
  const root = (shell.refs?.worktrees.find((row) => row.main)?.path ?? shell.repoPath() ?? "").replaceAll("\\", "/").replace(/\/+$/, "");
  const separator = root.lastIndexOf("/");
  const directory = root.slice(separator + 1);
  const stem = safeFolder(directory + "-" + label);
  const prefix = (separator < 0 ? "../" : root.slice(0, separator + 1)) + stem;
  const paths = new Set(shell.refs?.worktrees.map((row) => shell.jobKey(row.path)));
  let path = prefix;
  for (let index = 2; paths.has(shell.jobKey(path)); index += 1) path = prefix + "-" + index;
  return path;
}

export function worktreeMenu(shell: ShellState, row: WorktreeRow): MenuItem[] {
  const pending = shell.pendingReference(row.path);
  const unavailable = row.missing || pending;
  const changePath = row.main || row.locked != null || pending;
  const worktreeAction = (kind: WorktreeDialogState["kind"]) => () => beginWorktree(shell, { kind, path: row.path });
  return [
    { label: shell.tr("worktree.open"), disabled: unavailable, run: () => void openWorktree(shell, row) },
    { label: shell.tr("worktree.history"), disabled: !row.head, run: () => viewWorktreeHistory(shell, row) },
    { label: shell.tr("worktree.manage"), run: worktreeAction("manage") },
    { label: shell.tr("chrome.explorer"), disabled: unavailable, run: () => void shell.openLocal(row.path, true) },
    { label: shell.tr("chrome.terminal"), disabled: unavailable, run: () =>
      void invoke("open_terminal", { path: row.path }).catch((error) => { shell.actionError = shell.message(error); }) },
    { label: shell.tr("worktree.editor"), disabled: unavailable, run: () => {
      const editor = shell.defaultTool("editor");
      const tool = editor?.path && editor.args.includes("{file}") && !editor.args.includes("{line}")
        ? shell.composeTool(editor.path, editor.args) : null;
      void invoke("open_worktree_editor", { path: row.path, tool }).catch((error) => { shell.actionError = shell.message(error); });
    } },
    { label: shell.tr("worktree.copyPath"), run: () => shell.copyText(row.path) },
    { sep: true, label: "" },
    { label: shell.tr(row.locked != null ? "worktree.unlock" : "worktree.lock"), disabled: row.main || pending,
      run: row.locked == null ? worktreeAction("lock") : () => void shell.mutate({ action: "setWorktreeLock", path: row.path, locked: false }) },
    { label: shell.tr("worktree.move"), disabled: changePath || row.missing, run: worktreeAction("move") },
    { label: shell.tr("worktree.repair"), disabled: row.main || !row.missing || pending, run: worktreeAction("repair") },
    { label: shell.tr("worktree.remove"), disabled: changePath, run: worktreeAction("remove") },
  ];
}

export async function mutateWorktree(shell: ShellState, request: Record<string, unknown>, switchFrom?: string) {
  if (switchFrom && shell.viewing(switchFrom)) {
    const rows = shell.refs?.worktrees ?? [];
    const available = (row: WorktreeRow) => !row.missing && !shell.viewing(row.path) && !shell.pendingReference(row.path);
    const other = rows.find((row) => row.main && available(row)) ?? rows.find(available);
    if (!other) {
      shell.actionError = shell.tr("worktree.noFallback");
      return;
    }
    const expected = shell.openGen + 1;
    await openWorktree(shell, other);
    if (shell.openGen !== expected || !shell.viewing(other.path) || shell.mode !== "live") return;
  }
  return shell.mutate(request);
}

export function finishWorktreeMutation(shell: ShellState, request: Record<string, unknown>, result: MutationResult, current: boolean) {
  const path = String(request.path ?? "");
  const destination = result.worktreePath;
  if (request.action === "removeWorktree" && path) {
    shell.repositoryViews.delete(shell.jobKey(path));
    shell.forgetRepo(path);
  } else if (request.action === "moveWorktree" && path && destination) {
    shell.relocateRepo(path, destination);
  } else if (request.action === "addWorktree" && current) {
    shell.collapsedSide = shell.collapsedSide.filter((section) => section !== "worktrees");
    if (destination && request.openAfter === true) void shell.openRepo(destination);
  }
}
