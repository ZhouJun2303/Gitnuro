<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "./perf";
  import { translate, resolveLocale, type Locale } from "./i18n";
  import { broadcastLocale, onLocaleChange } from "./i18n/sync";
  import { isAppearance, onAppearanceChange, resolveAppearance } from "./appearance";
  import { shortcut } from "./keys";
  import { buildFlowPipeline, isFutureStep, stateSymbol, type FlowCommit, type FlowDelta, type FlowNode, type FlowSnapshot, type FlowSummary } from "./operation-flow";
  import type { Settings } from "./status";
  import "./FlowWindow.css";

  const ROW = 156;
  const EXPANDED_ROW = ROW + 280;
  let repo = $state("");
  let selectedId = $state("");
  let live = $state(false);
  let autoClose = $state(false);
  let retainOpen = $state(false);
  let closed = $state(false);
  let summary = $state.raw<FlowSummary | null>(null);
  let nodes = $state.raw<FlowNode[]>([]);
  let history = $state.raw<FlowSummary[]>([]);
  let filePages = $state<Record<string, number>>({});
  let expanded = $state<Record<string, boolean>>({});
  let locale = $state<Locale>("zh");
  let appearance = $state("apple");
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
  const title = (node: FlowNode) => node.kind === "finished" && node.state !== "completed" ? tr("phase-finished") : node.kind === "conflict" ? tr("round", { n: String(node.round) })
    : label(({ "local-saved": "saved", "local-restore": "restore" } as Record<string, string>)[node.kind] ?? node.kind);
  const short = (id: string) => id.slice(0, 10);
  const date = (at: number) => new Date(at).toLocaleString(locale === "zh" ? "zh-CN" : "en-US", { hour12: false });
  const future = isFutureStep;
  const pipeline = $derived(buildFlowPipeline(summary, nodes));
  const graph = $derived(pipeline.steps);
  const activeKey = $derived(pipeline.activeKey);
  const layout = $derived.by(() => {
    let top = 0;
    let heldStash = "";
    const rows = graph.map((node, index) => {
      const height = expanded[node.key] ? EXPANDED_ROW : ROW;
      const saved = node.phase === "saved" && node.stash && ["completed", "processed"].includes(node.state);
      if (saved) heldStash = node.stash;
      const restoring = node.phase === "restore" && heldStash === node.stash && !["failed", "skipped", "aborted", "superseded"].includes(node.state);
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
  const completed = $derived(graph.reduce((done, node) => done + Number(node.state === "completed" || node.state === "processed"), 0));
  const current = $derived(graph.find((node) => node.key === activeKey));
  const currentIndex = $derived(graph.findIndex((node) => node.key === activeKey));
  const shouldAutoClose = $derived(autoClose && live && !retainOpen && summary?.canAutoClose && summary.state === "completed");

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
    const rowHeight = expanded[key] ? EXPANDED_ROW : ROW;
    viewport.scrollTo({ top: Math.max(0, top - Math.max(12, (viewport.clientHeight - rowHeight) / 2)), behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth" });
  }

  function scrolled() {
    if (scrollFrame) return;
    scrollFrame = requestAnimationFrame(() => { scrollFrame = 0; scrollTop = viewport.scrollTop; });
  }

  $effect(() => {
    const active = summary ? `${summary.id}:${activeKey}` : "";
    if (active && ready && active !== lastActive) { lastActive = active; untrack(() => void locate(activeKey)); }
  });

  $effect(() => {
    document.documentElement.lang = locale === "zh" ? "zh-CN" : "en";
  });

  $effect(() => {
    if (!ready || !shouldAutoClose || !summary) return;
    const id = summary.id, sequence = summary.sequence;
    const timer = setTimeout(async () => {
      try {
        const settings = await invoke<Settings>("load_settings");
        if (!settings.flowAutoClose) { autoClose = false; return; }
        const value = fixture ? { summary } : await invoke<FlowSnapshot | null>("operation_flow", { path: repo, id: null });
        if (disposed || !autoClose || !live || retainOpen || summary?.id !== id || summary.sequence !== sequence) return;
        if (value?.summary && value.summary.id !== id) { await navigate(value.summary.id, true); return; }
        if (!value?.summary || value.summary.sequence !== sequence || !value.summary.canAutoClose) { await refresh(); return; }
        if (desktop) await getCurrentWindow().close();
        else { closed = true; window.dispatchEvent(new CustomEvent("flow-auto-closed", { detail: id })); }
      } catch (cause) { error = String(cause); }
    }, 1500);
    return () => clearTimeout(timer);
  });

  onMount(() => {
    const stopLocale = onLocaleChange((setting) => { locale = resolveLocale(setting); });
    const media = matchMedia("(prefers-color-scheme: dark)");
    const paint = () => {
      const theme = resolveAppearance(appearance, media.matches);
      document.documentElement.dataset.theme = theme;
      if (desktop) void getCurrentWindow().setTheme(theme === "dark" ? "dark" : "light").catch(() => {});
    };
    const stopAppearance = onAppearanceChange((setting) => { appearance = setting; paint(); });
    media.addEventListener("change", paint);
    const onLanguageShortcut = (event: KeyboardEvent) => {
      if (event.defaultPrevented || shortcut(event) !== "switchLanguage") return;
      event.preventDefault();
      if (event.repeat || event.isComposing) return;
      locale = locale === "zh" ? "en" : "zh";
      const next = locale;
      broadcastLocale(next);
      void invoke<Settings>("load_settings")
        .then((settings) => invoke("save_settings", { values: { ...settings, locale: next } }))
        .catch((cause) => { if (!disposed) error = String(cause); });
    };
    window.addEventListener("keydown", onLanguageShortcut);
    const params = new URLSearchParams(window.location.search);
    scene = params.get("scene") ?? ""; repo = params.get("repo") ?? "";
    selectedId = params.get("rev") ?? ""; live = params.get("file") === "live";
    const unlisten: UnlistenFn[] = [];
    const boot = async () => {
      try {
        const settings = await invoke<Settings>("load_settings");
        autoClose = settings.flowAutoClose === true;
        locale = scene ? params.get("locale") === "en" ? "en" : "zh" : resolveLocale(settings.locale);
        appearance = scene && isAppearance(params.get("theme")) ? params.get("theme")! : settings.theme;
        paint();
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
    const refreshAutoClose = () => {
      void invoke<Settings>("load_settings").then((settings) => {
        if (!disposed) autoClose = settings.flowAutoClose === true;
      }).catch((cause) => { if (!disposed) error = String(cause); });
    };
    window.addEventListener("focus", refreshAutoClose);
    const poll = setInterval(() => { if (!document.hidden) void refresh(); }, 1000);
    const historyPoll = setInterval(() => { if (!document.hidden) void loadHistory(); }, 10_000);
    return () => { disposed = true; generation++; stopLocale(); stopAppearance(); media.removeEventListener("change", paint); window.removeEventListener("keydown", onLanguageShortcut); clearInterval(poll); clearInterval(historyPoll); cancelAnimationFrame(scrollFrame); window.removeEventListener("focus", refreshAutoClose); unlisten.forEach((off) => off()); };
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
  <div class="flow-meta"><span>{tr("observerHint")}</span><span>{tr("progress", { done: String(completed), total: String(graph.length) })}</span></div>
  {#if error}<div class="flow-error" role="alert">{error}</div>{/if}
  {#if summary?.recordingError}<div class="flow-error" role="alert">{tr("recording", { error: summary.recordingError })}</div>{/if}
  <section class="flow-graph-pane" aria-label={tr("title")}>
    {#if summary}
      <nav class="flow-overview" aria-label={tr("overview")}>
        <div class="flow-overview-heading"><strong>{label(summary.kind)} · {tr("overview")}</strong><span>{tr("pipelineHint")}</span></div>
        <ol>{#each pipeline.phases as phase}<li data-phase={phase.phase} data-state={phase.state} class:active={phase.active}>
          <a href={`#${phase.key}`} aria-current={phase.active ? "step" : undefined} onclick={(event) => { event.preventDefault(); void locate(phase.key); }}><span class="flow-phase-symbol">{stateSymbol(phase.state)}</span><span><strong>{phase.phase === "operation" ? label(summary.kind) : tr(`phase-${phase.phase}`)}</strong><small>{label(phase.state)}</small></span></a>
        </li>{/each}</ol>
      </nav>
    {/if}
    <div class="flow-legend"><span class="flow-legend-key" data-state="completed">✓ {tr("completed")}</span><span class="flow-legend-key" data-state="current">◉ {tr("currentStep")}</span><span class="flow-legend-key" data-state="planned">○ {tr("planned")}</span><span class="flow-legend-key" data-state="stash">↗ {tr("stashLane")}</span><span class="flow-follow" aria-live="polite">{current ? `${tr("currentStep")} · ${currentIndex + 1} / ${graph.length} · ${title(current)}` : summary?.endedAt != null ? label(summary.state) : tr("scope")}</span></div>
    <div class="flow-scroll" bind:this={viewport} bind:clientHeight={height} onscroll={scrolled}>
      {#if !ready}<p class="flow-empty">{tr("loading")}</p>{:else if !summary}<p class="flow-empty">{tr("empty")}</p>{/if}
      <div class="flow-track" style:height={`${layout.height}px`}>
        {#each visible as row (row.node.key)}
          {@const node = row.node}
          <article id={node.key} class="flow-row" class:active={activeKey === node.key} class:past={row.index < currentIndex} class:has-stash={!!row.stashRoute} style:top={`${row.top}px`} style:height={`${row.height}px`} data-state={node.state} data-phase={node.phase} data-node={node.key} data-stash-route={row.stashRoute || undefined} data-stash={row.stash || undefined} aria-current={activeKey === node.key ? "step" : undefined} aria-label={`${row.index + 1} / ${graph.length} · ${title(node)} · ${label(node.state)}`}>
            <svg class="flow-roads" viewBox={`0 0 1000 ${row.height}`} preserveAspectRatio="none" aria-hidden="true">
              <path class="flow-axis" d={`M100 0V${row.height}`} />
              {#if row.stashRoute === "out"}
                <path class="flow-stash-road" d={`M100 30 C100 64 32 64 32 96 V${row.height}`} />
              {:else if row.stashRoute === "in"}
                <path class="flow-stash-road" class:predicted={future(node)} d="M32 0 V8 C32 30 78 30 100 30" />
              {:else if row.stashRoute === "held"}
                <path class="flow-stash-road" d={`M32 0V${row.height}`} />
              {/if}
              {#if row.stashRoute}<path class="flow-stash-direction" class:predicted={row.stashRoute === "in" && future(node)} d={row.stashRoute === "in" ? "m90 24 10 6 -10 6" : `M27 ${row.height - 8}l5 6 5-6`} />{/if}
              <path class="flow-direction" d={`M95 ${row.height - 8}l5 6 5-6`} />
            </svg>
            {#if row.stashRoute}<span class="flow-stash-label" title={row.stash}>{tr(row.stashRoute === "out" ? "stashDiverge" : row.stashRoute === "in" ? "stashRejoin" : "stashHeld")}</span>{/if}
            <span class="flow-junction" aria-hidden="true">{stateSymbol(node.state)}</span>
            <section class="flow-node" class:active={activeKey === node.key} class:expanded={expanded[node.key]}>
              <div class="flow-node-heading"><span class="flow-step-number">{row.index + 1} / {graph.length}</span><h2>{title(node)}</h2>{#if activeKey === node.key}<span class="flow-current">{tr("currentStep")}</span>{/if}<span class="flow-badge" data-state={node.state}>{stateSymbol(node.state)} {future(node) && node.state !== "not-run" ? tr("planned") : label(node.state)}</span><time>{future(node) ? tr("notExecuted") : date(node.at)}</time></div>
              {#if node.source || node.target}<div class="flow-commits">
                {#if node.target}<div class="flow-side flow-left">{@render commitCard(node.target, tr("target"))}</div>{/if}
                {#if node.source}<div class="flow-side flow-right">{@render commitCard(node.source, node.stash ? tr("localChanges") : tr("source"))}</div>{/if}
              </div>{/if}
              <p class="flow-node-note" title={label(node.detail)}>{label(node.detail) || (future(node) ? tr("preview-conditional") : tr("recorded"))}</p>
              <details class="flow-evidence" open={expanded[node.key] ?? false} ontoggle={(event) => expanded[node.key] = event.currentTarget.open}>
                <summary>{node.files.length ? tr("files", { n: String(node.files.length) }) : tr("details")}{#if node.repeatOf}<span class="flow-repeat">↻ {tr("repeatedRound", { n: String(nodes.find((item) => item.key === node.repeatOf)?.round ?? "") })}</span>{/if}</summary>
                <p>{tr("time")}: {date(node.createdAt)} → {future(node) ? tr("notExecuted") : date(node.at)}</p>
                {#if node.command}<p>{tr("command")}</p><pre>{node.command}</pre>{/if}
                {#if node.base}<p>{tr("base")}: <code>{node.base.id}</code> · {node.base.summary}</p>{/if}
                {#if node.stash}<p>{tr("stash")}: <code>{node.stash}</code></p>{/if}
                {#if node.detail}<p>{label(node.detail)}</p>{/if}
                {#if node.evidence.length > 1}<div class="flow-journal"><p>{tr("executionDetails")}</p>{#each node.evidence as entry (entry.key)}{#if entry.key !== node.key}<div><time>{date(entry.at)}</time><strong>{label(entry.kind)}</strong><span>{label(entry.state)}</span>{#if entry.command}<pre>{entry.command}</pre>{/if}{#if entry.detail}<pre>{label(entry.detail)}</pre>{/if}</div>{/if}{/each}</div>{/if}
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
  <footer class="flow-footer" aria-live="polite"><span>{shouldAutoClose ? tr("closing") : !live || retainOpen ? tr("retained") : summary?.canAutoClose ? tr("staysOpen") : tr("note")}</span>{#if summary?.pendingStash}<span title={summary.pendingStash}>{tr("pending")} <code>{short(summary.pendingStash)}</code></span>{/if}</footer>
</main>
