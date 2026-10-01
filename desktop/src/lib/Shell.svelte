<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { commits, diffs, unstaged as sampleUnstaged } from "./sample";
  import { badge, type FileDiff, type StatusFile, type StatusSnapshot } from "./status";

  type Mode = "sample" | "loading" | "live" | "error";
  type Side = "staged" | "unstaged";
  type Row = { path: string; letter: string; tone: "added" | "modified" | "deleted" };

  let section = $state<"changes" | "history">("changes");
  let selectedPath = $state<string | null>(sampleUnstaged[0]?.path ?? null);
  let summary = $state("");
  let description = $state("");
  let amend = $state(false);
  let signOff = $state(false);
  let selectedSide = $state<Side>("unstaged");
  let mode = $state<Mode>(
    typeof window !== "undefined" && "__TAURI_INTERNALS__" in window ? "loading" : "sample",
  );
  let snapshot = $state<StatusSnapshot | null>(null);
  let loadError = $state<string | null>(null);
  let actionError = $state<string | null>(null);
  let busy = $state(false);
  let diff = $state<FileDiff | null>(null);
  let diffError = $state<string | null>(null);
  let diffToken = 0;

  const summaryTooLong = $derived(summary.length > 72);
  const selectedDiff = $derived(mode === "sample" && selectedPath ? (diffs[selectedPath] ?? []) : []);
  const staged = $derived<Row[]>(mode === "live" ? (snapshot?.staged ?? []).map(toRow) : []);
  const unstaged = $derived<Row[]>(
    mode === "live"
      ? (snapshot?.unstaged ?? []).map(toRow)
      : sampleUnstaged.map((file) => ({
          path: file.path,
          letter: file.kind,
          tone: file.kind === "A" ? "added" : file.kind === "D" ? "deleted" : "modified",
        })),
  );
  const branchLabel = $derived(
    mode === "sample" ? "main" : (snapshot?.branch ?? (mode === "loading" ? "…" : "HEAD")),
  );
  const tabLabel = $derived(mode === "live" && snapshot ? folderName(snapshot.path) : "Gitnuro");
  const changeCount = $derived(
    mode === "live" ? staged.length + unstaged.length : mode === "sample" ? unstaged.length : 0,
  );
  const canCommit = $derived(
    summary.trim().length > 0 &&
      !summaryTooLong &&
      !busy &&
      (mode !== "live" || staged.length > 0 || amend),
  );

  function repoPath() {
    return snapshot?.path;
  }

  function message(error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }

  function applySnapshot(value: StatusSnapshot) {
    snapshot = value;
    mode = "live";
    const sameSide = (selectedSide === "staged" ? value.staged : value.unstaged).some(
      (file) => file.path === selectedPath,
    );
    if (selectedPath && sameSide) return;
    const otherSide: Side = selectedSide === "staged" ? "unstaged" : "staged";
    const other = otherSide === "staged" ? value.staged : value.unstaged;
    if (selectedPath && other.some((file) => file.path === selectedPath)) {
      selectedSide = otherSide;
      return;
    }
    if (value.unstaged[0]) {
      selectedSide = "unstaged";
      selectedPath = value.unstaged[0].path;
    } else if (value.staged[0]) {
      selectedSide = "staged";
      selectedPath = value.staged[0].path;
    } else {
      selectedPath = null;
    }
  }

  async function loadDiff() {
    if (mode !== "live" || !selectedPath || section !== "changes") {
      diff = null;
      diffError = null;
      return;
    }
    const token = ++diffToken;
    const file = selectedPath;
    const staged = selectedSide === "staged";
    try {
      const value = await invoke<FileDiff>("file_diff", { path: repoPath(), file, staged });
      if (token === diffToken) {
        diff = value;
        diffError = null;
      }
    } catch (error) {
      if (token === diffToken) {
        diff = null;
        diffError = message(error);
      }
    }
  }

  function selectFile(path: string, side: Side) {
    selectedPath = path;
    selectedSide = side;
    if (mode === "live") void loadDiff();
  }

  async function runChange(command: string, extra: Record<string, unknown> = {}) {
    if (mode !== "live" || busy) return;
    busy = true;
    actionError = null;
    try {
      const value = await invoke<StatusSnapshot>(command, { path: repoPath(), ...extra });
      applySnapshot(value);
      await loadDiff();
    } catch (error) {
      actionError = message(error);
    } finally {
      busy = false;
    }
  }

  function toRow(file: StatusFile): Row {
    const mark = badge(file.kind);
    return { path: file.path, letter: mark.letter, tone: mark.tone };
  }

  function fileName(path: string) {
    const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return slash >= 0 ? path.slice(slash + 1) : path;
  }

  function parentDir(path: string) {
    const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return slash >= 0 ? path.slice(0, slash) : "";
  }

  function folderName(path: string) {
    const trimmed = path.replace(/[\\/]+$/, "");
    const slash = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
    return slash >= 0 ? trimmed.slice(slash + 1) : trimmed;
  }

  onMount(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    mode = "loading";
    selectedPath = null;
    invoke<StatusSnapshot>("workspace_status")
      .then(async (value) => {
        applySnapshot(value);
        await loadDiff();
      })
      .catch((error: unknown) => {
        loadError = message(error);
        mode = "error";
      });
  });
</script>

<div class="shell">
  <header class="chrome">
    <div class="tabs">
      <button class="tab" type="button">Default</button>
      <button class="tab active" type="button">{tabLabel}</button>
    </div>
    <div class="toolbar">
      <button class="tool" type="button"><span class="glyph">↓</span>Fetch</button>
      <button class="tool" type="button"><span class="glyph">↓</span>Pull</button>
      <button class="tool" type="button"><span class="glyph">↑</span>Push</button>
      <button class="tool" type="button"><span class="glyph">▣</span>Stash</button>
      <button class="tool" type="button"><span class="glyph">▢</span>Pop</button>
      <button class="tool" type="button"><span class="glyph">⑂</span>Git Flow</button>
      <div class="spacer"></div>
      <input class="launch" placeholder="Quick Launch" aria-label="Quick Launch" />
      <button class="tool" type="button"><span class="glyph">▹</span>Terminal</button>
      <button class="tool" type="button"><span class="glyph">▤</span>Explorer</button>
      <button class="tool" type="button"><span class="glyph">⚙</span>Preferences</button>
    </div>
  </header>

  <div class="body">
    <aside class="sidebar">
      <button
        class="side"
        class:selected={section === "changes"}
        type="button"
        onclick={() => (section = "changes")}
      >
        <span>Local Changes</span>
        <span class="count">{changeCount}</span>
      </button>
      <button
        class="side"
        class:selected={section === "history"}
        type="button"
        onclick={() => (section = "history")}
      >
        <span>All Commits</span>
      </button>

      <div class="group">Local branches</div>
      <div class="side current">
        <span class="dot"></span>
        <span class="name">{branchLabel}</span>
        {#if mode === "sample"}
          <span class="ahead">↑5</span>
        {/if}
      </div>
      <div class="group">Remotes</div>
      <div class="side quiet"><span class="name">origin</span></div>
      <div class="side nested quiet"><span class="name">HEAD</span></div>
      <div class="side nested quiet"><span class="name">main</span></div>
      <div class="group">More</div>
      <div class="side quiet"><span>Tags</span><span class="count">0</span></div>
      <div class="side quiet"><span>Stashes</span><span class="count">0</span></div>
      <div class="side quiet"><span>Submodules</span><span class="count">0</span></div>
      <div class="side quiet"><span>Worktrees</span><span class="count">1</span></div>
    </aside>

    {#if section === "changes"}
      <section class="changes">
        <div class="pane-head">
          <span>Staged</span>
          <span class="count">{staged.length}</span>
          <button
            class="text-button"
            type="button"
            disabled={mode !== "live" || busy || staged.length === 0}
            onclick={() => runChange("unstage_all")}
          >Unstage all</button>
        </div>
        {#if staged.length === 0}
          <p class="empty">No staged changes</p>
        {:else}
          <div class="file-list staged-list">
            {#each staged as file (file.path)}
              {@render fileRow(file, "staged")}
            {/each}
          </div>
        {/if}

        <div class="pane-head">
          <span>Unstaged</span>
          <span class="count">{mode === "loading" ? "…" : mode === "error" ? "—" : unstaged.length}</span>
          <button
            class="text-button"
            type="button"
            disabled={mode !== "live" || busy || unstaged.length === 0}
            onclick={() => runChange("stage_all")}
          >Stage all</button>
        </div>
        <div class="file-list">
          {#if mode === "loading"}
            <p class="empty">Reading repository status…</p>
          {:else if mode === "error"}
            <p class="empty">{loadError}</p>
          {:else if unstaged.length === 0}
            <p class="empty">No unstaged changes</p>
          {:else}
            {#each unstaged as file (file.path)}
              {@render fileRow(file, "unstaged")}
            {/each}
          {/if}
        </div>

        {#snippet fileRow(file: Row, side: Side)}
          <div class="file" class:selected={selectedPath === file.path && (mode !== "live" || selectedSide === side)}>
            <button class="file-select" type="button" onclick={() => selectFile(file.path, side)}>
              <span class="badge {file.tone}">{file.letter}</span>
              <span class="file-name">{fileName(file.path)}</span>
              <span class="file-dir">{parentDir(file.path)}</span>
            </button>
            {#if mode === "live"}
              <button
                class="text-button row-action"
                type="button"
                disabled={busy}
                onclick={() => runChange(side === "staged" ? "unstage_path" : "stage_path", { file: file.path })}
              >{side === "staged" ? "Unstage" : "Stage"}</button>
            {/if}
          </div>
        {/snippet}

        <form
          class="composer"
          onsubmit={(event) => {
            event.preventDefault();
            if (mode !== "live" || !canCommit) return;
            void runChange("commit_changes", { summary, description, amend, signOff }).then(() => {
              if (!actionError) {
                summary = "";
                description = "";
                amend = false;
              }
            });
          }}
        >
          <label class="field">
            <input
              placeholder="Summary"
              bind:value={summary}
              maxlength="200"
              aria-invalid={summaryTooLong}
            />
            <span class="counter" class:over={summaryTooLong}>{summary.length}/72</span>
          </label>
          <textarea placeholder="Description" rows="3" bind:value={description}></textarea>
          <div class="composer-row">
            <label class="check"><input type="checkbox" bind:checked={amend} /> Amend</label>
            <label class="check"><input type="checkbox" bind:checked={signOff} /> Sign-off</label>
            <button class="commit" type="submit" disabled={!canCommit}>{busy ? "Working…" : "Commit"}</button>
          </div>
          {#if actionError}
            <p class="empty">{actionError}</p>
          {/if}
        </form>
      </section>
    {:else if mode !== "sample"}
      <section class="history">
        <p class="empty">History is not loaded yet</p>
      </section>
    {:else}
      <section class="history">
        {#each commits as commit (commit.id)}
          <button
            class="commit-row"
            class:selected={selectedPath === commit.id}
            type="button"
            onclick={() => (selectedPath = commit.id)}
          >
            <span class="lane" aria-hidden="true"></span>
            <span class="subject">{commit.summary}</span>
            <span class="badges">
              {#each commit.badges as label (label)}
                <span class="ref">{label}</span>
              {/each}
            </span>
            <span class="meta">{commit.author}</span>
            <span class="meta sha">{commit.id}</span>
            <span class="meta">{commit.when}</span>
          </button>
        {/each}
      </section>
    {/if}

    <section class="diff">
      {#if mode === "live" && section === "changes" && selectedPath}
        <header class="diff-head">{selectedPath}</header>
        {#if diffError}
          <p class="diff-empty">{diffError}</p>
        {:else if !diff}
          <p class="diff-empty">Reading diff…</p>
        {:else if diff.binary}
          <p class="diff-empty">Binary file</p>
        {:else if diff.lines.length === 0}
          <p class="diff-empty">No changes in this view</p>
        {:else}
          <pre class="diff-body">{#each diff.lines as line, index (index)}<span
                class:add={line.kind === "add"}
                class:del={line.kind === "delete"}
                class:hunk={line.kind === "hunk"}
                class:meta={line.kind === "meta"}>{line.text + "\n"}</span>{/each}{#if diff.truncated}<span class="meta">Diff truncated.</span>{/if}</pre>
        {/if}
      {:else if section === "changes" && selectedPath && selectedDiff.length > 0}
        <header class="diff-head">{selectedPath}</header>
        <pre class="diff-body">{#each selectedDiff as line, index (index)}<span
              class:add={line.startsWith("+") && !line.startsWith("+++")}
              class:del={line.startsWith("-") && !line.startsWith("---")}
              class:hunk={line.startsWith("@@")}>{line + "\n"}</span>{/each}</pre>
      {:else if mode !== "sample" && section === "history"}
        <p class="diff-empty">History is not loaded yet</p>
      {:else if section === "history" && selectedPath}
        <header class="diff-head">{commits.find((row) => row.id === selectedPath)?.summary}</header>
        <p class="diff-empty">Select a file in this commit to see changes</p>
      {:else}
        <p class="diff-empty">Select a file to see changes</p>
      {/if}
    </section>
  </div>
</div>

<style>
  .shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--canvas);
    color: var(--text);
    user-select: none;
  }

  .chrome {
    background: var(--sidebar);
    border-bottom: 1px solid var(--line);
  }

  .tabs {
    display: flex;
    gap: 4px;
    padding: 6px 8px 0;
  }

  .tab,
  .tool,
  .text-button,
  .side,
  .file,
  .commit-row,
  .commit {
    font: inherit;
    color: inherit;
    background: transparent;
    border: 0;
  }

  .tab {
    height: 26px;
    padding: 0 10px;
    border-radius: var(--radius) var(--radius) 0 0;
    color: var(--text-secondary);
  }

  .tab.active {
    background: var(--canvas);
    color: var(--text);
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 2px;
    min-height: 44px;
    padding: 0 6px 4px;
  }

  .tool {
    width: 58px;
    height: 40px;
    border-radius: var(--radius);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1px;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .tool:hover,
  .text-button:hover:not(:disabled),
  .side:hover,
  .file:hover,
  .commit-row:hover {
    background: var(--hover);
  }

  .glyph {
    font-size: 13px;
    line-height: 1;
    color: var(--text);
  }

  .spacer {
    flex: 1;
  }

  .launch {
    width: 220px;
    height: 26px;
    margin-right: 8px;
    padding: 0 12px;
    border: 0;
    border-radius: 13px;
    background: var(--field);
    color: var(--text);
    font: inherit;
  }

  .launch:focus-visible,
  .field input:focus-visible,
  textarea:focus-visible,
  .commit:focus-visible,
  .tool:focus-visible,
  .side:focus-visible,
  .file:focus-visible,
  .commit-row:focus-visible,
  .tab:focus-visible,
  .text-button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .sidebar {
    width: 248px;
    flex: none;
    background: var(--sidebar);
    border-right: 1px solid var(--line);
    padding: 8px 0;
    overflow: auto;
  }

  .side {
    width: calc(100% - 12px);
    height: var(--row-side);
    margin: 0 6px;
    padding: 0 8px;
    border-radius: var(--radius);
    display: flex;
    align-items: center;
    gap: 6px;
    text-align: left;
  }

  .side.selected {
    background: var(--selection);
  }

  .side.nested {
    padding-left: 22px;
  }

  .side.quiet,
  .count,
  .ahead,
  .file-dir,
  .meta,
  .empty,
  .diff-empty {
    color: var(--text-secondary);
  }

  .count,
  .ahead {
    margin-left: auto;
    font-size: 11px;
  }

  .group {
    margin: 10px 14px 2px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
  }

  .current .name {
    font-weight: 600;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }

  .changes,
  .history {
    width: 42%;
    min-width: 300px;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--line);
    min-height: 0;
  }

  .history {
    overflow: auto;
  }

  .pane-head {
    height: 28px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    font-weight: 600;
  }

  .text-button {
    margin-left: auto;
    color: var(--text-secondary);
    border-radius: var(--radius);
    padding: 2px 6px;
  }

  .text-button:disabled {
    opacity: 0.4;
  }

  .empty {
    margin: 0;
    padding: 4px 12px 10px;
  }

  .file-list {
    flex: 1;
    overflow: auto;
    padding-bottom: 8px;
  }

  .staged-list {
    flex: none;
    max-height: 30%;
  }

  .file,
  .commit-row {
    width: calc(100% - 12px);
    margin: 0 6px;
    border-radius: var(--radius);
    display: flex;
    align-items: center;
    gap: 8px;
    text-align: left;
  }

  .file {
    height: var(--row-file);
    padding: 0 8px;
  }

  .file-select {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    text-align: left;
  }

  .row-action {
    margin-left: 0;
    flex: none;
  }

  .file.selected,
  .commit-row.selected {
    background: var(--selection);
  }

  .badge {
    width: 14px;
    flex: none;
    font-size: 11px;
    font-weight: 700;
  }

  .added {
    color: var(--added);
  }

  .deleted {
    color: var(--deleted);
  }

  .modified {
    color: var(--modified);
  }

  .file-name {
    flex: none;
  }

  .file-dir {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
  }

  .composer {
    border-top: 1px solid var(--line);
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius);
    background: var(--field);
  }

  .field input,
  textarea {
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    width: 100%;
  }

  textarea {
    resize: vertical;
    min-height: 64px;
    padding: 6px 8px;
    border-radius: var(--radius);
    background: var(--field);
  }

  .field input:focus,
  textarea:focus {
    outline: none;
  }

  .counter {
    flex: none;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .counter.over {
    color: var(--deleted);
  }

  .composer-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-secondary);
  }

  .commit {
    margin-left: auto;
    height: 28px;
    padding: 0 14px;
    border-radius: 8px;
    background: var(--accent);
    color: var(--on-accent);
  }

  .commit:disabled {
    opacity: 0.4;
  }

  .commit-row {
    height: var(--row-commit);
    padding: 0 8px;
  }

  .lane {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 12px 0 0 var(--accent);
  }

  .subject {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badges {
    display: flex;
    gap: 4px;
  }

  .ref {
    height: 16px;
    padding: 0 4px;
    border-radius: 4px;
    background: var(--field);
    font-size: 11px;
    line-height: 16px;
  }

  .sha {
    font-family: var(--mono);
  }

  .diff {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .diff-head {
    height: 28px;
    display: flex;
    align-items: center;
    padding: 0 12px;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 12px;
    font-weight: 400;
  }

  .diff-body {
    margin: 0;
    padding: 8px 0;
    overflow: auto;
    font-family: var(--mono);
    font-size: 12px;
    line-height: 1.45;
    user-select: text;
  }

  .diff-body span {
    display: block;
    padding: 0 12px;
    white-space: pre;
  }

  .diff-body .add {
    background: var(--diff-add);
  }

  .diff-body .del {
    background: var(--diff-del);
  }

  .diff-body .hunk,
  .diff-body .meta {
    color: var(--text-secondary);
  }

  .diff-empty {
    margin: auto;
  }
</style>
