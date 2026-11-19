import { repositoryKey, repositoryWrites, requestTargets } from "./operation-policy";
import type { RefSnapshot } from "./status";

export type TaskState = "queued" | "running" | "verifying" | "cancelling" | "completed" | "conflict" | "failed" | "cancelled" | "blocked";
export type OperationTask = {
  id: string; path: string; action: string; label: string; targets: string[]; state: TaskState; dependsOn: string[];
  revision: number; queuedMs: number; executeMs: number; verifyMs: number;
  kind?: "repository" | "service"; cancelCommand?: string;
  preview?: Record<string, string>; phase?: string; percent?: number;
  sequence?: number; targetCount?: number;
  refreshScope?: "status" | "full";
};
const independentCommands = new Set(["suggest_commit_message", "run_lite_agent", "review_branch", "test_ai_endpoint", "list_ai_models", "sync_accounts", "test_account", "browser_login", "generate_ssh_key", "create_github_repo", "create_hosted_repo", "download_font", "download_google_font", "recognize_google_font", "check_for_update", "download_update", "scan_repositories", "repository_inspection", "launch_tool", "create_revision_copy", "remove_revision_copy", "save_revision", "external_tools", "editor_tools", "remote_matches", "forge_notifications", "forge_pulls"]);
const cancellation: Record<string, string> = { download_update: "cancel_update", browser_login: "cancel_browser_login" };
independentCommands.add("detect_terminal_clis");
for (const command of ["mark_notification", "device_start", "device_poll", "save_settings", "save_window_bounds", "import_fork_settings", "write_ssh_config", "read_ssh_config", "open_editor", "open_terminal", "open_file", "reveal_path", "open_log_folder", "reveal_commit", "request_history_action", "open_window", "diagnostics", "font_cache_info", "font_list", "clear_font_cache", "font_cache_clear", "revision_copy_formats", "revision_copy_support", "program_exists"]) independentCommands.add(command);
const records = new Map<string, OperationTask>();
const listeners = new Set<(tasks: OperationTask[]) => void>();
const lastWrites = new Map<string, string>();
const localTasks = new Set<string>();
const clientId = `client-${Math.random().toString(36).slice(2)}-${Date.now().toString(36)}`;
export const ownsTask = (id: string) => localTasks.has(id);
export const taskRecord = (id: string) => records.get(id);
export const taskDone = (task: OperationTask) => ["completed", "conflict", "failed", "cancelled", "blocked"].includes(task.state);
export const taskRefsPending = (task: OperationTask, revision: number) => task.kind !== "service"
  && (!taskDone(task) || (task.state === "completed" && task.refreshScope !== "status" && task.revision > revision));

const order: Record<TaskState, number> = { queued: 0, running: 1, verifying: 2, cancelling: 3, completed: 4, conflict: 4, failed: 4, cancelled: 4, blocked: 4 };

function publish() {
  const all = [...records.values()];
  for (const listener of listeners) listener(all);
}

export function clearRemoteTasks() {
  for (const id of records.keys()) if (!localTasks.has(id)) records.delete(id);
  publish();
}

export function acceptTask(task: OperationTask, native = true, notify = true) {
  const previous = records.get(task.id);
  if (previous) {
    if (taskDone(previous) && !taskDone(task)) return false;
    if (native && task.sequence != null && previous.sequence != null && task.sequence <= previous.sequence) return false;
    if (task.sequence == null && (order[task.state] < order[previous.state]
        || (task.state === previous.state && (task.percent ?? 0) < (previous.percent ?? 0)))) return false;
  }
  const merged = { ...previous, ...task };
  // Keep exact paths for this window's optimistic writes while they are active.
  if (!taskDone(merged) && previous && localTasks.has(task.id) && previous.targets.length > task.targets.length) merged.targets = previous.targets;
  if (taskDone(merged) && merged.targets.length > 16) {
    merged.targetCount = merged.targets.length;
    merged.targets = merged.targets.slice(0, 16);
  }
  records.set(task.id, merged);
  if (records.size > 256) for (const [id, entry] of records) {
    if (records.size <= 200) break;
    if (taskDone(entry)) { records.delete(id); localTasks.delete(id); }
  }
  if (notify) publish();
  return true;
}

export function acceptTasks(tasks: readonly OperationTask[]) {
  const changed = tasks.filter((task) => acceptTask(task, true, false));
  if (changed.length) publish();
  return changed;
}

export function subscribeTasks(listener: (tasks: OperationTask[]) => void) {
  listeners.add(listener); listener([...records.values()]);
  return () => { listeners.delete(listener); };
}

export function pendingFor(path: string, file?: string) {
  const repo = repositoryKey(path);
  return [...records.values()].filter((task) => !taskDone(task) && repositoryKey(task.path) === repo
    && (!file || task.targets.length === 0 || (task.targetCount ?? 0) > task.targets.length || task.targets.some((target) => target === file || file.startsWith(`${target}/`))));
}

/** Freeze a task's identity and inputs before the first asynchronous boundary. */
export async function runTask<T>(command: string, args: Record<string, unknown>, id: string, execute: (args: Record<string, unknown>) => Promise<T>): Promise<T> {
  // IPC values can contain Svelte proxies. Copy their plain fields before queuing.
  args = capture(args) as Record<string, unknown>;
  const write = repositoryWrites.has(command);
  if (!write && !independentCommands.has(command)) return execute(args);
  id = typeof args.__task === "string" ? args.__task : id;
  localTasks.add(id);
  const request = (args.request ?? args) as Record<string, unknown>;
  const action = String(request.action ?? command);
  const path = String(request.destination ?? args.path ?? "");
  const key = repositoryKey(path);
  const prior = lastWrites.get(key);
  const previous = prior ? records.get(prior) : undefined;
  const after = write && previous && !taskDone(previous) ? previous.id : undefined;
  const dependsOn = Array.isArray(args.__dependsOn) ? [...args.__dependsOn] as string[] : [];
  const task: OperationTask = { id, path, action, label: String(args.__taskLabel ?? ""), targets: requestTargets(args), state: "queued", dependsOn,
    revision: 0, queuedMs: 0, executeMs: 0, verifyMs: 0 };
  task.kind = write ? "repository" : "service";
  task.preview = Object.fromEntries(["name", "to", "branch", "rev", "start", "remote", "path"].flatMap((key) => typeof request[key] === "string" ? [[key, request[key]]] : []));
  if (action === "stashRename" && typeof request.message === "string") task.preview.message = request.message;
  task.cancelCommand = cancellation[command];
  if (!write) task.state = "running";
  if (write) lastWrites.set(key, id);
  acceptTask(task);
  const started = performance.now();
  try {
    const value = await execute({ ...args, __task: id, __client: clientId, __after: after, __dependsOn: dependsOn });
    const report = value as { outcome?: TaskState; refreshScope?: "status" | "full" } | null;
    const outcome = report?.outcome ?? "completed";
    const current = records.get(id) ?? task;
    acceptTask({ ...current, state: taskDone(current) ? current.state : outcome,
      ...(report?.refreshScope ? { refreshScope: report.refreshScope } : {}), executeMs: current.executeMs || performance.now() - started }, false);
    return value;
  } catch (error) {
    const current = records.get(id) ?? task;
    acceptTask({ ...current, state: taskDone(current) ? current.state : String(error).includes("awegit:cancelled:") ? "cancelled" : "failed", executeMs: current.executeMs || performance.now() - started }, false);
    throw error;
  }
}

function capture(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(capture);
  if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, capture(child)]));
  return value;
}

/** Display intentions independently of the refs used for Git decisions. */
export function projectRefs(snapshot: RefSnapshot | null, tasks: readonly OperationTask[], path: string, revision: number): RefSnapshot | null {
  if (!snapshot) return snapshot;
  const pending = tasks.filter((task) => repositoryKey(task.path) === repositoryKey(path) && taskRefsPending(task, revision));
  if (!pending.length) return snapshot;
  let branches = snapshot.branches, tags = snapshot.tags, remotes = snapshot.remotes, stashes = snapshot.stashes, worktrees = snapshot.worktrees, submodules = snapshot.submodules;
  for (const task of pending) {
    const preview = task.preview ?? {}, name = preview.name ?? "";
    if (task.action === "createBranch" && name && !branches.some((branch) => branch.name === name)) branches = [...branches, { name, current: false, upstream: null, ahead: 0, behind: 0 }];
    if (task.action === "renameBranch" && preview.to) branches = branches.map((branch) => branch.name === name ? { ...branch, name: preview.to } : branch);
    if (task.action === "tag" && name && !tags.some((tag) => tag.name === name)) tags = [{ name, id: "" }, ...tags];
    if (task.action === "addRemote" && name && !remotes.some((remote) => remote.name === name)) remotes = [...remotes, { name, url: null, head: null, branches: [] }];
    if (task.action === "addWorktree" && preview.path && !worktrees.some((row) => row.path === preview.path)) worktrees = [...worktrees, { path: preview.path, branch: preview.branch || null, detached: false }];
    if (task.action === "submoduleAdd" && preview.path && !submodules.some((row) => row.path === preview.path)) submodules = [...submodules, { path: preview.path, id: "", ready: false }];
    if (task.action === "stashRename" && preview.message) stashes = stashes.map((stash) => stash.name === name || stash.id === name ? { ...stash, summary: preview.message } : stash);
  }
  return branches === snapshot.branches && tags === snapshot.tags && remotes === snapshot.remotes && stashes === snapshot.stashes && worktrees === snapshot.worktrees && submodules === snapshot.submodules ? snapshot : { ...snapshot, branches, tags, remotes, stashes, worktrees, submodules };
}
