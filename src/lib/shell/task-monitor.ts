import { listen } from "@tauri-apps/api/event";
import { invoke } from "../perf";
import { areasFor } from "../operation-policy";
import { acceptTasks, clearRemoteTasks, ownsTask, taskDone, taskRecord, type OperationTask } from "../tasks";
import type { ShellState } from "./state.svelte";

export function watchOperationTasks(shell: ShellState) {
  if (!shell.backend()) return () => {};
  let stopped = false, unlisten = () => {}, timer: ReturnType<typeof setTimeout>;
  let cursor = 0;
  let session: string | undefined;

  function accept(tasks: readonly OperationTask[]) {
    if (!tasks.length) return;
    const previous = new Map(tasks.map((task) => [task.id, taskRecord(task.id)]));
    for (const task of acceptTasks(tasks)) {
      const prior = previous.get(task.id);
      if (!prior || !taskDone(prior)) shell.noteTaskProgress(task);
      if (ownsTask(task.id) || prior && taskDone(prior) || !taskDone(task) || !shell.viewing(task.path)) continue;
      const areas = areasFor(task.action);
      shell.scheduleAuxiliary(areas);
      if (areas.some((area) => area === "worktree" || area === "index") || task.state !== "completed") {
        shell.queueWatch("status", task.revision); shell.replayWatch();
      }
    }
  }

  const poll = async () => {
    try {
      const feed = await invoke<OperationTask[] | { session: string; tasks: OperationTask[] }>("operation_tasks", { after: cursor, session });
      if (stopped) return;
      if (!Array.isArray(feed) && feed.session !== session) {
        if (session != null) {
          clearRemoteTasks();
          shell.statusRevision = 0;
          shell.statusObservedRevision = 0;
          shell.contextRevision = 0;
          if (shell.repoPath()) { shell.queueWatch("full", 0); shell.replayWatch(); }
        }
        cursor = 0;
        session = feed.session;
      }
      const tasks = Array.isArray(feed) ? feed : feed.tasks;
      for (const task of tasks) cursor = Math.max(cursor, task.sequence ?? 0);
      accept(tasks);
    } catch { /* A reconnect retains local tasks and retries. */ }
    if (!stopped) timer = setTimeout(poll, shell.inApp() ? 15000 : shell.operationTasks.some((task) => !taskDone(task)) ? 250 : 1500);
  };
  if (shell.inApp()) void listen<OperationTask>("operation-task", (event) => {
    if (stopped) return;
    accept([event.payload]);
  }).then((stop) => { if (stopped) stop(); else unlisten = stop; }).catch(() => {}).finally(() => { if (!stopped) void poll(); });
  else void poll();
  return () => { stopped = true; clearTimeout(timer); unlisten(); };
}
