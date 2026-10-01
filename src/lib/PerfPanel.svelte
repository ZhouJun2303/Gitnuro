<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { clientStats, frame, type ClientStat } from "./perf";

  type Stat = { name: string; count: number; totalMs: number; maxMs: number; lastMs: number; slow: number; onMain: number; failed: number };
  type Snapshot = { uptimeMs: number; build: string; logFile: string; stalls: number; stallMaxMs: number; stallTotalMs: number; items: Stat[] };

  let open = $state(false);
  let snap = $state<Snapshot | null>(null);
  let web = $state<ClientStat[]>([]);
  let tasks = $state({ ...frame });

  const live = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

  async function poll() {
    web = clientStats().slice(0, 30);
    tasks = { ...frame };
    if (live) snap = await invoke<Snapshot>("perf_snapshot").catch(() => snap);
  }

  function ms(value: number) {
    return value >= 1000 ? `${(value / 1000).toFixed(1)}s` : `${Math.round(value)}ms`;
  }

  $effect(() => {
    if (!open) return;
    void poll();
    const timer = window.setInterval(() => void poll(), 1000);
    return () => window.clearInterval(timer);
  });

  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.altKey && !event.shiftKey && event.key.toLowerCase() === "p") {
        event.preventDefault();
        open = !open;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

{#if open}
  <aside class="perf">
    <header>
      <strong>Performance</strong>
      {#if snap}
        <span>{snap.build}</span>
        <span>up {ms(snap.uptimeMs)}</span>
        <span class:bad={snap.stalls > 0}>main-thread stalls {snap.stalls} · max {ms(snap.stallMaxMs)} · total {ms(snap.stallTotalMs)}</span>
      {/if}
      <span class:bad={tasks.longTasks > 0}>long tasks {tasks.longTasks} · max {ms(tasks.longTaskMax)}</span>
      <span class="grow"></span>
      {#if live}<button type="button" onclick={() => void invoke("open_log_folder")}>Open logs</button>{/if}
      <button type="button" aria-label="Close" onclick={() => (open = false)}>×</button>
    </header>
    {#if snap}
      <p class="path">{snap.logFile}</p>
      <table>
        <thead><tr><th>backend</th><th>count</th><th>avg</th><th>max</th><th>last</th><th>slow</th><th>on main</th><th>failed</th></tr></thead>
        <tbody>
          {#each snap.items.slice(0, 30) as item (item.name)}
            <tr class:bad={item.maxMs >= 200}>
              <td>{item.name}</td><td>{item.count}</td><td>{ms(item.totalMs / item.count)}</td><td>{ms(item.maxMs)}</td><td>{ms(item.lastMs)}</td><td>{item.slow}</td><td>{item.onMain}</td><td>{item.failed}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    <table>
      <thead><tr><th>invoke (round trip)</th><th>count</th><th>avg</th><th>max</th><th>last</th><th>failed</th></tr></thead>
      <tbody>
        {#each web as item (item.name)}
          <tr class:bad={item.maxMs >= 200}>
            <td>{item.name}</td><td>{item.count}</td><td>{ms(item.totalMs / item.count)}</td><td>{ms(item.maxMs)}</td><td>{ms(item.lastMs)}</td><td>{item.failed}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </aside>
{/if}

<style>
  .perf {
    position: fixed;
    right: 12px;
    bottom: 12px;
    z-index: 10000;
    width: min(760px, calc(100vw - 24px));
    max-height: 70vh;
    overflow: auto;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--elevated);
    color: var(--text);
    box-shadow: var(--shadow);
    font: 12px/1.5 var(--mono);
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
  }
  header span {
    color: var(--text-secondary);
  }
  .grow {
    flex: 1;
  }
  button {
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--field);
    color: var(--text);
    font: inherit;
    padding: 2px 8px;
    cursor: pointer;
  }
  button:hover {
    background: var(--hover);
  }
  .path {
    margin: 4px 0 8px;
    color: var(--text-secondary);
    word-break: break-all;
  }
  table {
    width: 100%;
    margin-top: 8px;
    border-collapse: collapse;
  }
  th,
  td {
    padding: 2px 6px;
    text-align: right;
    white-space: nowrap;
  }
  th:first-child,
  td:first-child {
    text-align: left;
    white-space: normal;
    word-break: break-all;
  }
  th {
    color: var(--text-secondary);
    font-weight: 500;
    border-bottom: 1px solid var(--separator);
  }
  .bad,
  header span.bad {
    color: var(--deleted);
  }
</style>
