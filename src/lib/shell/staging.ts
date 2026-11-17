import { invoke, traceOperation } from "../perf";
import type { MutationResult } from "../operation-flow";
import { statusPathBefore, type StatusFile, type StatusSnapshot } from "../status";
import type { ShellState } from "./state.svelte";

export type StageChange = { id: number; taskId?: string; generation?: number; repo: string; files: string[]; unstage: boolean };
type StageReport = StatusSnapshot & Partial<MutationResult>;
type StageLane = { tail: Promise<unknown>; generation: number; error?: string; reconcile?: Promise<StatusSnapshot | null> };
const lanes = new WeakMap<ShellState, Map<string, StageLane>>();
let nextId = 0;

const keyOf = (path: string) => path.replace(/^\\\\\?\\/, "").replaceAll("\\", "/").replace(/\/+$/, "").toLowerCase();
const pathOf = (path: string) => path.replaceAll("\\", "/");

/** Project pending intents without changing the snapshot used for Git decisions. */
export function projectStages(snapshot: StatusSnapshot | null, changes: readonly StageChange[]) {
  if (!snapshot) return { staged: [] as StatusFile[], unstaged: [] as StatusFile[] };
  const pending = changes.filter((change) => change.repo === keyOf(snapshot.path));
  if (pending.length === 0) return snapshot;
  const staged = new Map(snapshot.staged.map((file) => [file.path, file]));
  const unstaged = new Map(snapshot.unstaged.map((file) => [file.path, file]));
  for (const change of pending) {
    for (const path of change.files) {
      const index = staged.get(path), work = unstaged.get(path);
      // Rename boundaries and conflicts need the backend's actual index result.
      if (index?.kind === "renamed" || work?.kind === "renamed" || index?.kind === "conflict" || work?.kind === "conflict") continue;
      if (change.unstage) {
        if (!index) continue;
        staged.delete(path);
        if (index.kind === "added" && work?.kind === "deleted") unstaged.delete(path);
        else unstaged.set(path, { ...(work ?? index), kind: index.kind === "added" ? "untracked" : work?.kind === "untracked" ? "modified" : (work?.kind ?? index.kind), previousPath: null });
      } else {
        if (!work) continue;
        unstaged.delete(path);
        if (index?.kind === "added" && work.kind === "deleted") staged.delete(path);
        else staged.set(path, { ...work, kind: work.kind === "untracked" ? "added" : index?.kind === "added" ? "added" : work.kind, previousPath: null });
      }
    }
  }
  const ordered = (files: Map<string, StatusFile>) => [...files.values()].sort((left, right) => left.path === right.path ? 0 : statusPathBefore(left.path, right.path) ? -1 : 1);
  return { staged: ordered(staged), unstaged: ordered(unstaged) };
}

/** Include ancestors once so visible folder rows use constant-time lookups. */
export function stageActions(changes: readonly StageChange[], repo: string) {
  const actions = new Map<string, boolean>();
  for (const change of changes) {
    if (change.repo !== keyOf(repo)) continue;
    for (const file of change.files) {
      let path = pathOf(file);
      while (path) {
        actions.set(path, change.unstage);
        const slash = path.lastIndexOf("/");
        if (slash < 0) break;
        path = path.slice(0, slash);
      }
    }
  }
  return actions;
}

/** One frontend lane per repository; Rust still owns the actual write lock. */
export async function enqueueStage(shell: ShellState, command: string, extra: Record<string, unknown> = {}, source: "user" | "background" = "user"): Promise<StageReport | null | undefined> {
  const path = shell.repoPath();
  if (shell.mode !== "live" || !path || shell.switching || shell.stageDisabled) return undefined;
  const repo = shell.jobKey(path);
  const alreadyQueued = (shell.pendingStages ?? []).some((change) => change.repo === repo);
  const request = extra.request as { files?: string[]; unstage?: boolean } | undefined;
  const unstage = command.startsWith("unstage") || request?.unstage === true;
  const rows = unstage ? (shell.stagedFiles ?? shell.snapshot?.staged ?? []) : (shell.unstagedFiles ?? shell.snapshot?.unstaged ?? []);
  const selected = request?.files ?? (Array.isArray(extra.files) ? extra.files as string[] : null);
  const file = typeof extra.file === "string" ? extra.file : "";
  const files = [...new Set(selected ?? (command.endsWith("_all") ? rows.map((row) => row.path) : rows.filter((row) => row.path === file || row.path.startsWith(`${file}/`)).map((row) => row.path)))];
  if (files.length === 0) return undefined;
  let repositories = lanes.get(shell);
  if (!repositories) { repositories = new Map(); lanes.set(shell, repositories); }
  let lane = repositories.get(repo);
  if (!lane || lane.error) { lane = { tail: Promise.resolve(), generation: (lane?.generation ?? -1) + 1 }; repositories.set(repo, lane); }
  const activeLane = lane;
  const generation = lane.generation;
  const operation = traceOperation();
  const change = { id: ++nextId, taskId: operation, generation, repo, files, unstage };
  const args = request ? { ...extra, request: { ...request, files } } : selected ? { ...extra, files } : extra;
  const queuedAt = performance.now();
  const described = selected ? shell.describeRequest({ action: "stagePaths", files, unstage }) : null;
  const current = () => shell.viewing(path) && (!shell.focusPath || shell.jobKey(shell.focusPath) === repo);
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
  const promise = (async (): Promise<StageReport | null> => {
    if (activeLane.generation !== generation) return null;
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
      if (described) shell.recordCommand("stagePaths", described, executionLog(value.log || value.output || ""), true, source);
      return value;
    } catch (error) {
      // Later intents may depend on this write. Reconcile before permitting a retry.
      if (!activeLane.error) {
        activeLane.generation += 1;
        activeLane.error = shell.message(error);
        if (current()) shell.actionError = activeLane.error;
        const later = shell.pendingStages.filter((item) => item.repo === repo && item.generation === generation && item.id > change.id && item.taskId);
        await Promise.all(later.map((item) => invoke("cancel_task", { id: item.taskId }).catch(() => {})));
      }
      if (!backendFinished) backendFinished = performance.now();
      activeLane.reconcile ??= invoke<StatusSnapshot>("workspace_status", { path }, undefined, operation).catch(() => null);
      const status = await activeLane.reconcile;
      shell.pendingStages = shell.pendingStages.filter((item) => item.repo !== repo || item.generation !== generation);
      if (current()) {
        if (status) shell.applySnapshot(status);
        else shell.scheduleStageRefresh();
        if (report?.staleLock && request) shell.confirmStaleLock(path, report.staleLock, { ...request, files }, source);
        else shell.actionError = activeLane.error;
      }
      if (described) shell.recordCommand("stagePaths", described, executionLog(shell.message(error)), false, source);
      return null;
    }
  })().finally(() => {
    remove();
    shell.stopWork(path);
    if (current()) {
      if (!shell.pendingStages.some((item) => item.repo === repo)) void shell.loadDiff();
      shell.replayWatch();
    }
  });
  lane.tail = promise;
  const settled = () => {
    if (repositories!.get(repo) === activeLane && activeLane.tail === promise) repositories!.delete(repo);
  };
  void promise.then(settled, settled);
  return promise;
}
