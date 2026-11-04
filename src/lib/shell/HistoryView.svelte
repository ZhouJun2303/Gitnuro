<script lang="ts">
  import { getShell } from "./context";
  import StashPlates from "./StashPlates.svelte";
  import "./HistoryView.css";
  import SplitHandle from "../SplitHandle.svelte";
  import CommitDetail from "./CommitDetail.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { sampleReflog } from "../sample";
  import { gravatarUrl, laneColors, onVirtualScroll, rowGraph, type GraphCell } from "../view";
  import type { Row, SearchKey } from "./types";

  const shell = getShell();
  const focusOnMount = (node: HTMLInputElement) => shell.focusOnMount(node);
  const onHistoryScroll = onVirtualScroll((top) => (shell.historyTop = top));
  const activeColor = $derived(shell.activeLaneColor());
</script>

<div class="history-stage flex-fill">
        <section class="history flex-fill" bind:this={shell.historyEl} onscroll={onHistoryScroll}>
          {#if shell.filterNames.length > 0}
            <div class="filter-bar">
              <span>{shell.tr("chrome.filteredBy", { name: shell.filterNames.map((name) => `'${name}'`).join(", ") })}</span>
              <button class="text-button" type="button" onclick={() => shell.clearHistoryFilter()}>{shell.tr("chrome.clearFilter")}</button>
            </div>
          {/if}
          <div class="pane-head">
            {#if shell.searchOpen.commit}
              <input class="search" placeholder={shell.tr("chrome.findCommits")} aria-label={shell.tr("chrome.findCommits")} bind:this={shell.searchEl} bind:value={shell.commitQuery} use:focusOnMount onkeydown={(event) => { shell.searchKeys("commit", event); if (event.key === "Enter") shell.commitSearchEnter(); }} onblur={() => shell.searchBlur("commit")} />
            {:else}
              <span class="pane-title">{shell.tr("chrome.allCommits")}</span>
            {/if}
            {@render searchToggle("commit", shell.tr("chrome.findCommits"))}
            {#if shell.historyFilter}
              <button class="text-button" type="button" onclick={() => { shell.historyFilter = ""; void shell.loadContext(); }}>{shell.tr("chrome.allCommits")}</button>
            {/if}
          </div>
          {#if shell.historyFilter}
            <div class="pane-head"><span>{shell.historyFilter}</span></div>
          {/if}
          {#if shell.contentHits}
            <div class="pane-head"><span>{shell.tr("chrome.contentSearch")}</span><button class="text-button" type="button" onclick={() => (shell.contentHits = null)}>{shell.tr("chrome.close")}</button></div>
            {#each shell.contentHits as hit (hit.id)}
              <button class="side" type="button" onclick={() => void shell.selectCommit(hit.id)}><span class="name">{hit.shortId} {hit.summary}</span></button>
            {/each}
          {/if}
          {#if shell.reflogOn}
            {#each shell.reflogRows as row (`${row.selector}-${row.id}`)}
              <button class="commit-row" type="button" oncontextmenu={(event) => shell.openMenu(event, shell.reflogMenu(row))}>
                <span class="subject">{row.summary}</span>
                <span class="commit-side">
                  <span class="meta selector" title={row.selector}>{row.selector}</span>
                  <span class="meta sha">{row.shortId}</span>
                </span>
              </button>
            {/each}
          {:else if shell.mode !== "sample"}
            {#if shell.shownCommits.length === 0}
              <p class="empty">{shell.tr("chrome.noCommits")}</p>
            {:else}
              <div class="commit-window" style:height="{shell.historyItems.length * shell.rowCommit}px">
                {#each shell.historyRows as commit, index (commit.id)}
                  {@const graphRow = shell.liveGraph.rows[shell.historyStart + index] ?? []}
                {@const tone = shell.laneTone(graphRow)}
                {@const plate = shell.stashParts(commit.summary, commit.refs)}
                  <div
                    class="commit-row virtual"
                    class:selected={shell.selectedCommit === commit.id}
                    class:unpushed={shell.isCommitUnpushed(commit)}
                    class:good={shell.bisectMark(commit.id) === "good"}
                    class:bad={shell.bisectMark(commit.id) === "bad"}
                    style:top="{(shell.historyStart + index) * shell.rowCommit}px"
                    role="button"
                    tabindex="0"
                    draggable="true"
                    ondragstart={(event) => event.dataTransfer?.setData("text/plain", commit.id)}
                    ondragover={(event) => event.preventDefault()}
                    ondrop={(event) => { event.preventDefault(); const from = event.dataTransfer?.getData("text/plain") ?? ""; if (from && from !== commit.id) { shell.drops = [from, commit.id]; shell.draft = ""; shell.dialog = "squash"; } }}
                    onclick={(event) => shell.selectCommit(commit.id, event)}
                    onkeydown={(event) => { if (event.key === "Enter") void shell.selectCommit(commit.id); }}
                    oncontextmenu={(event) => shell.openMenu(event, shell.commitMenu(commit))}
                  >
                    {@render laneGraph(graphRow, commit.id)}
                    <span class="commit-copy">
                      {@render branchChips(shell.branchRefLabels(commit.refs), tone, activeColor)}
                      {#if plate}
                        <StashPlates branch={plate.branch} message={plate.message} />
                      {:else}
                        <span class="subject">{@render linked(commit.summary)}</span>
                      {/if}
                      {#if shell.plainRefLabels(commit.refs).length > 0}
                        <span class="badges">
                          {#each shell.plainRefLabels(commit.refs) as label (label)}<span class="ref {shell.refKind(label)}" class:compact={shell.settings.compactBranchLabels}>{label}</span>{/each}
                        </span>
                      {/if}
                    </span>
                    <span class="commit-side">
                      <span class="commit-author">
                        {#if shell.settings.gravatar && commit.email}
                          <img class="avatar tile" alt="" src={gravatarUrl(commit.email)} />
                        {:else}
                          <span class="avatar tile" style:background={shell.avatarColor(commit.author)} title={commit.author}>{commit.author.slice(0, 1).toUpperCase()}</span>
                        {/if}
                        <span class="meta author" title={commit.author}>{commit.author}</span>
                      </span>
                      <span class="meta sha">{commit.shortId}</span>
                      <span class="meta date" title={commit.at ? shell.forkWhen(commit.at) : commit.when}>{commit.at ? shell.forkWhen(commit.at) : commit.when}</span>
                    </span>
                  </div>
                {/each}
              </div>
            {/if}
          {:else}
            {#if shell.repoStats}
              <div class="pane-head"><span class="meta">{shell.repoStats.lastSummary}</span></div>
            {/if}
            {#if shell.reflogOn}
              {#each sampleReflog as row (row.selector)}
                <button class="commit-row" type="button" oncontextmenu={(event) => shell.openMenu(event, shell.reflogMenu(row))}>
                  <span class="subject">{row.summary}</span>
                  <span class="commit-side">
                    <span class="meta selector" title={row.selector}>{row.selector}</span>
                    <span class="meta sha">{row.shortId}</span>
                  </span>
                </button>
              {/each}
            {:else if shell.sampleShown.length === 0}
              <p class="empty">{shell.tr("chrome.noCommits")}</p>
            {:else}
              {#each shell.sampleShown as commit, index (commit.id)}
                {@const graphRow = shell.sampleShownGraph.rows[index] ?? []}
                {@const tone = shell.laneTone(graphRow)}
                {@const plate = shell.stashParts(commit.summary, commit.badges)}
                <button class="commit-row" class:selected={shell.selectedCommit === commit.id || shell.selectedPath === commit.id} class:unpushed={shell.isCommitUnpushed(commit)} type="button" onclick={() => shell.selectSample(commit.id)} oncontextmenu={(event) => commit.badges.includes("stash") ? shell.openMenu(event, [{ label: shell.tr("menu.apply"), disabled: true }, { label: shell.tr("menu.pop"), disabled: true }, { label: shell.tr("menu.drop"), disabled: true }]) : shell.openMenu(event, shell.commitMenu(commit))}>
                  {@render laneGraph(graphRow, commit.id)}
                  <span class="commit-copy">
                    {@render branchChips(shell.branchRefLabels(commit.badges), tone, activeColor)}
                    {#if plate}
                      <StashPlates branch={plate.branch} message={plate.message} />
                    {:else}
                      <span class="subject">{@render linked(commit.summary)}</span>
                    {/if}
                    {#if shell.plainRefLabels(commit.badges).length > 0}
                      <span class="badges">
                        {#each shell.plainRefLabels(commit.badges) as label (label)}<span class="ref {shell.refKind(label)}" class:compact={shell.settings.compactBranchLabels}>{label}</span>{/each}
                      </span>
                    {/if}
                  </span>
                  <span class="commit-side">
                    <span class="commit-author">
                      <span class="avatar tile" style:background={shell.avatarColor(commit.author || "?")}>{(commit.author || "?").slice(0, 1).toUpperCase()}</span>
                      <span class="meta author" title={commit.author}>{commit.author}</span>
                    </span>
                    <span class="meta sha">{commit.id}</span>
                    <span class="meta date" title={commit.when}>{commit.when}</span>
                  </span>
                </button>
              {/each}
            {/if}
          {/if}
        </section>

        <SplitHandle
          direction="horizontal"
          bind:size={shell.historyDetailHeight}
          min={160}
          max={650}
          defaultSize={280}
          reverse
          onchange={shell.handleLayoutChange.bind(shell)}
        />

        <CommitDetail />

      </div>

{#snippet laneGraph(graphRow: GraphCell[], commitId: string)}
      {@const drawn = rowGraph(graphRow)}
      {@const owner = shell.mergeOwner(commitId)}
      {@const folded = owner !== "" && !shell.mergeOpen(commitId)}
      <span class="graph" class:foldable={owner !== ""} style:width="{drawn.width}px" role="presentation" title={owner ? (folded ? shell.tr("menu.unfold") : shell.tr("menu.fold")) : undefined} onclick={(event) => { if (!owner) return; event.stopPropagation(); shell.toggleFold(commitId); }}>
        <svg class="graph-svg" viewBox="0 0 {drawn.width} 28" preserveAspectRatio="none">
          {#each drawn.lines as stroke, index (`${commitId}-l-${index}`)}
            <path d={stroke.d} class:dim={activeColor !== null && stroke.color !== activeColor} stroke={laneColors[stroke.color]} />
          {/each}
          {#each drawn.curves as stroke, index (`${commitId}-c-${index}`)}
            <path d={stroke.d} class:dim={activeColor !== null && stroke.color !== activeColor} stroke={laneColors[stroke.color]} />
          {/each}
        </svg>
        {#each drawn.nodes as node, index (`${commitId}-n-${index}`)}
          <i class="graph-node" class:folded class:dim={activeColor !== null && node.color !== activeColor} style:left="{node.x - 4}px" style:background={folded ? "var(--canvas)" : laneColors[node.color]} style:box-shadow={folded ? `inset 0 0 0 2px ${laneColors[node.color]}` : undefined}></i>
        {/each}
      </span>
    {/snippet}

{#snippet branchChips(labels: string[], tone: string | undefined, active: number | null)}
      {#if labels.length > 0}
        <span class="lane-names">
          {#each labels as label (label)}
            {@const missing = shell.branchMissingRemote(label)}
            <span class="lane-name" class:local-only={missing} class:compact={shell.settings.compactBranchLabels} class:quiet={active !== null && tone !== laneColors[active]} style:--lane={tone ?? "var(--text-secondary)"} title={missing ? `${label} · ${shell.tr("chrome.localOnly")}` : label}>
              {#if shell.refKind(label) === "remote" && !shell.refs?.branches.some((branch) => branch.name === label)}
                <svg class="lane-remote" viewBox="0 0 12 12" aria-hidden="true"><circle cx="6" cy="6" r="3.2" fill="none" stroke="currentColor" stroke-width="1.2"/><circle cx="6" cy="6" r="1.15" fill="currentColor"/></svg>
              {/if}
              <span>{label}</span>
            </span>
          {/each}
        </span>
      {/if}
    {/snippet}

{#snippet linked(text: string)}
      {#each shell.linkParts(text) as part, index (`${index}-${part.text}`)}
        {#if part.href}
          <a class="issue" href={part.href} onclick={(event) => { event.preventDefault(); event.stopPropagation(); void openUrl(part.href); }}>{part.text}</a>
        {:else if shell.settings.highlightIssues && (/^#\d+$/.test(part.text) || part.text.startsWith("http"))}
          <span class="issue">{part.text}</span>
        {:else}
          {part.text}
        {/if}
      {/each}
    {/snippet}

{#snippet searchToggle(key: SearchKey, label: string)}
    <button class="icon-btn search-toggle" class:on={shell.searchOpen[key]} type="button" aria-label={label} aria-pressed={shell.searchOpen[key]} title={label} onmousedown={(event) => event.preventDefault()} onclick={() => shell.toggleSearch(key)}>
      {#if shell.searchOpen[key]}
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/></svg>
      {:else}
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="7" cy="7" r="4.25"/><path d="M10.25 10.25L13.5 13.5"/></svg>
      {/if}
    </button>
  {/snippet}
