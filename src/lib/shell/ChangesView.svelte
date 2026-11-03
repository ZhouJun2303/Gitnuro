<script lang="ts">
  import { getShell } from "./context";
  import "./ChangesView.css";
  import SplitHandle from "../SplitHandle.svelte";
  import DiffPane from "./DiffPane.svelte";
  import type { Row, SearchKey, Side } from "./types";

  const shell = getShell();
  const focusOnMount = (node: HTMLInputElement) => shell.focusOnMount(node);
</script>

<section class="changes-column" style:width="{shell.changesWidth}px">
        {#if shell.fileLayout === "combined"}
          <div class="status-pane unstaged-pane flex-fill">{@render combinedPane()}</div>
        {:else if !shell.settings.swapPanes}
          <div class="status-pane unstaged-pane" style:height="{shell.unstagedHeight}px">
            {@render unstagedPane()}
          </div>

          <SplitHandle
            direction="horizontal"
            bind:size={shell.unstagedHeight}
            min={100}
            max={650}
            defaultSize={280}
            onchange={shell.handleLayoutChange.bind(shell)}
          />

          <div class="status-pane staged-pane flex-fill">
            {@render stagedPane()}
          </div>
        {:else}
          <div class="status-pane staged-pane" style:height="{shell.unstagedHeight}px">
            {@render stagedPane()}
          </div>

          <SplitHandle
            direction="horizontal"
            bind:size={shell.unstagedHeight}
            min={100}
            max={650}
            defaultSize={280}
            onchange={shell.handleLayoutChange.bind(shell)}
          />

          <div class="status-pane unstaged-pane flex-fill">
            {@render unstagedPane()}
          </div>
        {/if}
      </section>

      <SplitHandle
        direction="vertical"
        bind:size={shell.changesWidth}
        min={220}
        max={650}
        defaultSize={300}
        onchange={shell.handleLayoutChange.bind(shell)}
      />

      <div class="right-stage flex-fill">
      <DiffPane />

        <SplitHandle
          direction="horizontal"
          bind:size={shell.composerHeight}
          min={120}
          max={450}
          defaultSize={170}
          reverse
          onchange={shell.handleLayoutChange.bind(shell)}
        />

        <form class="composer" style:height="{shell.composerHeight}px" onsubmit={(event) => { event.preventDefault(); void shell.submitCommit(false); }}>
          <input class="summary-input {shell.summaryTone} guided" placeholder={shell.tr("chrome.commitSubject")} bind:value={shell.summary} maxlength="200" aria-invalid={shell.summaryTooLong} spellcheck={shell.settings.spellChecking === "enable"} />
          <textarea class="guided composer-desc flex-fill" placeholder={shell.tr("chrome.description")} bind:value={shell.description} spellcheck={shell.settings.spellChecking === "enable"}></textarea>
          <div class="composer-row">
            <label class="check"><input type="checkbox" bind:checked={shell.amend} onchange={() => { if (shell.amend) void shell.loadFacts().then((value) => { if (value?.headMessage && !shell.summary.trim()) void shell.fillFromMessage(value.headMessage); }); }} /> {shell.tr("chrome.amend")}</label>
            <button class="text-button more" type="button" onclick={(event) => shell.openBar(event, shell.commitExtras())}>⋯</button>
            <button class="commit" type="submit" disabled={!shell.canCommit}>{shell.busy ? shell.tr("chrome.working") : shell.tr("chrome.commit")}</button>
          </div>
        </form>
      </div>

{#snippet dirRow(path: string, side: Side, depth: number)}
      <button class="tree-row" type="button" style:padding-left="{8 + depth * 16}px" onclick={() => shell.toggleDir(path)} oncontextmenu={(event) => shell.openMenu(event, shell.dirMenu(path, side))}>
        {@render treeLine(true, !shell.collapsedDirs.includes(path), path, "")}
      </button>
    {/snippet}

{#snippet combinedPane()}
      <div class="pane-head">
        {#if shell.searchOpen.file}
          {@render fileFilter()}
        {:else}
          <div class="pane-label">
            <span class="pane-title">{shell.tr("chrome.combined")}</span>
          </div>
        {/if}
        <div class="pane-tools">
          {@render searchToggle("file", shell.tr("chrome.filterFiles"))}
          {@render fileViewButton()}
        </div>
      </div>
      {@render pickedActions(shell.pickedIn(shell.shownUnstaged), shell.pickedIn(shell.shownStaged))}
      <div class="file-list">
        {#each shell.shownStaged as file (`s:${file.path}`)}
          <button class="side" type="button" class:selected={shell.selectedPath === file.path && shell.selectedSide === "staged"} class:picked={shell.picked.includes(file.path) && !(shell.selectedPath === file.path && shell.selectedSide === "staged")} onclick={() => shell.selectFile(file.path, "staged")}><span class="meta">{shell.tr("chrome.staged")}</span> <span class="name">{file.path}</span></button>
        {/each}
        {#each shell.shownUnstaged as file (`u:${file.path}`)}
          <button class="side" type="button" class:selected={shell.selectedPath === file.path && shell.selectedSide === "unstaged"} class:picked={shell.picked.includes(file.path) && !(shell.selectedPath === file.path && shell.selectedSide === "unstaged")} onclick={() => shell.selectFile(file.path, "unstaged")}>{#if file.letter === "C"}{@render conflictMark()}{/if}<span class="meta">{shell.tr("chrome.unstaged")}</span> <span class="name">{file.path}</span></button>
        {/each}
      </div>
    {/snippet}

{#snippet stagedPane()}
      {@const stagedPicked = shell.pickedIn(shell.shownStaged)}
      <div class="pane-head">
        <div class="pane-label">
          <span class="pane-title">{shell.tr("chrome.staged")}</span>
          <span class="count">{shell.staged.length}</span>
        </div>
        <div class="pane-tools">
          <button class="pane-chip" type="button" disabled={shell.mode !== "live" || shell.busy || shell.staged.length === 0} onclick={() => shell.runChange("unstage_all")}>{shell.tr("chrome.unstageAll")}</button>
        </div>
      </div>
      {@render pickedActions([], stagedPicked)}
      {#if shell.shownStaged.length === 0}
        <p class="empty">{shell.staged.length === 0 ? shell.tr("chrome.noStaged") : shell.tr("chrome.noMatch")}</p>
      {:else}
        <div class="file-list staged-list" onscroll={(event) => (shell.stagedTop = (event.currentTarget as HTMLElement).scrollTop)}>
          <div class="commit-window" style:height="{shell.stagedTree.length * shell.fileStride}px">
            {#each shell.stagedWindow.rows as entry, index (entry.key)}
              <div class="virtual-row" style:top="{(shell.stagedWindow.start + index) * shell.fileStride}px">
                {#if entry.kind === "dir"}
                  {@render dirRow(entry.path, "staged", entry.depth)}
                {:else if entry.file}
                  {@render fileRow(entry.file, "staged", entry.depth)}
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {/if}
    {/snippet}

{#snippet unstagedPane()}
      {@const unstagedPicked = shell.pickedIn(shell.shownUnstaged)}
      <div class="pane-head">
        {#if shell.searchOpen.file}
          {@render fileFilter()}
        {:else}
          <div class="pane-label">
            <span class="pane-title">{shell.tr("chrome.unstaged")}</span>
            <span class="count">{shell.mode === "loading" ? "…" : shell.mode === "error" ? "—" : shell.shownUnstaged.length}</span>
          </div>
        {/if}
        <div class="pane-tools">
          {@render searchToggle("file", shell.tr("chrome.filterFiles"))}
          {@render fileViewButton()}
          <button class="pane-chip pane-go" type="button" disabled={shell.mode !== "live" || shell.busy || shell.unstaged.length === 0} onclick={() => shell.runChange("stage_all")}>{shell.tr("chrome.stageAll")}</button>
        </div>
      </div>
      {@render pickedActions(unstagedPicked, [])}
      <div class="file-list" onscroll={(event) => (shell.fileTop = (event.currentTarget as HTMLElement).scrollTop)}>
        {#if shell.showIgnored}
          {#if (shell.facts?.ignored.length ?? 0) === 0}
            <p class="empty">{shell.tr("chrome.noMatch")}</p>
          {:else}
            <div class="commit-window" style:height="{(shell.facts?.ignored.length ?? 0) * shell.fileStride}px">
              {#each shell.ignoredWindow.rows as path, index (path)}
                <div class="virtual-row side quiet" style:top="{(shell.ignoredWindow.start + index) * shell.fileStride}px"><span class="name">{path}</span></div>
              {/each}
            </div>
          {/if}
        {:else if shell.mode === "loading"}
          <p class="empty">{shell.tr("chrome.readingStatus")}</p>
        {:else if shell.mode === "error"}
          <p class="empty">{shell.loadError}</p>
        {:else if shell.shownUnstaged.length === 0}
          <p class="empty">{shell.unstaged.length === 0 ? shell.tr("chrome.noUnstaged") : shell.tr("chrome.noMatch")}</p>
        {:else}
          <div class="commit-window" style:height="{shell.unstagedTree.length * shell.fileStride}px">
            {#each shell.fileWindow.rows as entry, index (entry.key)}
              <div class="virtual-row" style:top="{(shell.fileWindow.start + index) * shell.fileStride}px">
                {#if entry.kind === "dir"}
                  {@render dirRow(entry.path, "unstaged", entry.depth)}
                {:else if entry.file}
                  {@render fileRow(entry.file, "unstaged", entry.depth)}
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/snippet}

{#snippet pickedActions(unstagedFiles: string[], stagedFiles: string[])}
      {#if unstagedFiles.length > 1 || stagedFiles.length > 1}
        <div class="pick-bar">
          {#if unstagedFiles.length > 1}
            <button class="pane-chip" type="button" disabled={shell.mode !== "live" || shell.busy} onclick={() => void shell.stagePicked(unstagedFiles)}>{shell.tr("chrome.stageSelected")}</button>
            <button class="pane-chip danger" type="button" disabled={shell.mode !== "live" || shell.busy} onclick={() => shell.discardChosen(unstagedFiles)}>{shell.tr("chrome.discardSelected")}</button>
            <button class="pane-chip" type="button" disabled={shell.mode !== "live" || shell.busy} onclick={() => shell.openStash(unstagedFiles)}>{shell.tr("chrome.stash")}</button>
            <button class="pane-chip danger" type="button" disabled={shell.mode !== "live" || shell.busy} onclick={() => shell.deleteChosen(unstagedFiles)}>{shell.tr("chrome.deleteSelected")}</button>
          {/if}
          {#if stagedFiles.length > 1}
            <button class="pane-chip" type="button" disabled={shell.mode !== "live" || shell.busy} onclick={() => void shell.unstagePicked(stagedFiles)}>{shell.tr("chrome.unstageSelected")}</button>
            <button class="pane-chip" type="button" disabled={shell.mode !== "live" || shell.busy} onclick={() => shell.openStash(stagedFiles)}>{shell.tr("chrome.stash")}</button>
            <button class="pane-chip danger" type="button" disabled={shell.mode !== "live" || shell.busy} onclick={() => shell.deleteChosen(stagedFiles)}>{shell.tr("chrome.deleteSelected")}</button>
          {/if}
        </div>
      {/if}
    {/snippet}

{#snippet fileRow(file: Row, side: Side, depth = 0)}
      <div
        class="file"
        class:tree-row={shell.fileLayout === "tree"}
        style:padding-left="{shell.fileLayout === "tree" ? 8 + depth * 16 : 8 + depth * 14}px"
        class:selected={shell.selectedPath === file.path && (shell.mode !== "live" || shell.selectedSide === side)}
        class:picked={shell.picked.includes(file.path) && !(shell.selectedPath === file.path && (shell.mode !== "live" || shell.selectedSide === side))}
        role="group"
        oncontextmenu={(event) => shell.openMenu(event, shell.fileMenu(file, side))}
      >
        <button class="file-select" type="button" onclick={(event) => shell.selectFile(file.path, side, event)}>
          {#if shell.fileLayout === "tree"}
            {@render treeLine(false, false, file.path, file.letter)}
          {:else}
            {#if file.letter === "C"}
              {@render conflictMark()}
            {:else}
              <span class="badge {file.tone}">{file.letter}</span>
            {/if}
            <span class="file-name">{shell.fileName(file.path)}</span>
          {/if}
        </button>
      </div>
    {/snippet}

{#snippet treeLine(dir: boolean, open: boolean, path: string, letter: string)}
    {#if dir}
      {@render disclosure(open)}
      {@render folderIcon()}
    {:else}
      <span class="twist"></span>
      {#if letter === "C"}
        {@render conflictMark()}
      {:else if letter}
        {@render statusBadge(letter)}
      {/if}
      {@render typeIcon(path)}
    {/if}
    <span class="name">{shell.fileName(path)}</span>
  {/snippet}

{#snippet disclosure(open: boolean)}
    <span class="twist" aria-hidden="true">
      <svg class="twist-icon" viewBox="0 0 12 12">
        <path fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" d={open ? "M2.2 4.2 6 8l3.8-3.8" : "M4.2 2.2 8 6l-3.8 3.8"} />
      </svg>
    </span>
  {/snippet}

{#snippet folderIcon()}
    <svg class="tree-icon folder" viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M1.5 4.1A1.4 1.4 0 0 1 2.9 2.7h3.2l1.3 1.4H13a1.5 1.5 0 0 1 1.5 1.5v6.9A1.5 1.5 0 0 1 13 14H2.9a1.4 1.4 0 0 1-1.4-1.4z"/></svg>
  {/snippet}

{#snippet conflictMark()}
  <svg class="conflict-mark" viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M8 1.4 14.8 13.4H1.2L8 1.4z"/><path d="M8 6.1v3.3" stroke="#fff" stroke-width="1.4" stroke-linecap="round"/><circle cx="8" cy="11.4" r=".75" fill="#fff"/></svg>
{/snippet}

{#snippet statusBadge(letter: string)}
    {#if letter === "A" || letter === "?"}
      <svg class="tree-icon status added" viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="6.2" fill="currentColor"/><path d="M8 4.9v6.2M4.9 8h6.2" stroke="#fff" stroke-width="1.5" stroke-linecap="round"/></svg>
    {:else if letter === "D"}
      <svg class="tree-icon status deleted" viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="6.2" fill="currentColor"/><path d="M4.9 8h6.2" stroke="#fff" stroke-width="1.5" stroke-linecap="round"/></svg>
    {:else if letter}
      <span class="tree-mark" class:alert={letter === "C"}>{letter}</span>
    {/if}
  {/snippet}

{#snippet typeIcon(path: string)}
    {@const kind = shell.fileKind(path)}
    <svg class="tree-icon type" class:code={kind === "code"} class:json={kind === "json"} class:markup={kind === "markup"} class:text={kind === "text"} class:image={kind === "image"} viewBox="0 0 16 16" aria-hidden="true">
      {#if kind === "image"}
        <rect x="1.8" y="3.1" width="12.4" height="9.8" rx="1.6" fill="none" stroke="currentColor" stroke-width="1.2"/>
        <circle cx="5.3" cy="6.3" r="1" fill="currentColor"/>
        <path fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" d="M2.4 11.3 5.7 8.4l2 1.7 2.2-1.5 3.5 2.7"/>
      {:else}
        <path fill="none" stroke="currentColor" stroke-width="1.2" d="M3.2 1.5h6L12.8 5v9.5H3.2z"/>
        <path fill="none" stroke="currentColor" stroke-width="1.2" d="M9.1 1.6V5.1h3.6"/>
        {#if kind === "json"}
          <path fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" d="M6.3 7.2c-.8.5-.8 1.1 0 1.6.8.5.8 1.1 0 1.6M9.7 7.2c.8.5.8 1.1 0 1.6-.8.5-.8 1.1 0 1.6"/>
        {:else if kind === "markup"}
          <path fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" d="M6.4 7.3 4.8 8.8l1.6 1.5M9.6 7.3l1.6 1.5-1.6 1.5M8.6 7.1 7.4 10.6"/>
        {:else}
          <path fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" d="M5.4 7.4h5.2M5.4 9.3h5.2M5.4 11.2h3.2"/>
        {/if}
      {/if}
    </svg>
  {/snippet}

{#snippet fileFilter()}
    <input class="search pane-filter" placeholder={shell.tr("chrome.filterFiles")} aria-label={shell.tr("chrome.filterFiles")} bind:value={shell.fileQuery} use:focusOnMount onkeydown={(event) => shell.searchKeys("file", event)} onblur={() => shell.searchBlur("file")} />
  {/snippet}

{#snippet fileViewButton()}
    <button class="icon-btn pane-more" type="button" aria-label={shell.tr("chrome.view")} aria-haspopup="menu" title={shell.tr("chrome.view")} onclick={shell.openFileView.bind(shell)}>
      <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4.5 6.25 8 9.75l3.5-3.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
    </button>
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
