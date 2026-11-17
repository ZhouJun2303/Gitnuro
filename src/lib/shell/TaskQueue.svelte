<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getShell } from "./context";
  import { invoke } from "../perf";
  import { acceptTask, ownsTask, taskDone, type OperationTask } from "../tasks";
  import { onVirtualScroll, windowSlice } from "../view";
  import { areasFor } from "../operation-policy";

  const shell = getShell();
  const active = $derived(shell.operationTasks.filter((task) => !taskDone(task)));
  const shown = $derived([...active, ...shell.operationTasks.filter(taskDone).slice(-15).reverse()]);
  let top = $state(0);
  const visible = $derived(windowSlice(shown, top, 56, 20, 32));
  const onScroll = onVirtualScroll((value) => { top = value; });
  let error = $state("");

  async function cancel(task: OperationTask) {
    error = "";
    acceptTask({ ...task, state: "cancelling" });
    try {
      const result = await invoke<OperationTask | null>(task.cancelCommand ?? "cancel_task", task.cancelCommand ? {} : { id: task.id });
      if (!task.cancelCommand) acceptTask(result ?? { ...task, state: "cancelled" });
    }
    catch (failure) { acceptTask(task); error = shell.message(failure); }
  }

  onMount(() => {
    if (!shell.backend()) return;
    let stopped = false, unlisten = () => {}, timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const tasks = await invoke<OperationTask[]>("operation_tasks");
        if (stopped) return;
        for (const task of tasks) accept(task);
      } catch { /* A reconnect retains local tasks and retries. */ }
      if (!stopped && !shell.inApp()) timer = setTimeout(poll, active.length ? 250 : 1500);
    };
    void poll();
    if (shell.inApp()) void listen<OperationTask>("operation-task", (event) => {
      if (stopped) return;
      accept(event.payload);
    }).then((stop) => { if (stopped) stop(); else unlisten = stop; });
    return () => { stopped = true; clearTimeout(timer); unlisten(); };
  });

  function accept(task: OperationTask) {
    const previous = shell.operationTasks.find((entry) => entry.id === task.id);
    acceptTask(task);
    if (ownsTask(task.id) || previous && taskDone(previous) || !taskDone(task) || !shell.viewing(task.path)) return;
    const areas = areasFor(task.action);
    shell.scheduleAuxiliary(areas);
    if (areas.some((area) => area === "worktree" || area === "index") || task.state !== "completed") {
      shell.queueWatch("status", task.revision); shell.replayWatch();
    }
  }
</script>

{#if shown.length}
  <details class="task-queue" data-layer="tasks">
    <summary>{shell.tr("chrome.backgroundTasks")} <span>{active.length}</span></summary>
    <div class="task-popover" onscroll={onScroll}>
      <div class="task-window" style:height="{shown.length * 56}px">
      {#each visible.rows as task, index (task.id)}
        <div class="task-row" style:top="{(visible.start + index) * 56}px" data-task-state={task.state} title={`${task.queuedMs} / ${task.executeMs} / ${task.verifyMs} ms`}>
          <div><strong>{task.label || shell.taskLabel(task.action)}</strong><small>{shell.folderName(task.path)}{task.targets[0] ? ` · ${task.targets[0]}` : ""}</small></div>
          <span>{task.state === "running" && task.phase ? shell.tr(`flow.phase-${task.phase}`) : shell.tr(`chrome.task${task.state[0].toUpperCase()}${task.state.slice(1)}`)}</span>
          {#if !taskDone(task) && (task.kind !== "service" || task.cancelCommand)}<button type="button" class="text-button" disabled={task.state === "cancelling"} onclick={() => void cancel(task)}>{shell.tr("chrome.cancel")}</button>{/if}
        </div>
      {/each}
      </div>
      {#if error}<p role="alert">{error}</p>{/if}
    </div>
  </details>
{/if}

<style>
  .task-queue { position: relative; flex: 0 0 auto; font-size: 11px; }
  summary { cursor: pointer; padding: 5px 8px; border-radius: 6px; background: var(--elevated); }
  summary span { margin-left: 5px; font-variant-numeric: tabular-nums; }
  .task-popover { position: absolute; right: 0; top: 100%; z-index: 100; width: min(440px, 80vw); max-height: min(360px, 60vh); overflow: auto; border: 1px solid var(--line); border-radius: 8px; background: var(--elevated); box-shadow: 0 8px 24px #0003; padding: 8px; }
  .task-window { position: relative; }
  .task-row { position: absolute; left: 0; right: 0; height: 56px; box-sizing: border-box; display: flex; align-items: center; gap: 8px; padding: 8px 4px; border-bottom: 1px solid var(--line); }
  .task-row div { flex: 1; min-width: 0; }
  strong, small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  small { margin-top: 3px; opacity: .7; }
  .task-row > span { flex: 0 0 auto; }
  [data-task-state="failed"], [data-task-state="blocked"] { color: var(--deleted); }
</style>
