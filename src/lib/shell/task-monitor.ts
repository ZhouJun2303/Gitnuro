import { listen } from "@tauri-apps/api/event";
import { invoke } from "../perf";
import { areasFor } from "../operation-policy";
import { acceptTask, ownsTask, taskDone, type OperationTask } from "../tasks";
import type { ShellState } from "./state.svelte";

export function watchOperationTasks(shell: ShellState) {
  if (!shell.backend()) return () => {};
  let stopped = false, unlisten = () => {}, timer: ReturnType<typeof setTimeout>;

  function accept(task: OperationTask) {
    const previous = shell.operationTasks.find((entry) => entry.id === task.id);
    if (!previous || !taskDone(previous)) shell.noteTaskProgress(task);
    acceptTask(task);
    if (ownsTask(task.id) || previous && taskDone(previous) || !taskDone(task) || !shell.viewing(task.path)) return;
    const areas = areasFor(task.action);
    shell.scheduleAuxiliary(areas);
    if (areas.some((area) => area === "worktree" || area === "index") || task.state !== "completed") {
      shell.queueWatch("status", task.revision); shell.replayWatch();
    }
  }

  const poll = async () => {
    try {
      const tasks = await invoke<OperationTask[]>("operation_tasks");
      if (stopped) return;
      for (const task of tasks) accept(task);
    } catch { /* A reconnect retains local tasks and retries. */ }
    if (!stopped && !shell.inApp()) timer = setTimeout(poll, shell.operationTasks.some((task) => !taskDone(task)) ? 250 : 1500);
  };
  void poll();
  if (shell.inApp()) void listen<OperationTask>("operation-task", (event) => {
    if (stopped) return;
    accept(event.payload);
  }).then((stop) => { if (stopped) stop(); else unlisten = stop; });
  return () => { stopped = true; clearTimeout(timer); unlisten(); };
}
