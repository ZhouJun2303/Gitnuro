<script lang="ts">
  import { getShell } from "./context";
  import { invoke } from "../perf";
  import { acceptTask, taskDone, type OperationTask } from "../tasks";
  import { onVirtualScroll, windowSlice } from "../view";

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
  .task-queue { position: relative; flex: 0 0 auto; color: var(--text-secondary); }
  summary { cursor: pointer; padding: 8px 0; border-bottom: 2px solid transparent; white-space: nowrap; }
  .task-queue[open] summary { color: var(--accent); border-bottom-color: var(--accent); font-weight: 650; }
  summary span { margin-left: 4px; font-weight: 650; font-variant-numeric: tabular-nums; }
  .task-popover { position: absolute; left: 0; top: 100%; z-index: 100; width: min(440px, 80vw); max-height: min(360px, 60vh); overflow: auto; border: 1px solid var(--line); border-radius: 8px; background: var(--elevated); color: var(--text); box-shadow: 0 8px 24px #0003; padding: 8px; }
  .task-window { position: relative; }
  .task-row { position: absolute; left: 0; right: 0; height: 56px; box-sizing: border-box; display: flex; align-items: center; gap: 8px; padding: 8px 4px; border-bottom: 1px solid var(--line); }
  .task-row div { flex: 1; min-width: 0; }
  strong, small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  small { margin-top: 3px; opacity: .7; }
  .task-row > span { flex: 0 0 auto; }
  [data-task-state="failed"], [data-task-state="blocked"] { color: var(--deleted); }
</style>
