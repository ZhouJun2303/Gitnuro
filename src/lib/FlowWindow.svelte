<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "./perf";
  import { translate, resolveLocale, type Locale } from "./i18n";
  import { stateSymbol, type FlowCommit, type FlowDelta, type FlowNode, type FlowSnapshot, type FlowSummary } from "./operation-flow";
  import type { Settings } from "./status";
  import "./FlowWindow.css";

  const ROW = 344;
  const EXPANDED_ROW = ROW + 260;
  let repo = $state("");
  let selectedId = $state("");
  let live = $state(false);
  let retainOpen = $state(false);
  let closed = $state(false);
  let summary = $state.raw<FlowSummary | null>(null);
  let nodes = $state.raw<FlowNode[]>([]);
  let history = $state.raw<FlowSummary[]>([]);
  let filePages = $state<Record<string, number>>({});
  let expanded = $state<Record<string, boolean>>({});
  let locale = $state<Locale>("zh");
  let error = $state("");
  let ready = $state(false);
  let scene = $state("");
  let fixture = $state(false);
  let lastActive = "";
  let disposed = false;
  let loading = false;
  let generation = 0;
  let scrollFrame = 0;
  let viewport: HTMLDivElement;
  let scrollTop = $state(0);
  let height = $state(600);
  const desktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  const tr = (key: string, vars: Record<string, string> = {}) => translate(locale, `flow.${key}`, vars);
  const label = (key: string) => { const text = tr(key); return text.startsWith("flow.") ? key : text; };
  const title = (node: FlowNode) => node.kind === "conflict" ? tr("round", { n: String(node.round) })
    : label(({ "local-saved": "saved", "local-restore": "restore" } as Record<string, string>)[node.kind] ?? node.kind);
  const short = (id: string) => id.slice(0, 10);
  const date = (at: number) => new Date(at).toLocaleString(locale === "zh" ? "zh-CN" : "en-US", { hour12: false });
  const future = (node: FlowNode) => ["planned", "predicted", "conditional"].includes(node.state);
  const graph = $derived.by(() => {
    const actual: FlowNode[] = [], upcoming: FlowNode[] = [];
    for (const node of nodes) (future(node) ? upcoming : actual).push(node);
    actual.sort((a, b) => (a.kind === "local-restore" ? a.at : a.createdAt) - (b.kind === "local-restore" ? b.at : b.createdAt));
    upcoming.sort((a, b) => Number(a.kind === "local-restore") - Number(b.kind === "local-restore"));
    return [...actual, ...upcoming];
  });
  const layout = $derived.by(() => {
    let top = 0;
    let heldStash = "";
    const rows = graph.map((node, index) => {
      const height = expanded[node.key] ? EXPANDED_ROW : ROW;
      const saved = node.kind === "local-saved" && node.stash && ["completed", "processed"].includes(node.state);
      if (saved) heldStash = node.stash;
      const restoring = node.kind === "local-restore" && heldStash === node.stash && !["failed", "skipped", "aborted", "superseded"].includes(node.state);
      const stashRoute = heldStash ? saved ? "out" : restoring ? "in" : "held" : "";
      const row = { node, index, top, height, stashRoute, stash: heldStash };
      if (restoring) heldStash = "";
      top += height;
      return row;
    });
    return { rows, height: top, positions: new Map(rows.map((row) => [row.node.key, row.top])) };
  });
  const from = $derived.by(() => {
    let low = 0, high = layout.rows.length;
    while (low < high) {
      const mid = (low + high) >>> 1;
      const row = layout.rows[mid];
      if (row.top + row.height <= scrollTop) low = mid + 1;
      else high = mid;
    }
    return Math.max(0, low - 2);
  });
  const visible = $derived(layout.rows.slice(from, from + Math.ceil(height / ROW) + 5));
  const counts = $derived.by(() => {
    let done = 0, pending = 0, conflicts = 0;
    for (const node of nodes) {
      if (node.state === "completed" || node.state === "processed") done++;
      if (future(node)) pending++;
      if (node.kind === "conflict") conflicts++;
    }
    return { done, pending, conflicts };
  });
  const current = $derived(nodes.find((node) => node.key === summary?.activeStep));
  const currentIndex = $derived(graph.findIndex((node) => node.key === summary?.activeStep));

  function replace(value: FlowSnapshot | null) {
    if (!value) { summary = null; nodes = []; return; }
    if (summary?.id === value.summary.id && summary.sequence >= value.summary.sequence) return;
    summary = value.summary; nodes = value.nodes; selectedId = value.summary.id;
  }

  async function refresh() {
    if (fixture || loading || !repo || disposed) return;
    loading = true;
    const token = generation;
    try {
      const value = await invoke<FlowSnapshot | null>("operation_flow", { path: repo, id: selectedId || null });
      if (token === generation && !disposed) { replace(value); error = ""; }
    } catch (cause) { if (token === generation) error = tr("updateError", { error: String(cause) }); }
    finally { loading = false; }
  }

  async function loadHistory() {
    if (fixture || !repo) return;
    const token = generation;
    try {
      const value = await invoke<FlowSummary[]>("operation_history", { path: repo, offset: 0, limit: 100 });
      if (token === generation && !disposed) history = value;
    } catch (cause) { error = String(cause); }
  }

  async function navigate(id: string, automatic = false) {
    generation++; selectedId = id; live = automatic; closed = false;
    lastActive = ""; filePages = {}; expanded = {};
    summary = null; nodes = [];
    const token = generation;
    try {
      const value = await invoke<FlowSnapshot | null>("operation_flow", { path: repo, id: id || null });
      if (generation === token && !disposed) replace(value);
    } catch (cause) { if (generation === token) error = String(cause); }
    await loadHistory();
  }

  async function locate(key: string) {
    const top = layout.positions.get(key);
    if (top == null) return;
    await tick();
    if (!viewport || disposed) return;
    viewport.scrollTo({ top: Math.max(0, top - 12), behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth" });
  }

  function scrolled() {
    if (scrollFrame) return;
    scrollFrame = requestAnimationFrame(() => { scrollFrame = 0; scrollTop = viewport.scrollTop; });
  }

  $effect(() => {
    const active = summary ? `${summary.id}:${summary.activeStep}` : "";
    if (active && ready && active !== lastActive) { lastActive = active; untrack(() => void locate(summary!.activeStep)); }
  });

  $effect(() => {
    if (!ready || !live || retainOpen || !summary?.canAutoClose || summary.state !== "completed") return;
    const id = summary.id, sequence = summary.sequence;
    const timer = setTimeout(async () => {
      try {
        const value = fixture ? { summary } : await invoke<FlowSnapshot | null>("operation_flow", { path: repo, id: null });
        if (disposed || !live || retainOpen || summary?.id !== id || summary.sequence !== sequence) return;
        if (value?.summary && value.summary.id !== id) { await navigate(value.summary.id, true); return; }
        if (!value?.summary || value.summary.sequence !== sequence || !value.summary.canAutoClose) { await refresh(); return; }
        if (desktop) await getCurrentWindow().close();
        else { closed = true; window.dispatchEvent(new CustomEvent("flow-auto-closed", { detail: id })); }
      } catch (cause) { error = String(cause); }
    }, 1500);
    return () => clearTimeout(timer);
  });

  onMount(() => {
    const params = new URLSearchParams(window.location.search);
    scene = params.get("scene") ?? ""; repo = params.get("repo") ?? "";
    selectedId = params.get("rev") ?? ""; live = params.get("file") === "live";
    const unlisten: UnlistenFn[] = [];
    const boot = async () => {
      try {
        const settings = await invoke<Settings>("load_settings");
        locale = scene ? "zh" : resolveLocale(settings.locale);
        document.documentElement.dataset.theme = settings.theme === "dark" || (settings.theme !== "light" && matchMedia("(prefers-color-scheme: dark)").matches) ? "dark" : "light";
        if (scene.startsWith("operation-flow") && !repo) {
          const { flowFixture } = await import("./lab/flow-fixture");
          fixture = true; const sample = flowFixture(scene); repo = sample.summary.repo; replace(sample); history = [sample.summary];
        } else {
          if (desktop) {
            const offFlow = await listen<FlowDelta>("operation-flow", ({ payload }) => {
              if (payload.summary.repo !== repo || (selectedId && payload.summary.id !== selectedId)) return;
              if (!summary || payload.summary.sequence > summary.sequence + 1) { void refresh(); return; }
              if (payload.summary.sequence <= summary.sequence) return;
              summary = payload.summary;
              if (payload.node) {
                const index = nodes.findIndex((node) => node.key === payload.node!.key);
                const next = nodes.slice(); if (index < 0) next.push(payload.node); else next[index] = payload.node; nodes = next;
              }
            });
            const offNav = await listen<{ view: string; repo: string; file: string; rev: string }>("awegit-navigate", ({ payload }) => {
              if (payload.view !== "flow" || payload.repo !== repo) return;
              if (payload.file === "live" && payload.rev === summary?.id) { void refresh(); return; }
              if (payload.file === "live" && !live) retainOpen = true;
              void navigate(payload.rev, payload.file === "live");
            });
            if (disposed) { offFlow(); offNav(); return; } unlisten.push(offFlow, offNav);
          }
          await refresh(); await loadHistory();
        }
        if (!disposed) ready = true;
      } catch (cause) { error = String(cause); ready = true; }
    };
    void boot();
    const poll = setInterval(() => { if (!document.hidden) void refresh(); }, 1000);
    const historyPoll = setInterval(() => { if (!document.hidden) void loadHistory(); }, 10_000);
    return () => { disposed = true; generation++; clearInterval(poll); clearInterval(historyPoll); cancelAnimationFrame(scrollFrame); unlisten.forEach((off) => off()); };
  });
</script>

{#snippet commitCard(commit: FlowCommit, caption: string)}
  <small class="flow-caption">{caption}</small>
  <strong class="flow-commit-title" title={commit.summary}>{commit.summary}</strong>
  <span class="flow-commit-meta"><code title={commit.id}>{short(commit.id)}</code><span>·</span>{commit.author}</span>
{/snippet}

<main class="flow-window" data-lab-scene={ready ? scene || undefined : undefined} data-closed={closed || undefined}>
  <header class="flow-header">
    <div class="flow-heading"><div class="flow-heading-line"><h1>{tr("title")}</h1><span class="flow-observer"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/></svg>{tr("observer")}</span></div><p title={repo}>{summary?.branch ?? ""}<span>·</span>{repo}</p></div>
    <label class="flow-history">{tr("history")}<select aria-label={tr("history")} value={summary?.id ?? ""} disabled={fixture || history.length === 0} onchange={(event) => void navigate(event.currentTarget.value)}>
      {#if summary && !history.some((item) => item.id === summary!.id)}<option value={summary.id}>{label(summary.kind)} · {date(summary.startedAt)}</option>{/if}
      {#each history as item (item.id)}<option value={item.id}>{label(item.kind)} · {date(item.startedAt)} · {label(item.state)}</option>{/each}
      {#if !summary && history.length === 0}<option value="">{tr("empty")}</option>{/if}
    </select></label>
    <span class="flow-badge" data-state={summary?.state}>{stateSymbol(summary?.state ?? "planned")} {label(summary?.state ?? "planned")}</span>
  </header>
  <div class="flow-meta"><span>{tr("observerHint")}</span><span>✓ {counts.done} <span>·</span> ○ {counts.pending} <span>·</span> ! {counts.conflicts}</span></div>
  {#if error}<div class="flow-error" role="alert">{error}</div>{/if}
  {#if summary?.recordingError}<div class="flow-error" role="alert">{tr("recording", { error: summary.recordingError })}</div>{/if}
  <section class="flow-graph-pane" aria-label={tr("title")}>
    <div class="flow-legend"><span class="flow-legend-key" data-state="completed">✓ {tr("completed")}</span><span class="flow-legend-key" data-state="current">◉ {tr("currentStep")}</span><span class="flow-legend-key" data-state="conflict">! {tr("conflict")}</span><span class="flow-legend-key" data-state="planned">○ {tr("planned")}</span><span class="flow-legend-key" data-state="stash">↗ {tr("stashLane")}</span><span class="flow-follow" aria-live="polite">{current ? `${tr("currentStep")} · ${currentIndex + 1} / ${graph.length} · ${title(current)}` : tr("scope")}</span></div>
    <div class="flow-lanes"><span>{tr("leftLane")}</span><span>{tr("timeAxis")} ↓</span><span>{tr("rightLane")}</span></div>
    <div class="flow-scroll" bind:this={viewport} bind:clientHeight={height} onscroll={scrolled}>
      {#if !ready}<p class="flow-empty">{tr("loading")}</p>{:else if !summary}<p class="flow-empty">{tr("empty")}</p>{/if}
      <div class="flow-track" style:height={`${layout.height}px`}>
        {#each visible as row (row.node.key)}
          {@const node = row.node}
          <article class="flow-row" class:active={summary?.activeStep === node.key} class:past={row.index < currentIndex} class:has-stash={!!row.stashRoute} style:top={`${row.top}px`} style:height={`${row.height}px`} data-state={node.state} data-node={node.key} data-stash-route={row.stashRoute || undefined} data-stash={row.stash || undefined} aria-current={summary?.activeStep === node.key ? "step" : undefined} aria-label={`${row.index + 1} / ${graph.length} · ${title(node)} · ${label(node.state)}`}>
            <svg class="flow-roads" viewBox={`0 0 1000 ${row.height}`} preserveAspectRatio="none" aria-hidden="true">
              <path class="flow-axis" d={`M500 0V${row.height}`} />
              {#if node.source && node.kind !== "local-saved" && node.kind !== "local-restore"}<path class="flow-road flow-road-right" d="M770 136 C770 180 580 150 500 194" />{/if}
              {#if row.stashRoute === "out"}
                <path class="flow-stash-road" d={`M500 194 C740 194 980 180 980 260 V${row.height}`} />
              {:else if row.stashRoute === "in"}
                <path class="flow-stash-road" class:predicted={future(node)} d="M980 0 V120 C980 180 650 194 500 194" />
              {:else if row.stashRoute === "held"}
                <path class="flow-stash-road" d={`M980 0V${row.height}`} />
              {/if}
              {#if row.stashRoute}<path class="flow-stash-direction" class:predicted={row.stashRoute === "in" && future(node)} d={row.stashRoute === "in" ? "m515 184 -15 10 15 5" : `M975 ${row.height - 8}l5 6 5-6`} />{/if}
              <path class="flow-direction" d={`m490 182 10 12 10-12 M495 ${row.height - 8}l5 6 5-6`} />
            </svg>
            {#if row.stashRoute}<span class="flow-stash-label" title={row.stash}>{tr(row.stashRoute === "out" ? "stashDiverge" : row.stashRoute === "in" ? "stashRejoin" : "stashHeld")}</span>{/if}
            <div class="flow-side flow-left">
              {#if node.target}{@render commitCard(node.target, tr("target"))}
              {:else}<small class="flow-caption">{tr("recordStart")}</small><strong>{title(node)}</strong><time>{date(node.createdAt)}</time>{/if}
              {#if node.base}<span class="flow-side-foot">{tr("base")} <code title={node.base.id}>{short(node.base.id)}</code><span title={node.base.summary}>{node.base.summary}</span></span>
              {:else if node.command}<code class="flow-command" title={node.command}>{node.command}</code>{/if}
            </div>
            <div class="flow-side flow-right">
              {#if node.source}{@render commitCard(node.source, node.stash ? tr("localChanges") : tr("source"))}
              {:else}<small class="flow-caption">{tr("recordResult")}</small><strong>{label(node.state)}</strong><span>{future(node) ? tr("notExecuted") : date(node.at)}</span>{/if}
              {#if node.stash}<span class="flow-side-foot">{tr("stash")} <code title={node.stash}>{short(node.stash)}</code></span>
              {:else if node.files.length}<span class="flow-side-foot">{tr("files", { n: String(node.files.length) })}<code title={node.files[0].path}>{node.files[0].path}</code></span>{/if}
            </div>
            <span class="flow-junction" aria-hidden="true">{stateSymbol(node.state)}</span>
            <section class="flow-node" class:active={summary?.activeStep === node.key} class:expanded={expanded[node.key]}>
              <div class="flow-node-heading"><span class="flow-step-number">{row.index + 1} / {graph.length}</span><h2>{title(node)}</h2>{#if summary?.activeStep === node.key}<span class="flow-current">{tr("currentStep")}</span>{/if}<span class="flow-badge" data-state={node.state}>{stateSymbol(node.state)} {label(node.state)}</span><time>{future(node) ? tr("notExecuted") : date(node.at)}</time></div>
              <p class="flow-node-note" title={label(node.detail)}>{label(node.detail) || (future(node) ? tr("preview-conditional") : tr("recorded"))}</p>
              <details class="flow-evidence" open={expanded[node.key] ?? false} ontoggle={(event) => expanded[node.key] = event.currentTarget.open}>
                <summary>{node.files.length ? tr("files", { n: String(node.files.length) }) : tr("details")}{#if node.repeatOf}<span class="flow-repeat">↻ {tr("repeatedRound", { n: String(nodes.find((item) => item.key === node.repeatOf)?.round ?? "") })}</span>{/if}</summary>
                <p>{tr("time")}: {date(node.createdAt)} → {future(node) ? tr("notExecuted") : date(node.at)}</p>
                {#if node.command}<p>{tr("command")}</p><pre>{node.command}</pre>{/if}
                {#if node.base}<p>{tr("base")}: <code>{node.base.id}</code> · {node.base.summary}</p>{/if}
                {#if node.stash}<p>{tr("stash")}: <code>{node.stash}</code></p>{/if}
                {#if node.detail}<p>{label(node.detail)}</p>{/if}
                {#if node.files.length > 100}<label class="flow-file-page">{tr("filePage")}<select aria-label={tr("filePage")} value={filePages[node.key] ?? 0} onchange={(event) => filePages[node.key] = Number(event.currentTarget.value)}>{#each Array.from({ length: Math.ceil(node.files.length / 100) }, (_, page) => page) as page}<option value={page}>{page + 1}</option>{/each}</select></label>{/if}
                {#each node.files.slice((filePages[node.key] ?? 0) * 100, ((filePages[node.key] ?? 0) + 1) * 100) as file (file.path)}
                  <div class="flow-file" data-resolved={file.resolved}><code>{file.path}</code><small>{future(node) ? tr("predicted") : ["skipped", "aborted"].includes(node.state) ? label(node.state) : file.resolved ? `✓ ${tr("resolved")}` : node.kind === "conflict" ? `! ${tr("unresolved")}` : ""}</small>
                    {#if node.kind === "conflict"}{#each [["stage1", file.base], ["stage2", file.ours], ["stage3", file.theirs]] as pair}<span>{tr(pair[0])}: <code>{pair[1] || tr("missingBlob")}</code></span>{/each}{/if}
                  </div>
                {/each}
              </details>
            </section>
          </article>
        {/each}
      </div>
    </div>
  </section>
  <footer class="flow-footer" aria-live="polite"><span>{live && !retainOpen && summary?.canAutoClose ? tr("closing") : !live || retainOpen ? tr("retained") : tr("note")}</span>{#if summary?.pendingStash}<span title={summary.pendingStash}>{tr("pending")} <code>{short(summary.pendingStash)}</code></span>{/if}</footer>
</main>
