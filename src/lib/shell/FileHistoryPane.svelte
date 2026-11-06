<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getShell } from "./context";
  import { onVirtualScroll, windowSlice } from "../view";

  const shell = getShell();
  const rowHeight = 48;
  const diffRow = $derived(shell.diffRowHeight);
  let listTop = $state(0);
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

  const place = (at: number) => (span ? ((at - span.min) / (span.max - span.min)) * 100 : 0);

  async function choose(id: string) {
    await shell.selectCommit(id);
    if (shell.historyFilter) await shell.pickCommitFile(shell.historyFilter);
  }

  $effect(() => {
    const file = shell.historyFilter;
    if (!file || !shell.inApp()) return;
    void getCurrentWindow().setTitle(`${shell.tr("menu.history")} — ${file}`).catch(() => {});
  });

  function closePane() {
    if (shell.inApp()) {
      void getCurrentWindow().close();
      return;
    }
    window.location.href = "/";
  }
</script>

<div class="file-history">
  <header class="history-file">
    <strong>{shell.historyFilter || shell.tr("menu.history")}</strong>
    <button class="text-button" type="button" aria-label={shell.tr("chrome.close")} onclick={closePane}>{shell.tr("chrome.close")}</button>
  </header>
  {#if span}
    <div class="history-time">
      {#each years as year (year)}
        {@const at = span ? Math.min(span.max, Math.max(span.min, new Date(year, 0, 1).getTime() / 1000)) : 0}
        {@const pos = place(at)}
        <span style:left="{pos}%" style:transform={pos < 4 ? "none" : pos > 96 ? "translateX(-100%)" : undefined}>{year}</span>
      {/each}
      {#each shell.commitsLive as commit (commit.id)}
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
            <button
              class="history-commit"
              class:selected={shell.selectedCommit === commit.id}
              type="button"
              style:top="{(windowed.start + index) * rowHeight}px"
              onclick={() => void choose(commit.id)}
              oncontextmenu={(event) => shell.openMenu(event, shell.historyFileMenu(commit))}
            >
              <span class="history-meta">
                <span>{commit.author}</span>
                <span>{shell.whenLabel(commit)}</span>
                <span class="sha">{commit.shortId}</span>
              </span>
              <span class="subject">{commit.summary}</span>
            </button>
          {/each}
        </div>
      {/if}
    </section>
    <section
      class="history-diff"
      role="group"
      onscroll={onDiffScroll}
      oncontextmenu={(event) => {
        const commit = shell.commitsLive.find((item) => item.id === shell.selectedCommit);
        if (commit) shell.openMenu(event, shell.historyFileMenu(commit));
      }}
    >
      {#if !shell.historyDiff}
        <p class="diff-empty">{shell.selectedCommit ? shell.tr("chrome.reading") : shell.tr("chrome.noCommits")}</p>
      {:else if shell.historyDiff.binary}
        <p class="diff-empty">{shell.tr("chrome.binary")}</p>
      {:else if shell.historyDiff.lines.length === 0}
        <p class="diff-empty">{shell.tr("chrome.noChanges")}</p>
      {:else}
        <div class="diff-body" class:wrap-window={shell.settings.diffWordWrap}>
          <div class="commit-window" class:wrap-window={shell.settings.diffWordWrap} style:height="{shell.historyDiffWindow.height}px">
            {#each shell.historyDiffWindow.rows as line, index (`h-${shell.historyDiffWindow.start + index}`)}
              {@const at = shell.historyDiffWindow.start + index}
              <span class="virtual-row {line.kind}" class:add={line.kind === "add"} class:del={line.kind === "delete"} class:hunk={line.kind === "hunk"} class:meta={line.kind === "meta"} style:top="{shell.historyDiffWindow.tops[index] ?? 0}px" style:height={shell.settings.diffWordWrap ? "auto" : `${diffRow}px`} style:line-height={shell.settings.diffWordWrap ? "1.45" : `${diffRow}px`}>{line.text}</span>
            {/each}
          </div>
        </div>
      {/if}
    </section>
  </div>
</div>

<style>
  .file-history {
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

  .history-commit {
    position: absolute;
    left: 0;
    right: 0;
    height: 48px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
    padding: 0 12px;
    border: 0;
    background: transparent;
    color: inherit;
    text-align: left;
    font: inherit;
  }

  .history-commit.selected { background: var(--selection); }

  .history-meta {
    display: flex;
    gap: 8px;
    min-width: 0;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .history-meta span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .history-commit .subject {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .history-diff {
    flex: 1;
    min-width: 0;
    overflow: auto;
  }

  .history-diff .diff-body { overflow: visible; height: auto; }

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
</style>
