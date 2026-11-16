<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { emit } from "@tauri-apps/api/event";
  import { invoke } from "../perf";
  import { getShell } from "./context";
  import BinaryDiff from "./BinaryDiff.svelte";
  import WrappedRows from "../WrappedRows.svelte";
  import type { DiffLine } from "../status";
  import { onVirtualScroll, windowSlice, gravatarUrl, splitDiff, virtualSplit, type SplitRow } from "../view";

  const shell = getShell();
  const rowHeight = 56;
  const splitScroller = (node: HTMLElement, columns?: { left: number; right: number }) => shell.splitScroller(node, columns);
  let diffHeight = $state(0);
  const diffRow = $derived(shell.diffRowHeight);
  const splitRows = $derived(shell.settings.diffStyle === "split" && shell.historyDiff?.lines.length ? splitDiff(shell.historyDiff.lines) : []);
  const splitColumns = $derived(shell.splitColumnsOf(splitRows));
  const splitWindow = $derived(virtualSplit(splitRows, shell.historyDiffTop, shell.splitRowHeight, diffHeight, shell.settings.diffWordWrap));
  let listTop = $state(0);
  let copySupport = $state<{ tool: string; kind: string } | null>(null);
  let copySupportError = $state("");
  type PreviewCopy = { repo: string; file: string; path: string; tool: string };
  let previews = $state<Record<string, PreviewCopy>>({});
  let autoClean = $state(true);
  let busy = $state(false);
  let closing = $state(false);
  let pendingClose = $state<PreviewCopy[]>([]);
  let allowClose = false;
  let pendingAction: Promise<void> | null = null;
  let copiedPath = $state("");
  const onListScroll = onVirtualScroll((top) => (listTop = top));
  const onDiffScroll = onVirtualScroll((top) => (shell.historyDiffTop = top));
  const windowed = $derived(windowSlice(shell.commitsLive, listTop, rowHeight));
  const span = $derived.by(() => {
    let min = 0;
    let max = 0;
    for (const commit of shell.commitsLive) {
      if (!commit.at) continue;
      if (!min || commit.at < min) min = commit.at;
      if (commit.at > max) max = commit.at;
    }
    if (!min) return null;
    return { min, max: Math.max(max, min + 1) };
  });
  const years = $derived.by(() => {
    if (!span) return [];
    const start = new Date(span.min * 1000).getFullYear();
    const end = new Date(span.max * 1000).getFullYear();
    const list: number[] = [];
    for (let year = start; year <= end; year += 1) list.push(year);
    return list;
  });
  const timelineCommits = $derived.by(() => {
    const commits = shell.commitsLive.filter((commit) => commit.at);
    const limit = 600;
    if (commits.length <= limit) return commits;
    const step = Math.ceil(commits.length / limit);
    const sampled = commits.filter((_, index) => index % step === 0);
    const selected = commits.find((commit) => commit.id === shell.selectedCommit);
    if (selected && !sampled.includes(selected)) sampled.push(selected);
    return sampled;
  });

  const place = (at: number) => (span ? ((at - span.min) / (span.max - span.min)) * 100 : 0);

  async function choose(id: string) {
    await shell.selectCommit(id);
    if (shell.historyFilter) await shell.pickCommitFile(shell.historyFilter);
  }

  $effect(() => {
    const file = shell.historyFilter;
    const path = shell.repoPath();
    let cancelled = false;
    copySupport = null;
    copySupportError = "";
    copiedPath = "";
    if (shell.mode === "live" && file) {
      void invoke<{ tool: string; kind: string } | null>("revision_copy_support", { path, file })
        .then((value) => { if (!cancelled) copySupport = value; })
        .catch((error) => { if (!cancelled) copySupportError = shell.message(error); });
    }
    return () => { cancelled = true; };
  });

  const previewKey = (id: string) => `${shell.repoPath()}\0${shell.historyFilter}\0${id}`;
  const previewName = (path: string) => path.split(/[\\/]/).pop() ?? path;
  function copyName(path: string, tool: string) {
    const name = previewName(path);
    if (tool !== "unity") return name;
    const script = name.match(/\.(?:cs|asmdef)\.txt$/i);
    if (script) return name.slice(0, -script[0].length);
    const dot = name.lastIndexOf(".");
    return dot > 0 ? name.slice(0, dot) : name;
  }

  function runAction(body: () => Promise<void>) {
    if (pendingAction || closing) return;
    busy = true;
    pendingAction = body().catch((error) => { copySupportError = shell.message(error); }).finally(() => {
      busy = false;
      pendingAction = null;
    });
  }

  function createCopy(id: string) {
    const repo = shell.repoPath();
    const file = shell.historyFilter;
    if (!copySupport || !repo || !file) return;
    const tool = copySupport.tool;
    const key = previewKey(id);
    runAction(async () => {
      copySupportError = "";
      copiedPath = "";
      const path = await invoke<string>("create_revision_copy", { path: repo, rev: id, file });
      previews = { ...previews, [key]: { repo, file, path, tool } };
      shell.copyText(copyName(path, tool));
      if (repo === shell.repoPath() && file === shell.historyFilter) copiedPath = path;
    });
  }

  function cancelPreview(key: string) {
    const preview = previews[key];
    if (!preview) return;
    runAction(async () => {
      copySupportError = "";
      await invoke("remove_revision_copy", { path: preview.repo, file: preview.file, copy: preview.path });
      const remaining = { ...previews };
      delete remaining[key];
      previews = remaining;
      if (copiedPath === preview.path) copiedPath = "";
    });
  }

  $effect(() => {
    const file = shell.historyFilter;
    if (!file || !shell.inApp()) return;
    void getCurrentWindow().setTitle(`${shell.tr("menu.history")} — ${file}`).catch(() => {});
  });

  async function closeWindow() {
    if (shell.inApp()) {
      allowClose = true;
      await getCurrentWindow().close();
    }
    else window.location.href = "/";
  }

  async function closePane() {
    if (closing || pendingClose.length) return;
    closing = true;
    try {
      await pendingAction;
      if (autoClean) {
        const modified: PreviewCopy[] = [];
        for (const [key, preview] of Object.entries(previews)) {
          try {
            await invoke("remove_revision_copy", { path: preview.repo, file: preview.file, copy: preview.path });
            const remaining = { ...previews };
            delete remaining[key];
            previews = remaining;
          } catch {
            modified.push(preview);
          }
        }
        if (modified.length) {
          pendingClose = modified;
          closing = false;
          return;
        }
      }
      await closeWindow();
    } catch (error) {
      copySupportError = shell.message(error);
      allowClose = false;
      closing = false;
    }
  }

  async function resolveClose(action: "keep" | "remove") {
    const targets = pendingClose;
    pendingClose = [];
    closing = true;
    try {
      if (action === "remove") {
        const kept: string[] = [];
        for (const preview of targets) {
          try {
            await invoke("remove_revision_copy", { path: preview.repo, file: preview.file, copy: preview.path, force: true });
            const remaining = { ...previews };
            for (const [key, item] of Object.entries(remaining)) if (item.path === preview.path) delete remaining[key];
            previews = remaining;
            if (copiedPath === preview.path) copiedPath = "";
          } catch {
            kept.push(previewName(preview.path));
          }
        }
        if (kept.length) await emit("awegit-preview-kept", { files: kept }).catch(() => {});
      }
      await closeWindow();
    } catch (error) {
      copySupportError = shell.message(error);
      allowClose = false;
      closing = false;
    }
  }

  onMount(() => {
    let disposed = false;
    let off: (() => void) | undefined;
    if (shell.inApp()) void getCurrentWindow().onCloseRequested((event) => {
      if (allowClose || (!closing && !pendingAction && (!autoClean || Object.keys(previews).length === 0))) return;
      event.preventDefault();
      void closePane();
    }).then((value) => { if (disposed) value(); else off = value; });
    return () => { disposed = true; off?.(); };
  });
</script>

<div class="file-history">
  <header class="history-file">
    <strong>{shell.historyFilter || shell.tr("menu.history")}</strong>
    {#if copySupportError}<span class="copy-error" role="alert" title={copySupportError}>{copySupportError}</span>{/if}
    {#if copiedPath}<span class="copy-result" role="status" title={copiedPath}>{shell.tr("chrome.copyCreated")}: {copiedPath}</span>{/if}
    <label class="auto-clean"><input type="checkbox" bind:checked={autoClean} disabled={closing} />{shell.tr("chrome.cleanPreviewOnClose")}</label>
    <button class="text-button" type="button" disabled={closing} aria-label={shell.tr("chrome.close")} onclick={() => void closePane()}>{shell.tr("chrome.close")}</button>
  </header>
  {#if span}
    <div class="history-time">
      {#each years as year (year)}
        {@const at = span ? Math.min(span.max, Math.max(span.min, new Date(year, 0, 1).getTime() / 1000)) : 0}
        {@const pos = place(at)}
        <span style:left="{pos}%" style:transform={pos < 4 ? "none" : pos > 96 ? "translateX(-100%)" : undefined}>{year}</span>
      {/each}
      {#each timelineCommits as commit (commit.id)}
        {#if commit.at}
          {@const pos = Math.max(1, Math.min(99, place(commit.at)))}
          <button type="button" class:on={shell.selectedCommit === commit.id} style:left="{pos}%" aria-label={`${commit.summary} · ${shell.whenLabel(commit)}`} title={commit.summary} onclick={() => void choose(commit.id)}></button>
        {/if}
      {/each}
    </div>
  {/if}
  <div class="history-split">
    <section class="history-list" onscroll={onListScroll}>
      {#if shell.commitsLive.length === 0}
        <p class="empty">{shell.tr("chrome.noCommits")}</p>
      {:else}
        <div class="commit-window" style:height="{shell.commitsLive.length * rowHeight}px">
          {#each windowed.rows as commit, index (commit.id)}
            {@const key = previewKey(commit.id)}
            <div class="history-item" class:selected={shell.selectedCommit === commit.id} class:has-preview={!!previews[key]} data-rev={commit.id} style:top="{(windowed.start + index) * rowHeight}px">
              <button
                class="history-commit"
                type="button"
                onclick={() => void choose(commit.id)}
                oncontextmenu={(event) => shell.openMenu(event, shell.historyFileMenu(commit))}
              >
                <span class="history-meta">
                  <span class="commit-author">
                    {#if shell.settings.gravatar && commit.email}
                      <img class="avatar" alt="" src={gravatarUrl(commit.email)} />
                    {:else}
                      <span class="avatar" style:background={shell.avatarColor(commit.author)} title={commit.author}>{(commit.author || "?").slice(0, 1).toUpperCase()}</span>
                    {/if}
                    <span class="author" title={commit.author}>{commit.author}</span>
                  </span>
                  <span class="commit-facts">
                    <span class="date" title={commit.at ? shell.forkWhenFull(commit.at) : commit.when}>{shell.whenLabel(commit)}</span>
                    <span class="sha" title={commit.shortId}>{commit.shortId}</span>
                  </span>
                </span>
                <span class="subject" title={commit.summary}>{commit.summary}</span>
              </button>
              {#if previews[key]}
                <div class="preview-actions">
                  <button class="revision-action" type="button" disabled={busy || closing} aria-label={shell.tr("chrome.copyPreviewName")} title={shell.tr("chrome.copyPreviewName")} onclick={() => shell.copyText(copyName(previews[key].path, previews[key].tool))}>
                    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" aria-hidden="true"><rect x="5" y="5" width="9" height="9" rx="1"/><path d="M3 11H2V2h9v1"/></svg>
                  </button>
                  <button class="revision-action" type="button" disabled={busy || closing} aria-label={shell.tr("chrome.cancelPreview")} title={shell.tr("chrome.cancelPreview")} onclick={() => cancelPreview(key)}>
                    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true"><path d="m4 4 8 8M12 4l-8 8"/></svg>
                  </button>
                </div>
              {:else if copySupport}
                <button class="revision-action preview-revision" type="button" disabled={busy || closing} aria-label={`${shell.tr("chrome.createTempCopy")} (${copySupport.tool}, ${copySupport.kind})`} title={`${shell.tr("chrome.createTempCopy")} (${copySupport.tool}, ${copySupport.kind})`} onclick={() => createCopy(commit.id)}>
                  <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" aria-hidden="true"><path d="M2 8s2.3-4 6-4 6 4 6 4-2.3 4-6 4-6-4-6-4Z"/><circle cx="8" cy="8" r="2"/></svg>
                </button>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </section>
    <section
      class="history-diff"
      role="group"
      bind:clientHeight={diffHeight}
      onscroll={onDiffScroll}
      oncontextmenu={(event) => {
        const commit = shell.commitsLive.find((item) => item.id === shell.selectedCommit);
        if (commit) shell.openMenu(event, shell.historyFileMenu(commit));
      }}
    >
      {#if !shell.historyDiff}
        <p class="diff-empty">{shell.selectedCommit ? shell.tr("chrome.reading") : shell.tr("chrome.noCommits")}</p>
      {:else if shell.historyDiff.binary}
        <BinaryDiff path={shell.historyFile ?? ""} oldBytes={shell.historyDiff.oldBytes} newBytes={shell.historyDiff.newBytes} compact />
      {:else if shell.historyDiff.lines.length === 0}
        <p class="diff-empty">{shell.tr("chrome.noChanges")}</p>
      {:else if shell.settings.diffStyle === "split"}
        <div class="diff-body split" class:virtual={!shell.settings.diffWordWrap} style:--split-row="{shell.splitRowHeight}px" role="presentation" use:splitScroller={splitColumns}>
          {#if shell.settings.diffWordWrap}
            <WrappedRows items={splitRows} rowHeight={diffRow} split>
              {#snippet children(row)}{@render historySplitRow(row)}{/snippet}
            </WrappedRows>
          {:else}
          {#if splitWindow.beforePx > 0}<div class="split-gap" style:height="{splitWindow.beforePx}px"></div>{/if}
          {#each splitWindow.rows as row, offset (splitWindow.start + offset)}
            {@render historySplitRow(row)}
          {/each}
          {#if splitWindow.afterPx > 0}<div class="split-gap" style:height="{splitWindow.afterPx}px"></div>{/if}
          {/if}
          {@render splitBar()}
        </div>
      {:else}
        <div class="diff-body">
          {#if shell.settings.diffWordWrap}
            <WrappedRows items={shell.historyDiff.lines} rowHeight={diffRow}>
              {#snippet children(line, at)}{@render historyUnifiedRow(line, at)}{/snippet}
            </WrappedRows>
          {:else}
          <div class="commit-window" style:height="{shell.historyDiffWindow.height}px">
            {#each shell.historyDiffWindow.rows as line, index (`h-${shell.historyDiffWindow.start + index}`)}
              {@const at = shell.historyDiffWindow.start + index}
              {@render historyUnifiedRow(line, at)}
            {/each}
          </div>
          {/if}
        </div>
      {/if}
    </section>
  </div>
  {#if pendingClose.length}
    <div class="preview-confirm" role="dialog" aria-modal="true" aria-label={shell.tr("chrome.previewModifiedTitle")}>
      <div class="preview-confirm-card">
        <h3>{shell.tr("chrome.previewModifiedTitle")}</h3>
        <p>{shell.tr("chrome.previewModifiedBody")}</p>
        <ul>
          {#each pendingClose as preview (preview.path)}
            <li title={preview.path}>{previewName(preview.path)}</li>
          {/each}
        </ul>
        <div class="preview-confirm-actions">
          <button type="button" onclick={() => void resolveClose("keep")}>{shell.tr("chrome.previewKeep")}</button>
          <button type="button" class="danger" onclick={() => void resolveClose("remove")}>{shell.tr("chrome.previewForceDelete")}</button>
        </div>
      </div>
    </div>
  {/if}
</div>

{#snippet historySplitRow(row: SplitRow)}
  {#if row.meta}
    <div class="split-meta">{row.left}</div>
  {:else}
    <div class="split-row">{@render splitCell(row, "left")}{@render splitCell(row, "right")}</div>
  {/if}
{/snippet}

{#snippet historyUnifiedRow(line: DiffLine, at: number)}
  <span class="virtual-row {line.kind}" class:add={line.kind === "add"} class:del={line.kind === "delete"} class:hunk={line.kind === "hunk"} class:meta={line.kind === "meta"} style:top="{at * diffRow}px" style:height={shell.settings.diffWordWrap ? "auto" : `${diffRow}px`} style:line-height={shell.settings.diffWordWrap ? "1.45" : `${diffRow}px`}><span class="diff-line-text">{line.text}</span></span>
{/snippet}

{#snippet splitCell(row: SplitRow, side: "left" | "right")}
  {@const kind = side === "left" ? row.leftKind : row.rightKind}
  {@const mark = side === "left" ? "delete" : "add"}
  {@const text = side === "left" ? row.left : row.right}
  {@const other = side === "left" ? row.right : row.left}
  {@const paired = Boolean(row.left && row.right && row.left !== row.right && (row.leftKind === "delete" || row.rightKind === "add"))}
  <span class="split-no" class:del={kind === "delete"} class:add={kind === "add"} class:meta={kind === "meta"}>{(side === "left" ? row.leftNo : row.rightNo) ?? ""}</span>
  <span class="split-code" data-side={side} class:del={kind === "delete"} class:add={kind === "add"} class:meta={kind === "meta"}><span class="split-text">{#if shell.settings.showDiffMarks && kind === mark}{side === "left" ? "-" : "+"}{/if}{#each shell.markedLine(text, paired ? other : "", shell.historyDiff?.path ?? "") as token, tokenIndex (`${side}-${tokenIndex}`)}<span class={token.cls} class:intra={token.hot}>{token.text}</span>{/each}</span></span>
{/snippet}

{#snippet splitBar()}
  <span class="split-bar-pad"></span>
  <div class="split-bar" data-bar="left"><div></div></div>
  <span class="split-bar-pad"></span>
  <div class="split-bar" data-bar="right"><div></div></div>
{/snippet}

<style>
  .file-history {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--canvas);
    color: var(--text);
  }

  .history-file {
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 0 10px 0 12px;
    border-bottom: 1px solid var(--line);
    flex: none;
    font-size: 12px;
  }

  .history-file strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  .history-file :global(.text-button) {
    margin-left: 0;
    flex: none;
  }

  .auto-clean { display: inline-flex; align-items: center; gap: 4px; flex: none; white-space: nowrap; color: var(--text-secondary); cursor: pointer; }
  .auto-clean input { margin: 0; accent-color: var(--accent); }

  .copy-result, .copy-error { min-width: 0; margin-left: auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 400; }
  .copy-result { color: var(--text-secondary); }
  .copy-error { color: var(--deleted); }

  .history-time {
    position: relative;
    height: 32px;
    border-bottom: 1px solid var(--line);
    flex: none;
    overflow: hidden;
  }

  .history-time span {
    position: absolute;
    top: 2px;
    font-size: 10px;
    color: var(--text-secondary);
    transform: translateX(-50%);
    white-space: nowrap;
  }

  .history-time button {
    position: absolute;
    top: 18px;
    width: 7px;
    height: 7px;
    margin-left: -3px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--accent);
  }

  .history-time button.on { outline: 2px solid var(--text); }

  .history-split {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .history-list {
    width: 360px;
    flex: none;
    overflow: auto;
    border-right: 1px solid var(--line);
  }

  .history-item {
    position: absolute;
    left: 0;
    right: 0;
    height: 56px;
  }

  .history-commit {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 4px;
    padding: 0 38px 0 28px;
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    font: inherit;
    transition: background 120ms ease;
  }

  .history-commit::before {
    content: "";
    position: absolute;
    left: 11px;
    top: 0;
    bottom: 0;
    width: 2px;
    background: color-mix(in srgb, var(--accent) 30%, transparent);
    pointer-events: none;
  }

  .history-commit::after {
    content: "";
    position: absolute;
    left: 8px;
    top: 50%;
    width: 8px;
    height: 8px;
    margin-top: -4px;
    border-radius: 50%;
    background: var(--accent);
    pointer-events: none;
  }

  .history-commit:hover { background: var(--hover); }

  .history-item.has-preview .history-commit { padding-right: 70px; }

  .history-item.selected .history-commit,
  .history-item.selected .history-commit:hover { background: var(--selection); }

  .preview-revision, .preview-actions { position: absolute; right: 7px; top: 27px; }
  .preview-actions { display: flex; gap: 3px; }
  .revision-action { width: 26px; height: 26px; display: grid; place-items: center; padding: 0; border: 1px solid var(--line); border-radius: 4px; background: var(--field); color: var(--text); cursor: pointer; }
  .revision-action:hover, .revision-action:focus-visible { background: var(--hover); color: var(--text); }
  .revision-action:disabled { opacity: .45; cursor: default; }
  .revision-action svg { width: 16px; height: 16px; }

  .history-meta {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-width: 0;
    color: var(--text-secondary);
    font-size: 12px;
  }

  .commit-author {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    flex: 0 1 auto;
  }

  .commit-author .avatar {
    width: 18px;
    height: 18px;
    flex: none;
    border-radius: 4px;
    background: var(--field);
    font-size: 10px;
    line-height: 18px;
    text-align: center;
    color: white;
  }

  .commit-author .author {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
    color: var(--text);
  }

  .commit-facts {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 10px;
    flex: none;
    min-width: 0;
  }

  .commit-facts .date,
  .commit-facts .sha {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .commit-facts .date { width: 116px; flex: none; text-align: right; }

  .commit-facts .sha {
    width: 56px;
    flex: none;
    text-align: right;
    font-family: var(--mono);
    font-size: 11px;
  }

  .history-commit .subject {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
  }

  .history-diff {
    flex: 1;
    min-width: 0;
    overflow: auto;
  }

  .history-diff .diff-body:not(.split) { overflow: visible; height: auto; }

  .history-diff .diff-body.split { overflow: visible; }

  .history-diff .virtual-row {
    position: absolute;
    left: 0;
    right: 0;
    padding: 0 12px;
    overflow: hidden;
    white-space: pre;
  }

  .history-diff .virtual-row.add { background: var(--diff-add); }

  .history-diff .virtual-row.del { background: var(--diff-del); }

  .history-diff .virtual-row.hunk,
  .history-diff .virtual-row.meta { color: var(--text-secondary); }

  .preview-confirm {
    position: absolute;
    inset: 0;
    z-index: 10;
    display: grid;
    place-items: center;
    padding: 24px;
    background: color-mix(in srgb, var(--text) 18%, transparent);
  }

  .preview-confirm-card {
    width: min(440px, 100%);
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 20px 22px 18px;
    border: 1px solid var(--line);
    border-radius: var(--radius-xl);
    background: var(--elevated);
    box-shadow: var(--shadow);
  }

  .preview-confirm-card h3 { margin: 0; font-size: 16px; font-weight: 650; }

  .preview-confirm-card p { margin: 0; color: var(--text-secondary); line-height: 1.5; }

  .preview-confirm-card ul {
    margin: 0;
    padding: 8px 10px;
    max-height: 160px;
    overflow: auto;
    list-style: none;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--field);
    font-family: var(--mono);
    font-size: 12px;
  }

  .preview-confirm-card li { overflow-wrap: anywhere; }

  .preview-confirm-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 4px; }

  .preview-confirm-actions button {
    height: 30px;
    padding: 0 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--elevated);
    color: var(--text);
    cursor: pointer;
  }

  .preview-confirm-actions button:hover { background: var(--hover); }

  .preview-confirm-actions button.danger { color: var(--deleted); }

  .preview-confirm-actions button.danger:hover {
    border-color: color-mix(in srgb, var(--deleted) 45%, transparent);
    background: color-mix(in srgb, var(--deleted) 10%, transparent);
  }
</style>
