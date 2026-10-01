<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { openPath, openUrl } from "@tauri-apps/plugin-opener";
  import { commits, diffs, unstaged as sampleUnstaged } from "./sample";
  import { highlight } from "./highlight";
  import { shortcut, typing } from "./keys";
  import { formatWhen, hiddenName, splitDiff, windowSlice } from "./view";
  import {
    badge,
    type BlameLine,
    type CommitRow,
    type DiffLine,
    type FileDiff,
    type FilePreview,
    type InProgress,
    type RefSnapshot,
    type Settings,
    type StatusFile,
    type StatusSnapshot,
    type Suggestion,
    type UpdateNotice,
  } from "./status";

  type Mode = "sample" | "loading" | "live" | "error";
  type Side = "staged" | "unstaged";
  type Row = { path: string; letter: string; tone: "added" | "modified" | "deleted" };
  type Dialog = "branch" | "flow" | "clone" | "open" | "prefs" | "reset" | "command" | "tag" | "remote" | "squash" | "credential" | null;

  const defaultSettings: Settings = {
    theme: "system",
    pullRebase: false,
    fetchPrune: true,
    diffStyle: "unified",
    proxy: "",
    aiBaseUrl: "https://api.x.ai/v1",
    aiModel: "grok-4.7",
    aiApiKey: "",
    terminal: "",
    swapPanes: false,
    showEntireFile: false,
    linesHeight: "compact",
    uiScale: 13,
    dateRelative: true,
    dateFormat: "",
    date24h: true,
    hiddenRefs: "",
    authorName: "",
    authorEmail: "",
    sslVerify: true,
    sslCaFile: "",
    proxyUser: "",
    proxyPassword: "",
    signCommits: false,
    mergeNoFf: false,
    mergeAutostash: false,
    cloneDirectory: "",
    windowX: 0,
    windowY: 0,
    windowWidth: 0,
    windowHeight: 0,
  };

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
  let refs = $state<RefSnapshot | null>(null);
  let commitsLive = $state<CommitRow[]>([]);
  let progress = $state<InProgress | null>(null);
  let selectedCommit = $state<string | null>(null);
  let commitFiles = $state<StatusFile[]>([]);
  let historyFile = $state<string | null>(null);
  let historyDiff = $state<FileDiff | null>(null);
  let historyFilter = $state("");
  let historyTop = $state(0);
  let blameLines = $state<BlameLine[] | null>(null);
  let settings = $state<Settings>({ ...defaultSettings });
  let repos = $state<string[]>([]);
  let dialog = $state<Dialog>(null);
  let draft = $state("");
  let draftExtra = $state("");
  let draftUser = $state("");
  let draftSecret = $state("");
  let launch = $state("");
  let launchOpen = $state(false);
  let expanded = $state<"tags" | "stashes" | "submodules" | "worktrees" | null>(null);
  let allBranches = $state(true);
  let commitQuery = $state("");
  let drops = $state<string[]>([]);
  let fileTop = $state(0);
  let diffTop = $state(0);
  let blameTop = $state(0);
  let preview = $state<FilePreview | null>(null);
  let updateNotice = $state<UpdateNotice | null>(null);
  let passphrase = $state("");
  let passUser = $state("");
  let searchEl = $state<HTMLInputElement | null>(null);
  let launchEl = $state<HTMLInputElement | null>(null);
  let historyEl = $state<HTMLElement | null>(null);
  let selectedIndex = $state(0);

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
  const rowCommit = $derived(settings.linesHeight === "spaced" ? 32 : 24);
  const rowFile = $derived(settings.linesHeight === "spaced" ? 28 : 22);
  const shownCommits = $derived(
    commitsLive.filter((commit) => {
      const query = commitQuery.trim().toLowerCase();
      if (!query) return true;
      return (
        commit.summary.toLowerCase().includes(query) ||
        commit.author.toLowerCase().includes(query) ||
        commit.shortId.startsWith(query)
      );
    }),
  );
  const historyWindow = $derived(windowSlice(shownCommits, historyTop, rowCommit));
  const historyStart = $derived(historyWindow.start);
  const historyRows = $derived(historyWindow.rows);
  const fileWindow = $derived(windowSlice(unstaged, fileTop, rowFile));
  const diffLines = $derived(diff?.lines ?? []);
  const diffWindow = $derived(windowSlice(diffLines, diffTop, 18));
  const blameWindow = $derived(windowSlice(blameLines ?? [], blameTop, 18));
  const splitRows = $derived(settings.diffStyle === "split" ? splitDiff(diffLines) : []);
  const visibleBranches = $derived((refs?.branches ?? []).filter((branch) => !hiddenName(branch.name, settings.hiddenRefs)));
  const visibleTags = $derived((refs?.tags ?? []).filter((tag) => !hiddenName(tag.name, settings.hiddenRefs)));
  const matches = $derived.by(() => {
    const query = launch.trim().toLowerCase();
    if (!query || mode !== "live") return [];
    if (query.startsWith("!")) {
      return [{ label: `Run ${query.slice(1)}`, run: () => mutate({ action: "custom", command: query.slice(1) }) }];
    }
    const items = [
      { label: "Fetch", run: () => doFetch() },
      { label: "Pull", run: () => doPull() },
      { label: "Push", run: () => doPush() },
      { label: "Stash", run: () => mutate({ action: "stash", message: "" }) },
      { label: "Pop stash", run: () => mutate({ action: "stashPop" }) },
      ...(refs?.branches ?? []).map((branch) => ({
        label: `Checkout ${branch.name}`,
        run: () => mutate({ action: "checkout", name: branch.name }),
      })),
    ];
    return items.filter((item) => item.label.toLowerCase().includes(query)).slice(0, 8);
  });

  function inApp() {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }

  function repoPath() {
    return snapshot?.path;
  }

  function message(error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }

  function applySnapshot(value: StatusSnapshot) {
    snapshot = value;
    mode = "live";
    if (value.path && !repos.includes(value.path)) repos = [...repos, value.path];
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
    const stagedSide = selectedSide === "staged";
    try {
      const value = await invoke<FileDiff>("file_diff", {
        path: repoPath(),
        file,
        staged: stagedSide,
        unified: settings.showEntireFile ? 100000 : 3,
      });
      if (token === diffToken) {
        diff = value;
        diffError = null;
        preview = value.binary ? await invoke<FilePreview | null>("file_preview", { path: repoPath(), file }) : null;
      }
    } catch (error) {
      if (token === diffToken) {
        diff = null;
        diffError = message(error);
      }
    }
  }

  async function loadContext() {
    const path = repoPath();
    if (historyFilter) {
      commitsLive = await invoke<CommitRow[]>("file_history", { path, file: historyFilter, limit: 200 });
    } else {
      commitsLive = await invoke<CommitRow[]>("commit_log", { path, limit: 500, all: allBranches });
    }
    refs = await invoke<RefSnapshot>("repository_refs", { path });
    progress = await invoke<InProgress | null>("in_progress", { path });
  }

  async function refresh() {
    if (!inApp() || busy) return;
    try {
      applySnapshot(await invoke<StatusSnapshot>("workspace_status", { path: repoPath() }));
      await loadContext();
      await loadDiff();
    } catch (error) {
      actionError = message(error);
    }
  }

  function selectFile(path: string, side: Side) {
    selectedPath = path;
    selectedSide = side;
    blameLines = null;
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

  async function mutate(request: Record<string, unknown>) {
    if (!inApp() || busy) return;
    busy = true;
    actionError = null;
    try {
      const value = await invoke<StatusSnapshot>("mutate", { path: repoPath(), request });
      applySnapshot(value);
      await loadContext();
      await loadDiff();
      dialog = null;
    } catch (error) {
      actionError = message(error);
      try {
        applySnapshot(await invoke<StatusSnapshot>("workspace_status", { path: repoPath() }));
        await loadContext();
      } catch {
        /* keep the action error */
      }
    } finally {
      busy = false;
    }
  }

  function doFetch() {
    const remote = refs?.remotes[0]?.name;
    return mutate({ action: "fetch", remote: remote ?? null, prune: settings.fetchPrune, tags: false });
  }

  function doPull() {
    return mutate({ action: "pull", rebase: settings.pullRebase });
  }

  function doPush() {
    const current = refs?.branches.find((branch) => branch.current);
    const remote = refs?.remotes[0]?.name ?? null;
    return mutate({
      action: "push",
      remote,
      setUpstream: !current?.upstream,
      tags: false,
      forceWithLease: false,
    });
  }

  async function submitCommit(pushAfter = false) {
    if (mode !== "live" || !canCommit) return;
    await runChange("commit_changes", { summary, description, amend, signOff });
    if (!actionError) {
      summary = "";
      description = "";
      amend = false;
      await loadContext();
      if (pushAfter) await doPush();
    }
  }

  async function suggestMessage() {
    if (mode !== "live" || busy) return;
    busy = true;
    actionError = null;
    try {
      const value = await invoke<Suggestion>("suggest_commit_message", { path: repoPath() });
      summary = value.summary;
      description = value.description;
    } catch (error) {
      actionError = message(error);
    } finally {
      busy = false;
    }
  }

  async function selectCommit(id: string) {
    selectedCommit = id;
    blameLines = null;
    try {
      commitFiles = await invoke<StatusFile[]>("commit_files", { path: repoPath(), id });
      historyFile = commitFiles[0]?.path ?? null;
      historyDiff = historyFile
        ? await invoke<FileDiff>("show_commit_file", {
            path: repoPath(),
            id,
            file: historyFile,
            unified: settings.showEntireFile ? 100000 : 3,
          })
        : null;
    } catch (error) {
      actionError = message(error);
    }
  }

  async function pickCommitFile(file: string) {
    if (!selectedCommit) return;
    historyFile = file;
    try {
      historyDiff = await invoke<FileDiff>("show_commit_file", {
        path: repoPath(),
        id: selectedCommit,
        file,
        unified: settings.showEntireFile ? 100000 : 3,
      });
    } catch (error) {
      actionError = message(error);
    }
  }

  async function showFileHistory(file: string) {
    historyFilter = file;
    section = "history";
    try {
      commitsLive = await invoke<CommitRow[]>("file_history", { path: repoPath(), file, limit: 200 });
    } catch (error) {
      actionError = message(error);
    }
  }

  async function showBlame(file: string) {
    blameLines = null;
    try {
      blameLines = await invoke<BlameLine[]>("blame_file", { path: repoPath(), file });
    } catch (error) {
      actionError = message(error);
    }
  }

  async function openRepo(path: string) {
    if (!inApp() || busy || !path.trim()) return;
    busy = true;
    actionError = null;
    try {
      applySnapshot(await invoke<StatusSnapshot>("workspace_status", { path }));
      historyFilter = "";
      await loadContext();
      await loadDiff();
      await invoke("watch_repository", { path: repoPath() });
      dialog = null;
    } catch (error) {
      actionError = message(error);
    } finally {
      busy = false;
    }
  }

  async function savePrefs(close = true) {
    settings = await invoke<Settings>("save_settings", { values: settings });
    if (close) dialog = null;
  }

  function removeBranch(name: string) {
    if (!confirm(`Delete branch ${name}?`)) return;
    void mutate({ action: "deleteBranch", name });
  }

  function hardReset() {
    if (!confirm("Hard reset discards uncommitted work. Continue?")) return;
    void mutate({ action: "reset", rev: draft || "HEAD", mode: "hard" });
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

  function whenLabel(commit: CommitRow) {
    if (settings.dateRelative) return commit.when;
    return formatWhen(commit.at, settings.dateFormat, settings.date24h) || commit.when;
  }

  function lineText(line: DiffLine) {
    return line.text.startsWith("+") || line.text.startsWith("-") || line.text.startsWith(" ")
      ? line.text.slice(1)
      : line.text;
  }

  function stageOne(line: DiffLine) {
    if (line.stageAt == null || !selectedPath) return;
    void mutate({
      action: "stageLine",
      file: selectedPath,
      text: lineText(line),
      addition: line.kind === "add",
      at: line.stageAt,
      unstage: selectedSide === "staged",
    });
  }

  function moveSelection(delta: number) {
    const rows = section === "history" ? shownCommits : selectedSide === "staged" ? staged : unstaged;
    if (rows.length === 0) return;
    selectedIndex = Math.min(rows.length - 1, Math.max(0, selectedIndex + delta));
    const row = rows[selectedIndex];
    if (section === "history" && "id" in row) void selectCommit(row.id);
    else if ("path" in row) selectFile(row.path, selectedSide);
  }

  function cycleTab(delta: number) {
    if (repos.length < 2 || !snapshot) return;
    const index = repos.indexOf(snapshot.path);
    const next = repos[(index + delta + repos.length) % repos.length];
    void openRepo(next);
  }

  function closeTab(path: string) {
    const next = repos.filter((repo) => repo !== path);
    repos = next;
    if (snapshot?.path === path && next[0]) void openRepo(next[0]);
  }

  function revealHead() {
    section = "history";
    const name = snapshot?.branch;
    const index = shownCommits.findIndex((commit) => (name ? commit.refs.includes(name) : commit.refs.length > 0));
    const top = Math.max(0, index) * rowCommit;
    historyTop = top;
    historyEl?.scrollTo({ top });
  }

  function dropCommits() {
    const chosen = shownCommits.filter((commit) => drops.includes(commit.id));
    if (chosen.length === 0) return;
    const oldest = chosen[chosen.length - 1];
    const parent = oldest.parents[0];
    if (!parent) {
      actionError = "The root commit cannot be dropped.";
      return;
    }
    drops = [];
    void mutate({ action: "rebaseInteractive", onto: parent, drop: chosen.map((commit) => commit.id) });
  }

  function toggleDrop(id: string) {
    drops = drops.includes(id) ? drops.filter((item) => item !== id) : [...drops, id];
  }

  async function keepPassphrase() {
    await invoke("set_passphrase", { user: passUser, secret: passphrase });
    passphrase = "";
  }

  async function saveWindow() {
    if (!inApp()) return;
    const win = getCurrentWindow();
    const factor = await win.scaleFactor();
    const size = await win.innerSize();
    const position = await win.outerPosition();
    settings.windowWidth = Math.round(size.width / factor);
    settings.windowHeight = Math.round(size.height / factor);
    settings.windowX = Math.round(position.x / factor);
    settings.windowY = Math.round(position.y / factor);
    settings = await invoke<Settings>("save_settings", { values: settings });
  }

  function onShortcut(event: KeyboardEvent) {
    const action = shortcut(event);
    if (!action) return;
    const editor = typing(event);
    if (editor && action !== "commit" && action !== "commitPush" && action !== "exit") return;
    if (action === "exit") {
      dialog = null;
      launchOpen = false;
      blameLines = null;
      return;
    }
    event.preventDefault();
    if (mode !== "live" && action !== "settings" && action !== "zoomIn" && action !== "zoomOut") return;
    if (action === "refresh") void refresh();
    else if (action === "commit") void submitCommit(false);
    else if (action === "commitPush") void submitCommit(true);
    else if (action === "up") moveSelection(-1);
    else if (action === "down") moveSelection(1);
    else if (action === "pull" || action === "quickPull") void doPull();
    else if (action === "push" || action === "quickPush") void doPush();
    else if (action === "fetch" || action === "quickFetch") void doFetch();
    else if (action === "branch") {
      draft = "";
      draftExtra = "";
      dialog = "branch";
    } else if (action === "stash") void mutate({ action: "stash", message: "" });
    else if (action === "open" || action === "newTab") {
      draft = "";
      dialog = "open";
    } else if (action === "closeTab" && snapshot) closeTab(snapshot.path);
    else if (action === "tabLeft") cycleTab(-1);
    else if (action === "tabRight") cycleTab(1);
    else if (action === "settings") dialog = "prefs";
    else if (action === "launch") launchEl?.focus();
    else if (action === "tag") {
      draft = "";
      draftExtra = snapshot?.branch ?? "HEAD";
      dialog = "tag";
    } else if (action === "clone" || action === "init") {
      draft = "";
      draftExtra = settings.cloneDirectory;
      dialog = "clone";
    } else if (action === "changes") section = "changes";
    else if (action === "commits") section = "history";
    else if (action === "reveal") revealHead();
    else if (action === "zoomIn") {
      settings.uiScale = Math.min(20, settings.uiScale + 1);
      void savePrefs(false);
    } else if (action === "zoomOut") {
      settings.uiScale = Math.max(11, settings.uiScale - 1);
      void savePrefs(false);
    } else if (action === "search") {
      section = "history";
      queueMicrotask(() => searchEl?.focus());
    } else if (action === "stageToggle" && selectedPath) {
      void runChange(selectedSide === "staged" ? "unstage_path" : "stage_path", { file: selectedPath });
    } else if (action === "stageAll") void runChange(selectedSide === "staged" ? "unstage_all" : "stage_all");
    else if (action === "discard" && selectedPath && selectedSide === "unstaged") {
      if (confirm(`Discard changes in ${selectedPath}?`)) void mutate({ action: "discard", file: selectedPath });
    } else if (action === "explorer" && snapshot) void openPath(snapshot.path);
    else if (action === "terminal") void invoke("open_terminal", { path: repoPath() });
    else if (action === "filterBranch") {
      allBranches = !allBranches;
      void loadContext();
    }
  }

  function progressLabel(value: InProgress) {
    if (value === "cherryPick") return "Cherry-pick";
    if (value === "rebase") return "Rebase";
    if (value === "revert") return "Revert";
    return "Merge";
  }

  $effect(() => {
    if (typeof document === "undefined") return;
    const root = document.documentElement;
    if (!settings.theme || settings.theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", settings.theme);
    root.style.setProperty("--ui-scale", `${settings.uiScale || 13}px`);
    root.style.setProperty("--row-file", settings.linesHeight === "spaced" ? "28px" : "22px");
    root.style.setProperty("--row-commit", settings.linesHeight === "spaced" ? "32px" : "24px");
    root.style.setProperty("--row-side", settings.linesHeight === "spaced" ? "28px" : "22px");
  });

  onMount(() => {
    if (!inApp()) return;
    window.addEventListener("keydown", onShortcut);
    let unlisten = () => {};
    let unmoved = () => {};
    let unresized = () => {};
    let placeTimer = 0;
    mode = "loading";
    selectedPath = null;
    invoke<Settings>("load_settings")
      .then(async (value) => {
        settings = { ...defaultSettings, ...value };
        if (value.windowWidth > 200 && value.windowHeight > 200) {
          const win = getCurrentWindow();
          await win.setPosition(new LogicalPosition(value.windowX, value.windowY));
          await win.setSize(new LogicalSize(value.windowWidth, value.windowHeight));
        }
      })
      .catch(() => {});
    invoke<UpdateNotice | null>("check_for_update")
      .then((value) => {
        updateNotice = value;
      })
      .catch(() => {});
    invoke<StatusSnapshot>("workspace_status")
      .then(async (value) => {
        applySnapshot(value);
        await loadContext();
        await loadDiff();
        await invoke("watch_repository", { path: repoPath() });
        unlisten = await listen("repo-changed", () => {
          if (!busy) void refresh();
        });
        try {
          const win = getCurrentWindow();
          const remember = () => {
            window.clearTimeout(placeTimer);
            placeTimer = window.setTimeout(() => void saveWindow(), 400);
          };
          unmoved = await win.onMoved(remember);
          unresized = await win.onResized(remember);
        } catch {
          /* placement events are optional */
        }
      })
      .catch((error: unknown) => {
        loadError = message(error);
        mode = "error";
      });
    return () => {
      window.removeEventListener("keydown", onShortcut);
      window.clearTimeout(placeTimer);
      unlisten();
      unmoved();
      unresized();
    };
  });
</script>

<div class="shell">
  <header class="chrome">
    <div class="tabs">
      <button class="tab" type="button" onclick={() => (dialog = "open")}>Open</button>
      {#each repos as repo (repo)}
        <div class="tab" class:active={snapshot?.path === repo}>
          <button class="file-select" type="button" onclick={() => openRepo(repo)}>{folderName(repo)}</button>
          <button class="text-button row-action" type="button" onclick={() => closeTab(repo)} aria-label="Close tab">×</button>
        </div>
      {:else}
        <button class="tab active" type="button">{tabLabel}</button>
      {/each}
    </div>
    <div class="toolbar">
      <button class="tool" type="button" disabled={busy} onclick={() => doFetch()}><span class="glyph">↓</span>Fetch</button>
      <button class="tool" type="button" disabled={busy} onclick={() => doPull()}><span class="glyph">↓</span>Pull</button>
      <button class="tool" type="button" disabled={busy} onclick={() => doPush()}><span class="glyph">↑</span>Push</button>
      <button class="tool" type="button" disabled={busy} onclick={() => mutate({ action: "stash", message: "" })}><span class="glyph">▣</span>Stash</button>
      <button class="tool" type="button" disabled={busy} onclick={() => mutate({ action: "stashPop" })}><span class="glyph">▢</span>Pop</button>
      <button class="tool" type="button" disabled={busy} onclick={() => { draft = ""; dialog = "flow"; }}><span class="glyph">⑂</span>Git Flow</button>
      <div class="spacer"></div>
      <div class="launch-wrap">
        <input
          class="launch"
          placeholder="Quick Launch"
          aria-label="Quick Launch"
          bind:this={launchEl}
          bind:value={launch}
          onfocus={() => (launchOpen = true)}
          onblur={() => setTimeout(() => (launchOpen = false), 150)}
          onkeydown={(event) => {
            if (event.key === "Enter" && matches[0]) {
              event.preventDefault();
              const run = matches[0].run;
              launch = "";
              launchOpen = false;
              void run();
            }
          }}
        />
        {#if launchOpen && matches.length > 0}
          <div class="launch-menu">
            {#each matches as item (item.label)}
              <button type="button" onclick={() => { launch = ""; launchOpen = false; void item.run(); }}>{item.label}</button>
            {/each}
          </div>
        {/if}
      </div>
      <button class="tool" type="button" onclick={() => invoke("open_terminal", { path: repoPath() })}><span class="glyph">▹</span>Terminal</button>
      <button class="tool" type="button" onclick={() => snapshot && openPath(snapshot.path)}><span class="glyph">▤</span>Explorer</button>
      <button class="tool" type="button" onclick={() => (dialog = "prefs")}><span class="glyph">⚙</span>Preferences</button>
    </div>
  </header>

  {#if busy}
    <div class="banner">
      <span>Working…</span>
      <button class="text-button" type="button" onclick={() => invoke("cancel_operation")}>Cancel</button>
    </div>
  {/if}
  {#if updateNotice}
    <div class="banner">
      <span>AweGit {updateNotice.appVersion} is available</span>
      {#if updateNotice.downloadUrl}
        <button class="text-button" type="button" onclick={() => openUrl(updateNotice?.downloadUrl ?? "")}>Download</button>
      {/if}
    </div>
  {/if}
  {#if mode === "live" && progress}
    <div class="banner">
      <span>{progressLabel(progress)} in progress</span>
      <button class="text-button" type="button" disabled={busy} onclick={() => mutate({ action: "abort" })}>Abort</button>
      <button class="text-button" type="button" disabled={busy} onclick={() => mutate({ action: "continue" })}>Continue</button>
      <button class="text-button" type="button" disabled={busy || progress === "merge"} onclick={() => mutate({ action: "skip" })}>Skip</button>
    </div>
  {/if}
  {#if actionError}
    <p class="banner error">{actionError}</p>
  {/if}

  <div class="body">
    <aside class="sidebar">
      <button class="side" class:selected={section === "changes"} type="button" onclick={() => (section = "changes")}>
        <span>Local Changes</span>
        <span class="count">{changeCount}</span>
      </button>
      <button class="side" class:selected={section === "history"} type="button" onclick={() => (section = "history")}>
        <span>{allBranches ? "All Commits" : "Current branch"}</span>
        {#if mode === "live"}<span class="count">{shownCommits.length}</span>{/if}
      </button>

      <div class="group">
        Local branches
        {#if mode === "live"}
          <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = ""; dialog = "branch"; }}>New</button>
        {/if}
      </div>
      {#if mode === "live" && refs}
        {#each visibleBranches as branch (branch.name)}
          <div class="side" class:current={branch.current}>
            <button class="file-select" type="button" onclick={() => mutate({ action: "checkout", name: branch.name })}>
              {#if branch.current}<span class="dot"></span>{/if}
              <span class="name">{branch.name}</span>
              {#if branch.ahead > 0}<span class="ahead">↑{branch.ahead}</span>{/if}
              {#if branch.behind > 0}<span class="ahead">↓{branch.behind}</span>{/if}
            </button>
            {#if !branch.current}
              <button class="text-button row-action" type="button" onclick={() => mutate({ action: "merge", name: branch.name, squash: false, noFf: settings.mergeNoFf, autostash: settings.mergeAutostash })}>Merge</button>
              <button class="text-button row-action" type="button" onclick={() => removeBranch(branch.name)}>Delete</button>
            {/if}
          </div>
        {/each}
        <div class="group">
          Remotes
          <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = ""; dialog = "remote"; }}>Add</button>
        </div>
        {#each refs.remotes as remote (remote.name)}
          <div class="side quiet">
            <span class="name">{remote.name}</span>
            <button class="text-button row-action" type="button" onclick={() => mutate({ action: "removeRemote", name: remote.name })}>Remove</button>
          </div>
          {#if remote.head}<div class="side nested quiet"><span class="name">HEAD</span></div>{/if}
          {#each remote.branches as branch (remote.name + branch)}
            <div class="side nested quiet"><span class="name">{branch}</span></div>
          {/each}
        {/each}
      {:else}
        <div class="side current">
          <span class="dot"></span>
          <span class="name">{branchLabel}</span>
          {#if mode === "sample"}<span class="ahead">↑5</span>{/if}
        </div>
        <div class="group">Remotes</div>
        <div class="side quiet"><span class="name">origin</span></div>
        <div class="side nested quiet"><span class="name">HEAD</span></div>
        <div class="side nested quiet"><span class="name">main</span></div>
      {/if}
      <div class="group">More</div>
      <div class="side quiet">
        <button class="file-select" type="button" onclick={() => (expanded = expanded === "tags" ? null : "tags")}>
          <span>Tags</span><span class="count">{visibleTags.length}</span>
        </button>
        <button class="text-button row-action" type="button" onclick={() => { draft = ""; draftExtra = "HEAD"; dialog = "tag"; }}>New</button>
      </div>
      {#if expanded === "tags"}
        {#each visibleTags as tag (tag.name)}
          <div class="side nested quiet">
            <span class="name">{tag.name}</span>
            <button class="text-button row-action" type="button" onclick={() => mutate({ action: "deleteTag", name: tag.name })}>Delete</button>
          </div>
        {/each}
      {/if}
      <button class="side quiet" type="button" onclick={() => (expanded = expanded === "stashes" ? null : "stashes")}>
        <span>Stashes</span><span class="count">{refs?.stashes.length ?? 0}</span>
      </button>
      {#if expanded === "stashes"}
        {#each refs?.stashes ?? [] as stash (stash.name)}
          <button class="side nested quiet" type="button" onclick={() => mutate({ action: "stashApply", name: stash.name })}>
            <span class="name">{stash.summary}</span>
          </button>
        {/each}
      {/if}
      <button class="side quiet" type="button" onclick={() => (expanded = expanded === "submodules" ? null : "submodules")}>
        <span>Submodules</span><span class="count">{refs?.submodules.length ?? 0}</span>
      </button>
      {#if expanded === "submodules"}
        {#each refs?.submodules ?? [] as row (row.path)}
          <div class="side nested quiet"><span class="name">{row.path}</span></div>
        {/each}
      {/if}
      <button class="side quiet" type="button" onclick={() => (expanded = expanded === "worktrees" ? null : "worktrees")}>
        <span>Worktrees</span><span class="count">{refs?.worktrees.length ?? (mode === "sample" ? 1 : 0)}</span>
      </button>
      {#if expanded === "worktrees"}
        {#each refs?.worktrees ?? [] as row (row.path)}
          <button class="side nested quiet" type="button" onclick={() => openRepo(row.path)}>
            <span class="name">{row.branch ?? folderName(row.path)}</span>
          </button>
        {/each}
      {/if}
    </aside>

    {#if section === "changes"}
      <section class="changes">
        <div class="status-pane" style:order={settings.swapPanes ? 2 : 1}>
        <div class="pane-head">
          <span>Staged</span>
          <span class="count">{staged.length}</span>
          <button class="text-button" type="button" disabled={mode !== "live" || busy || staged.length === 0} onclick={() => runChange("unstage_all")}>Unstage all</button>
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
        </div>

        <div class="status-pane" style:order={settings.swapPanes ? 1 : 2}>
        <div class="pane-head">
          <span>Unstaged</span>
          <span class="count">{mode === "loading" ? "…" : mode === "error" ? "—" : unstaged.length}</span>
          <button class="text-button" type="button" disabled={mode !== "live" || busy || unstaged.length === 0} onclick={() => runChange("stage_all")}>Stage all</button>
        </div>
        <div class="file-list" onscroll={(event) => (fileTop = (event.currentTarget as HTMLElement).scrollTop)}>
          {#if mode === "loading"}
            <p class="empty">Reading repository status…</p>
          {:else if mode === "error"}
            <p class="empty">{loadError}</p>
          {:else if unstaged.length === 0}
            <p class="empty">No unstaged changes</p>
          {:else}
            <div class="commit-window" style:height="{unstaged.length * rowFile}px">
              {#each fileWindow.rows as file, index (file.path)}
                <div class="virtual-row" style:top="{(fileWindow.start + index) * rowFile}px">
                  {@render fileRow(file, "unstaged")}
                </div>
              {/each}
            </div>
          {/if}
        </div>
        </div>

        {#snippet fileRow(file: Row, side: Side)}
          <div class="file" class:selected={selectedPath === file.path && (mode !== "live" || selectedSide === side)}>
            <button class="file-select" type="button" onclick={() => selectFile(file.path, side)}>
              <span class="badge {file.tone}">{file.letter}</span>
              <span class="file-name">{fileName(file.path)}</span>
              <span class="file-dir">{parentDir(file.path)}</span>
            </button>
            {#if mode === "live"}
              <button class="text-button row-action" type="button" disabled={busy} onclick={() => runChange(side === "staged" ? "unstage_path" : "stage_path", { file: file.path })}>{side === "staged" ? "Unstage" : "Stage"}</button>
              {#if side === "unstaged"}
                <button class="text-button row-action" type="button" disabled={busy} onclick={() => mutate({ action: "discard", file: file.path })}>Discard</button>
              {/if}
            {/if}
          </div>
        {/snippet}

        <form class="composer" style:order="3" onsubmit={(event) => { event.preventDefault(); void submitCommit(false); }}>
          <label class="field">
            <input placeholder="Summary" bind:value={summary} maxlength="200" aria-invalid={summaryTooLong} />
            <span class="counter" class:over={summaryTooLong}>{summary.length}/72</span>
          </label>
          <textarea placeholder="Description" rows="3" bind:value={description}></textarea>
          <div class="composer-row">
            <label class="check"><input type="checkbox" bind:checked={amend} /> Amend</label>
            <label class="check"><input type="checkbox" bind:checked={signOff} /> Sign-off</label>
            <button class="text-button" type="button" disabled={mode !== "live" || busy} onclick={() => suggestMessage()}>Suggest</button>
            <button class="text-button" type="button" disabled={!canCommit} onclick={() => submitCommit(true)}>Commit and push</button>
            <button class="commit" type="submit" disabled={!canCommit}>{busy ? "Working…" : "Commit"}</button>
          </div>
        </form>
      </section>
    {:else if mode !== "sample"}
      <section class="history" bind:this={historyEl} onscroll={(event) => (historyTop = (event.currentTarget as HTMLElement).scrollTop)}>
        <div class="pane-head">
          <input class="search" placeholder="Find commits" aria-label="Find commits" bind:this={searchEl} bind:value={commitQuery} />
          {#if historyFilter}
            <button class="text-button" type="button" onclick={() => { historyFilter = ""; void loadContext(); }}>All commits</button>
          {/if}
          <button class="text-button" type="button" disabled={drops.length === 0 || busy} onclick={dropCommits}>Drop</button>
          <button class="text-button" type="button" disabled={!selectedCommit || busy} onclick={() => { draft = "Squashed commits"; dialog = "squash"; }}>Squash</button>
        </div>
        {#if historyFilter}
          <div class="pane-head"><span>{historyFilter}</span></div>
        {/if}
        {#if shownCommits.length === 0}
          <p class="empty">No commits yet</p>
        {:else}
          <div class="commit-window" style:height="{shownCommits.length * rowCommit}px">
            {#each historyRows as commit, index (commit.id)}
              <div
                class="commit-row virtual"
                class:selected={selectedCommit === commit.id}
                style:top="{(historyStart + index) * rowCommit}px"
                role="button"
                tabindex="0"
                onclick={() => selectCommit(commit.id)}
                onkeydown={(event) => { if (event.key === "Enter") void selectCommit(commit.id); }}
              >
                <input type="checkbox" checked={drops.includes(commit.id)} aria-label="Drop commit" onclick={(event) => event.stopPropagation()} onchange={() => toggleDrop(commit.id)} />
                <span class="lane" style:margin-left="{commit.lane * 10}px"></span>
                <span class="avatar" title={commit.author}>{commit.author.slice(0, 1).toUpperCase()}</span>
                <span class="subject">{commit.summary}</span>
                <span class="badges">
                  {#each commit.refs as label (label)}<span class="ref">{label}</span>{/each}
                </span>
                <span class="meta">{commit.author}</span>
                <span class="meta sha">{commit.shortId}</span>
                <span class="meta">{whenLabel(commit)}</span>
              </div>
            {/each}
          </div>
        {/if}
      </section>
    {:else}
      <section class="history">
        {#each commits as commit (commit.id)}
          <button class="commit-row" class:selected={selectedPath === commit.id} type="button" onclick={() => (selectedPath = commit.id)}>
            <span class="lane" aria-hidden="true"></span>
            <span class="subject">{commit.summary}</span>
            <span class="badges">
              {#each commit.badges as label (label)}<span class="ref">{label}</span>{/each}
            </span>
            <span class="meta">{commit.author}</span>
            <span class="meta sha">{commit.id}</span>
            <span class="meta">{commit.when}</span>
          </button>
        {/each}
      </section>
    {/if}

    <section class="diff">
      {#if blameLines}
        <header class="diff-head">
          <span>Blame {selectedPath}</span>
          <button class="text-button" type="button" onclick={() => (blameLines = null)}>Close</button>
        </header>
        <div class="diff-body" onscroll={(event) => (blameTop = (event.currentTarget as HTMLElement).scrollTop)}>
          <div class="commit-window" style:height="{(blameLines?.length ?? 0) * 18}px">
            {#each blameWindow.rows as line, index (`${line.line}-${line.id}`)}
              <span class="virtual-row" style:top="{(blameWindow.start + index) * 18}px">{line.shortId} {line.author} {line.text}</span>
            {/each}
          </div>
        </div>
      {:else if mode === "live" && section === "changes" && selectedPath}
        <header class="diff-head">
          <span>{selectedPath}</span>
          <button class="text-button" type="button" onclick={() => showBlame(selectedPath!)}>Blame</button>
          <button class="text-button" type="button" onclick={() => showFileHistory(selectedPath!)}>History</button>
          {#if diff}
            {#each { length: diff.hunks } as _, index (index)}
              <button class="text-button" type="button" disabled={busy} onclick={() => mutate({ action: "stageHunk", file: selectedPath, index, unstage: selectedSide === "staged" })}>
                {selectedSide === "staged" ? "Unstage" : "Stage"} {index + 1}
              </button>
            {/each}
          {/if}
        </header>
        {#if diffError}
          <p class="diff-empty">{diffError}</p>
        {:else if !diff}
          <p class="diff-empty">Reading diff…</p>
        {:else if diff.binary}
          {#if preview}
            <img class="preview" alt="" src={preview.dataUrl} />
          {:else}
            <p class="diff-empty">Binary file</p>
          {/if}
        {:else if diff.lines.length === 0}
          <p class="diff-empty">No changes in this view</p>
        {:else if settings.diffStyle === "split"}
          <div class="diff-body split">
            {#each splitRows as row, index (index)}
              <div class="split-row">
                <span class:del={row.leftKind === "delete"} class:meta={row.leftKind === "meta"}>{row.left}</span>
                <span class:add={row.rightKind === "add"} class:meta={row.rightKind === "meta"}>{row.right}</span>
              </div>
            {/each}
          </div>
        {:else}
          <div class="diff-body" onscroll={(event) => (diffTop = (event.currentTarget as HTMLElement).scrollTop)}>
            <div class="commit-window" style:height="{diffLines.length * 18}px">
              {#each diffWindow.rows as line, index (`${diffWindow.start}-${index}`)}
                <span class="virtual-row {line.kind}" class:add={line.kind === "add"} class:del={line.kind === "delete"} class:hunk={line.kind === "hunk"} class:meta={line.kind === "meta"} style:top="{(diffWindow.start + index) * 18}px">
                  {#each highlight(line.text, selectedPath ?? "") as token, tokenIndex (`${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}
                  {#if line.stageAt != null && (line.kind === "add" || line.kind === "delete")}
                    <button class="text-button line-action" type="button" disabled={busy} onclick={() => stageOne(line)}>{selectedSide === "staged" ? "Unstage line" : "Stage line"}</button>
                  {/if}
                </span>
              {/each}
            </div>
            {#if diff.truncated}<span class="meta">Diff truncated.</span>{/if}
          </div>
        {/if}
      {:else if section === "changes" && selectedPath && selectedDiff.length > 0}
        <header class="diff-head">{selectedPath}</header>
        <pre class="diff-body">{#each selectedDiff as line, index (index)}<span class:add={line.startsWith("+") && !line.startsWith("+++")} class:del={line.startsWith("-") && !line.startsWith("---")} class:hunk={line.startsWith("@@")}>{line + "\n"}</span>{/each}</pre>
      {:else if mode === "live" && section === "history"}
        <header class="diff-head">
          <span>{commitsLive.find((row) => row.id === selectedCommit)?.summary ?? "History"}</span>
          {#if selectedCommit}
            <button class="text-button" type="button" disabled={busy} onclick={() => mutate({ action: "checkout", name: selectedCommit })}>Checkout</button>
            <button class="text-button" type="button" disabled={busy} onclick={() => mutate({ action: "cherryPick", rev: selectedCommit })}>Cherry-pick</button>
            <button class="text-button" type="button" disabled={busy} onclick={() => mutate({ action: "revert", rev: selectedCommit })}>Revert</button>
            <button class="text-button" type="button" onclick={() => { draft = selectedCommit ?? "HEAD"; dialog = "reset"; }}>Reset</button>
            <button class="text-button" type="button" disabled={busy} onclick={() => { draft = selectedCommit ?? ""; dialog = "squash"; }}>Squash to HEAD</button>
          {/if}
        </header>
        <div class="file-list staged-list">
          {#each commitFiles as file (file.path)}
            {@const mark = badge(file.kind)}
            <button class="file" class:selected={historyFile === file.path} type="button" onclick={() => pickCommitFile(file.path)}>
              <span class="badge {mark.tone}">{mark.letter}</span>
              <span class="file-name">{fileName(file.path)}</span>
              <span class="file-dir">{parentDir(file.path)}</span>
            </button>
          {/each}
        </div>
        {#if historyDiff && historyDiff.binary}
          <p class="diff-empty">Binary file</p>
        {:else if historyDiff && historyDiff.lines.length > 0}
          <div class="diff-body">
            {#each historyDiff.lines as line, index (index)}
              <span class:add={line.kind === "add"} class:del={line.kind === "delete"} class:hunk={line.kind === "hunk"} class:meta={line.kind === "meta"}>
                {#each highlight(line.text, historyFile ?? "") as token, tokenIndex (`h${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}
              </span>
            {/each}
          </div>
        {:else}
          <p class="diff-empty">Select a file in this commit to see changes</p>
        {/if}
      {:else if section === "history" && selectedPath}
        <header class="diff-head">{commits.find((row) => row.id === selectedPath)?.summary}</header>
        <p class="diff-empty">Select a file in this commit to see changes</p>
      {:else}
        <p class="diff-empty">Select a file to see changes</p>
      {/if}
    </section>
  </div>

  {#if dialog}
    <div class="scrim" role="presentation" onclick={() => (dialog = null)}>
      <div class="dialog" role="dialog" tabindex="-1" onclick={(event) => event.stopPropagation()} onkeydown={() => {}}>
      <form
        onsubmit={async (event) => {
          event.preventDefault();
          if (dialog === "branch") void mutate({ action: "createBranch", name: draft, start: draftExtra || null });
          else if (dialog === "clone") void mutate({ action: "clone", url: draft, destination: draftExtra });
          else if (dialog === "open") void openRepo(draft);
          else if (dialog === "command") void mutate({ action: "custom", command: draft });
          else if (dialog === "prefs") void savePrefs();
          else if (dialog === "tag") void mutate({ action: "tag", name: draft, rev: "HEAD", message: draftExtra });
          else if (dialog === "remote") void mutate({ action: "addRemote", name: draft, url: draftExtra });
          else if (dialog === "squash" && selectedCommit) void mutate({ action: "squash", from: selectedCommit, to: "HEAD", summary: draft });
          else if (dialog === "credential") {
            try {
              await invoke("approve_credential", {
                path: repoPath(),
                protocol: draft,
                host: draftExtra,
                username: draftUser,
                password: draftSecret,
              });
              draftSecret = "";
              dialog = null;
            } catch (error) {
              actionError = message(error);
            }
          }
        }}
      >
        {#if dialog === "prefs"}
          <h2>Preferences</h2>
          <label>Theme
            <select bind:value={settings.theme}>
              <option value="system">System</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </label>
          <label class="check"><input type="checkbox" bind:checked={settings.pullRebase} /> Pull with rebase</label>
          <label class="check"><input type="checkbox" bind:checked={settings.fetchPrune} /> Prune on fetch</label>
          <label class="check"><input type="checkbox" bind:checked={settings.swapPanes} /> Show unstaged above staged</label>
          <label class="check"><input type="checkbox" bind:checked={settings.showEntireFile} /> Show the whole file in diffs</label>
          <label class="check"><input type="checkbox" bind:checked={settings.mergeNoFf} /> Merge with --no-ff</label>
          <label class="check"><input type="checkbox" bind:checked={settings.mergeAutostash} /> Autostash before merge</label>
          <label class="check"><input type="checkbox" bind:checked={settings.signCommits} /> Sign commits with git</label>
          <label class="check"><input type="checkbox" bind:checked={settings.sslVerify} /> Verify SSL</label>
          <label class="check"><input type="checkbox" bind:checked={settings.dateRelative} /> Relative dates</label>
          <label class="check"><input type="checkbox" bind:checked={settings.date24h} /> 24-hour clock</label>
          <label>Date pattern <input bind:value={settings.dateFormat} placeholder="yyyy-MM-dd HH:mm" /></label>
          <label>Line height
            <select bind:value={settings.linesHeight}>
              <option value="compact">Compact</option>
              <option value="spaced">Spaced</option>
            </select>
          </label>
          <label>Diff
            <select bind:value={settings.diffStyle}>
              <option value="unified">Unified</option>
              <option value="split">Split</option>
            </select>
          </label>
          <label>Hidden refs <input bind:value={settings.hiddenRefs} placeholder="origin/backup, wip/" /></label>
          <label>Author name <input bind:value={settings.authorName} placeholder="uses git config when empty" /></label>
          <label>Author email <input bind:value={settings.authorEmail} /></label>
          <label>Proxy <input bind:value={settings.proxy} placeholder="http://host:port" /></label>
          <label>Proxy user <input bind:value={settings.proxyUser} /></label>
          <label>Proxy password <input type="password" bind:value={settings.proxyPassword} /></label>
          <label>CA file <input bind:value={settings.sslCaFile} placeholder="path to a CA bundle" /></label>
          <label>Clone directory <input bind:value={settings.cloneDirectory} /></label>
          <label>Terminal <input bind:value={settings.terminal} placeholder="empty opens cmd" /></label>
          <label>AI base URL <input bind:value={settings.aiBaseUrl} /></label>
          <label>AI model <input bind:value={settings.aiModel} /></label>
          <label>AI API key <input type="password" bind:value={settings.aiApiKey} placeholder="or set XAI_API_KEY" /></label>
          <label>Signing passphrase <input type="password" bind:value={passphrase} placeholder="kept in memory for this session" /></label>
          <label>Askpass user <input bind:value={passUser} /></label>
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => keepPassphrase()}>Use passphrase</button>
            <button class="text-button" type="button" onclick={() => { draft = "https"; draftExtra = ""; draftUser = ""; draftSecret = ""; dialog = "credential"; }}>Save HTTPS login</button>
            <button class="text-button" type="button" onclick={() => invoke("check_for_update").then((value) => (updateNotice = value as UpdateNotice | null))}>Check for updates</button>
            <button class="text-button" type="button" onclick={() => mutate({ action: "lfsPull" })}>LFS pull</button>
            <button class="text-button" type="button" onclick={() => mutate({ action: "lfsPush" })}>LFS push</button>
            <button class="text-button" type="button" onclick={() => (dialog = "command")}>Custom command</button>
            <button class="commit" type="submit">Save</button>
          </div>
        {:else if dialog === "reset"}
          <h2>Reset to {draft}</h2>
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => mutate({ action: "reset", rev: draft, mode: "soft" })}>Soft</button>
            <button class="text-button" type="button" onclick={() => mutate({ action: "reset", rev: draft, mode: "mixed" })}>Mixed</button>
            <button class="text-button" type="button" onclick={hardReset}>Hard</button>
          </div>
        {:else if dialog === "flow"}
          <h2>Git Flow</h2>
          <input placeholder="Feature name" bind:value={draft} />
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => mutate({ action: "gitFlowStart", name: draft })}>Start</button>
            <button class="text-button" type="button" onclick={() => mutate({ action: "gitFlowFinish", name: draft })}>Finish</button>
          </div>
        {:else if dialog === "branch"}
          <h2>New branch</h2>
          <input placeholder="Name" bind:value={draft} />
          <input placeholder="Start point (optional)" bind:value={draftExtra} />
          <button class="commit" type="submit">Create</button>
        {:else if dialog === "clone"}
          <h2>Clone</h2>
          <input placeholder="URL" bind:value={draft} />
          <input placeholder="Destination folder" bind:value={draftExtra} />
          <div class="composer-row">
            <button class="commit" type="submit">Clone</button>
            <button class="text-button" type="button" onclick={() => mutate({ action: "init", destination: draftExtra })}>Init</button>
          </div>
        {:else if dialog === "open"}
          <h2>Open repository</h2>
          <input placeholder="Path" bind:value={draft} />
          <div class="composer-row">
            <button class="commit" type="submit">Open</button>
            <button class="text-button" type="button" onclick={() => (dialog = "clone")}>Clone</button>
          </div>
        {:else if dialog === "tag"}
          <h2>New tag</h2>
          <input placeholder="Name" bind:value={draft} />
          <input placeholder="Message (empty makes a lightweight tag)" bind:value={draftExtra} />
          <button class="commit" type="submit">Create</button>
        {:else if dialog === "remote"}
          <h2>Remote</h2>
          <input placeholder="Name" bind:value={draft} />
          <input placeholder="URL" bind:value={draftExtra} />
          <div class="composer-row">
            <button class="commit" type="submit">Add</button>
            <button class="text-button" type="button" onclick={() => mutate({ action: "setRemoteUrl", name: draft, url: draftExtra })}>Set URL</button>
          </div>
        {:else if dialog === "squash"}
          <h2>Squash through {selectedCommit?.slice(0, 7)} into HEAD</h2>
          <input placeholder="Summary" bind:value={draft} />
          <button class="commit" type="submit">Squash</button>
        {:else if dialog === "credential"}
          <h2>HTTPS login</h2>
          <p class="empty">The password is sent to git credential approve and is not saved in settings.</p>
          <input placeholder="Protocol" bind:value={draft} />
          <input placeholder="Host" bind:value={draftExtra} />
          <input placeholder="Username" bind:value={draftUser} />
          <input placeholder="Password" type="password" bind:value={draftSecret} />
          <button class="commit" type="submit">Save in Git</button>
        {:else}
          <h2>Custom command</h2>
          <input placeholder="git status" bind:value={draft} />
          <button class="commit" type="submit">Run</button>
        {/if}
      </form>
      </div>
    </div>
  {/if}
</div>

<style>
  .shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--canvas);
    color: var(--text);
    font-size: var(--ui-scale, 13px);
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
    display: flex;
    align-items: center;
    gap: 4px;
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

  .launch-wrap {
    position: relative;
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

  .launch-menu {
    position: absolute;
    z-index: 2;
    top: 30px;
    left: 0;
    width: 220px;
    background: var(--canvas);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 4px;
  }

  .launch-menu button {
    display: block;
    width: 100%;
    height: 24px;
    text-align: left;
    padding: 0 8px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    border-radius: var(--radius);
  }

  .launch-menu button:hover {
    background: var(--hover);
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

  .banner {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 28px;
    padding: 0 12px;
    background: var(--selection);
  }

  .banner.error {
    color: var(--deleted);
    background: transparent;
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
    display: flex;
    align-items: center;
  }

  .current .name {
    font-weight: 600;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    flex: none;
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

  .status-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
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
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    padding: 0;
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

  .added { color: var(--added); }
  .deleted { color: var(--deleted); }
  .modified { color: var(--modified); }

  .file-name { flex: none; }

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
  textarea,
  .dialog input,
  .dialog select {
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

  .counter.over { color: var(--deleted); }

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

  .commit:disabled { opacity: 0.4; }

  .commit-window { position: relative; }

  .commit-row {
    height: var(--row-commit);
    padding: 0 8px;
  }

  .commit-row.virtual {
    position: absolute;
    left: 0;
  }

  .lane {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: var(--accent);
  }

  .subject {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badges { display: flex; gap: 4px; }

  .ref {
    height: 16px;
    padding: 0 4px;
    border-radius: 4px;
    background: var(--field);
    font-size: 11px;
    line-height: 16px;
  }

  .sha { font-family: var(--mono); }

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
    gap: 8px;
    padding: 0 12px;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 12px;
    font-weight: 400;
  }

  .diff-head span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  .diff-body > span,
  .diff-body .virtual-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    white-space: pre;
    min-height: 18px;
  }

  .diff-body span span {
    display: inline;
    padding: 0;
  }

  .virtual-row {
    position: absolute;
    left: 0;
    right: 0;
  }

  .diff-body .add { background: var(--diff-add); }
  .diff-body .del { background: var(--diff-del); }

  .diff-body .hunk,
  .diff-body .meta { color: var(--text-secondary); }

  .diff-empty { margin: auto; }

  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.28);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .dialog {
    width: 460px;
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 48px);
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px;
    border-radius: 12px;
    background: var(--canvas);
    border: 1px solid var(--line);
  }

  .dialog h2 {
    margin: 0 0 4px;
    font-size: 15px;
    font-weight: 600;
  }

  .dialog input,
  .dialog select {
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius);
    background: var(--field);
  }

  .dialog label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .avatar {
    width: 16px;
    height: 16px;
    flex: none;
    border-radius: 50%;
    background: var(--field);
    font-size: 10px;
    line-height: 16px;
    text-align: center;
  }

  .search {
    flex: 1;
    height: 22px;
    border: 0;
    border-radius: var(--radius);
    background: var(--field);
    color: inherit;
    padding: 0 8px;
  }

  .preview {
    display: block;
    max-width: calc(100% - 24px);
    max-height: 70vh;
    margin: 12px auto;
  }

  .split-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }

  .line-action {
    margin-left: auto;
  }

  .tok-word { color: #7030c0; }
  .tok-string { color: #1a7f37; }
  .tok-comment { color: var(--text-secondary); }
  .tok-number { color: #9a6700; }

  :global(:root[data-theme="dark"]) .tok-word { color: #c9a6ff; }
  :global(:root[data-theme="dark"]) .tok-string { color: #7ee787; }
  :global(:root[data-theme="dark"]) .tok-number { color: #e3b341; }
</style>
