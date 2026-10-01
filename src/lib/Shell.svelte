<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { commits, diffs, sampleImages, sampleNotices, samplePulls, sampleReflog, unstaged as sampleUnstaged } from "./sample";
  import { highlight } from "./highlight";
  import { resolveLocale, translate } from "./i18n";
  import { shortcut, shortcutLabel, typing } from "./keys";
  import { branchGroup, commitGraph, formatWhen, gravatarUrl, laneColors, numberedDiff, splitDiff, windowSlice } from "./view";
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
    | "merge"
    | "repo"
    | "continue"
    | null;
  type MenuItem = { label: string; run?: () => void; disabled?: boolean; shortcut?: string; children?: MenuItem[]; sep?: boolean };
  type ConfirmAsk = { title: string; body: string; run: () => void };
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
    dateFormat: "dd MMM yyyy",
    date24h: true,
    hiddenRefs: "",
    authorName: "",
    authorEmail: "",
    sslVerify: true,
    sslCaFile: "",
    proxyUser: "",
    proxyPassword: "",
    proxyEnabled: false,
    proxyType: "http",
    proxyHost: "",
    proxyPort: 0,
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
    locale: "system",
    diffTool: "",
    mergeTool: "",
    githubToken: "",
    gitlabToken: "",
    gitlabHost: "",
  };

  let section = $state<"changes" | "history">("changes");
  let selectedPath = $state<string | null>(sampleUnstaged[0]?.path ?? null);
  let summary = $state("");
  let description = $state("");
  let amend = $state(false);
  let signOff = $state(false);
  let skipHooks = $state(false);
  let repoSignOff = $state(false);
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
  let submenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let confirmAsk = $state<ConfirmAsk | null>(null);
  let imageMode = $state<"side" | "swipe" | "onion" | "pixel">("side");
  let imagePos = $state(50);
  let imageOld = $state<string | null>(null);
  let imageNew = $state<string | null>(null);
  let pixelUrl = $state("");
  let reflogOn = $state(false);
  let treeOn = $state(false);
  let reflogRows = $state<{ id: string; shortId: string; selector: string; summary: string }[]>([]);
  let treePaths = $state<string[]>([]);
  let treeText = $state("");
  let notices = $state<{ id: string; title: string; url: string; unread: boolean }[]>([]);
  let pulls = $state<{ title: string; author: string; branch: string; url: string }[]>([]);
  let forgeOpen = $state<"notes" | "pulls" | null>(null);
  let repoStats = $state<{ branch: string; ahead: number; behind: number; commits: number; branches: number; lastSummary: string } | null>(null);
  let rebaseConflictFiles = $state<string[]>([]);
  let updateRefs = $state(false);
  let lfsAvailable = $state(false);
  let prefTab = $state<"appearance" | "git" | "diff" | "network" | "ai" | "account">("appearance");
  let dragStep = $state<number | null>(null);
  let rebasePicked = $state<string[]>([]);
  let recentStats = $state<Record<string, { branch: string; ahead: number; behind: number; lastSummary: string }>>({});
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
  const tabLabel = $derived(mode === "live" && snapshot ? folderName(snapshot.path) : "AweGit");
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
  const activeLocale = $derived(resolveLocale(settings.locale));
  function tr(key: string, vars: Record<string, string> = {}) {
    return translate(activeLocale, key, vars);
  }
  const sampleRows = $derived([
    { id: "stash@{0}", summary: "WIP before the palette", author: "", when: "", badges: ["stash"], parents: [commits[0]?.id ?? ""], files: [] as (typeof commits)[number]["files"] },
    ...commits,
  ]);
  const sampleGraph = $derived(commitGraph(sampleRows.map((commit) => ({ id: commit.id, parents: commit.parents }))));
  const sampleTree = $derived([...new Set(commits.flatMap((commit) => commit.files.map((file) => file.path)))]);
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
  const historyItems = $derived.by(() => {
    const rows = [...shownCommits];
    for (const stash of refs?.stashes ?? []) {
      const parent = stash.parent;
      if (!parent) continue;
      const extra: CommitRow = { id: stash.id || stash.name, shortId: stash.name, summary: stash.summary, author: "", when: "", parents: [parent], refs: ["stash"], lane: 0, at: 0, email: "" };
      const at = rows.findIndex((row) => row.id === parent || row.id.startsWith(parent));
      if (at >= 0) rows.splice(at, 0, extra);
    }
    return rows;
  });
  const historyWindow = $derived(windowSlice(historyItems, historyTop, rowCommit));
  const historyStart = $derived(historyWindow.start);
  const historyRows = $derived(historyWindow.rows);
  const shownUnstaged = $derived(unstaged.filter((file) => matchesQuery(file.path, fileQuery)));
  const shownStaged = $derived(staged.filter((file) => matchesQuery(file.path, fileQuery)));
  const unstagedTree = $derived(asTree(shownUnstaged));
  const fileWindow = $derived(windowSlice(unstagedTree, fileTop, rowFile));
  const diffLines = $derived(diff?.lines ?? []);
  const shownDiffLines = $derived(numberedDiff(diffLines));
  const liveGraph = $derived(commitGraph(historyItems.map((commit) => ({ id: commit.id, parents: commit.parents ?? [] }))));
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
    const reposToOpen = [...new Set([...repos, ...settings.recent])];
    const items = [
      { label: "Fetch", run: () => doFetch() },
      { label: "Pull", run: () => doPull() },
      { label: "Push", run: () => doPush() },
      { label: "Stash", run: () => mutate({ action: "stash", message: "" }) },
      { label: "Pop stash", run: () => mutate({ action: "stashPop" }) },
      { label: "Refresh", run: () => refresh() },
      { label: "Stage all", run: () => runChange("stage_all") },
      { label: "Commit", run: () => submitCommit(false) },
      { label: "Settings", run: () => { dialog = "prefs"; } },
      { label: "Terminal", run: () => invoke("open_terminal", { path: repoPath() }) },
      { label: "Explorer", run: () => { if (snapshot) void openPath(snapshot.path); } },
      { label: "Git Flow", run: () => { draft = ""; dialog = "flow"; } },
      { label: "About", run: () => { dialog = "about"; } },
      ...settings.commands.map((command) => ({ label: command.name, run: () => runCommand(command) })),
      ...reposToOpen.map((path) => ({ label: `Repository ${path}`, run: () => openRepo(path) })),
      ...(refs?.branches ?? []).map((branch) => ({
        label: `Checkout ${branch.name}`,
        run: () => mutate({ action: "checkout", name: branch.name }),
      })),
      ...(refs?.tags ?? []).map((tag) => ({
        label: `Tag ${tag.name}`,
        run: () => mutate({ action: "checkout", name: tag.name }),
      })),
      ...(refs?.remotes ?? []).map((remote) => ({
        label: `Remote ${remote.name}`,
        run: () => mutate({ action: "fetch", remote: remote.name, prune: settings.fetchPrune, tags: false }),
      })),
      ...(refs?.stashes ?? []).map((stash) => ({
        label: `Stash ${stash.summary}`,
        run: () => mutate({ action: "stashPop", name: stash.name }),
      })),
    ];
    const seen = new Set<string>();
    return items
      .filter((item) => {
        if (seen.has(item.label) || !fuzzy(query, item.label)) return false;
        seen.add(item.label);
        return true;
      })
      .slice(0, 12);
  });

  function fuzzy(query: string, label: string) {
    const text = label.toLowerCase();
    if (text.includes(query)) return true;
    let at = 0;
    for (const char of query) {
      const found = text.indexOf(char, at);
      if (found < 0) return false;
      at = found + 1;
    }
    return query.length > 0;
  }

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
        if (value.binary) {
          preview = await invoke<FilePreview | null>("file_preview", { path: repoPath(), file });
          const older = await invoke<FilePreview | null>("revision_preview", { path: repoPath(), rev: "HEAD", file }).catch(() => null);
          imageNew = preview?.dataUrl ?? null;
          imageOld = older?.dataUrl ?? null;
        } else {
          preview = null;
          imageNew = null;
          imageOld = null;
        }
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
    repoStats = await invoke<NonNullable<typeof repoStats>>("repository_summary", { path }).catch(() => repoStats);
    const locks = await invoke<{ available: boolean; locks: { path: string }[] }>("lfs_locks", { path }).catch(() => ({ available: false, locks: [] }));
    lfsAvailable = locks.available;
    signOff = refs.signOffSet ? !!refs.signOff : settings.signOff;
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
    return mutate({ action: "pull", rebase: settings.pullRebase, autostash: settings.mergeAutostash });
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
    event.stopPropagation();
    submenu = null;
    menu = { x: Math.min(event.clientX, window.innerWidth - 240), y: Math.min(event.clientY, window.innerHeight - 40), items };
  }

  function ask(title: string, body: string, run: () => void) {
    confirmAsk = { title, body, run };
  }

  function liveOnly(run: () => void) {
    return mode === "live" ? run : undefined;
  }

  function refKind(label: string) {
    if (label === "stash" || label.startsWith("stash")) return "stash";
    if (label.includes("/")) return "remote";
    if (/^v?\d/.test(label)) return "tag";
    return "local";
  }

  async function showReflog() {
    reflogOn = !reflogOn;
    treeOn = false;
    if (!reflogOn || mode !== "live") return;
    reflogRows = await invoke("reflog", { path: repoPath(), limit: 200 });
  }

  async function showTree() {
    treeOn = !treeOn;
    if (!treeOn || !selectedCommit || mode !== "live") return;
    treePaths = await invoke<string[]>("commit_tree", { path: repoPath(), rev: selectedCommit });
  }

  async function openTreeFile(file: string) {
    if (!selectedCommit) return;
    historyFile = file;
    const view = await invoke<{ binary: boolean; text: string; preview: FilePreview | null }>("blob_view", { path: repoPath(), rev: selectedCommit, file });
    if (view.preview) {
      imageOld = null;
      imageNew = view.preview.dataUrl;
      preview = view.preview;
      treeText = "";
    } else {
      treeText = view.text;
      preview = null;
    }
  }

  async function openForge(kind: "notes" | "pulls") {
    forgeOpen = forgeOpen === kind ? null : kind;
    if (mode !== "live") {
      notices = sampleNotices;
      pulls = samplePulls;
      return;
    }
    if (kind === "notes") notices = await invoke<typeof notices>("forge_notifications").catch(() => []);
    else pulls = await invoke<typeof pulls>("forge_pulls", { path: repoPath() }).catch(() => []);
  }

  async function paintPixel(left: string, right: string) {
    const load = (src: string) =>
      new Promise<HTMLImageElement>((resolve, reject) => {
        const image = new Image();
        image.onload = () => resolve(image);
        image.onerror = () => reject(new Error("image"));
        image.src = src;
      });
    try {
      const [before, after] = await Promise.all([load(left), load(right)]);
      const width = Math.max(before.width, after.width);
      const height = Math.max(before.height, after.height);
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      const context = canvas.getContext("2d");
      if (!context) return;
      context.drawImage(before, 0, 0);
      const leftData = context.getImageData(0, 0, width, height);
      context.clearRect(0, 0, width, height);
      context.drawImage(after, 0, 0);
      const rightData = context.getImageData(0, 0, width, height);
      const out = context.createImageData(width, height);
      for (let index = 0; index < out.data.length; index += 4) {
        out.data[index] = Math.abs(leftData.data[index] - rightData.data[index]);
        out.data[index + 1] = Math.abs(leftData.data[index + 1] - rightData.data[index + 1]);
        out.data[index + 2] = Math.abs(leftData.data[index + 2] - rightData.data[index + 2]);
        out.data[index + 3] = 255;
      }
      context.putImageData(out, 0, 0);
      pixelUrl = canvas.toDataURL();
    } catch {
      pixelUrl = "";
    }
  }

  function fileMenu(file: Row, side: Side): MenuItem[] {
    const locked = mode !== "live";
    return [
      { label: tr(side === "staged" ? "menu.unstage" : "menu.stage"), disabled: locked, shortcut: shortcutLabel("stageToggle"), run: () => runChange(side === "staged" ? "unstage_path" : "stage_path", { file: file.path }) },
      { label: tr("menu.discard"), disabled: locked, shortcut: shortcutLabel("discard"), run: () => ask(tr("menu.discard"), tr("dialog.discardBody", { name: file.path }), () => void mutate({ action: "discard", file: file.path })) },
      { label: tr("menu.deleteFile"), disabled: locked, run: () => ask(tr("menu.deleteFile"), tr("dialog.deleteBody", { name: file.path }), () => void mutate({ action: "delete", file: file.path })) },
      { sep: true, label: "" },
      { label: tr("menu.blame"), disabled: locked, run: () => showBlame(file.path) },
      { label: tr("menu.history"), disabled: locked, run: () => showFileHistory(file.path) },
      { label: tr("menu.externalDiff"), disabled: locked || !settings.diffTool, run: () => void invoke("launch_tool", { path: repoPath(), kind: "diff", file: file.path }) },
      { label: tr("menu.externalMerge"), disabled: locked || file.letter !== "C" || !settings.mergeTool, run: () => void invoke("launch_tool", { path: repoPath(), kind: "merge", file: file.path }) },
      { label: tr("menu.lfsLock"), disabled: locked || !lfsAvailable, run: () => void mutate({ action: "lfsLock", path: file.path }) },
      { label: tr("menu.lfsUnlock"), disabled: locked || !lfsAvailable, run: () => void mutate({ action: "lfsUnlock", path: file.path }) },
      { sep: true, label: "" },
      { label: tr("menu.reveal"), disabled: locked, run: () => revealItemInDir(fullPath(file.path)) },
      { label: tr("menu.openFile"), disabled: locked, run: () => openPath(fullPath(file.path)) },
      { label: tr("menu.copyPath"), run: () => copyText(fullPath(file.path)) },
      { label: tr("menu.copyRelative"), run: () => copyText(file.path) },
      ...(file.letter === "C" ? [{ label: tr("menu.resolve"), disabled: locked, run: () => openResolve(file.path) }] : []),
      ...commandItems("file"),
    ];
  }

  function commitMenu(commit: { id: string; summary: string }): MenuItem[] {
    const locked = mode !== "live";
    return [
      { label: tr("menu.checkout"), disabled: locked, run: () => mutate({ action: "checkout", name: commit.id }) },
      { label: tr("menu.cherryPick"), disabled: locked, run: () => mutate({ action: "cherryPick", rev: commit.id }) },
      { label: tr("menu.revert"), disabled: locked, run: () => mutate({ action: "revert", rev: commit.id }) },
      {
        label: tr("menu.reset"),
        children: [
          { label: tr("menu.soft"), disabled: locked, run: () => mutate({ action: "reset", rev: commit.id, mode: "soft" }) },
          { label: tr("menu.mixed"), disabled: locked, run: () => mutate({ action: "reset", rev: commit.id, mode: "mixed" }) },
          { label: tr("menu.hard"), disabled: locked, run: () => ask(tr("dialog.hardReset"), tr("dialog.hardResetBody"), () => void mutate({ action: "reset", rev: commit.id, mode: "hard" })) },
        ],
      },
      { sep: true, label: "" },
      { label: tr("menu.createBranch"), run: () => { draft = ""; draftExtra = commit.id; dialog = "branch"; } },
      { label: tr("menu.createTag"), run: () => { draft = ""; draftExtra = commit.id; dialog = "tag"; } },
      { label: tr("menu.editMessage"), disabled: locked, run: () => { draft = commit.summary; draftExtra = commit.id; dialog = "reword"; } },
      { label: tr("menu.interactiveRebase"), run: () => { selectedCommit = commit.id; openRebase(); } },
      { label: tr("menu.copySha"), run: () => copyText(commit.id) },
      ...commandItems("commit"),
    ];
  }

  function reflogMenu(row: { id: string; selector: string }): MenuItem[] {
    return [
      { label: tr("menu.checkout"), disabled: mode !== "live", run: () => mutate({ action: "checkout", name: row.selector || row.id }) },
      { label: tr("menu.createBranch"), run: () => { draft = ""; draftExtra = row.id; dialog = "branch"; } },
    ];
  }

  function setRebaseVerb(rev: string, verb: string) {
    const targets = rebasePicked.includes(rev) && rebasePicked.length > 1 ? new Set(rebasePicked) : new Set([rev]);
    rebaseSteps = rebaseSteps.map((step) => (targets.has(step.rev) ? { ...step, verb } : step));
  }

  function rememberRepo(path: string) {
    const recent = [path, ...settings.recent.filter((item) => item !== path)].slice(0, 12);
    const currentId = settings.currentWorkspace;
    const workspaces = settings.workspaces.map((workspace) => {
      if (!currentId || workspace.id !== currentId) return workspace;
      const openTabs = workspace.openTabs.includes(path) ? workspace.openTabs : [...workspace.openTabs, path];
      return {
        ...workspace,
        repositories: workspace.repositories.includes(path) ? workspace.repositories : [...workspace.repositories, path],
        openTabs,
        selectedTab: Math.max(0, openTabs.indexOf(path)),
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
    await runChange("commit_changes", { summary, description, amend, signOff, skipHooks });
    if (!actionError) {
      summary = "";
      description = "";
      amend = false;
      skipHooks = false;
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
    const saved = refs?.expandedGroups ?? [];
    if (!group || saved.length === 0) return true;
    return saved.includes(group);
  }

  function toggleGroup(group: string) {
    const groups = [...new Set(visibleBranches.map((branch) => branchGroup(branch.name)).filter(Boolean))];
    const saved = refs?.expandedGroups ?? [];
    const current = saved.length === 0 ? groups : [...saved];
    const next = current.includes(group) ? current.filter((item) => item !== group) : [...current, group];
    const names = next.length === groups.length ? [] : next;
    void mutate({ action: "setExpanded", names });
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

  async function unstagePicked() {
    const files = [...picked];
    for (const file of files) await mutate({ action: "stagePaths", files: [file], unstage: true });
    picked = [];
  }

  async function deletePicked() {
    const files = [...picked];
    for (const file of files) await mutate({ action: "delete", file });
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
    const source = mode === "live" ? commitsLive : commits.map((commit) => ({ id: commit.id, summary: commit.summary }));
    const index = source.findIndex((commit) => commit.id === selectedCommit);
    const newer = index > 0 ? source.slice(0, index).slice().reverse() : source.slice(0, 3);
    rebaseSteps = newer.map((commit) => ({ verb: "pick", rev: commit.id, summary: commit.summary, message: commit.summary }));
    rebasePicked = [];
    rebaseConflictFiles = [];
    dialog = "rebase";
    if (mode === "live") {
      void invoke<string[]>("rebase_conflicts", { path: repoPath(), onto: selectedCommit }).then((files) => (rebaseConflictFiles = files)).catch(() => (rebaseConflictFiles = []));
    }
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
    ask(tr("dialog.deleteBranch"), tr("dialog.deleteBranchBody", { name }), () => void mutate({ action: "deleteBranch", name }));
  }

  function hardReset() {
    ask(tr("dialog.hardReset"), tr("dialog.hardResetBody"), () => void mutate({ action: "reset", rev: draft || "HEAD", mode: "hard" }));
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
    return formatWhen(commit.at, settings.dateFormat.trim() || "dd MMM yyyy", settings.date24h) || commit.when;
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
    const currentId = settings.currentWorkspace;
    if (currentId) {
      settings = {
        ...settings,
        workspaces: settings.workspaces.map((workspace) => {
          if (workspace.id !== currentId) return workspace;
          const openTabs = workspace.openTabs.filter((item) => item !== path);
          return { ...workspace, openTabs, selectedTab: Math.min(workspace.selectedTab, Math.max(0, openTabs.length - 1)) };
        }),
      };
    }
    if (snapshot?.path !== path) {
      if (currentId) void savePrefs(false);
      return;
    }
    if (next[0]) void openRepo(next[0]);
    else {
      snapshot = null;
      refs = null;
      mode = "error";
      loadError = "";
      if (currentId) void savePrefs(false);
    }
  }

  async function selectWorkspace() {
    await savePrefs(false);
    const workspace = settings.workspaces.find((item) => item.id === settings.currentWorkspace);
    if (!workspace) return;
    const tabs = workspace.openTabs.length > 0 ? [...workspace.openTabs] : [...workspace.repositories];
    repos = tabs;
    const path = tabs[Math.min(workspace.selectedTab, Math.max(0, tabs.length - 1))] ?? tabs[0];
    if (path) await openRepo(path);
  }

  function openRepoSettings() {
    draft = settings.authorName;
    draftExtra = settings.authorEmail;
    draftUser = refs?.signOffFormat || settings.signOffFormat;
    repoSignOff = refs?.signOffSet ? !!refs.signOff : settings.signOff;
    dialog = "repo";
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
    void mutate({ action: "rebaseInteractive", onto: parent, drop: chosen.map((commit) => commit.id), autostash: settings.mergeAutostash });
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
      ask(tr("menu.discard"), tr("dialog.discardBody", { name: selectedPath }), () => void mutate({ action: "discard", file: selectedPath }));
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

  $effect(() => {
    if (mode !== "error" || !inApp()) return;
    for (const path of settings.recent.slice(0, 12)) {
      if (recentStats[path]) continue;
      void invoke<{ branch: string; ahead: number; behind: number; lastSummary: string }>("repository_summary", { path })
        .then((value) => {
          recentStats = { ...recentStats, [path]: value };
        })
        .catch(() => {});
    }
  });

  onMount(() => {
    if (!inApp()) {
      notices = sampleNotices;
      pulls = samplePulls;
      repoStats = { branch: "main", ahead: 5, behind: 0, commits: commits.length, branches: 2, lastSummary: commits[0]?.summary ?? "" };
      return;
    }
    window.addEventListener("keydown", onShortcut);
    let unlisten = () => {};
    let unmoved = () => {};
    let unresized = () => {};
    let placeTimer = 0;
    let stopped = false;
    mode = "loading";
    selectedPath = null;
    invoke<UpdateNotice | null>("check_for_update")
      .then((value) => {
        updateNotice = value;
      })
      .catch(() => {});
    void (async () => {
      try {
        const value = await invoke<Settings>("load_settings");
        if (stopped) return;
        settings = { ...defaultSettings, ...value, recent: value.recent ?? [], workspaces: value.workspaces ?? [], commands: value.commands ?? [], expandedGroups: value.expandedGroups ?? [] };
        if (!settings.dateFormat.trim()) settings = { ...settings, dateFormat: "dd MMM yyyy" };
        if (!settings.proxyType) settings = { ...settings, proxyType: "http" };
        signOff = settings.signOff;
        if (value.windowWidth > 200 && value.windowHeight > 200) {
          try {
            const win = getCurrentWindow();
            await win.setPosition(new LogicalPosition(value.windowX, value.windowY));
            await win.setSize(new LogicalSize(value.windowWidth, value.windowHeight));
          } catch {
            /* a saved position can sit off the current desktop */
          }
        }
        try {
          unlisten = await listen("repo-changed", () => {
            if (!busy) void refresh();
          });
        } catch {
          /* the watcher is optional until a repository is open */
        }
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
        const workspace = settings.workspaces.find((item) => item.id === settings.currentWorkspace);
        if (workspace) {
          repos = workspace.openTabs.length > 0 ? [...workspace.openTabs] : [...workspace.repositories];
        }
        const recent = settings.recent[0];
        if (!recent) {
          mode = "error";
          loadError = "";
          return;
        }
        await openRepo(recent);
        if (!snapshot) {
          loadError = actionError || "Could not open the last repository.";
          mode = "error";
        }
      } catch (error) {
        if (!stopped) {
          loadError = message(error);
          mode = "error";
        }
      }
    })();
    return () => {
      stopped = true;
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
      <button class="tab" type="button" onclick={() => (dialog = "open")}>{tr("chrome.open")}</button>
      <button class="tab" type="button" onclick={() => { draft = ""; dialog = "workspace"; }}>{tr("chrome.workspace")}</button>
      {#if settings.workspaces.length > 0}
        <select class="search" aria-label="Workspace" bind:value={settings.currentWorkspace} onchange={() => selectWorkspace()}>
          <option value="">All</option>
          {#each settings.workspaces as workspace (workspace.id)}
            <option value={workspace.id}>{workspace.name}</option>
          {/each}
        </select>
      {/if}
      {#each repos as repo (repo)}
        <div class="tab" class:active={snapshot?.path === repo}>
          <button class="file-select" type="button" onclick={() => openRepo(repo)}>{folderName(repo)}</button>
          <button class="text-button row-action tab-close" type="button" onclick={() => closeTab(repo)} aria-label={tr("chrome.close")}>×</button>
        </div>
      {:else}
        <button class="tab active" type="button">{tabLabel}</button>
      {/each}
    </div>
    <div class="toolbar">
      <button class="tool" type="button" disabled={busy} onclick={(event) => openMenu(event, [
        { label: tr("chrome.fetch"), shortcut: shortcutLabel("fetch"), disabled: mode !== "live", run: () => void doFetch() },
        { label: tr("chrome.fetchAll"), disabled: mode !== "live", run: () => void mutate({ action: "fetch", remote: null, prune: settings.fetchPrune, tags: false }) },
      ])}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 2v8M5 7l3 3 3-3M3 13h10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>{tr("chrome.fetch")}</button>
      <button class="tool" type="button" disabled={busy} onclick={(event) => openMenu(event, [
        { label: tr("chrome.pull"), shortcut: shortcutLabel("pull"), disabled: mode !== "live", run: () => void doPull() },
        { label: tr("dialog.fastForward"), disabled: mode !== "live", run: () => { settings.pullRebase = false; void doPull(); } },
      ])}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 13V4M5 7l3-3 3 3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>{tr("chrome.pull")}</button>
      <button class="tool" type="button" disabled={busy} onclick={(event) => openMenu(event, [
        { label: tr("chrome.push"), shortcut: shortcutLabel("push"), disabled: mode !== "live", run: () => void doPush() },
        { label: tr("menu.createTag"), disabled: mode !== "live", run: () => void mutate({ action: "push", remote: null, setUpstream: true, tags: true, forceWithLease: settings.forceWithLease }) },
      ])}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 14V5M5 8l3-3 3 3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>{tr("chrome.push")}</button>
      <button class="tool" type="button" disabled={busy} onclick={(event) => openMenu(event, [
        { label: tr("chrome.stash"), shortcut: shortcutLabel("stash"), disabled: mode !== "live", run: () => { draft = ""; dialog = "stash"; } },
        { label: tr("chrome.pop"), disabled: mode !== "live", run: () => void mutate({ action: "stashPop" }) },
      ])}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 5h10v8H3zM5 5V3h6v2" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>{tr("chrome.stash")}</button>
      <button class="tool" type="button" disabled={busy} onclick={() => { draft = ""; dialog = "flow"; }}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 3h4v4H3zM9 9h4v4H9zM5 7v2h4" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>{tr("chrome.flow")}</button>
      <button class="tool icon-only" type="button" title={tr("chrome.notifications")} onclick={() => openForge("notes")}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 2a4 4 0 0 1 4 4v2l1 2H3l1-2V6a4 4 0 0 1 4-4zM6.5 12a1.5 1.5 0 0 0 3 0" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>{#if notices.some((item) => item.unread)}<span class="count badge-count">{notices.filter((item) => item.unread).length}</span>{/if}</button>
      <div class="spacer"></div>
      <div class="launch-wrap">
        <input
          class="launch"
          placeholder={tr("chrome.quickLaunch")}
          aria-label={tr("chrome.quickLaunch")}
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
      <button class="tool icon-only" type="button" title={tr("chrome.terminal")} onclick={() => invoke("open_terminal", { path: repoPath() })}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 4l4 4-4 4M8 12h5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg></button>
      <button class="tool icon-only" type="button" title={tr("chrome.explorer")} onclick={() => snapshot && openPath(snapshot.path)}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2 4h5l1 2h6v7H2z" fill="none" stroke="currentColor" stroke-width="1.4"/></svg></button>
      <button class="tool icon-only" type="button" title={tr("chrome.preferences")} onclick={() => (dialog = "prefs")}><svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="2.2" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M8 1.8v2M8 12.2v2M1.8 8h2M12.2 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M12.6 3.4l-1.4 1.4M4.8 11.2l-1.4 1.4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/></svg></button>
      <button class="tool icon-only" type="button" title={tr("chrome.pulls")} onclick={() => openForge("pulls")}><svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="4" cy="4" r="1.4" fill="none" stroke="currentColor"/><circle cx="12" cy="12" r="1.4" fill="none" stroke="currentColor"/><path d="M4 5.5v5a2 2 0 0 0 2 2h4.2" fill="none" stroke="currentColor" stroke-width="1.3"/></svg></button>
    </div>
  </header>

  {#if busy}
    <div class="banner">
      <span>{tr("chrome.working")}</span>
      <button class="text-button" type="button" onclick={() => invoke("cancel_operation")}>{tr("chrome.cancel")}</button>
    </div>
  {/if}
  {#if updateNotice}
    <div class="banner">
      <span>{tr("chrome.updateAvailable", { name: `AweGit ${updateNotice.appVersion}` })}</span>
      {#if updateNotice.downloadUrl}
        <button class="text-button" type="button" onclick={() => openUrl(updateNotice?.downloadUrl ?? "")}>{tr("chrome.download")}</button>
      {/if}
    </div>
  {/if}
  {#if mode === "live" && progress}
    <div class="banner">
      <span>{tr("chrome.inProgress", { name: progressLabel(progress) })}</span>
      <button class="text-button" type="button" disabled={busy} onclick={() => mutate({ action: "abort" })}>{tr("chrome.abort")}</button>
      <button class="text-button" type="button" disabled={busy || conflicted} onclick={() => { if (progress === "rebase") { draft = ""; dialog = "continue"; } else void mutate({ action: "continue" }); }}>{tr("chrome.continueAction")}</button>
      <button class="text-button" type="button" disabled={busy || progress === "merge"} onclick={() => mutate({ action: "skip" })}>{tr("chrome.skip")}</button>
    </div>
  {/if}
  {#if actionError}
    <p class="banner error">{actionError}</p>
  {/if}

  <div class="body">
    <aside class="sidebar">
      <button class="side" class:selected={section === "changes"} type="button" onclick={() => (section = "changes")}>
        <span>{tr("chrome.changes")}</span>
        <span class="count">{changeCount}</span>
      </button>
      <button class="side" class:selected={section === "history"} type="button" onclick={() => (section = "history")}>
        <span>{allBranches ? tr("chrome.allCommits") : tr("chrome.currentBranch")}</span>
        {#if mode === "live"}<span class="count">{shownCommits.length}</span>{/if}
      </button>

      <div class="group">
        {tr("chrome.localBranches")}
        {#if mode === "live"}
          <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = ""; dialog = "branch"; }}>{tr("chrome.new")}</button>
          <button class="text-button" type="button" onclick={showAllRefs}>{tr("chrome.showAll")}</button>
        {/if}
      </div>
      {#if mode === "live"}
        <input class="search" placeholder={tr("chrome.filterSidebar")} aria-label={tr("chrome.filterSidebar")} bind:value={sideQuery} />
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
                { label: tr("menu.checkout"), run: () => mutate({ action: "checkout", name: branch.name }) },
                { label: tr("menu.merge"), run: () => { draft = branch.name; draftExtra = settings.mergeNoFf ? "no-ff" : "ff"; dialog = "merge"; } },
                { label: tr("menu.rebaseOnto"), run: () => mutate({ action: "rebase", onto: branch.name, autostash: settings.mergeAutostash }) },
                { label: tr("menu.delete"), run: () => removeBranch(branch.name) },
              ] : []),
              { label: tr("menu.rename"), run: () => { draft = branch.name; draftExtra = branch.name; dialog = "rename"; } },
              { label: tr("menu.setUpstream"), run: () => { draft = branch.name; draftExtra = branch.upstream ?? ""; dialog = "upstream"; } },
              { label: tr("menu.copyName"), run: () => copyText(branch.name) },
              { label: tr("menu.hide"), run: () => hideRef(branch.name) },
              { label: tr("menu.showOnly"), run: () => showOnly(branch.name) },
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
          {tr("chrome.remotes")}
          <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = ""; dialog = "remote"; }}>{tr("chrome.add")}</button>
        </div>
        {#each refs.remotes as remote (remote.name)}
          {#if matchesQuery(remote.name, sideQuery)}
          <div class="side quiet" role="group" oncontextmenu={(event) => openMenu(event, [
            { label: tr("chrome.fetch"), run: () => mutate({ action: "fetch", remote: remote.name, prune: settings.fetchPrune, tags: false }) },
            { label: tr("dialog.url"), run: () => { draft = remote.name; draftExtra = remote.url ?? ""; dialog = "remote"; } },
            { label: tr("menu.remove"), run: () => mutate({ action: "removeRemote", name: remote.name }) },
          ])}>
            <span class="name">{remote.name}</span>
          </div>
          {#if remote.head}<div class="side nested quiet"><span class="name">HEAD</span></div>{/if}
          {#each remote.branches as branch (remote.name + branch)}
            {#if matchesQuery(branch, sideQuery)}
            <div
              class="side nested quiet"
              role="group"
              oncontextmenu={(event) => openMenu(event, [
                { label: tr("menu.checkout"), run: () => mutate({ action: "checkoutRemote", remote: remote.name, branch }) },
                { label: tr("menu.pullInto"), run: () => mutate({ action: "pullRef", remote: remote.name, branch, rebase: settings.pullRebase, autostash: settings.mergeAutostash }) },
                { label: tr("menu.pushHere"), run: () => mutate({ action: "pushRef", remote: remote.name, branch, forceWithLease: settings.forceWithLease }) },
                { label: tr("menu.deleteRemote"), run: () => ask(tr("menu.deleteRemote"), tr("dialog.deleteRemoteBody", { name: `${remote.name}/${branch}` }), () => void mutate({ action: "deleteRemoteBranch", remote: remote.name, branch })) },
                { label: tr("menu.copyName"), run: () => copyText(`${remote.name}/${branch}`) },
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
        <div
          class="side current"
          role="group"
          oncontextmenu={(event) => openMenu(event, [
            { label: tr("menu.checkout"), disabled: true },
            { label: tr("menu.merge"), run: () => { draft = branchLabel; draftExtra = "ff"; dialog = "merge"; } },
            { label: tr("menu.rebaseOnto"), disabled: true },
            { label: tr("menu.rename"), run: () => { draft = branchLabel; draftExtra = branchLabel; dialog = "rename"; } },
            { label: tr("menu.delete"), run: () => ask(tr("dialog.deleteBranch"), tr("dialog.deleteBranchBody", { name: branchLabel }), () => {}) },
            { label: tr("menu.setUpstream"), run: () => { draft = branchLabel; draftExtra = "origin/main"; dialog = "upstream"; } },
            { label: tr("menu.copyName"), run: () => copyText(branchLabel) },
            { label: tr("menu.hide"), disabled: true },
            { label: tr("menu.showOnly"), disabled: true },
          ])}
        >
          <span class="dot"></span>
          <span class="name">{branchLabel}</span>
          {#if mode === "sample"}<span class="ahead">↑5</span>{/if}
        </div>
        {#if repoStats}<p class="empty">{repoStats.branch} · {repoStats.commits} {tr("chrome.commits")} · {repoStats.branches} {tr("chrome.branches")} · ↑{repoStats.ahead} ↓{repoStats.behind}</p>{/if}
        <div class="group">{tr("chrome.remotes")}</div>
        <div class="side quiet"><span class="name">origin</span></div>
        <div class="side nested quiet"><span class="name">HEAD</span></div>
        <div
          class="side nested quiet"
          role="group"
          oncontextmenu={(event) => openMenu(event, [
            { label: tr("menu.checkout"), disabled: true },
            { label: tr("menu.pullInto"), disabled: true },
            { label: tr("menu.pushHere"), disabled: true },
            { label: tr("menu.deleteRemote"), run: () => ask(tr("menu.deleteRemote"), tr("dialog.deleteRemoteBody", { name: "origin/main" }), () => {}) },
            { label: tr("menu.copyName"), run: () => copyText("origin/main") },
          ])}
        ><span class="name">main</span></div>
      {/if}
      <div class="group" role="group" oncontextmenu={(event) => openMenu(event, [
        { label: tr("menu.createTag"), run: () => { draft = ""; draftExtra = "HEAD"; dialog = "tag"; } },
        { label: tr("chrome.stash"), run: () => { draft = ""; dialog = "stash"; } },
        { label: tr("chrome.add"), run: () => { draft = ""; draftExtra = ""; dialog = "submodule"; } },
        { label: tr("chrome.worktrees"), run: () => { draft = ""; draftExtra = ""; dialog = "worktree"; } },
      ])}>{tr("chrome.more")}</div>
      <div class="side quiet">
        <button class="file-select" type="button" onclick={() => (expanded = expanded === "tags" ? null : "tags")}>
          <span>{tr("chrome.tags")}</span><span class="count">{visibleTags.length}</span>
        </button>
      </div>
      {#if expanded === "tags"}
        {#each visibleTags as tag (tag.name)}
          <div class="side nested quiet" role="group" oncontextmenu={(event) => openMenu(event, [
            { label: tr("menu.checkout"), disabled: mode !== "live", run: () => mutate({ action: "checkout", name: tag.name }) },
            { label: tr("menu.delete"), disabled: mode !== "live", run: () => ask(tr("menu.delete"), tag.name, () => void mutate({ action: "deleteTag", name: tag.name })) },
            { label: tr("menu.copyName"), run: () => copyText(tag.name) },
          ])}>
            <button class="file-select" type="button" onclick={() => mutate({ action: "checkout", name: tag.name })}><span class="name">{tag.name}</span></button>
          </div>
        {/each}
      {/if}
      <button class="side quiet" type="button" onclick={() => (expanded = expanded === "stashes" ? null : "stashes")}>
        <span>{tr("chrome.stashes")}</span><span class="count">{refs?.stashes.length ?? (mode === "sample" ? 1 : 0)}</span>
      </button>
      {#if expanded === "stashes"}
        {#each (refs?.stashes ?? (mode === "sample" ? [{ name: "stash@{0}", summary: "WIP before the palette" }] : [])) as stash (stash.name)}
          <div class="side nested quiet" role="group" oncontextmenu={(event) => openMenu(event, [
            { label: tr("menu.apply"), disabled: mode !== "live", run: () => mutate({ action: "stashApply", name: stash.name }) },
            { label: tr("menu.pop"), disabled: mode !== "live", run: () => mutate({ action: "stashPop", name: stash.name }) },
            { label: tr("menu.drop"), disabled: mode !== "live", run: () => ask(tr("menu.drop"), stash.summary, () => void mutate({ action: "stashDrop", name: stash.name })) },
          ])}>
            <span class="name">{stash.summary}</span>
          </div>
        {/each}
      {/if}
      <button class="side quiet" type="button" onclick={() => (expanded = expanded === "submodules" ? null : "submodules")}>
        <span>{tr("chrome.submodules")}</span><span class="count">{refs?.submodules.length ?? 0}</span>
      </button>
      {#if expanded === "submodules"}
        {#each refs?.submodules ?? [] as row (row.path)}
          <div class="side nested quiet" role="group" oncontextmenu={(event) => openMenu(event, [
            { label: tr("menu.initialize"), disabled: mode !== "live" || row.ready, run: () => mutate({ action: "submoduleInit", path: row.path }) },
            { label: tr("menu.openFile"), disabled: !row.ready, run: () => openRepo(fullPath(row.path)) },
            { label: tr("menu.sync"), disabled: mode !== "live" || !row.ready, run: () => mutate({ action: "submoduleSync", path: row.path }) },
            { label: tr("menu.update"), disabled: mode !== "live" || !row.ready, run: () => mutate({ action: "submoduleUpdate" }) },
            { label: tr("menu.delete"), disabled: mode !== "live", run: () => ask(tr("menu.delete"), row.path, () => void mutate({ action: "submoduleRemove", path: row.path })) },
          ])}>
            <span class="name">{row.path}</span>
          </div>
        {/each}
      {/if}
      <button class="side quiet" type="button" onclick={() => (expanded = expanded === "worktrees" ? null : "worktrees")}>
        <span>{tr("chrome.worktrees")}</span><span class="count">{refs?.worktrees.length ?? (mode === "sample" ? 1 : 0)}</span>
      </button>
      {#if expanded === "worktrees"}
        {#each (refs?.worktrees ?? (mode === "sample" ? [{ path: "this repository", branch: "main" }] : [])) as row (row.path)}
          <div class="side nested quiet" role="group" oncontextmenu={(event) => openMenu(event, [
            { label: tr("menu.openFile"), disabled: mode !== "live", run: () => openRepo(row.path) },
            { label: tr("menu.remove"), disabled: mode !== "live", run: () => ask(tr("menu.remove"), row.path, () => void mutate({ action: "removeWorktree", path: row.path })) },
          ])}>
            <span class="name">{row.branch ?? folderName(row.path)}</span>
          </div>
        {/each}
      {/if}
    </aside>

    {#if section === "changes"}
      <section class="changes">
        <div class="status-pane" style:order={settings.swapPanes ? 2 : 1}>
        <div class="pane-head">
          <span>{tr("chrome.staged")}</span>
          <span class="count">{staged.length}</span>
          <button class="text-button" type="button" disabled={mode !== "live" || busy || staged.length === 0} onclick={() => runChange("unstage_all")}>{tr("chrome.unstageAll")}</button>
        </div>
        {#if shownStaged.length === 0}
          <p class="empty">{staged.length === 0 ? tr("chrome.noStaged") : tr("chrome.noMatch")}</p>
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
          <span>{tr("chrome.unstaged")}</span>
          <span class="count">{mode === "loading" ? "…" : mode === "error" ? "—" : shownUnstaged.length}</span>
          <input class="search" placeholder={tr("chrome.filterFiles")} aria-label={tr("chrome.filterFiles")} bind:value={fileQuery} />
          <button class="text-button" type="button" disabled={mode !== "live" || busy || unstaged.length === 0} onclick={() => runChange("stage_all")}>{tr("chrome.stageAll")}</button>
        </div>
        <div class="file-list" onscroll={(event) => (fileTop = (event.currentTarget as HTMLElement).scrollTop)}>
          {#if mode === "loading"}
            <p class="empty">{tr("chrome.readingStatus")}</p>
          {:else if mode === "error"}
            <p class="empty">{loadError}</p>
          {:else if shownUnstaged.length === 0}
            <p class="empty">{unstaged.length === 0 ? tr("chrome.noUnstaged") : tr("chrome.noMatch")}</p>
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
            oncontextmenu={(event) => openMenu(event, fileMenu(file, side))}
          >
            <button class="file-select" type="button" onclick={(event) => selectFile(file.path, side, event)}>
              <span class="badge {file.tone}">{file.letter}</span>
              <span class="file-name">{fileName(file.path)}</span>
              <span class="file-dir">{parentDir(file.path)}</span>
            </button>
          </div>
        {/snippet}

        <form class="composer" style:order="3" onsubmit={(event) => { event.preventDefault(); void submitCommit(false); }}>
          <label class="field">
            <input placeholder={tr("chrome.summary")} bind:value={summary} maxlength="200" aria-invalid={summaryTooLong} />
            <span class="counter" class:over={summaryTooLong}>{summary.length}/72</span>
          </label>
          <textarea placeholder={tr("chrome.description")} rows="3" bind:value={description}></textarea>
          <div class="composer-row">
            <label class="check"><input type="checkbox" bind:checked={amend} /> {tr("chrome.amend")}</label>
            <label class="check"><input type="checkbox" bind:checked={signOff} onchange={() => { if (mode === "live") void mutate({ action: "setSignOff", enabled: signOff, format: refs?.signOffFormat || settings.signOffFormat }); }} /> {tr("chrome.signOff")}</label>
            <label class="check"><input type="checkbox" bind:checked={skipHooks} /> {tr("chrome.skipHooks")}</label>
            <button class="text-button" type="button" disabled={mode !== "live" || busy} onclick={() => suggestMessage()}>{tr("chrome.suggest")}</button>
            <button class="text-button" type="button" disabled={!canCommit} onclick={() => submitCommit(true)}>{tr("chrome.commitAndPush")}</button>
            <button class="commit" type="submit" disabled={!canCommit}>{busy ? tr("chrome.working") : tr("chrome.commit")}</button>
          </div>
        </form>
      </section>
    {:else if mode !== "sample"}
      <section class="history" bind:this={historyEl} onscroll={(event) => (historyTop = (event.currentTarget as HTMLElement).scrollTop)}>
        <div class="pane-head">
          <input class="search" placeholder={tr("chrome.findCommits")} aria-label={tr("chrome.findCommits")} bind:this={searchEl} bind:value={commitQuery} />
          <button class="text-button" type="button" onclick={showReflog}>{tr("chrome.reflog")}</button>
          <button class="text-button" type="button" disabled={!selectedCommit} onclick={showTree}>{tr("chrome.fileTree")}</button>
          {#if historyFilter}
            <button class="text-button" type="button" onclick={() => { historyFilter = ""; void loadContext(); }}>{tr("chrome.allCommits")}</button>
          {/if}
          <button class="text-button" type="button" disabled={drops.length === 0 || busy} onclick={dropCommits}>{tr("menu.drop")}</button>
          <button class="text-button" type="button" disabled={!selectedCommit || busy} onclick={() => { draft = ""; dialog = "squash"; }}>{tr("dialog.squash")}</button>
          <button class="text-button" type="button" disabled={drops.length !== 2} onclick={() => compareDrops()}>{tr("chrome.compare")}</button>
          <button class="text-button" type="button" onclick={() => { logLimit += 200; void loadContext(); }}>{tr("chrome.loadMore")}</button>
        </div>
        {#if historyFilter}
          <div class="pane-head"><span>{historyFilter}</span></div>
        {/if}
        {#if reflogOn}
          {#each reflogRows as row (`${row.selector}-${row.id}`)}
            <button class="commit-row" type="button" oncontextmenu={(event) => openMenu(event, reflogMenu(row))}>
              <span class="subject">{row.summary}</span>
              <span class="meta">{row.selector}</span>
              <span class="meta sha">{row.shortId}</span>
            </button>
          {/each}
        {:else if shownCommits.length === 0}
          <p class="empty">{tr("chrome.noCommits")}</p>
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
                oncontextmenu={(event) => openMenu(event, commitMenu(commit))}
              >
                <input class="drop-check" type="checkbox" checked={drops.includes(commit.id)} aria-label={tr("menu.drop")} onclick={(event) => event.stopPropagation()} onchange={() => toggleDrop(commit.id)} />
                <span class="graph" aria-hidden="true">
                  {#each liveGraph[historyStart + index] ?? [] as cell, lane (`${commit.id}-${lane}`)}
                    <i class="graph-cell {cell?.role ?? ""}" style:color={cell ? laneColors[cell.color] : "transparent"}></i>
                  {/each}
                </span>
                {#if settings.gravatar && commit.email}
                  <img class="avatar" alt="" src={gravatarUrl(commit.email)} />
                {:else}
                  <span class="avatar" title={commit.author}>{commit.author.slice(0, 1).toUpperCase()}</span>
                {/if}
                <span class="subject">{commit.summary}</span>
                <span class="badges">
                  {#each commit.refs as label (label)}<span class="ref {refKind(label)}">{label}</span>{/each}
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
        <div class="pane-head">
          <button class="text-button" type="button" onclick={showReflog}>{tr("chrome.reflog")}</button>
          <button class="text-button" type="button" onclick={showTree}>{tr("chrome.fileTree")}</button>
          {#if repoStats}<span class="meta">{repoStats.lastSummary}</span>{/if}
        </div>
        {#if reflogOn}
          {#each sampleReflog as row (row.selector)}
            <button class="commit-row" type="button" oncontextmenu={(event) => openMenu(event, reflogMenu(row))}>
              <span class="subject">{row.summary}</span>
              <span class="meta">{row.selector}</span>
              <span class="meta sha">{row.shortId}</span>
            </button>
          {/each}
        {:else}
          {#each sampleRows as commit, index (commit.id)}
            <button class="commit-row" class:selected={selectedCommit === commit.id || selectedPath === commit.id} type="button" onclick={() => { selectedCommit = commit.id; selectedPath = commit.id; }} oncontextmenu={(event) => commit.badges.includes("stash") ? openMenu(event, [{ label: tr("menu.apply"), disabled: true }, { label: tr("menu.pop"), disabled: true }, { label: tr("menu.drop"), disabled: true }]) : openMenu(event, commitMenu(commit))}>
              <span class="graph" aria-hidden="true">
                {#each sampleGraph[index] ?? [] as cell, lane (`s-${commit.id}-${lane}`)}
                  <i class="graph-cell {cell?.role ?? ""}" style:color={cell ? laneColors[cell.color] : "transparent"}></i>
                {/each}
              </span>
              <span class="subject">{commit.summary}</span>
              <span class="badges">
                {#each commit.badges as label (label)}<span class="ref {refKind(label)}">{label}</span>{/each}
              </span>
              <span class="meta">{commit.author}</span>
              <span class="meta sha">{commit.id}</span>
              <span class="meta">{commit.when}</span>
            </button>
          {/each}
        {/if}
      </section>
    {/if}

    <section class="diff">
      {#if blameLines}
        <header class="diff-head">
          <span>{tr("chrome.blameOf", { name: selectedPath ?? "" })}</span>
          <button class="text-button" type="button" onclick={() => (blameLines = null)}>{tr("chrome.close")}</button>
        </header>
        <div class="diff-body" onscroll={(event) => (blameTop = (event.currentTarget as HTMLElement).scrollTop)}>
          <div class="commit-window" style:height="{(blameLines?.length ?? 0) * 18}px">
            {#each blameWindow.rows as line, index (`${line.line}-${line.id}`)}
              <button class="virtual-row" type="button" style:top="{(blameWindow.start + index) * 18}px" title={line.summary} onclick={() => { section = "history"; void selectCommit(line.id); }}>{line.line} {line.shortId} {line.author} {line.summary} {line.text}</button>
            {/each}
          </div>
        </div>
      {:else if mode === "live" && section === "changes" && selectedPath}
        <header class="diff-head">
          <span>{selectedPath}</span>
        </header>
        {#if diffError}
          <p class="diff-empty">{diffError}</p>
        {:else if !diff}
          <p class="diff-empty">{tr("chrome.reading")}</p>
        {:else if diff.binary}
          {#if imageOld || imageNew}
            {@render imageCompare(imageOld, imageNew)}
          {:else}
            <p class="diff-empty">{tr("chrome.binary")}</p>
          {/if}
        {:else if diff.lines.length === 0}
          <p class="diff-empty">{tr("chrome.noChanges")}</p>
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
                  <span class="gutter">{shownDiffLines[diffWindow.start + index]?.oldNo ?? ""}</span>
                  <span class="gutter">{shownDiffLines[diffWindow.start + index]?.newNo ?? ""}</span>
                  {#each highlight(line.text, selectedPath ?? "") as token, tokenIndex (`${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}
                  {#if line.kind === "hunk"}
                    <button class="text-button line-action" type="button" disabled={busy} onclick={() => mutate({ action: "stageHunk", file: selectedPath, index: shownDiffLines.slice(0, diffWindow.start + index + 1).filter((item) => item.kind === "hunk").length - 1, unstage: selectedSide === "staged" })}>{selectedSide === "staged" ? tr("menu.unstageHunk") : tr("menu.stageHunk")}</button>
                    {#if selectedSide === "unstaged"}
                      <button class="text-button line-action" type="button" disabled={busy} onclick={() => mutate({ action: "discardHunk", file: selectedPath, index: shownDiffLines.slice(0, diffWindow.start + index + 1).filter((item) => item.kind === "hunk").length - 1 })}>{tr("menu.discardHunk")}</button>
                    {/if}
                  {/if}
                  {#if line.stageAt != null && (line.kind === "add" || line.kind === "delete")}
                    <button class="text-button line-action" type="button" disabled={busy} onclick={() => stageOne(line)}>{selectedSide === "staged" ? tr("menu.unstageLine") : tr("menu.stageLine")}</button>
                    {#if selectedSide === "unstaged" && line.workAt != null}
                      <button class="text-button line-action" type="button" disabled={busy} onclick={() => mutate({ action: "discardLine", file: selectedPath, text: lineText(line), addition: line.kind === "add", at: line.workAt })}>{tr("menu.discardLine")}</button>
                    {/if}
                  {/if}
                </span>
              {/each}
            </div>
            {#if diff.truncated}<span class="meta">{tr("chrome.truncated")}</span>{/if}
          </div>
        {/if}
      {:else if section === "changes" && selectedPath && selectedDiff.length > 0}
        <header class="diff-head">{selectedPath}</header>
        <pre class="diff-body">{#each selectedDiff as line, index (index)}<span class:add={line.startsWith("+") && !line.startsWith("+++")} class:del={line.startsWith("-") && !line.startsWith("---")} class:hunk={line.startsWith("@@")}>{line + "\n"}</span>{/each}</pre>
      {:else if mode === "live" && section === "history"}
        <header class="diff-head">
          <span>{commitsLive.find((row) => row.id === selectedCommit)?.summary ?? tr("chrome.commits")}</span>
        </header>
        {#if treeOn}
          <div class="file-list staged-list">
            {#each treePaths as file (file)}
              <button class="file" type="button" onclick={() => openTreeFile(file)}><span class="file-name">{file}</span></button>
            {/each}
          </div>
          {#if treeText}<pre class="diff-body">{treeText}</pre>{/if}
        {/if}
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
        {#if imageOld || imageNew}
          {@render imageCompare(imageOld, imageNew)}
        {:else if historyDiff && historyDiff.binary}
          <p class="diff-empty">{tr("chrome.binary")}</p>
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
          <p class="diff-empty">{tr("chrome.selectCommitFile")}</p>
        {/if}
      {:else if section === "history" && treeOn}
        {@const sampleCommit = sampleRows.find((row) => row.id === (selectedCommit || selectedPath))}
        <header class="diff-head">{tr("chrome.fileTree")}</header>
        <div class="file-list staged-list">
          {#each (sampleCommit && sampleCommit.files.length > 0 ? sampleCommit.files.map((file) => file.path) : sampleTree) as file (file)}
            <button class="file" class:selected={historyFile === file} type="button" onclick={() => (historyFile = file)}><span class="file-name">{file}</span></button>
          {/each}
        </div>
        {#if historyFile === "art/mark.png"}
          {@render imageCompare(sampleImages.before, sampleImages.after)}
        {:else if historyFile && diffs[historyFile]}
          <pre class="diff-body">{#each diffs[historyFile] as line, index (index)}<span class:add={line.startsWith("+") && !line.startsWith("+++")} class:del={line.startsWith("-") && !line.startsWith("---")} class:hunk={line.startsWith("@@")}>{line + "\n"}</span>{/each}</pre>
        {/if}
      {:else if section === "history" && (selectedCommit || selectedPath)}
        {@const sampleCommit = sampleRows.find((row) => row.id === (selectedCommit || selectedPath))}
        <header class="diff-head">{sampleCommit?.summary}</header>
        <div class="file-list staged-list">
          {#each sampleCommit?.files ?? [] as file (file.path)}
            <button class="file" class:selected={historyFile === file.path} type="button" onclick={() => (historyFile = file.path)}>
              <span class="badge {file.kind === "A" ? "added" : file.kind === "D" ? "deleted" : "modified"}">{file.kind}</span>
              <span class="file-name">{fileName(file.path)}</span>
            </button>
          {/each}
        </div>
        {#if historyFile === "art/mark.png"}
          {@render imageCompare(sampleImages.before, sampleImages.after)}
        {:else if historyFile && diffs[historyFile]}
          <pre class="diff-body">{#each diffs[historyFile] as line, index (index)}<span class:add={line.startsWith("+") && !line.startsWith("+++")} class:del={line.startsWith("-") && !line.startsWith("---")} class:hunk={line.startsWith("@@")}>{line + "\n"}</span>{/each}</pre>
        {:else}
          <p class="diff-empty">{tr("chrome.selectCommitFile")}</p>
        {/if}
      {:else if selectedPath === "art/mark.png"}
        {@render imageCompare(sampleImages.before, sampleImages.after)}
      {:else}
        <p class="diff-empty">{tr("chrome.selectFile")}</p>
      {/if}
    </section>
  </div>

  {#if mode === "error"}
    <section class="changes">
      <h2>{tr("chrome.openRepo")}</h2>
      <p class="empty">{loadError || tr("chrome.recent")}</p>
      {#if repoStats}
        <p class="empty">{repoStats.branch} · {repoStats.commits} {tr("chrome.commits")} · ↑{repoStats.ahead} ↓{repoStats.behind} · {repoStats.lastSummary}</p>
      {/if}
      <input class="search" placeholder={tr("chrome.searchRecent")} aria-label={tr("chrome.searchRecent")} bind:value={recentQuery} />
      {#each settings.recent.filter((path) => matchesQuery(path, recentQuery)) as path (path)}
        <button class="side" type="button" onclick={() => openRepo(path)}>
          <span class="name">{folderName(path)}</span>
          {#if recentStats[path]}<span class="meta">{recentStats[path].branch} · ↑{recentStats[path].ahead} ↓{recentStats[path].behind} · {recentStats[path].lastSummary}</span>{/if}
        </button>
      {/each}
      <div class="composer-row">
        <button class="commit" type="button" onclick={() => (dialog = "open")}>{tr("chrome.open")}</button>
        <button class="text-button" type="button" onclick={() => { draft = ""; draftExtra = settings.cloneDirectory; dialog = "clone"; }}>{tr("chrome.clone")}</button>
        <button class="text-button" type="button" onclick={() => { draft = ""; dialog = "clone"; }}>{tr("chrome.newRepo")}</button>
      </div>
    </section>
  {/if}
  {#snippet imageCompare(before: string | null, after: string | null)}
    <div class="image-toolbar">
      <button type="button" class:on={imageMode === "side"} onclick={() => (imageMode = "side")}>{tr("dialog.imageSide")}</button>
      <button type="button" class:on={imageMode === "swipe"} onclick={() => (imageMode = "swipe")}>{tr("dialog.imageSwipe")}</button>
      <button type="button" class:on={imageMode === "onion"} onclick={() => (imageMode = "onion")}>{tr("dialog.imageOnion")}</button>
      <button type="button" class:on={imageMode === "pixel"} onclick={() => { imageMode = "pixel"; if (before && after) void paintPixel(before, after); }}>{tr("dialog.imagePixel")}</button>
      {#if imageMode === "swipe" || imageMode === "onion"}
        <input type="range" min="0" max="100" bind:value={imagePos} aria-label={imageMode} />
      {/if}
    </div>
    <div class="image-stage">
      {#if imageMode === "pixel" && pixelUrl}
        <img alt="" src={pixelUrl} />
      {:else if imageMode === "side"}
        {#if before}<img alt="" src={before} />{/if}
        {#if after}<img alt="" src={after} />{/if}
      {:else}
        {#if before}<img class="under" alt="" src={before} />{/if}
        {#if after}<img class="over" alt="" src={after} style:opacity={imageMode === "onion" ? imagePos / 100 : 1} style:clip-path={imageMode === "swipe" ? `inset(0 ${100 - imagePos}% 0 0)` : "none"} />{/if}
      {/if}
    </div>
  {/snippet}

  {#snippet menuList(items: MenuItem[])}
    {#each items as item, index (`${item.sep ? "sep" : item.label}-${index}`)}
      {#if item.sep}
        <hr />
      {:else}
        <button type="button" disabled={item.disabled} onmouseenter={(event) => { if (!item.children) { submenu = null; return; } const rect = (event.currentTarget as HTMLElement).getBoundingClientRect(); submenu = { x: rect.right - 4, y: rect.top, items: item.children }; }} onclick={() => { if (item.disabled || item.children || !item.run) return; menu = null; submenu = null; void item.run(); }}>
          <span>{item.label}</span>
          {#if item.shortcut}<span class="shortcut">{item.shortcut}</span>{/if}
          {#if item.children}<span class="shortcut">›</span>{/if}
        </button>
      {/if}
    {/each}
  {/snippet}

  {#if menu}
    <div class="menu" style:left="{menu.x}px" style:top="{menu.y}px" role="menu">
      {@render menuList(menu.items)}
    </div>
    {#if submenu}
      <div class="menu" style:left="{submenu.x}px" style:top="{submenu.y}px" role="menu">
        {@render menuList(submenu.items)}
      </div>
    {/if}
    <button class="scrim menu-dismiss" type="button" aria-label={tr("chrome.close")} onclick={() => { menu = null; submenu = null; }}></button>
  {/if}
  {#if confirmAsk}
    <div class="scrim" role="presentation">
      <div class="dialog" role="dialog">
        <h2>{confirmAsk.title}</h2>
        <p>{confirmAsk.body}</p>
        <div class="composer-row">
          <button class="text-button" type="button" onclick={() => (confirmAsk = null)}>{tr("dialog.cancel")}</button>
          <button class="commit" type="button" onclick={() => { const run = confirmAsk?.run; confirmAsk = null; run?.(); }}>{tr("dialog.confirm")}</button>
        </div>
      </div>
    </div>
  {/if}
  {#if forgeOpen}
    <div class="scrim" role="presentation" onclick={() => (forgeOpen = null)} onkeydown={() => {}}>
      <div class="dialog" role="dialog" tabindex="-1" onclick={(event) => event.stopPropagation()} onkeydown={() => {}}>
        <h2>{forgeOpen === "notes" ? tr("chrome.notifications") : tr("chrome.pulls")}</h2>
        {#if forgeOpen === "notes"}
          {#if notices.length === 0}<p class="empty">{tr("dialog.noToken")}</p>{/if}
          {#each notices as notice (notice.id)}
            <div class="composer-row"><span>{notice.title}</span><button class="text-button" type="button" onclick={() => openUrl(notice.url)}>{tr("dialog.openBrowser")}</button></div>
          {/each}
        {:else}
          {#if pulls.length === 0}<p class="empty">{settings.githubToken || settings.gitlabToken || mode !== "live" ? tr("dialog.noPulls") : tr("dialog.noToken")}</p>{/if}
          {#each pulls as pull (pull.url)}
            <div class="composer-row"><span>{pull.title}</span><span class="meta">{pull.author}</span><button class="text-button" type="button" disabled={mode !== "live"} onclick={() => mutate({ action: "checkout", name: pull.branch })}>{tr("dialog.checkoutBranch")}</button></div>
          {/each}
        {/if}
      </div>
    </div>
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
          else if (dialog === "rebase" && selectedCommit) void mutate({ action: "rebaseInteractive", onto: selectedCommit, drop: [], steps: rebaseSteps.map((step) => ({ verb: step.verb, rev: step.rev, message: step.verb === "reword" || step.verb === "squash" ? step.message : "" })), autostash: settings.mergeAutostash, updateRefs });
          else if (dialog === "merge") void mutate({ action: "merge", name: draft, squash: draftExtra === "squash", noFf: draftExtra === "no-ff", autostash: settings.mergeAutostash });
          else if (dialog === "continue") void mutate({ action: "continue", message: draft });
          else if (dialog === "repo") {
            try {
              await invoke("set_repo_author", { path: repoPath(), name: draft, email: draftExtra });
              await mutate({ action: "setSignOff", enabled: repoSignOff, format: draftUser || settings.signOffFormat });
            } catch (error) {
              actionError = message(error);
            }
          }
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
          <h2>{tr("dialog.preferences")}</h2>
          <div class="pref-tabs">
            {#each ["appearance", "git", "diff", "network", "ai", "account"] as tab (tab)}
              <button type="button" class:on={prefTab === tab} onclick={() => (prefTab = tab as typeof prefTab)}>{tr(`dialog.${tab}`)}</button>
            {/each}
          </div>
          {#if prefTab === "appearance"}
          <label>{tr("dialog.language")}
            <select bind:value={settings.locale}>
              <option value="system">{tr("dialog.followSystem")}</option>
              <option value="zh">{tr("dialog.chinese")}</option>
              <option value="en">{tr("dialog.english")}</option>
            </select>
          </label>
          <label>{tr("dialog.theme")}
            <select bind:value={settings.theme}>
              <option value="system">{tr("dialog.system")}</option>
              <option value="light">{tr("dialog.light")}</option>
              <option value="dark">{tr("dialog.dark")}</option>
            </select>
          </label>
          {/if}
          {#if prefTab === "account"}
          <label>{tr("dialog.githubToken")} <input type="password" bind:value={settings.githubToken} /></label>
          <label>{tr("dialog.gitlabToken")} <input type="password" bind:value={settings.gitlabToken} /></label>
          <label>{tr("dialog.gitlabHost")} <input bind:value={settings.gitlabHost} placeholder="gitlab.com" /></label>
          {:else if prefTab === "diff"}
          <label>{tr("dialog.diffTool")} <input bind:value={settings.diffTool} /></label>
          <label>{tr("dialog.mergeTool")} <input bind:value={settings.mergeTool} /></label>
          <p class="empty">{tr("dialog.toolHint")}</p>
          <label class="check"><input type="checkbox" bind:checked={settings.showEntireFile} /> Show the whole file in diffs</label>
          <label>Diff
            <select bind:value={settings.diffStyle}>
              <option value="unified">Unified</option>
              <option value="split">Split</option>
            </select>
          </label>
          <label>Line height
            <select bind:value={settings.linesHeight}>
              <option value="compact">Compact</option>
              <option value="spaced">Spaced</option>
            </select>
          </label>
          {:else if prefTab === "network"}
          <label class="check"><input type="checkbox" bind:checked={settings.sslVerify} /> Verify SSL</label>
          {#if !settings.sslVerify}<p class="empty">SSL verification is off. Connections can be intercepted.</p>{/if}
          <label class="check"><input type="checkbox" bind:checked={settings.proxyEnabled} /> Use proxy host</label>
          <label>Proxy type
            <select bind:value={settings.proxyType}>
              <option value="http">HTTP</option>
              <option value="socks">SOCKS</option>
            </select>
          </label>
          <label>Proxy host <input bind:value={settings.proxyHost} placeholder="proxy.example" /></label>
          <label>Proxy port <input type="number" bind:value={settings.proxyPort} /></label>
          <label>Proxy user <input bind:value={settings.proxyUser} /></label>
          <label>Proxy password <input type="password" bind:value={settings.proxyPassword} /></label>
          <label>Proxy URL <input bind:value={settings.proxy} placeholder="used when host is empty" /></label>
          <label>CA file <input bind:value={settings.sslCaFile} placeholder="path to a CA bundle" /></label>
          <label>Clone directory <input bind:value={settings.cloneDirectory} /></label>
          <label>Terminal <input bind:value={settings.terminal} placeholder="empty opens cmd" /></label>
          {:else if prefTab === "ai"}
          <label>AI base URL <input bind:value={settings.aiBaseUrl} /></label>
          <label>AI model <input bind:value={settings.aiModel} /></label>
          <label class="check"><input type="checkbox" bind:checked={settings.aiEnabled} /> AI commit messages</label>
          <label>AI language <input bind:value={settings.aiLanguage} placeholder="English" /></label>
          <label>AI max characters <input type="number" bind:value={settings.aiMaxChars} /></label>
          <label>AI prompt <input bind:value={settings.aiPrompt} placeholder={'{diff} {files} {branch} {recent_commits} {language}'} /></label>
          <label>AI temperature <input type="number" step="0.1" bind:value={settings.aiTemperature} /></label>
          <label>AI API key <input type="password" bind:value={settings.aiApiKey} placeholder="or set XAI_API_KEY or OPENAI_API_KEY" /></label>
          {:else if prefTab === "git"}
          <label class="check"><input type="checkbox" bind:checked={settings.pullRebase} /> Pull with rebase</label>
          <label class="check"><input type="checkbox" bind:checked={settings.fetchPrune} /> Prune on fetch</label>
          <label class="check"><input type="checkbox" bind:checked={settings.swapPanes} /> Show unstaged above staged</label>
          <label class="check"><input type="checkbox" bind:checked={settings.mergeNoFf} /> Merge with --no-ff</label>
          <label class="check"><input type="checkbox" bind:checked={settings.mergeAutostash} /> Autostash before merge</label>
          <label class="check"><input type="checkbox" bind:checked={settings.signCommits} /> Sign commits with git</label>
          <label class="check"><input type="checkbox" bind:checked={settings.signOff} onchange={() => (signOff = settings.signOff)} /> Sign-off by default</label>
          <label>Sign-off format <input bind:value={settings.signOffFormat} /></label>
          <label class="check"><input type="checkbox" bind:checked={settings.forceWithLease} /> Push with --force-with-lease</label>
          <label class="check"><input type="checkbox" bind:checked={settings.treeFiles} /> Group changed files by folder</label>
          <label class="check"><input type="checkbox" bind:checked={settings.gravatar} /> Gravatar avatars</label>
          <label class="check"><input type="checkbox" bind:checked={settings.dateRelative} /> Relative dates</label>
          <label class="check"><input type="checkbox" bind:checked={settings.date24h} /> 24-hour clock</label>
          <label>Date pattern <input bind:value={settings.dateFormat} placeholder="dd MMM yyyy" /></label>
          <p class="empty">Hidden branches are stored in this repository. Use Hide, Show only this, or Show all in the sidebar. The current branch stays visible.</p>
          <label>Author name <input bind:value={settings.authorName} placeholder="uses git config when empty" /></label>
          <label>Author email <input bind:value={settings.authorEmail} /></label>
          <label>Log directory <input bind:value={settings.logDirectory} placeholder="action name, branch, and path" /></label>
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
          </div>
          {/if}
          <div class="composer-row">
            <button class="commit" type="submit">{tr("dialog.save")}</button>
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
          <h2>{tr("dialog.newBranch")}</h2>
          <input placeholder={tr("dialog.name")} bind:value={draft} />
          <input placeholder={tr("dialog.startPoint")} bind:value={draftExtra} />
          <button class="commit" type="submit">{tr("dialog.create")}</button>
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
          <h2>{tr("dialog.rebase")}</h2>
          <p class="empty">{rebaseConflictFiles.length === 0 ? tr("dialog.conflictNone") : tr("dialog.conflictSome", { name: rebaseConflictFiles.join(", ") })}</p>
          <label class="check"><input type="checkbox" bind:checked={updateRefs} /> {tr("dialog.updateRefs")}</label>
          {#each rebaseSteps as step, index (step.rev)}
            <div class="composer-row" role="listitem" draggable="true" ondragstart={() => (dragStep = index)} ondragover={(event) => event.preventDefault()} ondrop={() => { if (dragStep == null || dragStep === index) return; const copy = [...rebaseSteps]; const [item] = copy.splice(dragStep, 1); copy.splice(index, 0, item); rebaseSteps = copy; dragStep = null; }}>
              <input type="checkbox" checked={rebasePicked.includes(step.rev)} aria-label={step.summary} onchange={() => { rebasePicked = rebasePicked.includes(step.rev) ? rebasePicked.filter((rev) => rev !== step.rev) : [...rebasePicked, step.rev]; }} />
              <select value={step.verb} onchange={(event) => setRebaseVerb(step.rev, event.currentTarget.value)}>
                <option value="pick">pick</option>
                <option value="reword">reword</option>
                <option value="squash">squash</option>
                <option value="fixup">fixup</option>
                <option value="drop">drop</option>
              </select>
              <span>{step.summary}</span>
            </div>
            {#if step.verb === "reword" || step.verb === "squash"}
              <input placeholder="Message" bind:value={step.message} />
            {/if}
          {/each}
          <button class="commit" type="submit" disabled={mode !== "live"}>{tr("dialog.startRebase")}</button>
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
        {:else if dialog === "merge"}
          <h2>Merge {draft}</h2>
          <select bind:value={draftExtra}>
            <option value="ff">Fast-forward when possible</option>
            <option value="no-ff">No fast-forward</option>
            <option value="squash">Squash</option>
          </select>
          <p class="empty">{settings.mergeAutostash ? "Stashes local changes first." : "Local changes must be clean."}</p>
          <button class="commit" type="submit">Merge</button>
        {:else if dialog === "repo"}
          <h2>Repository</h2>
          <label>Author name <input bind:value={draft} placeholder="uses git config when empty" /></label>
          <label>Author email <input bind:value={draftExtra} /></label>
          <label class="check"><input type="checkbox" bind:checked={repoSignOff} /> Sign-off in this repository</label>
          <label>Sign-off format <input bind:value={draftUser} /></label>
          <p class="empty">Author and sign-off are stored in this repository. Preferences still holds the default for new repositories.</p>
          <button class="commit" type="submit">Save</button>
        {:else if dialog === "continue"}
          <h2>Continue rebase</h2>
          <p class="empty">Leave this empty to keep the current message. A message is applied when this step is a reword.</p>
          <input placeholder="New message (optional)" bind:value={draft} />
          <button class="commit" type="submit">Continue</button>
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
    min-width: 220px;
    padding: 4px;
    background: var(--canvas);
    border: 1px solid var(--line);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.16);
    display: flex;
    flex-direction: column;
  }
  .menu button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    width: 100%;
    height: 28px;
    padding: 0 8px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    border-radius: 4px;
  }
  .menu button:disabled { opacity: 0.4; }
  .menu hr { border: 0; border-top: 1px solid var(--line); margin: 4px 6px; }
  .shortcut { color: var(--text-secondary); font-size: 11px; }
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
    position: relative;
    width: auto;
    min-width: 28px;
    height: 36px;
    padding: 0 6px;
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

  .tool svg { width: 16px; height: 16px; }
  .tool.icon-only { width: 32px; padding: 0; position: relative; }
  .badge-count {
    position: absolute;
    top: 1px;
    right: 1px;
    min-width: 14px;
    height: 14px;
    padding: 0 3px;
    border-radius: 7px;
    background: var(--accent);
    color: #fff;
    font-size: 10px;
    line-height: 14px;
  }
  .tab-close { opacity: 0; }
  .tab:hover .tab-close, .tab.active .tab-close { opacity: 1; }
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
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
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
    width: 340px;
    flex: none;
    min-width: 280px;
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
    overflow: hidden;
  }

  .history {
    width: auto;
    flex: 1.1;
    min-width: 320px;
    overflow: auto;
  }

  .pane-head {
    min-height: 28px;
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
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
    flex: none;
    background: var(--canvas);
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
    flex-wrap: wrap;
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
  .ref.local { background: #dbeafe; color: #1d4ed8; }
  .ref.remote { background: #e5e7eb; color: #374151; }
  .ref.tag { background: #dcfce7; color: #166534; }
  .ref.stash { background: #fef3c7; color: #92400e; }
  .graph { display: flex; align-items: center; flex: none; height: 100%; }
  .graph-cell { width: 12px; height: 100%; position: relative; flex: none; }
  .graph-cell.node::after { content: ""; position: absolute; left: 2px; top: 50%; width: 8px; height: 8px; margin-top: -4px; border-radius: 50%; background: currentColor; }
  .graph-cell.line::before { content: ""; position: absolute; left: 5px; top: -1px; bottom: -1px; width: 2px; background: currentColor; }
  .drop-check { opacity: 0; }
  .commit-row:hover .drop-check, .drop-check:checked { opacity: 1; }
  .gutter { width: 32px; flex: none; text-align: right; color: var(--text-secondary); }
  .image-toolbar { display: flex; gap: 6px; align-items: center; padding: 8px 12px; }
  .image-toolbar button { border: 0; background: transparent; font: inherit; color: inherit; padding: 2px 8px; border-radius: 6px; }
  .image-toolbar button.on { background: var(--selection); }
  .image-stage { position: relative; display: flex; gap: 12px; padding: 12px; min-height: 80px; }
  .image-stage img { max-width: 48%; background: repeating-conic-gradient(#ddd 0 25%, #fff 0 50%) 0 0 / 16px 16px; }
  .image-stage .under, .image-stage .over { position: absolute; left: 12px; top: 12px; max-width: calc(100% - 24px); }
  .pref-tabs { display: flex; flex-wrap: wrap; gap: 4px; }
  .pref-tabs button { border: 0; background: transparent; color: inherit; font: inherit; padding: 4px 8px; border-radius: 6px; }
  .pref-tabs button.on { background: var(--selection); }

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
