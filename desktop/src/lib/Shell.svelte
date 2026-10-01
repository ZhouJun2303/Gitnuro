<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { commits, diffs, unstaged as sampleUnstaged } from "./sample";
  import { highlight } from "./highlight";
  import { shortcut, typing } from "./keys";
  import { branchGroup, formatWhen, gravatarUrl, splitDiff, windowSlice } from "./view";
  import {
    badge,
    type BlameLine,
    type CommandRecord,
    type CommitRow,
    type ConflictSides,
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
  type Dialog =
    | "branch"
    | "flow"
    | "clone"
    | "open"
    | "prefs"
    | "reset"
    | "command"
    | "tag"
    | "remote"
    | "squash"
    | "credential"
    | "fetch"
    | "pull"
    | "push"
    | "stash"
    | "rename"
    | "upstream"
    | "resolve"
    | "rebase"
    | "patch"
    | "about"
    | "submodule"
    | "worktree"
    | "output"
    | "workspace"
    | "reword"
    | null;
  type MenuItem = { label: string; run: () => void };
  type TreeEntry = { key: string; kind: "dir" | "file"; path: string; file?: Row };

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
    treeFiles: false,
    gravatar: false,
    signOff: false,
    signOffFormat: "Signed-off-by: %user <%email>",
    forceWithLease: true,
    aiEnabled: true,
    aiLanguage: "",
    aiMaxChars: 12000,
    aiPrompt: "",
    aiTemperature: 0,
    logDirectory: "",
    recent: [],
    workspaces: [],
    currentWorkspace: "",
    commands: [],
    flowMaster: "main",
    flowDevelop: "develop",
    flowFeature: "feature/",
    flowRelease: "release/",
    flowHotfix: "hotfix/",
    flowSupport: "support/",
    expandedGroups: [],
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
  let sideQuery = $state("");
  let fileQuery = $state("");
  let recentQuery = $state("");
  let historyDiffTop = $state(0);
  let picked = $state<string[]>([]);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let commandOutput = $state("");
  let conflict = $state<ConflictSides | null>(null);
  let logLimit = $state(500);
  let rebaseSteps = $state<{ verb: string; rev: string; summary: string; message: string }[]>([]);
  let comparePair = $state<[string, string] | null>(null);
  const drafts = new Map<string, { summary: string; description: string }>();

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
  const conflicted = $derived(
    [...(snapshot?.staged ?? []), ...(snapshot?.unstaged ?? [])].some((file) => file.kind === "conflict"),
  );
  const canCommit = $derived(
    summary.trim().length > 0 &&
      !summaryTooLong &&
      !busy &&
      !conflicted &&
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
  const shownUnstaged = $derived(unstaged.filter((file) => matchesQuery(file.path, fileQuery)));
  const shownStaged = $derived(staged.filter((file) => matchesQuery(file.path, fileQuery)));
  const unstagedTree = $derived(asTree(shownUnstaged));
  const fileWindow = $derived(windowSlice(unstagedTree, fileTop, rowFile));
  const diffLines = $derived(diff?.lines ?? []);
  const diffWindow = $derived(windowSlice(diffLines, diffTop, 18));
  const blameWindow = $derived(windowSlice(blameLines ?? [], blameTop, 18));
  const historyDiffWindow = $derived(windowSlice(historyDiff?.lines ?? [], historyDiffTop, 18));
  const splitRows = $derived(settings.diffStyle === "split" ? splitDiff(diffLines) : []);
  const visibleBranches = $derived(
    (refs?.branches ?? []).filter((branch) => (branch.current || !refHidden(branch.name)) && matchesQuery(branch.name, sideQuery)),
  );
  const visibleTags = $derived(
    (refs?.tags ?? []).filter((tag) => !refHidden(tag.name) && matchesQuery(tag.name, sideQuery)),
  );
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
      ...settings.commands.map((command) => ({ label: command.name, run: () => runCommand(command) })),
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
      commitsLive = await invoke<CommitRow[]>("commit_log", { path, limit: logLimit, all: allBranches });
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
      const value = await invoke<StatusSnapshot & { output?: string }>("mutate", { path: repoPath(), request });
      applySnapshot(value);
      const output = value.output ?? "";
      await loadContext();
      await loadDiff();
      if (output) {
        commandOutput = output;
        dialog = "output";
      } else {
        dialog = null;
      }
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
      forceWithLease: settings.forceWithLease,
    });
  }

  function matchesQuery(value: string, query: string) {
    const needle = query.trim().toLowerCase();
    return !needle || value.toLowerCase().includes(needle);
  }

  function openMenu(event: MouseEvent, items: MenuItem[]) {
    event.preventDefault();
    menu = { x: event.clientX, y: event.clientY, items };
  }

  function rememberRepo(path: string) {
    const recent = [path, ...settings.recent.filter((item) => item !== path)].slice(0, 12);
    const currentId = settings.currentWorkspace;
    const workspaces = settings.workspaces.map((workspace) => {
      if (!currentId || workspace.id !== currentId) return workspace;
      return {
        ...workspace,
        repositories: workspace.repositories.includes(path) ? workspace.repositories : [...workspace.repositories, path],
        openTabs: workspace.openTabs.includes(path) ? workspace.openTabs : [...workspace.openTabs, path],
      };
    });
    settings = { ...settings, recent, workspaces };
    void invoke("save_settings", { values: settings });
  }

  function quick(event: MouseEvent, dialogName: Dialog, run: () => void) {
    if (event.ctrlKey || event.metaKey) void run();
    else {
      draft = "";
      draftExtra = "";
      dialog = dialogName;
    }
  }

  async function submitCommit(pushAfter = false) {
    if (mode !== "live" || !canCommit) return;
    await runChange("commit_changes", { summary, description, amend, signOff });
    if (!actionError) {
      summary = "";
      description = "";
      amend = false;
      if (snapshot?.path) drafts.delete(snapshot.path);
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

  async function compareDrops() {
    if (drops.length !== 2) return;
    comparePair = [drops[0], drops[1]];
    try {
      commitFiles = await invoke<StatusFile[]>("compare_files", { path: repoPath(), from: drops[0], to: drops[1] });
      selectedCommit = drops[1];
      historyFile = commitFiles[0]?.path ?? null;
      if (historyFile) await pickCommitFile(historyFile);
    } catch (error) {
      actionError = message(error);
    }
  }

  async function selectCommit(id: string, event?: MouseEvent) {
    if (event?.shiftKey && selectedCommit) {
      const ids = shownCommits.map((commit) => commit.id);
      const start = ids.indexOf(selectedCommit);
      const end = ids.indexOf(id);
      if (start >= 0 && end >= 0) {
        const [from, to] = start < end ? [start, end] : [end, start];
        drops = ids.slice(from, to + 1);
      }
    }
    comparePair = null;
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
    if (comparePair) {
      historyFile = file;
      try {
        historyDiff = await invoke<FileDiff>("compare_file", {
          path: repoPath(),
          from: comparePair[0],
          to: comparePair[1],
          file,
          unified: settings.showEntireFile ? 100000 : 3,
        });
      } catch (error) {
        actionError = message(error);
      }
      return;
    }
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
    if (snapshot?.path) drafts.set(snapshot.path, { summary, description });
    busy = true;
    actionError = null;
    try {
      applySnapshot(await invoke<StatusSnapshot>("workspace_status", { path }));
      const saved = drafts.get(path);
      summary = saved?.summary ?? "";
      description = saved?.description ?? "";
      historyFilter = "";
      await loadContext();
      await loadDiff();
      await invoke("watch_repository", { path: repoPath() });
      rememberRepo(path);
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

  function groupOpen(name: string) {
    const group = branchGroup(name);
    if (!group || settings.expandedGroups.length === 0) return true;
    return settings.expandedGroups.includes(group);
  }

  function toggleGroup(group: string) {
    const groups = [...new Set(visibleBranches.map((branch) => branchGroup(branch.name)).filter(Boolean))];
    const current = settings.expandedGroups.length === 0 ? groups : [...settings.expandedGroups];
    const next = current.includes(group) ? current.filter((item) => item !== group) : [...current, group];
    settings = { ...settings, expandedGroups: next };
    void savePrefs(false);
  }

  function refHidden(name: string) {
    return (refs?.hiddenRefs ?? []).some((token) => name === token || (token.endsWith("/") && name.startsWith(token)));
  }

  function hideRef(name: string) {
    const names = [...(refs?.hiddenRefs ?? [])];
    if (!names.includes(name)) names.push(name);
    void mutate({ action: "setHidden", names });
  }

  function showOnly(name: string) {
    const names = (refs?.branches ?? []).map((branch) => branch.name).filter((item) => item !== name);
    void mutate({ action: "setHidden", names });
  }

  function showAllRefs() {
    void mutate({ action: "setHidden", names: [] });
  }

  function asTree(files: Row[]): TreeEntry[] {
    if (!settings.treeFiles) return files.map((file) => ({ key: file.path, kind: "file", path: file.path, file }));
    const groups = new Map<string, Row[]>();
    for (const file of files) {
      const dir = parentDir(file.path);
      const list = groups.get(dir) ?? [];
      list.push(file);
      groups.set(dir, list);
    }
    const entries: TreeEntry[] = [];
    for (const dir of [...groups.keys()].sort()) {
      if (dir) entries.push({ key: `dir:${dir}`, kind: "dir", path: dir });
      for (const file of groups.get(dir) ?? []) entries.push({ key: file.path, kind: "file", path: file.path, file });
    }
    return entries;
  }

  async function discardPicked() {
    const files = [...picked];
    for (const file of files) await mutate({ action: "discard", file });
    picked = [];
  }

  function stageFolder(dir: string, unstage: boolean) {
    const source = unstage ? shownStaged : shownUnstaged;
    const files = source
      .filter((file) => file.path === dir || file.path.startsWith(`${dir}/`) || file.path.startsWith(`${dir}\\`))
      .map((file) => file.path);
    if (files.length > 0) void mutate({ action: "stagePaths", files, unstage });
  }

  function runCommand(command: Pick<CommandRecord, "command">) {
    void mutate({
      action: "custom",
      command: command.command,
      repo: snapshot?.path ?? "",
      sha: selectedCommit ?? "",
      branch: snapshot?.branch ?? "",
      file: selectedPath ?? historyFile ?? "",
    });
  }

  function saveCommand() {
    const name = draft.trim();
    const command = draftUser.trim();
    if (!name || !command) return;
    const record: CommandRecord = { id: `cmd-${Date.now()}`, name, target: draftExtra || "repository", command };
    settings = { ...settings, commands: [...settings.commands, record] };
    draft = "";
    draftUser = "";
    void savePrefs(false);
  }

  function removeCommand(id: string) {
    settings = { ...settings, commands: settings.commands.filter((command) => command.id !== id) };
    void savePrefs(false);
  }

  function commandItems(target: string): MenuItem[] {
    return settings.commands.filter((command) => command.target === target).map((command) => ({ label: command.name, run: () => runCommand(command) }));
  }

  function copyText(text: string) {
    void navigator.clipboard?.writeText(text);
  }

  function fullPath(file: string) {
    const root = snapshot?.path ?? "";
    const sep = root.includes("\\") ? "\\" : "/";
    const relative = sep === "\\" ? file.replaceAll("/", "\\") : file.replaceAll("\\", "/");
    return root ? `${root.replace(/[\\/]+$/, "")}${sep}${relative}` : relative;
  }

  async function openResolve(file: string) {
    try {
      conflict = await invoke<ConflictSides>("conflict_sides", { path: repoPath(), file });
      draft = file;
      dialog = "resolve";
    } catch (error) {
      actionError = message(error);
    }
  }

  function openRebase() {
    if (!selectedCommit) return;
    const index = commitsLive.findIndex((commit) => commit.id === selectedCommit);
    const newer = index > 0 ? commitsLive.slice(0, index).slice().reverse() : [];
    rebaseSteps = newer.map((commit) => ({ verb: "pick", rev: commit.id, summary: commit.summary, message: commit.summary }));
    dialog = "rebase";
  }

  function moveStep(index: number, delta: number) {
    const next = index + delta;
    if (next < 0 || next >= rebaseSteps.length) return;
    const copy = [...rebaseSteps];
    const [item] = copy.splice(index, 1);
    copy.splice(next, 0, item);
    rebaseSteps = copy;
  }

  function selectFile(path: string, side: Side, event?: MouseEvent) {
    selectedPath = path;
    selectedSide = side;
    blameLines = null;
    if (event?.shiftKey || event?.ctrlKey || event?.metaKey) {
      picked = picked.includes(path) ? picked.filter((item) => item !== path) : [...picked, path];
    } else {
      picked = [path];
    }
    if (mode === "live") void loadDiff();
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
    if (snapshot?.path !== path) return;
    if (next[0]) void openRepo(next[0]);
    else {
      snapshot = null;
      refs = null;
      mode = "error";
      loadError = "";
    }
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
        settings = { ...defaultSettings, ...value, recent: value.recent ?? [], workspaces: value.workspaces ?? [], commands: value.commands ?? [], expandedGroups: value.expandedGroups ?? [] };
        signOff = settings.signOff;
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
      <button class="tab" type="button" onclick={() => { draft = ""; dialog = "workspace"; }}>Workspace</button>
      {#if settings.workspaces.length > 0}
        <select class="search" aria-label="Workspace" bind:value={settings.currentWorkspace} onchange={() => savePrefs(false)}>
          <option value="">All</option>
          {#each settings.workspaces as workspace (workspace.id)}
            <option value={workspace.id}>{workspace.name}</option>
          {/each}
        </select>
      {/if}
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
      <button class="tool" type="button" disabled={busy} onclick={(event) => quick(event, "fetch", doFetch)}><span class="glyph">↓</span>Fetch</button>
      <button class="tool" type="button" disabled={busy} onclick={(event) => quick(event, "pull", doPull)}><span class="glyph">↓</span>Pull</button>
      <button class="tool" type="button" disabled={busy} onclick={(event) => quick(event, "push", doPush)}><span class="glyph">↑</span>Push</button>
      <button class="tool" type="button" disabled={busy} onclick={(event) => quick(event, "stash", () => mutate({ action: "stash", message: "" }))}><span class="glyph">▣</span>Stash</button>
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
      <button class="tool" type="button" onclick={() => (dialog = "about")}><span class="glyph">i</span>About</button>
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
      <button class="text-button" type="button" disabled={busy || conflicted} onclick={() => mutate({ action: "continue" })}>Continue</button>
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
          <button class="text-button" type="button" onclick={showAllRefs}>Show all</button>
        {/if}
      </div>
      {#if mode === "live"}
        <input class="search" placeholder="Filter sidebar" aria-label="Filter sidebar" bind:value={sideQuery} />
      {/if}
      {#if mode === "live" && refs}
        {#each visibleBranches as branch, index (branch.name)}
          {#if branchGroup(branch.name) && branchGroup(branch.name) !== branchGroup(visibleBranches[index - 1]?.name ?? "")}
            <button class="side nested quiet" type="button" onclick={() => toggleGroup(branchGroup(branch.name))}>{branchGroup(branch.name)}</button>
          {/if}
          {#if groupOpen(branch.name)}
          <div
            class="side"
            class:current={branch.current}
            role="group"
            oncontextmenu={(event) => openMenu(event, [
              ...(!branch.current ? [
                { label: "Checkout", run: () => mutate({ action: "checkout", name: branch.name }) },
                { label: "Merge", run: () => mutate({ action: "merge", name: branch.name, squash: false, noFf: settings.mergeNoFf, autostash: settings.mergeAutostash }) },
                { label: "Rebase", run: () => mutate({ action: "rebase", onto: branch.name }) },
                { label: "Delete", run: () => removeBranch(branch.name) },
              ] : []),
              { label: "Rename", run: () => { draft = branch.name; draftExtra = branch.name; dialog = "rename"; } },
              { label: "Set upstream", run: () => { draft = branch.name; draftExtra = branch.upstream ?? ""; dialog = "upstream"; } },
              { label: "Copy name", run: () => copyText(branch.name) },
              { label: "Hide", run: () => hideRef(branch.name) },
              { label: "Show only this", run: () => showOnly(branch.name) },
              ...commandItems("branch"),
            ])}
          >
            <button class="file-select" type="button" onclick={() => mutate({ action: "checkout", name: branch.name })}>
              {#if branch.current}<span class="dot"></span>{/if}
              <span class="name">{branch.name}</span>
              {#if branch.ahead > 0}<span class="ahead">↑{branch.ahead}</span>{/if}
              {#if branch.behind > 0}<span class="ahead">↓{branch.behind}</span>{/if}
            </button>
          </div>
          {/if}
        {/each}
        <div class="group">
          Remotes
          <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = ""; dialog = "remote"; }}>Add</button>
          <button class="text-button" type="button" onclick={() => mutate({ action: "fetch", remote: null, prune: settings.fetchPrune, tags: false })}>Fetch all</button>
        </div>
        {#each refs.remotes as remote (remote.name)}
          {#if matchesQuery(remote.name, sideQuery)}
          <div class="side quiet">
            <span class="name">{remote.name}</span>
            <button class="text-button row-action" type="button" onclick={() => mutate({ action: "fetch", remote: remote.name, prune: settings.fetchPrune, tags: false })}>Fetch</button>
            <button class="text-button row-action" type="button" onclick={() => { draft = remote.name; draftExtra = remote.url ?? ""; dialog = "remote"; }}>URL</button>
            <button class="text-button row-action" type="button" onclick={() => mutate({ action: "removeRemote", name: remote.name })}>Remove</button>
          </div>
          {#if remote.head}<div class="side nested quiet"><span class="name">HEAD</span></div>{/if}
          {#each remote.branches as branch (remote.name + branch)}
            {#if matchesQuery(branch, sideQuery)}
            <div
              class="side nested quiet"
              role="group"
              oncontextmenu={(event) => openMenu(event, [
                { label: "Checkout", run: () => mutate({ action: "checkoutRemote", remote: remote.name, branch }) },
                { label: "Pull into current", run: () => mutate({ action: "pullRef", remote: remote.name, branch, rebase: settings.pullRebase, autostash: settings.mergeAutostash }) },
                { label: "Push current here", run: () => mutate({ action: "pushRef", remote: remote.name, branch, forceWithLease: settings.forceWithLease }) },
                { label: "Delete on remote", run: () => { if (confirm(`Delete ${remote.name}/${branch}?`)) void mutate({ action: "deleteRemoteBranch", remote: remote.name, branch }); } },
                { label: "Copy name", run: () => copyText(`${remote.name}/${branch}`) },
              ])}
            >
              <button class="file-select" type="button" onclick={() => mutate({ action: "checkoutRemote", remote: remote.name, branch })}>
                <span class="name">{branch}</span>
              </button>
            </div>
            {/if}
          {/each}
          {/if}
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
            <button class="file-select" type="button" onclick={() => mutate({ action: "checkout", name: tag.name })}><span class="name">{tag.name}</span></button>
            <button class="text-button row-action" type="button" onclick={() => mutate({ action: "deleteTag", name: tag.name })}>Delete</button>
          </div>
        {/each}
      {/if}
      <button class="side quiet" type="button" onclick={() => (expanded = expanded === "stashes" ? null : "stashes")}>
        <span>Stashes</span><span class="count">{refs?.stashes.length ?? 0}</span>
      </button>
      {#if expanded === "stashes"}
        {#each refs?.stashes ?? [] as stash (stash.name)}
          <div class="side nested quiet">
            <button class="file-select" type="button" onclick={() => mutate({ action: "stashApply", name: stash.name })}>
              <span class="name">{stash.summary}</span>
            </button>
            <button class="text-button row-action" type="button" onclick={() => mutate({ action: "stashDrop", name: stash.name })}>Drop</button>
          </div>
        {/each}
      {/if}
      <button class="side quiet" type="button" onclick={() => (expanded = expanded === "submodules" ? null : "submodules")}>
        <span>Submodules</span><span class="count">{refs?.submodules.length ?? 0}</span>
      </button>
      {#if expanded === "submodules"}
        <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = ""; dialog = "submodule"; }}>Add</button>
        {#each refs?.submodules ?? [] as row (row.path)}
          <div class="side nested quiet">
            <span class="name">{row.path}</span>
            {#if !row.ready}
              <button class="text-button row-action" type="button" onclick={() => mutate({ action: "submoduleInit", path: row.path })}>Initialize</button>
            {:else}
              <button class="text-button row-action" type="button" onclick={() => openRepo(fullPath(row.path))}>Open</button>
              <button class="text-button row-action" type="button" onclick={() => mutate({ action: "submoduleSync", path: row.path })}>Sync</button>
              <button class="text-button row-action" type="button" onclick={() => mutate({ action: "submoduleUpdate" })}>Update</button>
              <button class="text-button row-action" type="button" onclick={() => { if (confirm(`Delete submodule ${row.path}?`)) void mutate({ action: "submoduleRemove", path: row.path }); }}>Delete</button>
            {/if}
          </div>
        {/each}
      {/if}
      <button class="side quiet" type="button" onclick={() => (expanded = expanded === "worktrees" ? null : "worktrees")}>
        <span>Worktrees</span><span class="count">{refs?.worktrees.length ?? (mode === "sample" ? 1 : 0)}</span>
      </button>
      {#if expanded === "worktrees"}
        <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = ""; dialog = "worktree"; }}>Add</button>
        {#each refs?.worktrees ?? [] as row (row.path)}
          <div class="side nested quiet">
            <button class="file-select" type="button" onclick={() => openRepo(row.path)}>
              <span class="name">{row.branch ?? folderName(row.path)}</span>
            </button>
            <button class="text-button row-action" type="button" onclick={() => mutate({ action: "removeWorktree", path: row.path })}>Remove</button>
          </div>
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
        {#if shownStaged.length === 0}
          <p class="empty">{staged.length === 0 ? "No staged changes" : "No matching files"}</p>
        {:else}
          <div class="file-list staged-list">
            {#each asTree(shownStaged) as entry (entry.key)}
              {#if entry.kind === "dir"}
                <button class="text-button" type="button" onclick={() => stageFolder(entry.path, true)}>{entry.path}</button>
              {:else if entry.file}
                {@render fileRow(entry.file, "staged")}
              {/if}
            {/each}
          </div>
        {/if}
        </div>

        <div class="status-pane" style:order={settings.swapPanes ? 1 : 2}>
        <div class="pane-head">
          <span>Unstaged</span>
          <span class="count">{mode === "loading" ? "…" : mode === "error" ? "—" : shownUnstaged.length}</span>
          <input class="search" placeholder="Filter files" aria-label="Filter files" bind:value={fileQuery} />
          <button class="text-button" type="button" disabled={picked.length === 0 || busy} onclick={() => mutate({ action: "stagePaths", files: picked, unstage: false })}>Stage selected</button>
          <button class="text-button" type="button" disabled={picked.length === 0 || busy} onclick={() => { if (confirm(`Discard ${picked.length} files?`)) void discardPicked(); }}>Discard selected</button>
          <button class="text-button" type="button" disabled={mode !== "live" || busy || unstaged.length === 0} onclick={() => runChange("stage_all")}>Stage all</button>
        </div>
        <div class="file-list" onscroll={(event) => (fileTop = (event.currentTarget as HTMLElement).scrollTop)}>
          {#if mode === "loading"}
            <p class="empty">Reading repository status…</p>
          {:else if mode === "error"}
            <p class="empty">{loadError}</p>
          {:else if shownUnstaged.length === 0}
            <p class="empty">{unstaged.length === 0 ? "No unstaged changes" : "No matching files"}</p>
          {:else}
            <div class="commit-window" style:height="{unstagedTree.length * rowFile}px">
              {#each fileWindow.rows as entry, index (entry.key)}
                <div class="virtual-row" style:top="{(fileWindow.start + index) * rowFile}px">
                  {#if entry.kind === "dir"}
                    <button class="text-button" type="button" onclick={() => stageFolder(entry.path, false)}>{entry.path}</button>
                  {:else if entry.file}
                    {@render fileRow(entry.file, "unstaged")}
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        </div>
        </div>

        {#snippet fileRow(file: Row, side: Side)}
          <div
            class="file"
            class:selected={selectedPath === file.path && (mode !== "live" || selectedSide === side)}
            role="group"
            oncontextmenu={(event) => openMenu(event, [
              { label: side === "staged" ? "Unstage" : "Stage", run: () => runChange(side === "staged" ? "unstage_path" : "stage_path", { file: file.path }) },
              { label: "Discard", run: () => { if (confirm(`Discard changes in ${file.path}?`)) void mutate({ action: "discard", file: file.path }); } },
              { label: "Delete", run: () => { if (confirm(`Delete ${file.path}?`)) void mutate({ action: "delete", file: file.path }); } },
              { label: "Blame", run: () => showBlame(file.path) },
              { label: "History", run: () => showFileHistory(file.path) },
              { label: "Reveal", run: () => revealItemInDir(fullPath(file.path)) },
              { label: "Open file", run: () => openPath(fullPath(file.path)) },
              { label: "Copy path", run: () => copyText(fullPath(file.path)) },
              { label: "Copy relative path", run: () => copyText(file.path) },
              ...(file.letter === "C" ? [{ label: "Resolve", run: () => openResolve(file.path) }] : []),
              ...commandItems("file"),
            ])}
          >
            <button class="file-select" type="button" onclick={(event) => selectFile(file.path, side, event)}>
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
          <button class="text-button" type="button" disabled={drops.length !== 2} onclick={() => compareDrops()}>Compare</button>
          <button class="text-button" type="button" onclick={() => { logLimit += 200; void loadContext(); }}>Load more</button>
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
                onclick={(event) => selectCommit(commit.id, event)}
                onkeydown={(event) => { if (event.key === "Enter") void selectCommit(commit.id); }}
                oncontextmenu={(event) => openMenu(event, [
                  ...(commitsLive[0]?.id === commit.id ? [{ label: "Amend", run: () => { section = "changes"; amend = true; summary = commit.summary; } }] : []),
                  { label: "Edit message", run: () => { draft = commit.summary; draftExtra = commit.id; dialog = "reword"; } },
                  { label: "Checkout", run: () => mutate({ action: "checkout", name: commit.id }) },
                  { label: "Create branch", run: () => { draft = ""; draftExtra = commit.id; dialog = "branch"; } },
                  { label: "Create tag", run: () => { draft = ""; draftExtra = commit.id; dialog = "tag"; } },
                  { label: "Rebase interactive", run: () => { selectedCommit = commit.id; openRebase(); } },
                  { label: "Revert", run: () => mutate({ action: "revert", rev: commit.id }) },
                  { label: "Cherry-pick", run: () => mutate({ action: "cherryPick", rev: commit.id }) },
                  { label: "Reset", run: () => { draft = commit.id; dialog = "reset"; } },
                  ...commandItems("commit"),
                ])}
              >
                <input type="checkbox" checked={drops.includes(commit.id)} aria-label="Drop commit" onclick={(event) => event.stopPropagation()} onchange={() => toggleDrop(commit.id)} />
                <span class="lane" style:margin-left="{commit.lane * 10}px"></span>
                {#if settings.gravatar && commit.email}
                  <img class="avatar" alt="" src={gravatarUrl(commit.email)} />
                {:else}
                  <span class="avatar" title={commit.author}>{commit.author.slice(0, 1).toUpperCase()}</span>
                {/if}
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
              <button class="virtual-row" type="button" style:top="{(blameWindow.start + index) * 18}px" onclick={() => { section = "history"; void selectCommit(line.id); }}>{line.shortId} {line.author} {line.text}</button>
            {/each}
          </div>
        </div>
      {:else if mode === "live" && section === "changes" && selectedPath}
        <header class="diff-head">
          <span>{selectedPath}</span>
          <button class="text-button" type="button" onclick={() => showBlame(selectedPath!)}>Blame</button>
          <button class="text-button" type="button" onclick={() => showFileHistory(selectedPath!)}>History</button>
          {#if snapshot?.unstaged.some((file) => file.path === selectedPath && file.kind === "conflict") || snapshot?.staged.some((file) => file.path === selectedPath && file.kind === "conflict")}
            <button class="text-button" type="button" onclick={() => openResolve(selectedPath!)}>Resolve</button>
          {/if}
          {#if diff}
            {#each { length: diff.hunks } as _, index (index)}
              <button class="text-button" type="button" disabled={busy} onclick={() => mutate({ action: "stageHunk", file: selectedPath, index, unstage: selectedSide === "staged" })}>
                {selectedSide === "staged" ? "Unstage" : "Stage"} {index + 1}
              </button>
              {#if selectedSide === "unstaged"}
                <button class="text-button" type="button" disabled={busy} onclick={() => mutate({ action: "discardHunk", file: selectedPath, index })}>Discard {index + 1}</button>
              {/if}
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
            <button class="text-button" type="button" onclick={() => openPath(fullPath(selectedPath ?? ""))}>Open</button>
          {:else}
            <p class="diff-empty">Binary file</p>
            <button class="text-button" type="button" onclick={() => openPath(fullPath(selectedPath ?? ""))}>Open</button>
          {/if}
        {:else if diff.lines.length === 0}
          <p class="diff-empty">No changes in this view</p>
        {:else if settings.diffStyle === "split"}
          <div class="diff-body split">
            {#each splitRows as row, index (index)}
              <div class="split-row">
                <span class:del={row.leftKind === "delete"} class:meta={row.leftKind === "meta"}>{#each highlight(row.left, selectedPath ?? "") as token, tokenIndex (`l${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
                <span class:add={row.rightKind === "add"} class:meta={row.rightKind === "meta"}>{#each highlight(row.right, selectedPath ?? "") as token, tokenIndex (`r${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
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
                    {#if selectedSide === "unstaged" && line.workAt != null}
                      <button class="text-button line-action" type="button" disabled={busy} onclick={() => mutate({ action: "discardLine", file: selectedPath, text: lineText(line), addition: line.kind === "add", at: line.workAt })}>Discard line</button>
                    {/if}
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
            <button class="text-button" type="button" disabled={busy} onclick={() => { draft = commitsLive.find((row) => row.id === selectedCommit)?.summary ?? ""; draftExtra = selectedCommit ?? ""; dialog = "reword"; }}>Reword</button>
            <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = selectedCommit ?? ""; dialog = "branch"; }}>Branch</button>
            <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = selectedCommit ?? "HEAD"; dialog = "tag"; }}>Tag</button>
            <button class="text-button" type="button" disabled={busy} onclick={openRebase}>Rebase interactive</button>
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
          <div class="diff-body" onscroll={(event) => (historyDiffTop = (event.currentTarget as HTMLElement).scrollTop)}>
            <div class="commit-window" style:height="{historyDiff.lines.length * 18}px">
              {#each historyDiffWindow.rows as line, index (`hd-${historyDiffWindow.start}-${index}`)}
                <span class="virtual-row" class:add={line.kind === "add"} class:del={line.kind === "delete"} class:hunk={line.kind === "hunk"} class:meta={line.kind === "meta"} style:top="{(historyDiffWindow.start + index) * 18}px">
                  {#each highlight(line.text, historyFile ?? "") as token, tokenIndex (`h${historyDiffWindow.start}-${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}
                </span>
              {/each}
            </div>
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

  {#if mode === "error"}
    <section class="changes">
      <h2>Open a repository</h2>
      <p class="empty">{loadError || "Open, clone, or create a repository."}</p>
      <input class="search" placeholder="Search recent repositories" aria-label="Search recent repositories" bind:value={recentQuery} />
      {#each settings.recent.filter((path) => matchesQuery(path, recentQuery)) as path (path)}
        <button class="side" type="button" onclick={() => openRepo(path)}>{path}</button>
      {/each}
      <div class="composer-row">
        <button class="commit" type="button" onclick={() => (dialog = "open")}>Open</button>
        <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = settings.cloneDirectory; dialog = "clone"; }}>Clone</button>
        <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = settings.cloneDirectory; dialog = "clone"; }}>New repository</button>
      </div>
    </section>
  {/if}
  {#if menu}
    <div class="menu" style:left="{menu.x}px" style:top="{menu.y}px" role="menu">
      {#each menu.items as item (item.label)}
        <button type="button" onclick={() => { menu = null; void item.run(); }}>{item.label}</button>
      {/each}
    </div>
    <button class="scrim menu-dismiss" type="button" aria-label="Close menu" onclick={() => (menu = null)}></button>
  {/if}
  {#if dialog}
    <div class="scrim" role="presentation" onclick={() => (dialog = null)}>
      <div class="dialog" role="dialog" tabindex="-1" onclick={(event) => event.stopPropagation()} onkeydown={() => {}}>
      <form
        onsubmit={async (event) => {
          event.preventDefault();
          if (dialog === "branch") void mutate({ action: "createBranch", name: draft, start: draftExtra || null });
          else if (dialog === "clone") void mutate({ action: "clone", url: draft, destination: draftExtra });
          else if (dialog === "open") void openRepo(draft);
          else if (dialog === "command") void runCommand({ command: draftUser || draft });
          else if (dialog === "prefs") void savePrefs();
          else if (dialog === "tag") void mutate({ action: "tag", name: draft, rev: draftExtra || "HEAD", message: draftUser });
          else if (dialog === "remote") void mutate({ action: "addRemote", name: draft, url: draftExtra });
          else if (dialog === "fetch") void mutate({ action: "fetch", remote: draft || null, prune: settings.fetchPrune, tags: draftExtra === "tags" });
          else if (dialog === "pull") void mutate({ action: "pullRef", remote: draft || (refs?.remotes[0]?.name ?? "origin"), branch: draftExtra || snapshot?.branch || "HEAD", rebase: settings.pullRebase, autostash: settings.mergeAutostash });
          else if (dialog === "push") void mutate({ action: "push", remote: draft || null, setUpstream: true, tags: draftExtra === "tags", forceWithLease: settings.forceWithLease });
          else if (dialog === "stash") void mutate({ action: "stash", message: draft });
          else if (dialog === "rename") void mutate({ action: "renameBranch", name: draft, to: draftExtra });
          else if (dialog === "upstream") void mutate({ action: "setUpstream", branch: draft, upstream: draftExtra });
          else if (dialog === "reword") void mutate({ action: "reword", rev: draftExtra || "HEAD", summary: draft });
          else if (dialog === "rebase" && selectedCommit) void mutate({ action: "rebaseInteractive", onto: selectedCommit, drop: [], steps: rebaseSteps.map((step) => ({ verb: step.verb, rev: step.rev, message: step.verb === "reword" || step.verb === "squash" ? step.message : "" })) });
          else if (dialog === "patch") void mutate({ action: "applyPatch", patch: draft });
          else if (dialog === "submodule") void mutate({ action: "submoduleAdd", url: draft, path: draftExtra });
          else if (dialog === "worktree") void mutate({ action: "addWorktree", path: draft, branch: draftExtra });
          else if (dialog === "workspace") {
            const id = `ws-${Date.now()}`;
            settings = { ...settings, currentWorkspace: id, workspaces: [...settings.workspaces, { id, name: draft || "Workspace", repositories: repos, openTabs: repos, selectedTab: 0 }] };
            void savePrefs();
          }
          else if (dialog === "resolve") void mutate({ action: "resolve", file: draft, side: draftExtra || "mark" });
          else if (dialog === "squash" && selectedCommit) {
            const chosen = shownCommits.filter((commit) => drops.includes(commit.id));
            const from = chosen.length >= 2 ? chosen[chosen.length - 1].id : selectedCommit;
            const to = chosen.length >= 2 ? chosen[0].id : "HEAD";
            void mutate({ action: "squash", from, to, summary: draft });
          }
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
          <label class="check"><input type="checkbox" bind:checked={settings.signOff} onchange={() => (signOff = settings.signOff)} /> Sign-off by default</label>
          <label>Sign-off format <input bind:value={settings.signOffFormat} /></label>
          <label class="check"><input type="checkbox" bind:checked={settings.forceWithLease} /> Push with --force-with-lease</label>
          <label class="check"><input type="checkbox" bind:checked={settings.treeFiles} /> Group changed files by folder</label>
          <label class="check"><input type="checkbox" bind:checked={settings.gravatar} /> Gravatar avatars</label>
          <label class="check"><input type="checkbox" bind:checked={settings.sslVerify} /> Verify SSL</label>
          {#if !settings.sslVerify}<p class="empty">SSL verification is off. Connections can be intercepted.</p>{/if}
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
          <p class="empty">Hidden branches are stored in this repository. Use Hide, Show only this, or Show all in the sidebar. The current branch stays visible.</p>
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
          <label class="check"><input type="checkbox" bind:checked={settings.aiEnabled} /> AI commit messages</label>
          <label>AI language <input bind:value={settings.aiLanguage} placeholder="English" /></label>
          <label>AI max characters <input type="number" bind:value={settings.aiMaxChars} /></label>
          <label>AI prompt <input bind:value={settings.aiPrompt} placeholder={'{diff} {files} {branch} {recent_commits} {language}'} /></label>
          <label>AI temperature <input type="number" step="0.1" bind:value={settings.aiTemperature} /></label>
          <label>AI API key <input type="password" bind:value={settings.aiApiKey} placeholder="or set XAI_API_KEY" /></label>
          <label>Log directory <input bind:value={settings.logDirectory} placeholder="records action names only" /></label>
          <label>Signing passphrase <input type="password" bind:value={passphrase} placeholder="kept in memory for this session" /></label>
          <label>Askpass user <input bind:value={passUser} /></label>
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => keepPassphrase()}>Use passphrase</button>
            <button class="text-button" type="button" onclick={() => { draft = "https"; draftExtra = ""; draftUser = ""; draftSecret = ""; dialog = "credential"; }}>Save HTTPS login</button>
            <button class="text-button" type="button" onclick={() => invoke("check_for_update").then((value) => (updateNotice = value as UpdateNotice | null))}>Check for updates</button>
            <button class="text-button" type="button" onclick={() => mutate({ action: "lfsPull" })}>LFS pull</button>
            <button class="text-button" type="button" onclick={() => mutate({ action: "lfsPush" })}>LFS push</button>
            <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = "repository"; draftUser = ""; dialog = "command"; }}>Custom command</button>
            <button class="text-button" type="button" onclick={() => (dialog = "patch")}>Apply patch</button>
            <button class="text-button" type="button" onclick={() => invoke("set_repo_author", { path: repoPath(), name: settings.authorName, email: settings.authorEmail })}>Save author in this repo</button>
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
          <input placeholder="master branch" bind:value={settings.flowMaster} />
          <input placeholder="develop branch" bind:value={settings.flowDevelop} />
          <input placeholder="feature prefix" bind:value={settings.flowFeature} />
          <input placeholder="release prefix" bind:value={settings.flowRelease} />
          <input placeholder="hotfix prefix" bind:value={settings.flowHotfix} />
          <input placeholder="support prefix" bind:value={settings.flowSupport} />
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => mutate({ action: "gitFlowInit", master: settings.flowMaster, develop: settings.flowDevelop, feature: settings.flowFeature, release: settings.flowRelease, hotfix: settings.flowHotfix, support: settings.flowSupport })}>Init</button>
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
          <input placeholder="Revision" bind:value={draftExtra} />
          <input placeholder="Message (empty makes a lightweight tag)" bind:value={draftUser} />
          <button class="commit" type="submit">Create</button>
        {:else if dialog === "fetch"}
          <h2>Fetch</h2>
          <input placeholder="Remote (empty fetches all)" bind:value={draft} />
          <label class="check"><input type="checkbox" checked={draftExtra === "tags"} onchange={(event) => (draftExtra = event.currentTarget.checked ? "tags" : "")} /> Tags</label>
          <button class="commit" type="submit">Fetch</button>
        {:else if dialog === "pull"}
          <h2>Pull</h2>
          <input placeholder="Remote" bind:value={draft} />
          <input placeholder="Branch" bind:value={draftExtra} />
          <button class="commit" type="submit">Pull</button>
        {:else if dialog === "push"}
          <h2>Push</h2>
          <input placeholder="Remote" bind:value={draft} />
          <label class="check"><input type="checkbox" checked={draftExtra === "tags"} onchange={(event) => (draftExtra = event.currentTarget.checked ? "tags" : "")} /> Tags</label>
          <p class="empty">{settings.forceWithLease ? "Uses --force-with-lease." : "Does not force."}</p>
          <button class="commit" type="submit">Push</button>
        {:else if dialog === "stash"}
          <h2>Stash</h2>
          <input placeholder="Message (optional)" bind:value={draft} />
          <button class="commit" type="submit">Stash</button>
        {:else if dialog === "rename"}
          <h2>Rename branch</h2>
          <input placeholder="Current name" bind:value={draft} />
          <input placeholder="New name" bind:value={draftExtra} />
          <button class="commit" type="submit">Rename</button>
        {:else if dialog === "upstream"}
          <h2>Upstream for {draft}</h2>
          <input placeholder="origin/main (empty clears it)" bind:value={draftExtra} />
          <button class="commit" type="submit">Save</button>
        {:else if dialog === "reword"}
          <h2>Reword {draftExtra.slice(0, 7)}</h2>
          <input placeholder="Summary" bind:value={draft} />
          <button class="commit" type="submit">Reword</button>
        {:else if dialog === "rebase"}
          <h2>Interactive rebase onto {selectedCommit?.slice(0, 7)}</h2>
          {#each rebaseSteps as step, index (step.rev)}
            <div class="composer-row">
              <select bind:value={step.verb}>
                <option value="pick">pick</option>
                <option value="reword">reword</option>
                <option value="squash">squash</option>
                <option value="fixup">fixup</option>
                <option value="drop">drop</option>
              </select>
              <span>{step.summary}</span>
              <button class="text-button" type="button" onclick={() => moveStep(index, -1)}>Up</button>
              <button class="text-button" type="button" onclick={() => moveStep(index, 1)}>Down</button>
            </div>
            {#if step.verb === "reword" || step.verb === "squash"}
              <input placeholder="Message" bind:value={step.message} />
            {/if}
          {/each}
          <button class="commit" type="submit">Start rebase</button>
        {:else if dialog === "patch"}
          <h2>Apply patch</h2>
          <textarea rows="8" placeholder="Patch text" bind:value={draft}></textarea>
          <button class="commit" type="submit">Apply</button>
        {:else if dialog === "submodule"}
          <h2>Add submodule</h2>
          <input placeholder="URL" bind:value={draft} />
          <input placeholder="Path" bind:value={draftExtra} />
          <button class="commit" type="submit">Add</button>
        {:else if dialog === "worktree"}
          <h2>Add worktree</h2>
          <input placeholder="Path" bind:value={draft} />
          <input placeholder="Branch" bind:value={draftExtra} />
          <button class="commit" type="submit">Add</button>
        {:else if dialog === "workspace"}
          <h2>New workspace</h2>
          <input placeholder="Name" bind:value={draft} />
          <button class="commit" type="submit">Create</button>
        {:else if dialog === "resolve"}
          <h2>Resolve {draft}</h2>
          <p class="empty">Ours and theirs are the two stages. Mark resolved keeps the working tree file.</p>
          <textarea rows="4" readonly value={conflict?.ours ?? ""}></textarea>
          <textarea rows="4" readonly value={conflict?.theirs ?? ""}></textarea>
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => { draftExtra = "ours"; }}>Ours</button>
            <button class="text-button" type="button" onclick={() => { draftExtra = "theirs"; }}>Theirs</button>
            <button class="text-button" type="button" onclick={() => { draftExtra = "both"; }}>Keep both</button>
            <button class="commit" type="button" onclick={() => { draftExtra = draftExtra || "mark"; }}>Use {draftExtra || "mark"}</button>
          </div>
          <button class="commit" type="submit">Apply</button>
        {:else if dialog === "about"}
          <h2>AweGit 2.0.0</h2>
          <p class="empty">com.zhoujun.awegit. Windows builds include MinGit 2.56.0. macOS uses the system Git.</p>
          <button class="commit" type="button" onclick={() => (dialog = null)}>Close</button>
        {:else if dialog === "output"}
          <h2>Command output</h2>
          <pre class="diff-body">{commandOutput}</pre>
          <button class="commit" type="button" onclick={() => (dialog = null)}>Close</button>
        {:else if dialog === "remote"}
          <h2>Remote</h2>
          <input placeholder="Name" bind:value={draft} />
          <input placeholder="URL" bind:value={draftExtra} />
          <div class="composer-row">
            <button class="commit" type="submit">Add</button>
            <button class="text-button" type="button" onclick={() => mutate({ action: "setRemoteUrl", name: draft, url: draftExtra })}>Set URL</button>
          </div>
        {:else if dialog === "squash"}
          <h2>Squash</h2>
          <p class="empty">Uses the selected commit through HEAD, or the consecutive commits you checked. Later commits are replayed. The range must contain no merge.</p>
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
          <h2>Custom commands</h2>
          {#each settings.commands as command (command.id)}
            <div class="composer-row">
              <span>{command.name}</span>
              <span class="meta">{command.target}</span>
              <button class="text-button" type="button" onclick={() => runCommand(command)}>Run</button>
              <button class="text-button" type="button" onclick={() => removeCommand(command.id)}>Delete</button>
            </div>
          {/each}
          <input placeholder="Name" bind:value={draft} />
          <select bind:value={draftExtra}>
            <option value="repository">Repository</option>
            <option value="commit">Commit</option>
            <option value="branch">Branch</option>
            <option value="file">File</option>
          </select>
          <input placeholder={"Template. Placeholders: ${repo} ${sha} ${branch} ${file}"} bind:value={draftUser} />
          <div class="composer-row">
            <button class="text-button" type="button" onclick={saveCommand}>Save</button>
            <button class="commit" type="submit">Run</button>
          </div>
        {/if}
      </form>
      </div>
    </div>
  {/if}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 30;
    min-width: 180px;
    padding: 4px;
    background: var(--canvas);
    border: 1px solid var(--separator);
    border-radius: 8px;
    display: flex;
    flex-direction: column;
  }
  .menu button, .menu-dismiss { font: inherit; }
  .menu-dismiss { position: fixed; inset: 0; z-index: 29; background: transparent; border: 0; }
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
