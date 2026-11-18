import { invoke, traceOperation } from "../perf";
import { repositoryKey } from "../operation-policy";
import type { MutationResult } from "../operation-flow";
import { statusPathBefore, type StatusFile, type StatusSnapshot } from "../status";
import type { ShellState } from "./state.svelte";

export type StageChange = { id: number; repo: string; files: string[]; unstage: boolean };
type StageReport = StatusSnapshot & Partial<MutationResult>;
let nextId = 0;

const pathOf = (path: string) => path.replaceAll("\\", "/");

/** Project pending intents without changing the snapshot used for Git decisions. */
export function projectStages(snapshot: StatusSnapshot | null, changes: readonly StageChange[]) {
  if (!snapshot) return { staged: [] as StatusFile[], unstaged: [] as StatusFile[] };
  const repo = repositoryKey(snapshot.path);
  const directions = new Map<string, number>();
  for (const change of changes) {
    if (change.repo !== repo) continue;
    const direction = Number(change.unstage);
    for (const path of change.files) {
      const first = directions.get(path);
      directions.set(path, (first == null ? direction * 2 : first & 2) | direction);
    }
  }
  if (!directions.size) return snapshot;
  const staged: StatusFile[] = [], unstaged: StatusFile[] = [];
  let i = 0, j = 0;
  while (i < snapshot.staged.length || j < snapshot.unstaged.length) {
    const left = snapshot.staged[i], right = snapshot.unstaged[j];
    const path = !right || left && statusPathBefore(left.path, right.path) ? left.path : right.path;
    let index = left?.path === path ? snapshot.staged[i++] : undefined;
    let work = right?.path === path ? snapshot.unstaged[j++] : undefined;
    // Preserve ambiguous rows until Git reports the actual result.
    if (snapshot.staged[i]?.path === path || snapshot.unstaged[j]?.path === path) {
      if (index) staged.push(index);
      if (work) unstaged.push(work);
      while (snapshot.staged[i]?.path === path) staged.push(snapshot.staged[i++]);
      while (snapshot.unstaged[j]?.path === path) unstaged.push(snapshot.unstaged[j++]);
      continue;
    }
    const direction = directions.get(path);
    if (direction != null && index?.kind !== "renamed" && work?.kind !== "renamed" && index?.kind !== "conflict" && work?.kind !== "conflict") {
      // The first effective intent collapses the two sides to at most one row.
      // Intermediate flips only move that row; the final direction chooses its side.
      const firstUnstage = direction >= 2;
      const lastUnstage = (direction & 1) !== 0;
      const move = (unstage: boolean) => {
        if (unstage && index) {
          work = index.kind === "added" && work?.kind === "deleted" ? undefined
            : { ...(work ?? index), kind: index.kind === "added" ? "untracked" : work?.kind === "untracked" ? "modified" : work?.kind ?? index.kind, previousPath: null };
          index = undefined;
        } else if (!unstage && work) {
          index = index?.kind === "added" && work.kind === "deleted" ? undefined
            : { ...work, kind: work.kind === "untracked" || index?.kind === "added" ? "added" : work.kind, previousPath: null };
          work = undefined;
        }
      };
      move(firstUnstage);
      if (lastUnstage !== firstUnstage) move(lastUnstage);
    }
    if (index) staged.push(index);
    if (work) unstaged.push(work);
  }
  return { staged, unstaged };
}

/** Include ancestors once so visible folder rows use constant-time lookups. */
export function stageActions(changes: readonly StageChange[], repo: string) {
  const actions = new Map<string, boolean>();
  const key = repositoryKey(repo);
  for (let i = changes.length - 1; i >= 0; i--) {
    const change = changes[i];
    if (change.repo !== key) continue;
    for (const file of change.files) {
      let path = pathOf(file);
      while (path && !actions.has(path)) {
        actions.set(path, change.unstage);
        const slash = path.lastIndexOf("/");
        if (slash < 0) break;
        path = path.slice(0, slash);
      }
    }
  }
  return actions;
}

/** Display the frozen selection while the native lane executes it. */
export async function enqueueStage(shell: ShellState, command: string, extra: Record<string, unknown> = {}, source: "user" | "background" = "user"): Promise<StageReport | null | undefined> {
  const path = shell.repoPath();
  if (shell.mode !== "live" || !path || shell.switching || shell.stageDisabled) return undefined;
  const repo = shell.jobKey(path);
  const request = extra.request as { files?: string[]; unstage?: boolean } | undefined;
  const unstage = command.startsWith("unstage") || request?.unstage === true;
  const rows = unstage ? (shell.stagedFiles ?? shell.snapshot?.staged ?? []) : (shell.unstagedFiles ?? shell.snapshot?.unstaged ?? []);
  const selected = request?.files ?? (Array.isArray(extra.files) ? extra.files as string[] : null);
  const file = typeof extra.file === "string" ? extra.file : "";
  const files = [...new Set(selected ?? (command.endsWith("_all") ? rows.map((row) => row.path) : rows.filter((row) => row.path === file || row.path.startsWith(`${file}/`)).map((row) => row.path)))];
  if (files.length === 0) return undefined;
  const operation = traceOperation();
  const change = { id: ++nextId, repo, files, unstage };
  const args = request ? { ...extra, request: { ...request, files } } :
    file ? { ...extra, files, filtered: extra.filtered ?? true } : extra;
  const openGen = shell.openGen;
  const queuedAt = performance.now();
  const described = selected ? shell.describeRequest({ action: "stagePaths", files, unstage }) : null;
  const current = () => openGen === shell.openGen && shell.viewing(path) && (!shell.focusPath || shell.jobKey(shell.focusPath) === repo);
  shell.actionError = null;
  shell.stashRetry = null;
  shell.pendingStages = [...(shell.pendingStages ?? []), change];
  shell.statusGen += 1;
  shell.startWork(shell.tr(unstage ? "chrome.unstagePending" : "chrome.stagePending"), path);
  if (shell.selectedPath && files.includes(shell.selectedPath)) {
    const target = unstage ? "unstaged" : "staged";
    const visible = shell.stageView ?? projectStages(shell.snapshot, shell.pendingStages);
    if (visible[target].some((row) => row.path === shell.selectedPath)) shell.selectedSide = target;
    else if (!visible[shell.selectedSide].some((row) => row.path === shell.selectedPath)) shell.selectedPath = null;
    shell.diffToken += 1;
    shell.diff = null;
    shell.diffError = null;
    shell.conflictView = null;
    shell.preview = null;
    shell.imageOld = null;
    shell.imageNew = null;
  }
  const remove = () => {
    if (shell.pendingStages.some((item) => item.id === change.id)) shell.pendingStages = shell.pendingStages.filter((item) => item.id !== change.id);
  };
  try {
    const startedAt = performance.now();
    let backendFinished = 0;
    let report: StageReport | null = null;
    const executionLog = (output: string) => {
      const now = performance.now();
      return `${output.trim()}\n\n[AweGit] backend=${Math.round((backendFinished || now) - startedAt)} ms; refresh=${backendFinished ? Math.round(now - backendFinished) : 0} ms; wait=${Math.round(startedAt - queuedAt)} ms; total=${Math.round(now - queuedAt)} ms`.trim();
    };
    try {
      const incremental = command === "stage_path" || command === "unstage_path";
      const value = await invoke<StageReport>(command, { path, ...args, __task: operation, __selection: files, __taskLabel: shell.tr(unstage ? "chrome.unstagePending" : "chrome.stagePending"), ...(incremental ? { incremental: true } : {}) }, undefined, operation);
      report = value;
      backendFinished = performance.now();
      if (value.outcome === "failed" || value.error) throw new Error([value.error, value.log || value.output].filter(Boolean).join("\n\n") || "Git stage failed");
      remove();
      if (current()) shell.applySnapshot(value);
      else if (shell.viewing(path)) shell.scheduleStageRefresh();
      if (described) shell.recordCommand("stagePaths", described, executionLog(value.log || value.output || ""), true, source);
      return value;
    } catch (error) {
      remove();
      if (current()) {
        if (report?.path && report.staged && report.unstaged) shell.applySnapshot(report);
        else shell.scheduleStageRefresh();
        if (report?.staleLock && request) shell.confirmStaleLock(path, report.staleLock, { ...request, files }, source);
        else shell.actionError = shell.message(error);
      } else if (shell.viewing(path)) {
        shell.scheduleStageRefresh();
      }
      if (described) shell.recordCommand("stagePaths", described, executionLog(shell.message(error)), false, source);
      return null;
    }
  } finally {
    remove();
    shell.stopWork(path);
    if (current()) {
      if (!shell.pendingStages.some((item) => item.repo === repo)) void shell.loadDiff();
      shell.replayWatch();
    }
  }
}
