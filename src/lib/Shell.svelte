<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { commits, diffs, sampleImages, sampleNotices, samplePulls, sampleReflog, unstaged as sampleUnstaged } from "./sample";
  import { highlight } from "./highlight";
  import SplitHandle from "./SplitHandle.svelte";
  import { resolveLocale, translate } from "./i18n";
  import { shortcut, shortcutLabel, typing } from "./keys";
  import { branchGroup, commitGraph, formatWhen, gravatarUrl, laneColors, numberedDiff, splitDiff, windowSlice } from "./view";
  import {
    badge,
    type BlameLine,
    type CommandRecord,
    type CommitDetail,
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
  type TreeEntry = { key: string; kind: "dir" | "file"; path: string; file?: Row; depth: number };

  const defaultSettings: Settings = {
    theme: "system",
    pullRebase: false,
    fetchPrune: true,
    diffStyle: "split",
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
    showDiffMarks: false,
    diffFontSize: 13,
    disableSyntaxHighlight: false,
    commitSort: "date",
    fetchAutomatically: false,
    fetchTags: false,
    tabIndicator: true,
    updateSubmodulesOnCheckout: false,
    branchSpace: "-",
    pushOnCommit: false,
    compactBranchLabels: false,
    messageLow: 50,
    messageHigh: 70,
    spellChecking: "disable",
    pageGuide: 72,
    highlightIssues: true,
    shellKind: "default",
    shellPath: "",
    shellArgs: "",
    diffToolName: "",
    diffToolPath: "",
    diffToolArgs: "",
    mergeToolName: "",
    mergeToolPath: "",
    mergeToolArgs: "",
  };

  const desktopApp = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  let section = $state<"changes" | "history">("changes");
  let selectedPath = $state<string | null>(desktopApp ? null : (sampleUnstaged[0]?.path ?? null));
  let summary = $state("");
  let description = $state("");
  let amend = $state(false);
  let signOff = $state(false);
  let skipHooks = $state(false);
  let repoSignOff = $state(false);
  let selectedSide = $state<Side>("unstaged");
  let mode = $state<Mode>(desktopApp ? "loading" : "sample");
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
  let launchIndex = $state(0);
  let launchList = $state<HTMLElement | null>(null);
  let allBranches = $state(false);
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
  let searchOpen = $state({ recent: false, side: false, commit: false, file: false });
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
  let prefTab = $state<"general" | "commit" | "git" | "integration" | "commands" | "updates">("general");
  let commitDetail = $state<CommitDetail | null>(null);
  let historyTab = $state<"commit" | "changes" | "tree">("commit");
  let collapsedDirs = $state<string[]>([]);
  let collapsedSide = $state<string[]>(["tags", "stashes", "submodules", "worktrees"]);
  let sideSelected = $state<string | null>(null);
  let commitCollapsed = $state<string[]>([]);
  let aiFieldsOpen = $state(false);
  let dragStep = $state<number | null>(null);
  let rebasePicked = $state<string[]>([]);
  let recentStats = $state<Record<string, { branch: string; ahead: number; behind: number; lastSummary: string }>>({});
  let commandOutput = $state("");
  let conflict = $state<ConflictSides | null>(null);
  let logLimit = $state(500);
  let rebaseSteps = $state<{ verb: string; rev: string; summary: string; message: string }[]>([]);
  let comparePair = $state<[string, string] | null>(null);
  const drafts = new Map<string, { summary: string; description: string }>();

  const LAYOUT_KEY = "awegit.layout.settings";
  function getStoredLayout(): {
    sidebarWidth?: number;
    sidebarCollapsed?: boolean;
    changesWidth?: number;
    unstagedHeight?: number;
    composerHeight?: number;
    historyDetailHeight?: number;
  } {
    try {
      const raw = typeof localStorage !== "undefined" ? localStorage.getItem(LAYOUT_KEY) : null;
      if (raw) return JSON.parse(raw);
    } catch {}
    return {};
  }

  function saveLayoutState(partial: Record<string, unknown>) {
    try {
      const cur = getStoredLayout();
      localStorage.setItem(LAYOUT_KEY, JSON.stringify({ ...cur, ...partial }));
    } catch {}
  }

  const initialLayout = getStoredLayout();
  let sidebarWidth = $state<number>(initialLayout.sidebarWidth ?? 240);
  let sidebarCollapsed = $state<boolean>(initialLayout.sidebarCollapsed ?? false);
  let changesWidth = $state<number>(initialLayout.changesWidth ?? 300);
  let unstagedHeight = $state<number>(initialLayout.unstagedHeight ?? 280);
  let composerHeight = $state<number>(initialLayout.composerHeight ?? 170);
  let historyDetailHeight = $state<number>(initialLayout.historyDetailHeight ?? 280);

  function handleLayoutChange() {
    saveLayoutState({
      sidebarWidth,
      sidebarCollapsed,
      changesWidth,
      unstagedHeight,
      composerHeight,
      historyDetailHeight,
    });
  }

  const messageHigh = $derived(settings.messageHigh > 0 ? settings.messageHigh : 70);
  const messageLow = $derived(settings.messageLow > 0 ? settings.messageLow : 50);
  const summaryTooLong = $derived(summary.length > messageHigh);
  const summaryTone = $derived(summary.length > messageHigh ? "over" : summary.length >= messageLow && summary.length > 0 ? "warn" : "");
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
  const rowCommit = $derived(settings.linesHeight === "spaced" ? 36 : 28);
  const rowFile = $derived(settings.linesHeight === "spaced" ? 32 : 28);
  const activeLocale = $derived(resolveLocale(settings.locale));
  function tr(key: string, vars: Record<string, string> = {}) {
    return translate(activeLocale, key, vars);
  }
  const sampleRows = $derived([
    { id: "stash@{0}", summary: "WIP before the palette", author: "", when: "", badges: ["stash"], parents: [commits[0]?.id ?? ""], files: [] as (typeof commits)[number]["files"], unpushed: true },
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
      const extra: CommitRow = { id: stash.id || stash.name, shortId: stash.name, summary: stash.summary, author: "", when: "", parents: [parent], refs: ["stash"], lane: 0, at: 0, email: "", unpushed: true };
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
  const unstagedTree = $derived(asTree(shownUnstaged, collapsedDirs));
  const fileWindow = $derived(windowSlice(unstagedTree, fileTop, rowFile));
  const diffLines = $derived(diff?.lines ?? []);
  const shownDiffLines = $derived(numberedDiff(diffLines));
  const liveGraph = $derived(commitGraph(historyItems.map((commit) => ({ id: commit.id, parents: commit.parents ?? [] }))));
  const diffWindow = $derived(windowSlice(diffLines, diffTop, 18));
  const blameWindow = $derived(windowSlice(blameLines ?? [], blameTop, 18));
  const historyDiffWindow = $derived(windowSlice(historyDiff?.lines ?? [], historyDiffTop, 18));
  const splitRows = $derived(settings.diffStyle === "split" ? splitDiff(diffLines) : []);
  function sampleSplitOf(lines: string[]) {
    return splitDiff(
      lines.map((text) => ({
        kind: text.startsWith("@@") ? "hunk" as const : text.startsWith("+") && !text.startsWith("+++") ? "add" as const : text.startsWith("-") && !text.startsWith("---") ? "delete" as const : "context" as const,
        text,
      })),
    );
  }
  const sampleSplit = $derived(sampleSplitOf(selectedDiff));
  const historySplit = $derived(settings.diffStyle === "split" ? splitDiff(historyDiff?.lines ?? []) : []);
  const filterNames = $derived.by(() => {
    if (historyFilter) return [historyFilter];
    if (allBranches) return [];
    if (mode === "sample") return ["main", "origin/main"];
    const current = refs?.branches.find((branch) => branch.current);
    if (!current) return [];
    const names = [current.name];
    if (current.upstream && !names.includes(current.upstream)) names.push(current.upstream);
    return names;
  });
  const visibleBranches = $derived(
    (refs?.branches ?? []).filter((branch) => (branch.current || !refHidden(branch.name)) && matchesQuery(branch.name, sideQuery)),
  );
  const visibleTags = $derived(
    (refs?.tags ?? []).filter((tag) => !refHidden(tag.name) && matchesQuery(tag.name, sideQuery)),
  );
  type LaunchItem = { label: string; group: string; run: () => void };
  type Place = {
    section: "changes" | "history";
    path: string | null;
    side: Side;
    commit: string | null;
    blame: boolean;
    tree: boolean;
    historyFilter: string;
  };
  let navBack: Place[] = [];
  let navForward: Place[] = [];
  let navLock = 0;
  let lastPlace: Place | null = null;
  let navStamp = 0;

  const matches = $derived.by(() => {
    const query = launch.trim().toLowerCase();
    const action = tr("chrome.groupAction");
    const repoGroup = tr("chrome.groupRepo");
    if (query.startsWith("!")) {
      const command = query.slice(1);
      return [{ label: command ? `! ${command}` : "!", group: action, run: () => mutate({ action: "custom", command }) }];
    }
    const items: LaunchItem[] = [
      { label: tr("chrome.fetch"), group: action, run: () => void doFetch() },
      { label: tr("chrome.fetchAll"), group: action, run: () => void mutate({ action: "fetch", remote: null, prune: settings.fetchPrune, tags: settings.fetchTags }) },
      { label: tr("chrome.pull"), group: action, run: () => void doPull() },
      { label: tr("dialog.fastForward"), group: action, run: () => { settings.pullRebase = false; void doPull(); } },
      { label: tr("chrome.push"), group: action, run: () => void doPush() },
      { label: tr("menu.createTag"), group: action, run: () => void mutate({ action: "push", remote: null, setUpstream: true, tags: true, forceWithLease: settings.forceWithLease }) },
      { label: tr("chrome.stash"), group: action, run: () => { draft = ""; dialog = "stash"; } },
      { label: tr("chrome.pop"), group: action, run: () => void mutate({ action: "stashPop" }) },
      { label: tr("chrome.refresh"), group: action, run: () => void refresh() },
      { label: tr("chrome.stageAll"), group: action, run: () => void runChange("stage_all") },
      { label: tr("chrome.commit"), group: action, run: () => void submitCommit(false) },
      { label: tr("chrome.commitAndPush"), group: action, run: () => void submitCommit(true) },
      { label: tr("chrome.preferences"), group: action, run: () => { dialog = "prefs"; } },
      { label: tr("chrome.terminal"), group: action, run: () => void invoke("open_terminal", { path: repoPath() }) },
      { label: tr("chrome.explorer"), group: action, run: () => { if (snapshot) void openPath(snapshot.path); } },
      { label: tr("chrome.flow"), group: action, run: () => { draft = ""; dialog = "flow"; } },
      { label: tr("chrome.about"), group: action, run: () => { dialog = "about"; } },
      { label: tr("chrome.open"), group: action, run: () => { draft = ""; dialog = "open"; } },
      { label: tr("chrome.workspace"), group: action, run: () => { draft = ""; dialog = "workspace"; } },
      { label: tr("chrome.clone"), group: action, run: () => { draft = ""; draftExtra = settings.cloneDirectory; dialog = "clone"; } },
      { label: tr("chrome.newRepo"), group: action, run: () => { draft = ""; draftExtra = settings.cloneDirectory; dialog = "clone"; } },
      { label: tr("chrome.notifications"), group: action, run: () => openForge("notes") },
      { label: tr("chrome.pulls"), group: action, run: () => openForge("pulls") },
      { label: tr("chrome.changes"), group: action, run: () => { section = "changes"; } },
      { label: tr("chrome.commits"), group: action, run: () => { section = "history"; } },
      ...settings.workspaces.map((workspace) => ({
        label: workspace.name,
        group: tr("chrome.workspace"),
        run: () => {
          settings.currentWorkspace = workspace.id;
          void selectWorkspace();
        },
      })),
      ...[...new Set([...repos, ...settings.recent])].map((path) => ({
        label: folderName(path),
        group: repoGroup,
        run: () => void openRepo(path),
      })),
    ];
    if (mode === "live") {
      items.push(
        ...settings.commands.map((command) => ({ label: command.name, group: action, run: () => runCommand(command) })),
        ...(refs?.branches ?? []).map((branch) => ({
          label: `${tr("menu.checkout")} ${branch.name}`,
          group: tr("chrome.groupBranch"),
          run: () => void mutate({ action: "checkout", name: branch.name }),
        })),
        ...(refs?.tags ?? []).map((tag) => ({
          label: tag.name,
          group: tr("chrome.groupTag"),
          run: () => void mutate({ action: "checkout", name: tag.name }),
        })),
        ...(refs?.remotes ?? []).map((remote) => ({
          label: remote.name,
          group: tr("chrome.groupRemote"),
          run: () => void mutate({ action: "fetch", remote: remote.name, prune: settings.fetchPrune, tags: settings.fetchTags }),
        })),
        ...(refs?.stashes ?? []).map((stash) => ({
          label: stash.summary || stash.name,
          group: tr("chrome.groupStash"),
          run: () => void mutate({ action: "stashPop", name: stash.name }),
        })),
      );
    }
    const seen = new Set<string>();
    const unique: LaunchItem[] = [];
    for (const item of items) {
      const key = `${item.group}\0${item.label}`;
      if (seen.has(key)) continue;
      if (query && !fuzzy(query, item.label)) continue;
      seen.add(key);
      unique.push(item);
    }
    return unique.slice(0, query ? 30 : 60);
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

  async function openLaunch() {
    launch = "";
    launchIndex = 0;
    launchOpen = true;
    await tick();
    launchEl?.focus();
  }

  function closeLaunch() {
    launchOpen = false;
    launch = "";
    launchIndex = 0;
  }

  function runLaunch(item: LaunchItem) {
    closeLaunch();
    item.run();
  }

  function onLaunchKey(event: KeyboardEvent) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      event.stopPropagation();
      launchIndex = Math.min(matches.length - 1, launchIndex + 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      launchIndex = Math.max(0, launchIndex - 1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      const item = matches[launchIndex] ?? matches[0];
      if (item) runLaunch(item);
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      closeLaunch();
    }
  }

  function snapshotPlace(): Place {
    return {
      section,
      path: selectedPath,
      side: selectedSide,
      commit: selectedCommit,
      blame: blameLines != null,
      tree: treeOn,
      historyFilter,
    };
  }

  function samePlace(a: Place, b: Place) {
    return (
      a.section === b.section &&
      a.path === b.path &&
      a.side === b.side &&
      a.commit === b.commit &&
      a.blame === b.blame &&
      a.tree === b.tree &&
      a.historyFilter === b.historyFilter
    );
  }

  async function restorePlace(place: Place) {
    navLock += 1;
    section = place.section;
    selectedSide = place.side;
    selectedPath = place.path;
    selectedCommit = place.commit;
    treeOn = place.tree;
    const filterChanged = historyFilter !== place.historyFilter;
    historyFilter = place.historyFilter;
    if (!place.blame) blameLines = null;
    try {
      if (mode === "live" && filterChanged) {
        if (place.historyFilter) {
          commitsLive = await invoke<CommitRow[]>("file_history", { path: repoPath(), file: place.historyFilter, limit: 200 });
        } else {
          await loadContext();
        }
      }
      if (mode === "live" && place.section === "history" && place.commit) await selectCommit(place.commit);
      if (place.blame && place.path && mode === "live") await showBlame(place.path);
      if (place.tree && place.commit && mode === "live") {
        treePaths = await invoke<string[]>("commit_tree", { path: repoPath(), rev: place.commit });
      }
      if (mode === "live" && place.section === "changes" && place.path) await loadDiff();
    } finally {
      lastPlace = snapshotPlace();
      navLock -= 1;
    }
  }

  function goBack() {
    const now = Date.now();
    if (now - navStamp < 150) return;
    navStamp = now;
    if (navBack.length === 0) return;
    const previous = navBack[navBack.length - 1];
    navForward = [snapshotPlace(), ...navForward].slice(0, 50);
    navBack = navBack.slice(0, -1);
    void restorePlace(previous);
  }

  function goForward() {
    const now = Date.now();
    if (now - navStamp < 150) return;
    navStamp = now;
    if (navForward.length === 0) return;
    const next = navForward[0];
    navBack = [...navBack, snapshotPlace()].slice(-50);
    navForward = navForward.slice(1);
    void restorePlace(next);
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

  function currentFilterRefs() {
    const current = refs?.branches.find((branch) => branch.current);
    if (!current) return [];
    const names = [current.name];
    if (current.upstream && !names.includes(current.upstream)) names.push(current.upstream);
    return names;
  }

  async function loadContext() {
    const path = repoPath();
    refs = await invoke<RefSnapshot>("repository_refs", { path });
    const order = settings.commitSort === "topo" ? "topo" : "date";
    if (historyFilter) {
      commitsLive = await invoke<CommitRow[]>("file_history", { path, file: historyFilter, limit: 200 });
    } else if (!allBranches && currentFilterRefs().length > 0) {
      try {
        commitsLive = await invoke<CommitRow[]>("commit_log", { path, limit: logLimit, order, revs: currentFilterRefs() });
      } catch {
        commitsLive = await invoke<CommitRow[]>("commit_log", { path, limit: logLimit, all: false, order });
      }
    } else {
      commitsLive = await invoke<CommitRow[]>("commit_log", { path, limit: logLimit, all: allBranches || currentFilterRefs().length === 0, order });
    }
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
      const action = String(request.action ?? "");
      if (settings.updateSubmodulesOnCheckout && (action === "checkout" || action === "checkoutRemote") && (refs?.submodules.length ?? 0) > 0) {
        const follow = await invoke<StatusSnapshot & { output?: string }>("mutate", { path: repoPath(), request: { action: "submoduleUpdate" } });
        applySnapshot(follow);
        await loadContext();
      }
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
    return mutate({ action: "fetch", remote: remote ?? null, prune: settings.fetchPrune, tags: settings.fetchTags });
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

  type SearchKey = keyof typeof searchOpen;

  function searchQuery(key: SearchKey) {
    return { recent: recentQuery, side: sideQuery, commit: commitQuery, file: fileQuery }[key];
  }

  function closeSearch(key: SearchKey) {
    searchOpen[key] = false;
    if (key === "recent") recentQuery = "";
    else if (key === "side") sideQuery = "";
    else if (key === "commit") commitQuery = "";
    else fileQuery = "";
  }

  function toggleSearch(key: SearchKey) {
    if (searchOpen[key]) closeSearch(key);
    else searchOpen[key] = true;
  }

  function searchKeys(key: SearchKey, event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    closeSearch(key);
  }

  function searchBlur(key: SearchKey) {
    if (!searchQuery(key).trim()) searchOpen[key] = false;
  }

  function focusOnMount(node: HTMLInputElement) {
    node.focus();
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

  function isCommitUnpushed(commit: { unpushed?: boolean; refs?: string[]; badges?: string[]; id?: string }) {
    if (commit.unpushed === true) return true;
    if (commit.refs?.includes("stash") || commit.badges?.includes("stash") || commit.id?.startsWith("stash@")) return true;
    return false;
  }

  async function showReflog() {
    reflogOn = !reflogOn;
    treeOn = false;
    if (!reflogOn || mode !== "live") return;
    reflogRows = await invoke("reflog", { path: repoPath(), limit: 200 });
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
      if (pushAfter || settings.pushOnCommit) await doPush();
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
    historyTab = "commit";
    const row = commitsLive.find((commit) => commit.id === id);
    commitDetail = {
      id,
      body: "",
      author: row?.author ?? "",
      authorEmail: row?.email ?? "",
      authorAt: row?.at ?? 0,
      committer: row?.author ?? "",
      committerEmail: row?.email ?? "",
      committerAt: row?.at ?? 0,
      parents: row?.parents ?? [],
    };
    try {
      commitDetail = await invoke<CommitDetail>("commit_detail", { path: repoPath(), id });
    } catch (error) {
      actionError = message(error);
    }
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
    if (settings.fetchAutomatically && snapshot?.path === path) void doFetch();
  }

  function composeTool(path: string, args: string) {
    const file = path.trim();
    const rest = args.trim();
    if (!file) return rest;
    const quoted = file.includes(" ") ? `"${file}"` : file;
    return rest ? `${quoted} ${rest}` : quoted;
  }

  function applyToolSettings() {
    settings.diffTool = composeTool(settings.diffToolPath, settings.diffToolArgs);
    settings.mergeTool = composeTool(settings.mergeToolPath, settings.mergeToolArgs);
    if (settings.shellKind === "custom" && settings.shellPath.trim()) {
      settings.terminal = composeTool(settings.shellPath, settings.shellArgs);
    }
  }

  async function savePrefs(close = true) {
    applyToolSettings();
    settings = await invoke<Settings>("save_settings", { values: settings });
    if (close) dialog = null;
  }

  async function pickFolder() {
    if (!inApp()) return;
    try {
      const folder = await invoke<string | null>("pick_directory");
      if (folder) settings = { ...settings, cloneDirectory: folder };
    } catch (error) {
      actionError = message(error);
    }
  }

  function branchName(name: string) {
    const space = settings.branchSpace ?? "-";
    const trimmed = name.trim();
    return space ? trimmed.replaceAll(" ", space) : trimmed;
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

  function sideOpen(key: string) {
    return sideQuery.trim() !== "" || !collapsedSide.includes(key);
  }

  function toggleSide(key: string) {
    collapsedSide = collapsedSide.includes(key) ? collapsedSide.filter((item) => item !== key) : [...collapsedSide, key];
  }

  function pickSide(key: string) {
    sideSelected = key;
    toggleSide(key);
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

  function asTree(files: Row[], collapsed = collapsedDirs): TreeEntry[] {
    type Node = { name: string; path: string; kind: "dir" | "file"; file?: Row; children: Node[] };
    const root: Node = { name: "", path: "", kind: "dir", children: [] };
    for (const file of files) {
      const parts = file.path.split(/[/\\]/);
      let node = root;
      let acc = "";
      parts.forEach((part, index) => {
        acc = acc ? `${acc}/${part}` : part;
        const last = index === parts.length - 1;
        let child = node.children.find((item) => item.name === part);
        if (!child) {
          child = { name: part, path: acc, kind: last ? "file" : "dir", file: last ? file : undefined, children: [] };
          node.children.push(child);
        } else if (last) {
          child.kind = "file";
          child.file = file;
        }
        node = child;
      });
    }
    const sortNodes = (nodes: Node[]) => {
      nodes.sort((a, b) => (a.kind === b.kind ? a.name.localeCompare(b.name) : a.kind === "dir" ? -1 : 1));
      for (const node of nodes) sortNodes(node.children);
    };
    sortNodes(root.children);
    const hidden = new Set(collapsed);
    const flat: TreeEntry[] = [];
    const walk = (nodes: Node[], depth: number) => {
      for (const node of nodes) {
        flat.push({ key: `${node.kind}:${node.path}`, kind: node.kind, path: node.path, file: node.file, depth });
        if (node.kind === "dir" && !hidden.has(node.path)) walk(node.children, depth + 1);
      }
    };
    walk(root.children, 0);
    return flat;
  }

  function toggleDir(path: string) {
    collapsedDirs = collapsedDirs.includes(path) ? collapsedDirs.filter((item) => item !== path) : [...collapsedDirs, path];
  }

  function toggleCommitDir(path: string) {
    commitCollapsed = commitCollapsed.includes(path) ? commitCollapsed.filter((item) => item !== path) : [...commitCollapsed, path];
  }

  function paint(text: string, path: string) {
    if (settings.disableSyntaxHighlight) return [{ text, cls: "" }];
    return highlight(text, path);
  }

  function forkWhen(at: number) {
    if (!at) return "";
    const date = new Date(at * 1000);
    const hours = String(date.getHours()).padStart(2, "0");
    const minutes = String(date.getMinutes()).padStart(2, "0");
    if (activeLocale === "zh") return `${date.getDate()} ${date.getMonth() + 1}月 ${date.getFullYear()} ${hours}:${minutes}`;
    const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    return `${date.getDate()} ${months[date.getMonth()]} ${date.getFullYear()} ${hours}:${minutes}`;
  }

  function avatarColor(name: string) {
    const colors = ["#5b8def", "#e07a3d", "#3aa76d", "#c45b8a", "#7a6ad8", "#c9a227", "#d16464"];
    let hash = 0;
    for (const ch of name) hash = (hash + ch.charCodeAt(0)) % colors.length;
    return colors[hash] ?? colors[0];
  }

  function issueHref(token: string) {
    if (token.startsWith("http")) return token;
    if (!token.startsWith("#")) return "";
    const remote = refs?.remotes.find((item) => item.url)?.url ?? "";
    const number = token.slice(1);
    const github = remote.match(/github\.com[:/]([^/\s]+\/[^/\s.]+)/i);
    if (github) return `https://github.com/${github[1].replace(/\.git$/, "")}/issues/${number}`;
    const gitlab = remote.match(/gitlab[^/:]*[:/]([^/\s]+\/[^/\s.]+)/i);
    if (gitlab) return `https://gitlab.com/${gitlab[1].replace(/\.git$/, "")}/-/issues/${number}`;
    return "";
  }

  function linkParts(text: string) {
    if (!settings.highlightIssues) return [{ text, href: "" }];
    const parts: { text: string; href: string }[] = [];
    const pattern = /https?:\/\/[^\s]+|#\d+/g;
    let cursor = 0;
    for (const match of text.matchAll(pattern)) {
      const index = match.index ?? 0;
      if (index > cursor) parts.push({ text: text.slice(cursor, index), href: "" });
      const token = match[0];
      parts.push({ text: token, href: issueHref(token) });
      cursor = index + token.length;
    }
    if (cursor < text.length) parts.push({ text: text.slice(cursor), href: "" });
    return parts.length > 0 ? parts : [{ text, href: "" }];
  }

  function openBar(event: MouseEvent, items: MenuItem[]) {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    submenu = null;
    menu = { x: rect.left, y: rect.bottom + 2, items };
  }

  function viewItems(): MenuItem[] {
    return [
      { label: tr("chrome.localChanges"), run: () => (section = "changes") },
      { label: tr("chrome.allCommits"), run: () => (section = "history") },
      { label: tr("chrome.fileTree"), disabled: !selectedCommit, run: () => { section = "history"; void openHistoryTab("tree"); } },
      { label: tr("chrome.reflog"), run: () => { section = "history"; void showReflog(); } },
      { label: tr("chrome.loadMore"), disabled: mode !== "live", run: () => { section = "history"; logLimit += 200; void loadContext(); } },
      { label: tr("menu.drop"), disabled: mode !== "live" || drops.length === 0 || busy, run: () => dropCommits() },
      { label: tr("dialog.squash"), disabled: mode !== "live" || !selectedCommit || busy, run: () => { draft = ""; dialog = "squash"; } },
      { label: tr("chrome.compare"), disabled: mode !== "live" || drops.length !== 2, run: () => compareDrops() },
      { label: tr("chrome.refresh"), shortcut: shortcutLabel("refresh"), run: () => void refresh() },
      { label: tr("chrome.preferences"), shortcut: shortcutLabel("settings"), run: () => (dialog = "prefs") },
    ];
  }

  function repositoryItems(): MenuItem[] {
    const locked = mode !== "live";
    return [
      { label: tr("chrome.fetch"), disabled: locked || busy, run: () => void doFetch() },
      { label: tr("chrome.pull"), disabled: locked || busy, run: () => void doPull() },
      { label: tr("chrome.push"), disabled: locked || busy, run: () => void doPush() },
      { label: tr("chrome.stash"), disabled: locked || busy, run: () => { draft = ""; dialog = "stash"; } },
      { sep: true, label: "" },
      { label: tr("menu.createBranch"), run: () => { draft = ""; draftExtra = ""; dialog = "branch"; } },
      { label: tr("menu.createTag"), run: () => { draft = ""; draftExtra = "HEAD"; dialog = "tag"; } },
      { sep: true, label: "" },
      { label: tr("chrome.clone"), run: () => { draft = ""; draftExtra = settings.cloneDirectory; dialog = "clone"; } },
      { label: tr("chrome.open"), run: () => { draft = ""; dialog = "open"; } },
      { label: tr("chrome.newRepo"), run: () => { draft = ""; draftExtra = settings.cloneDirectory; dialog = "clone"; } },
      { sep: true, label: "" },
      { label: tr("chrome.terminal"), disabled: locked, run: () => void invoke("open_terminal", { path: repoPath() }) },
      { label: tr("chrome.explorer"), disabled: !snapshot, run: () => snapshot && openPath(snapshot.path) },
    ];
  }

  function windowItems(): MenuItem[] {
    return [
      { label: tr("chrome.close"), disabled: !snapshot, run: () => snapshot && closeTab(snapshot.path) },
      { label: tr("chrome.notifications"), run: () => void openForge("notes") },
      { label: tr("chrome.pulls"), run: () => void openForge("pulls") },
    ];
  }

  function helpItems(): MenuItem[] {
    return [
      { label: tr("chrome.about"), run: () => (dialog = "about") },
      { label: tr("chrome.download"), run: () => void invoke("check_for_update").then((value) => (updateNotice = value as UpdateNotice | null)) },
    ];
  }

  function commitExtras(): MenuItem[] {
    return [
      { label: `${signOff ? "✓ " : ""}${tr("chrome.signOff")}`, run: () => { signOff = !signOff; if (mode === "live") void mutate({ action: "setSignOff", enabled: signOff, format: refs?.signOffFormat || settings.signOffFormat }); } },
      { label: `${skipHooks ? "✓ " : ""}${tr("chrome.skipHooks")}`, run: () => (skipHooks = !skipHooks) },
      { label: tr("chrome.suggest"), disabled: mode !== "live" || busy, run: () => void suggestMessage() },
      { label: tr("chrome.commitAndPush"), disabled: !canCommit, run: () => void submitCommit(true) },
    ];
  }

  const toolPresets = [
    { name: "Araxis Merge", mergeArgs: "-wait -merge -3 -a1 {ours} {theirs} {working} {file}", diffArgs: "{left} {right}" },
    { name: "Beyond Compare", mergeArgs: "{ours} {theirs} {working} {file}", diffArgs: "{left} {right}" },
    { name: "CodeBuddy", mergeArgs: "{ours} {theirs} {working} {file}", diffArgs: "{left} {right}" },
    { name: "Cursor", mergeArgs: "{working}", diffArgs: "{right}" },
  ];

  function useMergePreset(name: string) {
    const preset = toolPresets.find((item) => item.name === name);
    if (!preset) return;
    settings = { ...settings, mergeToolName: preset.name, mergeToolArgs: preset.mergeArgs };
  }

  function useDiffPreset(name: string) {
    const preset = toolPresets.find((item) => item.name === name);
    if (!preset) return;
    settings = { ...settings, diffToolName: preset.name, diffToolArgs: preset.diffArgs };
  }

  async function openHistoryTab(tab: "commit" | "changes" | "tree") {
    historyTab = tab;
    if (tab !== "tree" || !selectedCommit || mode !== "live" || treePaths.length > 0) return;
    treePaths = await invoke<string[]>("commit_tree", { path: repoPath(), rev: selectedCommit });
  }

  function selectSample(id: string) {
    const commit = sampleRows.find((row) => row.id === id);
    selectedCommit = id;
    selectedPath = id;
    historyTab = "commit";
    historyFile = commit?.files[0]?.path ?? null;
    commitFiles = (commit?.files ?? []).map((file) => ({ path: file.path, kind: file.kind === "A" ? "added" : file.kind === "D" ? "deleted" : "modified", previousPath: null }));
    commitDetail = {
      id,
      body: "",
      author: commit?.author ?? "",
      authorEmail: "",
      authorAt: 0,
      committer: commit?.author ?? "",
      committerEmail: "",
      committerAt: 0,
      parents: commit?.parents ?? [],
    };
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
    if (launchOpen && (action === "up" || action === "down" || action === "exit")) {
      event.preventDefault();
      if (action === "exit") closeLaunch();
      else if (action === "down") launchIndex = Math.min(matches.length - 1, launchIndex + 1);
      else launchIndex = Math.max(0, launchIndex - 1);
      return;
    }
    if (editor && action !== "commit" && action !== "commitPush" && action !== "exit" && action !== "launch") return;
    if (action === "exit") {
      dialog = null;
      closeLaunch();
      blameLines = null;
      return;
    }
    event.preventDefault();
    if (mode !== "live" && action !== "settings" && action !== "zoomIn" && action !== "zoomOut" && action !== "launch" && action !== "back" && action !== "forward" && action !== "open" && action !== "clone" && action !== "init") return;
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
    else if (action === "launch") void openLaunch();
    else if (action === "back") goBack();
    else if (action === "forward") goForward();
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
      searchOpen.commit = true;
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
    root.style.setProperty("--row-file", settings.linesHeight === "spaced" ? "32px" : "28px");
    root.style.setProperty("--row-commit", settings.linesHeight === "spaced" ? "36px" : "28px");
    root.style.setProperty("--row-side", settings.linesHeight === "spaced" ? "32px" : "28px");
  });

  $effect(() => {
    const next = snapshotPlace();
    if (navLock > 0) {
      lastPlace = next;
      return;
    }
    if (!lastPlace) {
      lastPlace = next;
      return;
    }
    if (samePlace(lastPlace, next)) return;
    navBack = [...navBack, lastPlace].slice(-50);
    navForward = [];
    lastPlace = next;
  });

  $effect(() => {
    const max = Math.max(0, matches.length - 1);
    if (launchIndex > max) launchIndex = max;
  });

  $effect(() => {
    if (!launchOpen) return;
    launchIndex;
    queueMicrotask(() => launchList?.querySelector(".on")?.scrollIntoView({ block: "nearest" }));
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
    const onMouseNav = (event: MouseEvent) => {
      if (event.button !== 3 && event.button !== 4) return;
      event.preventDefault();
      if (event.button === 3) goBack();
      else goForward();
    };
    const onPop = () => {
      history.pushState({ awegit: 1 }, "");
      goBack();
    };
    window.addEventListener("keydown", onShortcut);
    window.addEventListener("mousedown", onMouseNav);
    window.addEventListener("mouseup", onMouseNav);
    history.pushState({ awegit: 1 }, "");
    window.addEventListener("popstate", onPop);
    const detachNav = () => {
      window.removeEventListener("keydown", onShortcut);
      window.removeEventListener("mousedown", onMouseNav);
      window.removeEventListener("mouseup", onMouseNav);
      window.removeEventListener("popstate", onPop);
    };
    if (!inApp()) {
      notices = sampleNotices;
      pulls = samplePulls;
      repoStats = { branch: "main", ahead: 5, behind: 0, commits: commits.length, branches: 2, lastSummary: commits[0]?.summary ?? "" };
      return detachNav;
    }
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
      detachNav();
      window.clearTimeout(placeTimer);
      unlisten();
      unmoved();
      unresized();
    };
  });
</script>

<div class="shell" style:--diff-font="{settings.diffFontSize || 13}px" style:--guide="{settings.pageGuide || 72}">
  <nav class="menubar">
    <button type="button" onclick={(event) => openBar(event, viewItems())}>{tr("chrome.view")}</button>
    <button type="button" onclick={(event) => openBar(event, repositoryItems())}>{tr("chrome.repository")}</button>
    <button type="button" onclick={(event) => openBar(event, windowItems())}>{tr("chrome.window")}</button>
    <button type="button" onclick={(event) => openBar(event, helpItems())}>{tr("chrome.help")}</button>
  </nav>
  <header class="chrome">
    <div class="chrome-side tabs">
      {#each repos as repo (repo)}
        <div class="tab" class:active={snapshot?.path === repo} class:dirty={settings.tabIndicator && snapshot?.path === repo && changeCount > 0}>
          <button class="file-select" type="button" onclick={() => openRepo(repo)}>{folderName(repo)}</button>
          <button class="text-button row-action tab-close" type="button" onclick={() => closeTab(repo)} aria-label={tr("chrome.close")}>×</button>
        </div>
      {:else}
        <button class="tab active" type="button">{tabLabel}</button>
      {/each}
    </div>
    <button class="launch-trigger" type="button" onclick={() => void openLaunch()}>
      <span>{tr("chrome.quickLaunch")}</span>
      <kbd>{shortcutLabel("launch")}</kbd>
    </button>
    <div class="chrome-side end">
      <div class="segment">
        <button type="button" disabled={busy} onclick={() => void doFetch()} oncontextmenu={(event) => openMenu(event, [
          { label: tr("chrome.fetch"), shortcut: shortcutLabel("fetch"), disabled: mode !== "live", run: () => void doFetch() },
          { label: tr("chrome.fetchAll"), disabled: mode !== "live", run: () => void mutate({ action: "fetch", remote: null, prune: settings.fetchPrune, tags: settings.fetchTags }) },
        ])}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 2v8M5 7l3 3 3-3M3 13h10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>{tr("chrome.fetch")}</button>
        <button type="button" disabled={busy} onclick={() => void doPull()} oncontextmenu={(event) => openMenu(event, [
          { label: tr("chrome.pull"), shortcut: shortcutLabel("pull"), disabled: mode !== "live", run: () => void doPull() },
          { label: tr("dialog.fastForward"), disabled: mode !== "live", run: () => { settings.pullRebase = false; void doPull(); } },
        ])}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 13V4M5 7l3-3 3 3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>{tr("chrome.pull")}</button>
        <button type="button" disabled={busy} onclick={() => void doPush()} oncontextmenu={(event) => openMenu(event, [
          { label: tr("chrome.push"), shortcut: shortcutLabel("push"), disabled: mode !== "live", run: () => void doPush() },
          { label: tr("menu.createTag"), disabled: mode !== "live", run: () => void mutate({ action: "push", remote: null, setUpstream: true, tags: true, forceWithLease: settings.forceWithLease }) },
        ])}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 14V5M5 8l3-3 3 3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>{tr("chrome.push")}</button>
      </div>
      <button class="icon-btn" type="button" disabled={busy} title={tr("chrome.stash")} onclick={() => { draft = ""; dialog = "stash"; }} oncontextmenu={(event) => openMenu(event, [
        { label: tr("chrome.stash"), shortcut: shortcutLabel("stash"), disabled: mode !== "live", run: () => { draft = ""; dialog = "stash"; } },
        { label: tr("chrome.pop"), disabled: mode !== "live", run: () => void mutate({ action: "stashPop" }) },
      ])}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 5h10v8H3zM5 5V3h6v2" fill="none" stroke="currentColor" stroke-width="1.4"/></svg></button>
      <button class="icon-btn" type="button" disabled={busy} title={tr("chrome.flow")} onclick={() => { draft = ""; dialog = "flow"; }}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 3h4v4H3zM9 9h4v4H9zM5 7v2h4" fill="none" stroke="currentColor" stroke-width="1.4"/></svg></button>
      <button class="icon-btn" type="button" title={tr("chrome.notifications")} onclick={() => openForge("notes")}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 2a4 4 0 0 1 4 4v2l1 2H3l1-2V6a4 4 0 0 1 4-4zM6.5 12a1.5 1.5 0 0 0 3 0" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>{#if notices.some((item) => item.unread)}<span class="count badge-count">{notices.filter((item) => item.unread).length}</span>{/if}</button>
      <button class="icon-btn" type="button" title={tr("chrome.terminal")} onclick={() => invoke("open_terminal", { path: repoPath() })}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 4l4 4-4 4M8 12h5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg></button>
      <button class="icon-btn" type="button" title={tr("chrome.explorer")} onclick={() => snapshot && openPath(snapshot.path)}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2 4h5l1 2h6v7H2z" fill="none" stroke="currentColor" stroke-width="1.4"/></svg></button>
      <button class="icon-btn" type="button" title={tr("chrome.preferences")} onclick={() => (dialog = "prefs")}><svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="2.2" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M8 1.8v2M8 12.2v2M1.8 8h2M12.2 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M12.6 3.4l-1.4 1.4M4.8 11.2l-1.4 1.4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/></svg></button>
      <button class="icon-btn" type="button" title={tr("chrome.pulls")} onclick={() => openForge("pulls")}><svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="4" cy="4" r="1.4" fill="none" stroke="currentColor"/><circle cx="12" cy="12" r="1.4" fill="none" stroke="currentColor"/><path d="M4 5.5v5a2 2 0 0 0 2 2h4.2" fill="none" stroke="currentColor" stroke-width="1.3"/></svg></button>
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

  {#if !snapshot && mode !== "sample"}
    <div class="welcome-view">
      <div class="welcome-card">
        <div class="welcome-brand">
          <svg class="welcome-logo" viewBox="0 0 32 32" fill="none">
            <circle cx="16" cy="16" r="14" stroke="var(--accent)" stroke-width="2.5" />
            <circle cx="11" cy="11" r="3" fill="var(--accent)" />
            <circle cx="21" cy="14" r="3" fill="var(--accent)" />
            <circle cx="11" cy="21" r="3" fill="var(--accent)" />
            <path d="M11 11v10M11 16c4 0 10-2 10-2" stroke="var(--accent)" stroke-width="2" stroke-linecap="round" />
          </svg>
          <div class="welcome-titles">
            <h1>AweGit</h1>
            <p>{tr("chrome.openRepo")}</p>
          </div>
        </div>

        {#if loadError}
          <div class="welcome-error">
            <span>⚠️ {loadError}</span>
          </div>
        {/if}

        <div class="welcome-actions">
          <button class="welcome-btn" type="button" onclick={() => (dialog = "open")}>
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M2 4a1 1 0 0 1 1-1h3.5l1.5 2H13a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V4z"/></svg>
            <div class="btn-meta">
              <strong>{tr("chrome.open")}</strong>
              <span>{tr("chrome.openRepo")}</span>
            </div>
          </button>
          <button class="welcome-btn" type="button" onclick={() => { draft = ""; draftExtra = settings.cloneDirectory; dialog = "clone"; }}>
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M3 11V4a1 1 0 0 1 1-1h7a1 1 0 0 1 1 1v7M6 14h7M10 11l3 3-3 3"/></svg>
            <div class="btn-meta">
              <strong>{tr("chrome.clone")}</strong>
              <span>{tr("chrome.clone")}</span>
            </div>
          </button>
          <button class="welcome-btn" type="button" onclick={() => { draft = ""; dialog = "clone"; }}>
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M8 3v10M3 8h10"/></svg>
            <div class="btn-meta">
              <strong>{tr("chrome.newRepo")}</strong>
              <span>{tr("chrome.new")}</span>
            </div>
          </button>
        </div>

        <div class="welcome-recent">
          <div class="recent-bar">
            <h3>{tr("chrome.recent")}</h3>
            {#if settings.recent.length > 2}
              <div class="search-wrap">
                {#if searchOpen.recent}
                  <input class="search recent-input" placeholder={tr("chrome.searchRecent")} aria-label={tr("chrome.searchRecent")} bind:value={recentQuery} use:focusOnMount onkeydown={(event) => searchKeys("recent", event)} onblur={() => searchBlur("recent")} />
                {/if}
                {@render searchToggle("recent", tr("chrome.searchRecent"))}
              </div>
            {/if}
          </div>
          <div class="recent-list">
            {#each settings.recent.filter((path) => matchesQuery(path, recentQuery)) as path (path)}
              <button class="recent-card" type="button" onclick={() => openRepo(path)}>
                <div class="recent-main">
                  <span class="recent-title">{folderName(path)}</span>
                  <span class="recent-path" title={path}>{path}</span>
                </div>
                {#if recentStats[path]}
                  <span class="recent-stats">{recentStats[path].branch} · ↑{recentStats[path].ahead} ↓{recentStats[path].behind} · {recentStats[path].lastSummary}</span>
                {/if}
              </button>
            {:else}
              <p class="empty">{tr("chrome.noMatch")}</p>
            {/each}
          </div>
        </div>
      </div>
    </div>
  {:else}
  <div class="body">
    {#if !sidebarCollapsed}
    <aside class="sidebar" style:width="{sidebarWidth}px">
      <button class="side nav" type="button" class:selected={section === "changes"} onclick={() => (section = "changes")}>
        <span class="name">{tr("chrome.localChanges")}</span>
        {#if changeCount > 0}<span class="count">{changeCount}</span>{/if}
      </button>
      <button class="side nav" type="button" class:selected={section === "history"} onclick={() => (section = "history")}>
        <span class="name">{tr("chrome.allCommits")}</span>
      </button>
      <div class="side-search">
        {#if searchOpen.side}
          <input class="search" placeholder={tr("chrome.filterSidebar")} aria-label={tr("chrome.filterSidebar")} bind:value={sideQuery} use:focusOnMount onkeydown={(event) => searchKeys("side", event)} onblur={() => searchBlur("side")} />
        {/if}
        {@render searchToggle("side", tr("chrome.filterSidebar"))}
      </div>

      {#snippet sideHead(key: string, label: string, count: number | null = null, actions: { label: string; run: () => void }[] = [], empty = count === 0)}
        <div class="side section" class:selected={sideSelected === key} role="group">
          <button class="file-select" type="button" aria-expanded={empty ? undefined : sideOpen(key)} onclick={() => (empty ? (sideSelected = key) : pickSide(key))}>
            <span class="twist">{empty ? "" : sideOpen(key) ? "▾" : "▸"}</span>
            <span class="name">{label}</span>
            {#if count !== null}<span class="count">{count}</span>{/if}
          </button>
          {#each actions as action (action.label)}
            <button class="text-button" type="button" onclick={action.run}>{action.label}</button>
          {/each}
        </div>
      {/snippet}

      {#if mode === "live" && refs}
        {@render sideHead("branches", tr("chrome.localBranches"), null, [
          { label: tr("chrome.new"), run: () => { draft = ""; draftExtra = ""; dialog = "branch"; } },
          { label: tr("chrome.showAll"), run: showAllRefs },
        ], visibleBranches.length === 0)}
        {#if sideOpen("branches")}
        {#each visibleBranches as branch, index (branch.name)}
          {@const group = branchGroup(branch.name)}
          {#if group && group !== branchGroup(visibleBranches[index - 1]?.name ?? "")}
            <button class="side nested quiet" type="button" class:selected={sideSelected === `local:${group}/`} aria-expanded={groupOpen(branch.name)} onclick={() => { sideSelected = `local:${group}/`; toggleGroup(group); }}>
              <span class="twist">{groupOpen(branch.name) ? "▾" : "▸"}</span>
              <span class="name">{group}</span>
            </button>
          {/if}
          {#if groupOpen(branch.name)}
          <div
            class="side nested"
            class:deep={!!group}
            class:current={branch.current}
            class:selected={sideSelected === `local:${branch.name}`}
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
            <button class="file-select" type="button" title={branch.name} onclick={() => (sideSelected = `local:${branch.name}`)} ondblclick={() => !branch.current && mutate({ action: "checkout", name: branch.name })}>
              <span class="twist">{#if branch.current}<span class="dot"></span>{/if}</span>
              <span class="name">{group ? branch.name.slice(group.length + 1) : branch.name}</span>
              {#if branch.ahead > 0}<span class="ahead">↑{branch.ahead}</span>{/if}
              {#if branch.behind > 0}<span class="ahead">↓{branch.behind}</span>{/if}
            </button>
          </div>
          {/if}
        {/each}
        {/if}
        {@render sideHead("remotes", tr("chrome.remotes"), null, [
          { label: tr("chrome.add"), run: () => { draft = ""; draftExtra = ""; dialog = "remote"; } },
        ], refs.remotes.length === 0)}
        {#if sideOpen("remotes")}
        {#each refs.remotes as remote (remote.name)}
          {#if matchesQuery(remote.name, sideQuery) || remote.branches.some((branch) => matchesQuery(branch, sideQuery))}
          {@const remoteKey = `remote:${remote.name}`}
          {@const remoteEmpty = remote.branches.length === 0 && !remote.head}
          <div class="side nested quiet" class:selected={sideSelected === remoteKey} role="group" oncontextmenu={(event) => openMenu(event, [
            { label: tr("chrome.fetch"), run: () => mutate({ action: "fetch", remote: remote.name, prune: settings.fetchPrune, tags: settings.fetchTags }) },
            { label: tr("dialog.url"), run: () => { draft = remote.name; draftExtra = remote.url ?? ""; dialog = "remote"; } },
            { label: tr("menu.remove"), run: () => mutate({ action: "removeRemote", name: remote.name }) },
          ])}>
            <button class="file-select" type="button" aria-expanded={remoteEmpty ? undefined : sideOpen(remoteKey)} title={remote.url ?? ""} onclick={() => (remoteEmpty ? (sideSelected = remoteKey) : pickSide(remoteKey))}>
              <span class="twist">{remoteEmpty ? "" : sideOpen(remoteKey) ? "▾" : "▸"}</span>
              <span class="name">{remote.name}</span>
              <span class="count">{remote.branches.length}</span>
            </button>
          </div>
          {#if sideOpen(remoteKey)}
          {#if remote.head && !sideQuery.trim()}<div class="side nested deep quiet"><span class="twist"></span><span class="name">HEAD → {remote.head}</span></div>{/if}
          {#each remote.branches as branch, index (remote.name + branch)}
            {@const folder = branchGroup(branch)}
            {@const folderKey = `${remoteKey}/${folder}`}
            {#if folder && folder !== branchGroup(remote.branches[index - 1] ?? "") && remote.branches.some((item) => branchGroup(item) === folder && matchesQuery(item, sideQuery))}
              <button class="side nested deep quiet" type="button" class:selected={sideSelected === folderKey} aria-expanded={sideOpen(folderKey)} onclick={() => pickSide(folderKey)}>
                <span class="twist">{sideOpen(folderKey) ? "▾" : "▸"}</span>
                <span class="name">{folder}</span>
              </button>
            {/if}
            {#if matchesQuery(branch, sideQuery) && (!folder || sideOpen(folderKey))}
            <div
              class="side nested deep quiet"
              class:deeper={!!folder}
              class:selected={sideSelected === `${remoteKey}:${branch}`}
              role="group"
              oncontextmenu={(event) => openMenu(event, [
                { label: tr("menu.checkout"), run: () => mutate({ action: "checkoutRemote", remote: remote.name, branch }) },
                { label: tr("menu.pullInto"), run: () => mutate({ action: "pullRef", remote: remote.name, branch, rebase: settings.pullRebase, autostash: settings.mergeAutostash }) },
                { label: tr("menu.pushHere"), run: () => mutate({ action: "pushRef", remote: remote.name, branch, forceWithLease: settings.forceWithLease }) },
                { label: tr("menu.deleteRemote"), run: () => ask(tr("menu.deleteRemote"), tr("dialog.deleteRemoteBody", { name: `${remote.name}/${branch}` }), () => void mutate({ action: "deleteRemoteBranch", remote: remote.name, branch })) },
                { label: tr("menu.copyName"), run: () => copyText(`${remote.name}/${branch}`) },
              ])}
            >
              <button class="file-select" type="button" title={`${remote.name}/${branch}`} onclick={() => (sideSelected = `${remoteKey}:${branch}`)} ondblclick={() => mutate({ action: "checkoutRemote", remote: remote.name, branch })}>
                <span class="twist"></span>
                <span class="name">{folder ? branch.slice(folder.length + 1) : branch}</span>
              </button>
            </div>
            {/if}
          {/each}
          {/if}
          {/if}
        {/each}
        {/if}
      {:else}
        {@render sideHead("branches", tr("chrome.localBranches"))}
        {#if sideOpen("branches")}
        <div
          class="side nested current"
          class:selected={sideSelected === `local:${branchLabel}`}
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
          <button class="file-select" type="button" onclick={() => (sideSelected = `local:${branchLabel}`)}>
            <span class="twist"><span class="dot"></span></span>
            <span class="name">{branchLabel}</span>
            {#if mode === "sample"}<span class="ahead">↑5</span>{/if}
          </button>
        </div>
        {#if repoStats}<p class="empty">{repoStats.branch} · {repoStats.commits} {tr("chrome.commits")} · {repoStats.branches} {tr("chrome.branches")} · ↑{repoStats.ahead} ↓{repoStats.behind}</p>{/if}
        {/if}
        {@render sideHead("remotes", tr("chrome.remotes"))}
        {#if sideOpen("remotes")}
        <div class="side nested quiet" class:selected={sideSelected === "remote:origin"}>
          <button class="file-select" type="button" aria-expanded={sideOpen("remote:origin")} onclick={() => pickSide("remote:origin")}>
            <span class="twist">{sideOpen("remote:origin") ? "▾" : "▸"}</span>
            <span class="name">origin</span>
            <span class="count">1</span>
          </button>
        </div>
        {#if sideOpen("remote:origin")}
        <div class="side nested deep quiet"><span class="twist"></span><span class="name">HEAD → main</span></div>
        <div
          class="side nested deep quiet"
          class:selected={sideSelected === "remote:origin:main"}
          role="group"
          oncontextmenu={(event) => openMenu(event, [
            { label: tr("menu.checkout"), disabled: true },
            { label: tr("menu.pullInto"), disabled: true },
            { label: tr("menu.pushHere"), disabled: true },
            { label: tr("menu.deleteRemote"), run: () => ask(tr("menu.deleteRemote"), tr("dialog.deleteRemoteBody", { name: "origin/main" }), () => {}) },
            { label: tr("menu.copyName"), run: () => copyText("origin/main") },
          ])}
        ><button class="file-select" type="button" onclick={() => (sideSelected = "remote:origin:main")}><span class="twist"></span><span class="name">main</span></button></div>
        {/if}
        {/if}
      {/if}
      {@render sideHead("tags", tr("chrome.tags"), visibleTags.length)}
      {#if sideOpen("tags")}
        {#each visibleTags as tag (tag.name)}
          <div class="side nested quiet" class:selected={sideSelected === `tag:${tag.name}`} role="group" oncontextmenu={(event) => openMenu(event, [
            { label: tr("menu.checkout"), disabled: mode !== "live", run: () => mutate({ action: "checkout", name: tag.name }) },
            { label: tr("menu.delete"), disabled: mode !== "live", run: () => ask(tr("menu.delete"), tag.name, () => void mutate({ action: "deleteTag", name: tag.name })) },
            { label: tr("menu.copyName"), run: () => copyText(tag.name) },
          ])}>
            <button class="file-select" type="button" title={tag.name} onclick={() => (sideSelected = `tag:${tag.name}`)} ondblclick={() => mode === "live" && mutate({ action: "checkout", name: tag.name })}><span class="twist"></span><span class="name">{tag.name}</span></button>
          </div>
        {/each}
      {/if}
      {@render sideHead("stashes", tr("chrome.stashes"), refs?.stashes.length ?? (mode === "sample" ? 1 : 0))}
      {#if sideOpen("stashes")}
        {#each (refs?.stashes ?? (mode === "sample" ? [{ name: "stash@{0}", summary: "WIP before the palette" }] : [])) as stash (stash.name)}
          <div class="side nested quiet" class:selected={sideSelected === `stash:${stash.name}`} role="group" oncontextmenu={(event) => openMenu(event, [
            { label: tr("menu.apply"), disabled: mode !== "live", run: () => mutate({ action: "stashApply", name: stash.name }) },
            { label: tr("menu.pop"), disabled: mode !== "live", run: () => mutate({ action: "stashPop", name: stash.name }) },
            { label: tr("menu.drop"), disabled: mode !== "live", run: () => ask(tr("menu.drop"), stash.summary, () => void mutate({ action: "stashDrop", name: stash.name })) },
          ])}>
            <button class="file-select" type="button" title={stash.name} onclick={() => (sideSelected = `stash:${stash.name}`)}><span class="twist"></span><span class="name">{stash.summary}</span></button>
          </div>
        {/each}
      {/if}
      {@render sideHead("submodules", tr("chrome.submodules"), refs?.submodules.length ?? 0)}
      {#if sideOpen("submodules")}
        {#each refs?.submodules ?? [] as row (row.path)}
          <div class="side nested quiet" class:selected={sideSelected === `submodule:${row.path}`} role="group" oncontextmenu={(event) => openMenu(event, [
            { label: tr("menu.initialize"), disabled: mode !== "live" || row.ready, run: () => mutate({ action: "submoduleInit", path: row.path }) },
            { label: tr("menu.openFile"), disabled: !row.ready, run: () => openRepo(fullPath(row.path)) },
            { label: tr("menu.sync"), disabled: mode !== "live" || !row.ready, run: () => mutate({ action: "submoduleSync", path: row.path }) },
            { label: tr("menu.update"), disabled: mode !== "live" || !row.ready, run: () => mutate({ action: "submoduleUpdate" }) },
            { label: tr("menu.delete"), disabled: mode !== "live", run: () => ask(tr("menu.delete"), row.path, () => void mutate({ action: "submoduleRemove", path: row.path })) },
          ])}>
            <button class="file-select" type="button" title={row.path} onclick={() => (sideSelected = `submodule:${row.path}`)} ondblclick={() => row.ready && openRepo(fullPath(row.path))}><span class="twist"></span><span class="name">{row.path}</span></button>
          </div>
        {/each}
      {/if}
      {@render sideHead("worktrees", tr("chrome.worktrees"), refs?.worktrees.length ?? (mode === "sample" ? 1 : 0))}
      {#if sideOpen("worktrees")}
        {#each (refs?.worktrees ?? (mode === "sample" ? [{ path: "this repository", branch: "main" }] : [])) as row (row.path)}
          <div class="side nested quiet" class:selected={sideSelected === `worktree:${row.path}`} role="group" oncontextmenu={(event) => openMenu(event, [
            { label: tr("menu.openFile"), disabled: mode !== "live", run: () => openRepo(row.path) },
            { label: tr("menu.remove"), disabled: mode !== "live", run: () => ask(tr("menu.remove"), row.path, () => void mutate({ action: "removeWorktree", path: row.path })) },
          ])}>
            <button class="file-select" type="button" title={row.path} onclick={() => (sideSelected = `worktree:${row.path}`)} ondblclick={() => mode === "live" && openRepo(row.path)}><span class="twist"></span><span class="name">{row.branch ?? folderName(row.path)}</span></button>
          </div>
        {/each}
      {/if}
    </aside>
    {/if}

    <SplitHandle
      direction="vertical"
      bind:size={sidebarWidth}
      min={180}
      max={480}
      defaultSize={240}
      collapsible
      bind:collapsed={sidebarCollapsed}
      onchange={handleLayoutChange}
    />

    {#if section === "changes"}
      <section class="changes-column" style:width="{changesWidth}px">
        {#if !settings.swapPanes}
          <div class="status-pane unstaged-pane" style:height="{unstagedHeight}px">
            {@render unstagedPane()}
          </div>

          <SplitHandle
            direction="horizontal"
            bind:size={unstagedHeight}
            min={100}
            max={650}
            defaultSize={280}
            onchange={handleLayoutChange}
          />

          <div class="status-pane staged-pane flex-fill">
            {@render stagedPane()}
          </div>
        {:else}
          <div class="status-pane staged-pane" style:height="{unstagedHeight}px">
            {@render stagedPane()}
          </div>

          <SplitHandle
            direction="horizontal"
            bind:size={unstagedHeight}
            min={100}
            max={650}
            defaultSize={280}
            onchange={handleLayoutChange}
          />

          <div class="status-pane unstaged-pane flex-fill">
            {@render unstagedPane()}
          </div>
        {/if}
      </section>

      <SplitHandle
        direction="vertical"
        bind:size={changesWidth}
        min={220}
        max={650}
        defaultSize={300}
        onchange={handleLayoutChange}
      />

      <div class="right-stage flex-fill">
        <section class="diff flex-fill">
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
              <button class="text-button" type="button" disabled={busy} onclick={() => runChange(selectedSide === "staged" ? "unstage_path" : "stage_path", { file: selectedPath })}>{selectedSide === "staged" ? tr("menu.unstage") : tr("chrome.stage")}</button>
              <button class="text-button" type="button" disabled={busy} onclick={() => ask(tr("menu.discard"), tr("dialog.discardBody", { name: selectedPath ?? "" }), () => void mutate({ action: "discard", file: selectedPath }))}>{tr("menu.discard")}</button>
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
                  <div class="split-row actionable">
                    <span class:del={row.leftKind === "delete"} class:meta={row.leftKind === "meta"}>{#if settings.showDiffMarks && row.leftKind === "delete"}-{/if}{#each paint(row.left, selectedPath ?? "") as token, tokenIndex (`l${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
                    <span class:add={row.rightKind === "add"} class:meta={row.rightKind === "meta"}>{#if settings.showDiffMarks && row.rightKind === "add"}+{/if}{#each paint(row.right, selectedPath ?? "") as token, tokenIndex (`r${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
                    <span class="split-actions">
                      {#if row.hunkIndex != null}
                        <button class="text-button line-action" type="button" disabled={busy} onclick={() => mutate({ action: "stageHunk", file: selectedPath, index: row.hunkIndex, unstage: selectedSide === "staged" })}>{selectedSide === "staged" ? tr("menu.unstageHunk") : tr("menu.stageHunk")}</button>
                        {#if selectedSide === "unstaged"}
                          <button class="text-button line-action" type="button" disabled={busy} onclick={() => mutate({ action: "discardHunk", file: selectedPath, index: row.hunkIndex })}>{tr("menu.discardHunk")}</button>
                        {/if}
                      {:else if row.leftLine?.stageAt != null || row.rightLine?.stageAt != null}
                        {@const line = row.rightLine?.stageAt != null ? row.rightLine : row.leftLine}
                        {#if line}
                          <button class="text-button line-action" type="button" disabled={busy} onclick={() => stageOne(line)}>{selectedSide === "staged" ? tr("menu.unstageLine") : tr("menu.stageLine")}</button>
                          {#if selectedSide === "unstaged" && line.workAt != null}
                            <button class="text-button line-action" type="button" disabled={busy} onclick={() => mutate({ action: "discardLine", file: selectedPath, text: lineText(line), addition: line.kind === "add", at: line.workAt })}>{tr("menu.discardLine")}</button>
                          {/if}
                        {/if}
                      {/if}
                    </span>
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
                      {#each paint(line.text, selectedPath ?? "") as token, tokenIndex (`${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}
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
            <header class="diff-head">
              <span>{selectedPath}</span>
              <button class="text-button" type="button" disabled>{tr("chrome.stage")}</button>
              <button class="text-button" type="button" disabled>{tr("menu.discard")}</button>
            </header>
            {#if settings.diffStyle === "split"}
              <div class="diff-body split">
                {#each sampleSplit as row, index (index)}
                  <div class="split-row">
                    <span class:del={row.leftKind === "delete"} class:meta={row.leftKind === "meta"}>{#if settings.showDiffMarks && row.leftKind === "delete"}-{/if}{#each paint(row.left, selectedPath ?? "") as token, tokenIndex (`sl${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
                    <span class:add={row.rightKind === "add"} class:meta={row.rightKind === "meta"}>{#if settings.showDiffMarks && row.rightKind === "add"}+{/if}{#each paint(row.right, selectedPath ?? "") as token, tokenIndex (`sr${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
                  </div>
                {/each}
              </div>
            {:else}
              <pre class="diff-body">{#each selectedDiff as line, index (index)}<span class:add={line.startsWith("+") && !line.startsWith("+++")} class:del={line.startsWith("-") && !line.startsWith("---")} class:hunk={line.startsWith("@@")}>{line + "\n"}</span>{/each}</pre>
            {/if}
          {:else if selectedPath === "art/mark.png"}
            {@render imageCompare(sampleImages.before, sampleImages.after)}
          {:else}
            <p class="diff-empty">{tr("chrome.selectFile")}</p>
          {/if}
        </section>

        <SplitHandle
          direction="horizontal"
          bind:size={composerHeight}
          min={120}
          max={450}
          defaultSize={170}
          reverse
          onchange={handleLayoutChange}
        />

        <form class="composer" style:height="{composerHeight}px" onsubmit={(event) => { event.preventDefault(); void submitCommit(false); }}>
          <input class="summary-input {summaryTone} guided" placeholder={tr("chrome.commitSubject")} bind:value={summary} maxlength="200" aria-invalid={summaryTooLong} spellcheck={settings.spellChecking === "enable"} />
          <textarea class="guided composer-desc flex-fill" placeholder={tr("chrome.description")} bind:value={description} spellcheck={settings.spellChecking === "enable"}></textarea>
          <div class="composer-row">
            <label class="check"><input type="checkbox" bind:checked={amend} /> {tr("chrome.amend")}</label>
            <button class="text-button more" type="button" onclick={(event) => openBar(event, commitExtras())}>⋯</button>
            <button class="commit" type="submit" disabled={!canCommit}>{busy ? tr("chrome.working") : tr("chrome.commit")}</button>
          </div>
        </form>
      </div>
    {:else}
      <div class="history-stage flex-fill">
        <section class="history flex-fill" bind:this={historyEl} onscroll={(event) => (historyTop = (event.currentTarget as HTMLElement).scrollTop)}>
          {#if filterNames.length > 0}
            <div class="filter-bar">
              <span>{tr("chrome.filteredBy", { name: filterNames.map((name) => `'${name}'`).join(", ") })}</span>
              <button class="text-button" type="button" onclick={() => { allBranches = true; historyFilter = ""; void loadContext(); }}>{tr("chrome.clearFilter")}</button>
            </div>
          {/if}
          <div class="pane-head">
            {#if searchOpen.commit}
              <input class="search" placeholder={tr("chrome.findCommits")} aria-label={tr("chrome.findCommits")} bind:this={searchEl} bind:value={commitQuery} use:focusOnMount onkeydown={(event) => searchKeys("commit", event)} onblur={() => searchBlur("commit")} />
            {:else}
              <span class="pane-title">{tr("chrome.allCommits")}</span>
            {/if}
            {@render searchToggle("commit", tr("chrome.findCommits"))}
            {#if historyFilter}
              <button class="text-button" type="button" onclick={() => { historyFilter = ""; void loadContext(); }}>{tr("chrome.allCommits")}</button>
            {/if}
          </div>
          {#if historyFilter}
            <div class="pane-head"><span>{historyFilter}</span></div>
          {/if}
          {#if reflogOn}
            {#each reflogRows as row (`${row.selector}-${row.id}`)}
              <button class="commit-row" type="button" oncontextmenu={(event) => openMenu(event, reflogMenu(row))}>
                <span class="subject">{row.summary}</span>
                <span class="commit-side">
                  <span class="meta selector" title={row.selector}>{row.selector}</span>
                  <span class="meta sha">{row.shortId}</span>
                </span>
              </button>
            {/each}
          {:else if mode !== "sample"}
            {#if shownCommits.length === 0}
              <p class="empty">{tr("chrome.noCommits")}</p>
            {:else}
              <div class="commit-window" style:height="{shownCommits.length * rowCommit}px">
                {#each historyRows as commit, index (commit.id)}
                  <div
                    class="commit-row virtual"
                    class:selected={selectedCommit === commit.id}
                    class:unpushed={isCommitUnpushed(commit)}
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
                        <i class="graph-cell {cell?.role ?? ""}" style:color={cell ? laneColors[cell.color] : "transparent"}>
                          {#if cell?.forkFromLeft}
                            <svg class="graph-curve" viewBox="0 0 12 28" preserveAspectRatio="none"><path d="M -6 14 C 0 14, 6 20, 6 28" fill="none" stroke="currentColor" stroke-width="2" /></svg>
                          {/if}
                          {#if cell?.mergeToLeft}
                            <svg class="graph-curve" viewBox="0 0 12 28" preserveAspectRatio="none"><path d="M 6 14 C 6 22, -6 28, -6 28" fill="none" stroke="currentColor" stroke-width="2" /></svg>
                          {/if}
                        </i>
                      {/each}
                    </span>
                    <span class="subject">{@render linked(commit.summary)}</span>
                    <span class="badges">
                      {#each commit.refs as label (label)}<span class="ref {refKind(label)}" class:compact={settings.compactBranchLabels}>{label}</span>{/each}
                    </span>
                    <span class="commit-side">
                      <span class="commit-author">
                        {#if settings.gravatar && commit.email}
                          <img class="avatar tile" alt="" src={gravatarUrl(commit.email)} />
                        {:else}
                          <span class="avatar tile" style:background={avatarColor(commit.author)} title={commit.author}>{commit.author.slice(0, 1).toUpperCase()}</span>
                        {/if}
                        <span class="meta author" title={commit.author}>{commit.author}</span>
                      </span>
                      <span class="meta sha">{commit.shortId}</span>
                      <span class="meta date" title={commit.at ? forkWhen(commit.at) : commit.when}>{commit.at ? forkWhen(commit.at) : commit.when}</span>
                    </span>
                  </div>
                {/each}
              </div>
            {/if}
          {:else}
            {#if filterNames.length > 0}
              <div class="filter-bar">
                <span>{tr("chrome.filteredBy", { name: filterNames.map((name) => `'${name}'`).join(", ") })}</span>
                <button class="text-button" type="button" onclick={() => (allBranches = true)}>{tr("chrome.clearFilter")}</button>
              </div>
            {/if}
            {#if repoStats}
              <div class="pane-head"><span class="meta">{repoStats.lastSummary}</span></div>
            {/if}
            {#if reflogOn}
              {#each sampleReflog as row (row.selector)}
                <button class="commit-row" type="button" oncontextmenu={(event) => openMenu(event, reflogMenu(row))}>
                  <span class="subject">{row.summary}</span>
                  <span class="commit-side">
                    <span class="meta selector" title={row.selector}>{row.selector}</span>
                    <span class="meta sha">{row.shortId}</span>
                  </span>
                </button>
              {/each}
            {:else}
              {#each sampleRows as commit, index (commit.id)}
                <button class="commit-row" class:selected={selectedCommit === commit.id || selectedPath === commit.id} class:unpushed={isCommitUnpushed(commit)} type="button" onclick={() => selectSample(commit.id)} oncontextmenu={(event) => commit.badges.includes("stash") ? openMenu(event, [{ label: tr("menu.apply"), disabled: true }, { label: tr("menu.pop"), disabled: true }, { label: tr("menu.drop"), disabled: true }]) : openMenu(event, commitMenu(commit))}>
                  <span class="graph" aria-hidden="true">
                    {#each sampleGraph[index] ?? [] as cell, lane (`s-${commit.id}-${lane}`)}
                      <i class="graph-cell {cell?.role ?? ""}" style:color={cell ? laneColors[cell.color] : "transparent"}>
                        {#if cell?.forkFromLeft}
                          <svg class="graph-curve" viewBox="0 0 12 28" preserveAspectRatio="none"><path d="M -6 14 C 0 14, 6 20, 6 28" fill="none" stroke="currentColor" stroke-width="2" /></svg>
                        {/if}
                        {#if cell?.mergeToLeft}
                          <svg class="graph-curve" viewBox="0 0 12 28" preserveAspectRatio="none"><path d="M 6 14 C 6 22, -6 28, -6 28" fill="none" stroke="currentColor" stroke-width="2" /></svg>
                        {/if}
                      </i>
                    {/each}
                  </span>
                  <span class="subject">{@render linked(commit.summary)}</span>
                  <span class="badges">
                    {#each commit.badges as label (label)}<span class="ref {refKind(label)}" class:compact={settings.compactBranchLabels}>{label}</span>{/each}
                  </span>
                  <span class="commit-side">
                    <span class="commit-author">
                      <span class="avatar tile" style:background={avatarColor(commit.author || "?")}>{(commit.author || "?").slice(0, 1).toUpperCase()}</span>
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
          bind:size={historyDetailHeight}
          min={160}
          max={650}
          defaultSize={280}
          reverse
          onchange={handleLayoutChange}
        />

        <section class="detail" style:height="{historyDetailHeight}px">
          <div class="detail-tabs">
            <button type="button" class:on={historyTab === "commit"} onclick={() => (historyTab = "commit")}>{tr("chrome.commit")}</button>
            <button type="button" class:on={historyTab === "changes"} onclick={() => (historyTab = "changes")}>{tr("chrome.changes")}</button>
            <button type="button" class:on={historyTab === "tree"} onclick={() => openHistoryTab("tree")}>{tr("chrome.fileTree")}</button>
          </div>
          <div class="detail-scroll">
            {#if !selectedCommit && !commitDetail}
              <p class="empty">{tr("chrome.selectCommitFile")}</p>
            {:else if historyTab === "commit"}
              <div class="detail-people">
                <div class="person">
                  <span class="avatar tile" style:background={avatarColor(commitDetail?.author || "")}>{(commitDetail?.author || "?").slice(0, 1).toUpperCase()}</span>
                  <div>
                    <div class="k">{tr("chrome.author")}</div>
                    <div>{commitDetail?.author}</div>
                    <div class="meta">{commitDetail?.authorEmail}</div>
                    <div class="meta">{commitDetail?.authorAt ? forkWhen(commitDetail.authorAt) : ""}</div>
                  </div>
                </div>
                <div class="person">
                  <span class="avatar tile" style:background={avatarColor(commitDetail?.committer || "")}>{(commitDetail?.committer || "?").slice(0, 1).toUpperCase()}</span>
                  <div>
                    <div class="k">{tr("chrome.committer")}</div>
                    <div>{commitDetail?.committer}</div>
                    <div class="meta">{commitDetail?.committerEmail}</div>
                    <div class="meta">{commitDetail?.committerAt ? forkWhen(commitDetail.committerAt) : ""}</div>
                  </div>
                </div>
              </div>
              <div class="detail-line"><span class="k">{tr("chrome.refsLabel")}</span>
                {#each (mode === "live" ? commitsLive.find((row) => row.id === selectedCommit)?.refs : sampleRows.find((row) => row.id === selectedCommit)?.badges) ?? [] as label (label)}
                  <span class="ref {refKind(label)}" class:compact={settings.compactBranchLabels}>{label}</span>
                {/each}
              </div>
              <div class="detail-line"><span class="k">{tr("chrome.sha")}</span><button class="text-button sha" type="button" onclick={() => copyText(selectedCommit ?? "")}>{selectedCommit}</button></div>
              <div class="detail-line"><span class="k">{tr("chrome.parents")}</span>
                {#each commitDetail?.parents ?? [] as parent (parent)}
                  <button class="text-button sha" type="button" onclick={() => { if (mode === "live") void selectCommit(parent); else selectSample(parent); }}>{parent.slice(0, 7)}</button>
                {/each}
              </div>
              <p class="detail-message">{@render linked(mode === "live" ? (commitsLive.find((row) => row.id === selectedCommit)?.summary ?? "") : (sampleRows.find((row) => row.id === selectedCommit)?.summary ?? ""))}</p>
              {#if commitDetail?.body}<pre class="detail-body-text">{@render linked(commitDetail.body)}</pre>{/if}
              <div class="detail-line">
                <button class="text-button" type="button" onclick={() => (commitCollapsed = [])}>{tr("chrome.expandAll")}</button>
              </div>
              {#each asTree(commitFiles.map((file) => toRow(file)), commitCollapsed) as entry (entry.key)}
                {#if entry.kind === "dir"}
                  <button class="side" type="button" style:padding-left="{8 + entry.depth * 14}px" onclick={() => toggleCommitDir(entry.path)}>
                    <span class="twist">{commitCollapsed.includes(entry.path) ? "▸" : "▾"}</span>
                    <span class="name">{fileName(entry.path)}</span>
                  </button>
                {:else if entry.file}
                  <button class="side" class:selected={historyFile === entry.file.path} type="button" style:padding-left="{22 + entry.depth * 14}px" onclick={() => { const path = entry.file?.path; if (!path) return; historyFile = path; historyTab = "changes"; if (mode === "live") void pickCommitFile(path); }}>
                    <span class="badge {entry.file.tone}">{entry.file.letter}</span>
                    <span class="name">{fileName(entry.file.path)}</span>
                  </button>
                {/if}
              {/each}
            {:else if historyTab === "changes"}
              <header class="diff-head"><span>{historyFile ?? ""}</span></header>
              {#if mode === "live" && historyDiff && settings.diffStyle === "split"}
                <div class="diff-body split">
                  {#each historySplit as row, index (index)}
                    <div class="split-row">
                      <span class:del={row.leftKind === "delete"} class:meta={row.leftKind === "meta"}>{#if settings.showDiffMarks && row.leftKind === "delete"}-{/if}{#each paint(row.left, historyFile ?? "") as token, tokenIndex (`hd-l${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
                      <span class:add={row.rightKind === "add"} class:meta={row.rightKind === "meta"}>{#if settings.showDiffMarks && row.rightKind === "add"}+{/if}{#each paint(row.right, historyFile ?? "") as token, tokenIndex (`hd-r${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
                    </div>
                  {/each}
                </div>
              {:else if mode === "live" && historyDiff}
                <div class="diff-body">
                  {#each historyDiff.lines as line, index (index)}
                    <span class:add={line.kind === "add"} class:del={line.kind === "delete"} class:hunk={line.kind === "hunk"} class:meta={line.kind === "meta"}>{#each paint(line.text, historyFile ?? "") as token, tokenIndex (`hd-${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
                  {/each}
                </div>
              {:else if historyFile && diffs[historyFile]}
                {#if settings.diffStyle === "split"}
                  <div class="diff-body split">
                    {#each sampleSplitOf(diffs[historyFile]) as row, index (`hf-${index}`)}
                      <div class="split-row">
                        <span class:del={row.leftKind === "delete"} class:meta={row.leftKind === "meta"}>{#if settings.showDiffMarks && row.leftKind === "delete"}-{/if}{#each paint(row.left, historyFile) as token, tokenIndex (`hfl-${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
                        <span class:add={row.rightKind === "add"} class:meta={row.rightKind === "meta"}>{#if settings.showDiffMarks && row.rightKind === "add"}+{/if}{#each paint(row.right, historyFile) as token, tokenIndex (`hfr-${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
                      </div>
                    {/each}
                  </div>
                {:else}
                  <pre class="diff-body">{#each diffs[historyFile] as line, index (index)}<span class:add={line.startsWith("+") && !line.startsWith("+++")} class:del={line.startsWith("-") && !line.startsWith("---")} class:hunk={line.startsWith("@@")}>{line + "\n"}</span>{/each}</pre>
                {/if}
              {:else}
                <p class="diff-empty">{tr("chrome.selectCommitFile")}</p>
              {/if}
            {:else}
              {#each (mode === "live" ? treePaths : sampleTree) as file (file)}
                <button class="side" type="button" class:selected={historyFile === file} onclick={() => { if (mode === "live") void openTreeFile(file); else historyFile = file; }}><span class="name">{file}</span></button>
              {/each}
              {#if treeText}<pre class="diff-body">{treeText}</pre>{/if}
            {/if}
          </div>
        </section>
      </div>
    {/if}

    {#snippet stagedPane()}
      <div class="pane-head">
        <span class="pane-title">{tr("chrome.staged")}</span>
        <span class="count">{staged.length}</span>
        <button class="text-button" type="button" disabled={mode !== "live" || busy || staged.length === 0} onclick={() => runChange("unstage_all")}>{tr("chrome.unstageAll")}</button>
      </div>
      {#if shownStaged.length === 0}
        <p class="empty">{staged.length === 0 ? tr("chrome.noStaged") : tr("chrome.noMatch")}</p>
      {:else}
        <div class="file-list staged-list">
          {#each asTree(shownStaged) as entry (entry.key)}
            {#if entry.kind === "dir"}
              <button class="side tree-dir" type="button" style:padding-left="{8 + entry.depth * 14}px" onclick={() => toggleDir(entry.path)}>
                <span class="twist">{collapsedDirs.includes(entry.path) ? "▸" : "▾"}</span>
                <span class="name">{fileName(entry.path)}</span>
              </button>
            {:else if entry.file}
              {@render fileRow(entry.file, "staged", entry.depth)}
            {/if}
          {/each}
        </div>
      {/if}
    {/snippet}

    {#snippet unstagedPane()}
      <div class="pane-head">
        <span class="pane-title">{tr("chrome.unstaged")}</span>
        <span class="count">{mode === "loading" ? "…" : mode === "error" ? "—" : shownUnstaged.length}</span>
        {#if searchOpen.file}
          <input class="search pane-filter" placeholder={tr("chrome.filterFiles")} aria-label={tr("chrome.filterFiles")} bind:value={fileQuery} use:focusOnMount onkeydown={(event) => searchKeys("file", event)} onblur={() => searchBlur("file")} />
        {/if}
        {@render searchToggle("file", tr("chrome.filterFiles"))}
        <button class="text-button" type="button" disabled={mode !== "live" || busy || unstaged.length === 0} onclick={() => runChange("stage_all")}>{tr("chrome.stage")}</button>
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
                  <button class="side tree-dir" type="button" style:padding-left="{8 + entry.depth * 14}px" onclick={() => toggleDir(entry.path)}>
                    <span class="twist">{collapsedDirs.includes(entry.path) ? "▸" : "▾"}</span>
                    <span class="name">{fileName(entry.path)}</span>
                  </button>
                {:else if entry.file}
                  {@render fileRow(entry.file, "unstaged", entry.depth)}
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/snippet}

    {#snippet fileRow(file: Row, side: Side, depth = 0)}
      <div
        class="file"
        style:padding-left="{8 + depth * 14}px"
        class:selected={selectedPath === file.path && (mode !== "live" || selectedSide === side)}
        role="group"
        oncontextmenu={(event) => openMenu(event, fileMenu(file, side))}
      >
        <button class="file-select" type="button" onclick={(event) => selectFile(file.path, side, event)}>
          <span class="badge {file.tone}">{file.letter}</span>
          <span class="file-name">{fileName(file.path)}</span>
        </button>
      </div>
    {/snippet}

    {#snippet linked(text: string)}
      {#each linkParts(text) as part, index (`${index}-${part.text}`)}
        {#if part.href}
          <a class="issue" href={part.href} onclick={(event) => { event.preventDefault(); event.stopPropagation(); void openUrl(part.href); }}>{part.text}</a>
        {:else if settings.highlightIssues && (/^#\d+$/.test(part.text) || part.text.startsWith("http"))}
          <span class="issue">{part.text}</span>
        {:else}
          {part.text}
        {/if}
      {/each}
    {/snippet}
  </div>
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

  {#snippet searchToggle(key: SearchKey, label: string)}
    <button class="icon-btn search-toggle" class:on={searchOpen[key]} type="button" aria-label={label} aria-pressed={searchOpen[key]} title={label} onmousedown={(event) => event.preventDefault()} onclick={() => toggleSearch(key)}>
      {#if searchOpen[key]}
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/></svg>
      {:else}
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="7" cy="7" r="4.25"/><path d="M10.25 10.25L13.5 13.5"/></svg>
      {/if}
    </button>
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
    <div class="scrim" role="presentation" onclick={() => { if (dialog === "prefs") void savePrefs(true); else dialog = null; }}>
      <div class="dialog" class:wide={dialog === "prefs"} role="dialog" tabindex="-1" onclick={(event) => event.stopPropagation()} onkeydown={() => {}}>
      <form
        onchange={() => { if (dialog === "prefs") void savePrefs(false); }}
        onsubmit={async (event) => {
          event.preventDefault();
          if (dialog === "branch") void mutate({ action: "createBranch", name: branchName(draft), start: draftExtra || null });
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
          <div class="pref-tabs">
            <button type="button" class:on={prefTab === "general"} onclick={() => (prefTab = "general")}>{tr("dialog.general")}</button>
            <button type="button" class:on={prefTab === "commit"} onclick={() => (prefTab = "commit")}>{tr("dialog.commitPage")}</button>
            <button type="button" class:on={prefTab === "git"} onclick={() => (prefTab = "git")}>{tr("dialog.git")}</button>
            <button type="button" class:on={prefTab === "integration"} onclick={() => (prefTab = "integration")}>{tr("dialog.integration")}</button>
            <button type="button" class:on={prefTab === "commands"} onclick={() => (prefTab = "commands")}>{tr("dialog.customCommands")}</button>
            <button type="button" class:on={prefTab === "updates"} onclick={() => (prefTab = "updates")}>{tr("dialog.updates")}</button>
          </div>
          <div class="pref-page">
          {#if prefTab === "general"}
            <label>{tr("dialog.sourceFolder")}
              <span class="pref-inline">
                <input bind:value={settings.cloneDirectory} />
                <button class="text-button" type="button" onclick={() => pickFolder()}>{tr("dialog.browse")}</button>
              </span>
            </label>
            <div class="diff-sample" class:plain={settings.disableSyntaxHighlight}>
              <div><span class="tok-word">public func</span> elementsEqual&lt;OtherSequence&gt;(</div>
              <div class="del">{settings.showDiffMarks ? "- " : ""}isEquivalent isEquivalent: ({"{GElement}"}, {"{GElement}"})</div>
              <div class="add">{settings.showDiffMarks ? "+ " : ""}isEquivalent: ({"{GElement}"}, {"{GElement}"}) thr</div>
            </div>
            <label class="check"><input type="checkbox" bind:checked={settings.showDiffMarks} /> {tr("dialog.showMarks")}</label>
            <label class="pref-inline">{tr("dialog.fontSize")} <input type="number" min="8" max="32" bind:value={settings.diffFontSize} /></label>
            <label class="check"><input type="checkbox" bind:checked={settings.disableSyntaxHighlight} /> {tr("dialog.disableHighlight")}</label>
            <div class="pref-inline">{tr("dialog.sortCommits")}
              <label class="check"><input type="radio" name="commit-sort" value="topo" checked={settings.commitSort === "topo"} onchange={() => { settings.commitSort = "topo"; if (mode === "live") void loadContext(); }} /> {tr("dialog.topologically")}</label>
              <label class="check"><input type="radio" name="commit-sort" value="date" checked={settings.commitSort !== "topo"} onchange={() => { settings.commitSort = "date"; if (mode === "live") void loadContext(); }} /> {tr("dialog.byDate")}</label>
            </div>
            <label class="check"><input type="checkbox" bind:checked={settings.fetchAutomatically} /> {tr("dialog.fetchAuto")}</label>
            <label class="check"><input type="checkbox" bind:checked={settings.fetchTags} /> {tr("dialog.fetchTagsAlways")}</label>
            <label class="check"><input type="checkbox" bind:checked={settings.tabIndicator} /> {tr("dialog.tabIndicator")}</label>
            <label class="check"><input type="checkbox" bind:checked={settings.updateSubmodulesOnCheckout} /> {tr("dialog.submoduleCheckout")}</label>
            <label class="pref-inline">{tr("dialog.branchSpace")}
              <select bind:value={settings.branchSpace}>
                <option value="-">-</option>
                <option value="_">_</option>
                <option value=".">.</option>
                <option value="">{tr("dialog.keepSpace")}</option>
              </select>
            </label>
            <label class="check"><input type="checkbox" bind:checked={settings.pushOnCommit} /> {tr("dialog.pushOnCommit")}</label>
            <label class="check"><input type="checkbox" bind:checked={settings.compactBranchLabels} /> {tr("dialog.compactLabels")}</label>
          {:else if prefTab === "commit"}
            <div class="pref-inline">{tr("dialog.lengthIndicator")}
              <label>{tr("dialog.lowLimit")} <input type="number" min="1" bind:value={settings.messageLow} /></label>
              <label>{tr("dialog.highLimit")} <input type="number" min="1" bind:value={settings.messageHigh} /></label>
            </div>
            <label class="pref-inline">{tr("dialog.spellChecking")}
              <select bind:value={settings.spellChecking}>
                <option value="disable">{tr("dialog.spellDisable")}</option>
                <option value="enable">{tr("dialog.spellEnable")}</option>
              </select>
            </label>
            <label class="pref-inline">{tr("dialog.pageGuide")} <input type="number" min="20" max="200" bind:value={settings.pageGuide} /></label>
            <div class="k">{tr("dialog.generateMessage")}</div>
            {#if settings.aiApiKey && !aiFieldsOpen}
              <button class="text-button" type="button" disabled={mode !== "live" || busy} onclick={() => suggestMessage()}>{settings.aiModel || tr("chrome.suggest")}</button>
            {:else}
              <button class="text-button" type="button" onclick={() => (aiFieldsOpen = true)}>{tr("chrome.suggest")}</button>
            {/if}
            {#if aiFieldsOpen || !settings.aiApiKey}
              <label>Base URL <input bind:value={settings.aiBaseUrl} /></label>
              <label>Model <input bind:value={settings.aiModel} /></label>
              <label>API key <input type="password" bind:value={settings.aiApiKey} /></label>
            {/if}
          {:else if prefTab === "integration"}
            <label class="check"><input type="checkbox" bind:checked={settings.highlightIssues} /> {tr("dialog.issueLinks")}</label>
            <div class="k">{tr("dialog.externalMerge")}</div>
            <div class="tool-grid">
              <div class="tool-list">
                {#each toolPresets as preset (preset.name)}
                  <button type="button" class:on={settings.mergeToolName === preset.name} onclick={() => useMergePreset(preset.name)}>{preset.name}</button>
                {/each}
              </div>
              <div class="tool-fields">
                <label>{tr("dialog.name")} <input bind:value={settings.mergeToolName} /></label>
                <label>{tr("dialog.toolPath")} <input bind:value={settings.mergeToolPath} placeholder={tr("dialog.toolHint")} /></label>
                <label>{tr("dialog.toolArguments")} <input bind:value={settings.mergeToolArgs} /></label>
              </div>
            </div>
            <div class="k">{tr("dialog.externalDiff")}</div>
            <div class="tool-grid">
              <div class="tool-list">
                {#each toolPresets as preset (`diff-${preset.name}`)}
                  <button type="button" class:on={settings.diffToolName === preset.name} onclick={() => useDiffPreset(preset.name)}>{preset.name}</button>
                {/each}
              </div>
              <div class="tool-fields">
                <label>{tr("dialog.name")} <input bind:value={settings.diffToolName} /></label>
                <label>{tr("dialog.toolPath")} <input bind:value={settings.diffToolPath} /></label>
                <label>{tr("dialog.toolArguments")} <input bind:value={settings.diffToolArgs} /></label>
              </div>
            </div>
            <div class="k">{tr("dialog.shell")}</div>
            <label class="pref-inline">{tr("dialog.shell")}
              <select bind:value={settings.shellKind}>
                <option value="default">{tr("dialog.shellDefault")}</option>
                <option value="custom">{tr("dialog.shellPath")}</option>
              </select>
            </label>
            {#if settings.shellKind === "custom"}
              <label>{tr("dialog.shellPath")} <input bind:value={settings.shellPath} /></label>
              <label>{tr("dialog.shellArgs")} <input bind:value={settings.shellArgs} /></label>
            {/if}
            <p class="empty">{tr("dialog.toolHint")}</p>
          {:else if prefTab === "commands"}
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
            <input placeholder={"${repo} ${sha} ${branch} ${file}"} bind:value={draftUser} />
            <button class="text-button" type="button" onclick={saveCommand}>Save</button>
          {:else if prefTab === "updates"}
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
            <button class="text-button" type="button" onclick={() => invoke("check_for_update").then((value) => (updateNotice = value as UpdateNotice | null))}>{tr("chrome.download")}</button>
          {:else}
            <label class="check"><input type="checkbox" bind:checked={settings.pullRebase} /> Pull with rebase</label>
            <label class="check"><input type="checkbox" bind:checked={settings.fetchPrune} /> Prune on fetch</label>
            <label class="check"><input type="checkbox" bind:checked={settings.swapPanes} /> Show staged above unstaged</label>
            <label class="check"><input type="checkbox" bind:checked={settings.mergeNoFf} /> Merge with --no-ff</label>
            <label class="check"><input type="checkbox" bind:checked={settings.mergeAutostash} /> Autostash before merge</label>
            <label class="check"><input type="checkbox" bind:checked={settings.signCommits} /> Sign commits with git</label>
            <label class="check"><input type="checkbox" bind:checked={settings.signOff} onchange={() => (signOff = settings.signOff)} /> Sign-off by default</label>
            <label>Sign-off format <input bind:value={settings.signOffFormat} /></label>
            <label class="check"><input type="checkbox" bind:checked={settings.forceWithLease} /> Push with --force-with-lease</label>
            <label class="check"><input type="checkbox" bind:checked={settings.gravatar} /> Gravatar avatars</label>
            <label class="check"><input type="checkbox" bind:checked={settings.dateRelative} /> Relative dates</label>
            <label class="check"><input type="checkbox" bind:checked={settings.date24h} /> 24-hour clock</label>
            <label>Date pattern <input bind:value={settings.dateFormat} placeholder="dd MMM yyyy" /></label>
            <label>Diff
              <select bind:value={settings.diffStyle}>
                <option value="split">Split</option>
                <option value="unified">Unified</option>
              </select>
            </label>
            <label>Line height
              <select bind:value={settings.linesHeight}>
                <option value="compact">Compact</option>
                <option value="spaced">Spaced</option>
              </select>
            </label>
            <label class="check"><input type="checkbox" bind:checked={settings.showEntireFile} /> Show the whole file in diffs</label>
            <label class="check"><input type="checkbox" bind:checked={settings.aiEnabled} /> AI commit messages</label>
            <label>AI language <input bind:value={settings.aiLanguage} placeholder="English" /></label>
            <label>AI max characters <input type="number" bind:value={settings.aiMaxChars} /></label>
            <label>AI prompt <input bind:value={settings.aiPrompt} /></label>
            <label>AI temperature <input type="number" step="0.1" bind:value={settings.aiTemperature} /></label>
            <label>Author name <input bind:value={settings.authorName} placeholder="uses git config when empty" /></label>
            <label>Author email <input bind:value={settings.authorEmail} /></label>
            <label>Log directory <input bind:value={settings.logDirectory} /></label>
            <label class="check"><input type="checkbox" bind:checked={settings.sslVerify} /> Verify SSL</label>
            <label class="check"><input type="checkbox" bind:checked={settings.proxyEnabled} /> Use proxy host</label>
            <label>Proxy type
              <select bind:value={settings.proxyType}>
                <option value="http">HTTP</option>
                <option value="socks">SOCKS</option>
              </select>
            </label>
            <label>Proxy host <input bind:value={settings.proxyHost} /></label>
            <label>Proxy port <input type="number" bind:value={settings.proxyPort} /></label>
            <label>Proxy user <input bind:value={settings.proxyUser} /></label>
            <label>Proxy password <input type="password" bind:value={settings.proxyPassword} /></label>
            <label>Proxy URL <input bind:value={settings.proxy} /></label>
            <label>CA file <input bind:value={settings.sslCaFile} /></label>
            <label>{tr("dialog.githubToken")} <input type="password" bind:value={settings.githubToken} /></label>
            <label>{tr("dialog.gitlabToken")} <input type="password" bind:value={settings.gitlabToken} /></label>
            <label>{tr("dialog.gitlabHost")} <input bind:value={settings.gitlabHost} placeholder="gitlab.com" /></label>
            <label>Signing passphrase <input type="password" bind:value={passphrase} /></label>
            <label>Askpass user <input bind:value={passUser} /></label>
            <div class="composer-row">
              <button class="text-button" type="button" onclick={() => keepPassphrase()}>Use passphrase</button>
              <button class="text-button" type="button" onclick={() => { draft = "https"; draftExtra = ""; draftUser = ""; draftSecret = ""; dialog = "credential"; }}>Save HTTPS login</button>
              <button class="text-button" type="button" onclick={() => mutate({ action: "lfsPull" })}>LFS pull</button>
              <button class="text-button" type="button" onclick={() => mutate({ action: "lfsPush" })}>LFS push</button>
              <button class="text-button" type="button" onclick={() => (dialog = "patch")}>Apply patch</button>
              <button class="text-button" type="button" onclick={() => invoke("set_repo_author", { path: repoPath(), name: settings.authorName, email: settings.authorEmail })}>Save author in this repo</button>
            </div>
          {/if}
          </div>
          <div class="composer-row">
            <button class="commit" type="button" onclick={() => savePrefs(true)}>{tr("chrome.close")}</button>
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
  {#if launchOpen}
    <div class="palette-scrim" role="presentation" onclick={() => closeLaunch()}>
      <div class="palette" role="dialog" tabindex="-1" aria-label={tr("chrome.quickLaunch")} onclick={(event) => event.stopPropagation()} onkeydown={() => {}}>
        <input
          class="palette-input"
          placeholder={tr("chrome.quickLaunch")}
          aria-label={tr("chrome.quickLaunch")}
          bind:this={launchEl}
          bind:value={launch}
          oninput={() => (launchIndex = 0)}
          onkeydown={onLaunchKey}
        />
        <div class="palette-list" bind:this={launchList}>
          {#each matches as item, index (`${item.group}-${item.label}`)}
            <button type="button" class:on={index === launchIndex} onmouseenter={() => (launchIndex = index)} onclick={() => runLaunch(item)}>
              <span class="name">{item.label}</span>
              <span class="meta">{item.group}</span>
            </button>
          {:else}
            <p class="empty">{tr("chrome.noMatch")}</p>
          {/each}
        </div>
        <div class="palette-hint">{tr("chrome.paletteHint")}</div>
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
    background: var(--elevated);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: var(--shadow);
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
    border-radius: 6px;
    transition: background 120ms ease, color 120ms ease;
  }
  .menu button:disabled { opacity: 0.4; }
  .menu hr { border: 0; border-top: 1px solid var(--line); margin: 4px 6px; }
  .shortcut { color: var(--text-secondary); font-size: 11px; }
  .menu button, .menu-dismiss { font: inherit; }
  .scrim.menu-dismiss { position: fixed; inset: 0; z-index: 29; background: transparent; border: 0; backdrop-filter: none; animation: none; }
  .shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--canvas);
    color: var(--text);
    font-size: var(--ui-scale, 13px);
    user-select: none;
    overflow: hidden;
  }

  .chrome {
    height: 48px;
    display: grid;
    grid-template-columns: auto minmax(140px, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 0 12px;
    background: var(--sidebar);
    border-bottom: 1px solid var(--line);
  }

  .chrome-side {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .chrome-side.end {
    justify-content: flex-end;
    min-width: max-content;
  }

  .tabs {
    max-width: 320px;
    overflow: auto;
    scrollbar-width: none;
  }

  .tabs::-webkit-scrollbar { display: none; }

  .tab,
  .text-button,
  .side,
  .file,
  .commit-row,
  .commit,
  .icon-btn,
  .segment button,
  .launch-trigger {
    font: inherit;
    color: inherit;
    background: transparent;
    border: 0;
  }

  .tab {
    height: 28px;
    padding: 0 10px;
    border-radius: 8px;
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
    transition: background 120ms ease, color 120ms ease;
  }

  .tab.active {
    background: var(--elevated);
    color: var(--text);
    box-shadow: 0 1px 2px rgba(24, 24, 27, 0.06);
  }

  .text-button:hover:not(:disabled),
  .side:hover,
  .file:hover,
  .commit-row:hover,
  .tab:hover {
    background: var(--hover);
  }

  .tab.active:hover { background: var(--elevated); }

  .icon-btn {
    position: relative;
    width: 32px;
    height: 32px;
    padding: 0;
    border-radius: 8px;
    color: var(--text-secondary);
    display: grid;
    place-items: center;
    flex: none;
    transition: background 120ms ease, color 120ms ease;
  }

  .icon-btn:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }

  .icon-btn:disabled,
  .segment button:disabled { opacity: 0.4; }

  .icon-btn svg,
  .segment svg { width: 16px; height: 16px; }

  .segment {
    display: flex;
    align-items: center;
    height: 28px;
    padding: 2px;
    gap: 2px;
    border-radius: 8px;
    background: var(--field);
    flex: none;
  }

  .segment button {
    height: 24px;
    padding: 0 8px;
    border-radius: 6px;
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    gap: 6px;
    transition: background 120ms ease, color 120ms ease;
  }

  .segment button:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }

  .launch-trigger {
    justify-self: center;
    height: 32px;
    width: min(100%, 480px);
    padding: 0 12px;
    border-radius: 10px;
    background: var(--field);
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    transition: background 120ms ease;
  }

  .launch-trigger:hover { background: var(--hover); }

  .launch-trigger span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .launch-trigger kbd {
    flex: none;
    font: inherit;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .badge-count {
    position: absolute;
    top: 1px;
    right: 1px;
    min-width: 14px;
    height: 14px;
    padding: 0 3px;
    border-radius: 7px;
    background: var(--accent);
    color: var(--on-accent);
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

  .palette-scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgba(24, 24, 27, 0.28);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .palette {
    width: 520px;
    max-width: calc(100vw - 32px);
    border-radius: 12px;
    background: var(--elevated);
    border: 1px solid var(--line);
    box-shadow: var(--shadow);
    overflow: hidden;
  }

  .palette-input {
    width: 100%;
    height: 44px;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 15px;
    padding: 0 16px;
    outline: none;
  }

  .palette-list {
    max-height: 360px;
    overflow: auto;
    padding: 4px;
    border-top: 1px solid var(--line);
  }

  .palette-list button {
    width: 100%;
    height: 36px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 10px;
    text-align: left;
  }

  .palette-list button.on { background: var(--selection); }

  .palette-hint {
    height: 32px;
    display: flex;
    align-items: center;
    padding: 0 16px;
    border-top: 1px solid var(--line);
    color: var(--text-secondary);
    font-size: 12px;
  }

  textarea:focus-visible,
  .commit:focus-visible,
  .icon-btn:focus-visible,
  .segment button:focus-visible,
  .side:focus-visible,
  .file:focus-visible,
  .commit-row:focus-visible,
  .tab:focus-visible,
  .text-button:focus-visible,
  .launch-trigger:focus-visible,
  .palette-list button:focus-visible {
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
    min-width: 0;
    display: flex;
    overflow: hidden;
    position: relative;
  }

  .flex-fill {
    flex: 1;
    min-width: 0;
    min-height: 0;
  }

  .sidebar {
    flex: none;
    background: var(--sidebar);
    border-right: 1px solid var(--line);
    padding: 8px 0;
    overflow-x: hidden;
    overflow-y: auto;
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

  .side.selected,
  .file.selected,
  .commit-row.selected {
    background: var(--selection);
  }

  .side.nav.selected,
  .side.section.selected {
    color: var(--accent);
    font-weight: 600;
  }

  .side.nav.selected .count {
    color: var(--accent);
  }

  .side.nested {
    padding-left: 22px;
  }

  .side.nested.deep {
    padding-left: 36px;
  }

  .side.nested.deep.deeper {
    padding-left: 50px;
  }

  .side > .file-select {
    gap: 6px;
  }

  .side.section {
    margin-top: 6px;
    color: var(--text);
  }

  .side.section .text-button {
    margin-left: 0;
    flex: none;
  }

  .twist {
    display: inline-flex;
    align-items: center;
    justify-content: center;
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
    margin: 14px 12px 4px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0;
    text-transform: none;
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

  .changes-column {
    flex: none;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--canvas);
    overflow: hidden;
  }

  .status-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    overflow: hidden;
  }

  .right-stage {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--canvas);
  }

  .history-stage {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .history {
    min-width: 0;
    min-height: 0;
    overflow-x: hidden;
    overflow-y: auto;
  }

  .pane-head {
    height: 32px;
    min-height: 32px;
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: nowrap;
    padding: 0 10px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    border-bottom: 1px solid var(--line);
    background: var(--canvas);
  }

  .pane-title {
    font-size: 12px;
    font-weight: 600;
  }

  .search.pane-filter {
    min-width: 60px;
    height: 24px;
    font-size: 12px;
    font-weight: 400;
    padding: 0 6px;
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
    overflow-x: hidden;
    overflow-y: auto;
    padding-bottom: 8px;
  }

  .staged-list {
    flex: none;
    max-height: 30%;
  }

  .changes .staged-list,
  .changes .file-list {
    flex: 1;
    max-height: none;
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

  .badge {
    width: 14px;
    flex: none;
    font-size: 11px;
    font-weight: 700;
  }

  .added { color: var(--added); }
  .deleted { color: var(--deleted); }
  .modified { color: var(--modified); }

  .file-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
    flex: none;
    background: var(--canvas);
    border-top: 1px solid var(--line);
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    overflow: hidden;
    box-sizing: border-box;
  }

  .composer-desc {
    resize: none !important;
    min-height: 0;
  }

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
    border: 1px solid var(--line);
    background: var(--elevated);
  }

  textarea:focus {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
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
    height: 32px;
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

  .commit-row .subject { font-weight: 400; flex: 1; min-width: 0; }

  .badges { display: flex; gap: 4px; }

  .ref {
    height: 16px;
    padding: 0 4px;
    border-radius: 4px;
    background: var(--field);
    font-size: 11px;
    line-height: 16px;
  }
  .ref.local { background: var(--ref-local-bg); color: var(--ref-local); }
  .ref.remote { background: var(--ref-remote-bg); color: var(--ref-remote); }
  .ref.tag { background: var(--ref-tag-bg); color: var(--ref-tag); }
  .ref.stash { background: var(--ref-stash-bg); color: var(--ref-stash); }
  .graph { display: flex; align-items: center; flex: none; height: 100%; }
  .graph-cell { width: 12px; height: 100%; position: relative; flex: none; }
  .graph-cell.node::after {
    content: "";
    position: absolute;
    left: 2px;
    top: 50%;
    width: 8px;
    height: 8px;
    margin-top: -4px;
    border-radius: 50%;
    background: currentColor;
    z-index: 1;
    transition: opacity 120ms ease;
  }
  .graph-cell.line::before,
  .graph-cell.line-up::before,
  .graph-cell.line-down::before {
    content: "";
    position: absolute;
    left: 5px;
    width: 2px;
  }
  .graph-cell.line::before {
    top: -1px;
    bottom: -1px;
    background: currentColor;
  }
  .graph-cell.line-up.line-down::before {
    top: -1px;
    bottom: -1px;
    background: linear-gradient(
      to bottom,
      currentColor calc(50% - 3px),
      transparent calc(50% - 3px),
      transparent calc(50% + 3px),
      currentColor calc(50% + 3px)
    );
  }
  .graph-cell.line-up:not(.line-down)::before {
    top: -1px;
    bottom: calc(50% + 3px);
    background: currentColor;
  }
  .graph-cell.line-down:not(.line-up)::before {
    top: calc(50% + 3px);
    bottom: -1px;
    background: currentColor;
  }
  .graph-curve {
    position: absolute;
    left: 0;
    top: 0;
    width: 12px;
    height: 100%;
    overflow: visible;
    pointer-events: none;
  }
  .commit-row.unpushed .subject,
  .commit-row.unpushed .badges,
  .commit-row.unpushed .commit-side {
    opacity: 0.65;
    transition: opacity 120ms ease;
  }
  .commit-row.unpushed .graph-cell.node::after {
    opacity: 0.65;
  }
  .commit-row.unpushed:hover .subject,
  .commit-row.unpushed:hover .badges,
  .commit-row.unpushed:hover .commit-side,
  .commit-row.unpushed:hover .graph-cell.node::after,
  .commit-row.unpushed.selected .subject,
  .commit-row.unpushed.selected .badges,
  .commit-row.unpushed.selected .commit-side,
  .commit-row.unpushed.selected .graph-cell.node::after {
    opacity: 1;
  }
  .drop-check { opacity: 0; }
  .commit-row:hover .drop-check, .drop-check:checked { opacity: 1; }
  .gutter { width: 32px; flex: none; text-align: right; color: var(--text-secondary); }
  .image-toolbar { display: flex; gap: 6px; align-items: center; padding: 8px 12px; }
  .image-toolbar button { border: 0; background: transparent; font: inherit; color: inherit; padding: 2px 8px; border-radius: 6px; }
  .image-toolbar button.on { background: var(--selection); }
  .image-stage { position: relative; display: flex; gap: 12px; padding: 12px; min-height: 80px; }
  .image-stage img { max-width: 48%; background: repeating-conic-gradient(#ddd 0 25%, #fff 0 50%) 0 0 / 16px 16px; }
  .image-stage .under, .image-stage .over { position: absolute; left: 12px; top: 12px; max-width: calc(100% - 24px); }
  .pref-tabs { display: flex; gap: 2px; padding: 3px; border-radius: 10px; background: var(--field); }
  .pref-tabs button { flex: 1; height: 28px; border: 0; background: transparent; color: var(--text-secondary); font: inherit; padding: 0 10px; border-radius: 7px; white-space: nowrap; transition: background 120ms ease, color 120ms ease; }
  .pref-tabs button:hover { color: var(--text); }
  .pref-tabs button.on { color: var(--text); background: var(--elevated); font-weight: 500; box-shadow: 0 1px 2px rgba(24, 24, 27, 0.08), 0 0 0 1px var(--line); }

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
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .diff-head .text-button {
    margin-left: 0;
    flex: none;
  }

  .diff-body {
    margin: 0;
    padding: 8px 0;
    overflow: auto;
    font-family: var(--mono);
    font-size: var(--diff-font, 13px);
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
    z-index: 50;
    background: rgba(9, 9, 11, 0.32);
    backdrop-filter: blur(2px);
    display: flex;
    align-items: center;
    justify-content: center;
    animation: scrim-in 140ms ease-out;
  }

  @keyframes scrim-in {
    from { opacity: 0; }
  }

  @keyframes dialog-in {
    from { opacity: 0; transform: translateY(6px) scale(0.98); }
  }

  .dialog.wide {
    width: min(760px, calc(100vw - 32px));
    min-height: 520px;
  }

  .dialog {
    width: 460px;
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 48px);
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 22px 24px 20px;
    border-radius: 14px;
    background: var(--elevated);
    border: 1px solid var(--line);
    box-shadow: var(--shadow);
    animation: dialog-in 160ms ease-out;
  }

  .dialog form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .dialog h2 {
    margin: 0 0 2px;
    font-size: 16px;
    font-weight: 600;
  }

  .dialog p {
    margin: 0;
    line-height: 1.5;
  }

  .dialog .empty {
    padding: 0;
    font-size: 12px;
  }

  .dialog input:not([type="checkbox"]):not([type="radio"]):not([type="range"]),
  .dialog select {
    height: 32px;
    padding: 0 10px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--canvas);
    color: var(--text);
    transition: border-color 120ms ease, box-shadow 120ms ease, background 120ms ease;
  }

  .dialog select {
    appearance: none;
    padding-right: 30px;
    cursor: pointer;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16' fill='none' stroke='%2371717a' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M4.5 6.5 8 10l3.5-3.5'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 9px center;
    background-size: 14px;
  }

  .dialog input:not([type="checkbox"]):not([type="radio"]):not([type="range"]):hover,
  .dialog select:hover {
    border-color: color-mix(in srgb, var(--text-secondary) 40%, transparent);
  }

  .dialog input:not([type="checkbox"]):not([type="radio"]):not([type="range"]):focus,
  .dialog select:focus,
  .dialog textarea:focus {
    outline: none;
    border-color: var(--accent);
    background: var(--elevated);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 20%, transparent);
  }

  .dialog input::placeholder,
  .dialog textarea::placeholder,
  .search::placeholder {
    color: color-mix(in srgb, var(--text-secondary) 80%, transparent);
  }

  .dialog label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .dialog label.check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    font-size: inherit;
    color: var(--text);
    cursor: pointer;
  }

  .dialog label.pref-inline {
    flex-direction: row;
    align-items: center;
    gap: 10px;
    font-size: inherit;
    color: var(--text);
  }

  .dialog .pref-inline select,
  .dialog .pref-inline input[type="number"] {
    width: auto;
    min-width: 88px;
  }

  .dialog .pref-inline > input:not([type="number"]) {
    flex: 1;
  }

  .dialog .text-button {
    height: 30px;
    padding: 0 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--elevated);
    color: var(--text);
    transition: background 120ms ease, border-color 120ms ease;
  }

  .dialog .text-button:hover:not(:disabled) {
    background: var(--hover);
    border-color: color-mix(in srgb, var(--text-secondary) 35%, transparent);
  }

  .dialog .commit {
    height: 30px;
    padding: 0 16px;
    font-weight: 500;
    transition: filter 120ms ease;
  }

  .dialog .commit:hover:not(:disabled) {
    filter: brightness(1.08);
  }

  .dialog .composer-row {
    gap: 8px;
  }

  .dialog .composer-row > :is(.text-button, .commit) {
    margin-left: 0;
  }

  .dialog .composer-row > button:is(.text-button, .commit):first-of-type {
    margin-left: auto;
  }

  .dialog .composer-row > select {
    width: auto;
  }

  .dialog .pref-page > .text-button,
  .dialog form > .text-button {
    align-self: flex-start;
    margin-left: 0;
  }

  .dialog .diff-body {
    max-height: 50vh;
    border-radius: var(--radius);
    background: var(--canvas);
    border: 1px solid var(--line);
  }

  input[type="checkbox"],
  input[type="radio"] {
    appearance: none;
    width: 16px;
    height: 16px;
    margin: 0;
    flex: none;
    display: inline-grid;
    place-content: center;
    border: 1.5px solid color-mix(in srgb, var(--text-secondary) 55%, transparent);
    background: var(--elevated);
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease, box-shadow 120ms ease;
  }

  input[type="checkbox"] { border-radius: 4px; }
  input[type="radio"] { border-radius: 50%; }

  input[type="checkbox"]:hover,
  input[type="radio"]:hover {
    border-color: var(--accent);
  }

  input[type="checkbox"]::before {
    content: "";
    width: 10px;
    height: 10px;
    background: var(--on-accent);
    clip-path: polygon(14% 44%, 0 59%, 39% 96%, 100% 22%, 85% 8%, 38% 66%);
    transform: scale(0);
    transition: transform 120ms ease;
  }

  input[type="radio"]::before {
    content: "";
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    transform: scale(0);
    transition: transform 120ms ease;
  }

  input[type="checkbox"]:checked {
    background: var(--accent);
    border-color: var(--accent);
  }

  input[type="radio"]:checked {
    border-color: var(--accent);
  }

  input[type="checkbox"]:checked::before,
  input[type="radio"]:checked::before {
    transform: scale(1);
  }

  input[type="checkbox"]:focus-visible,
  input[type="radio"]:focus-visible {
    outline: none;
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
  }

  input[type="checkbox"]:disabled,
  input[type="radio"]:disabled {
    opacity: 0.4;
    cursor: default;
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
    min-width: 0;
    height: 26px;
    border: 1px solid transparent;
    border-radius: var(--radius);
    background: var(--field);
    color: inherit;
    font: inherit;
    font-weight: 400;
    padding: 0 8px;
    transition: border-color 120ms ease, box-shadow 120ms ease, background 120ms ease;
    animation: search-in 140ms ease-out;
  }

  @keyframes search-in {
    from { opacity: 0; transform: scaleX(0.92); transform-origin: right center; }
  }

  .search:focus {
    outline: none;
    border-color: var(--accent);
    background: var(--elevated);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .icon-btn.search-toggle {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    font-weight: 400;
  }

  .icon-btn.search-toggle svg {
    width: 14px;
    height: 14px;
  }

  .icon-btn.search-toggle.on {
    color: var(--text);
  }

  .pane-head .search-toggle {
    margin-left: auto;
  }

  .pane-head .search + .search-toggle {
    margin-left: 0;
  }

  .pane-head .search-toggle + .text-button {
    margin-left: 0;
  }

  .side-search {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    height: 30px;
    margin: 2px 8px 2px;
  }

  .search-wrap {
    display: flex;
    align-items: center;
    gap: 4px;
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

  .split-row > span {
    display: block;
    min-height: 18px;
    padding: 0 8px;
    white-space: pre;
    overflow: hidden;
  }

  .split-row > span.del { background: var(--diff-del); }
  .split-row > span.add { background: var(--diff-add); }
  .split-row.actionable { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) auto; }
  .split-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .split-actions .line-action { margin-left: 0; }

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

  .menubar {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 28px;
    padding: 0 6px;
    background: var(--sidebar);
    border-bottom: 1px solid var(--line);
  }

  .menubar button {
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    padding: 2px 8px;
    border-radius: 4px;
  }

  .menubar button:hover { background: var(--selection); }

  .tab.dirty .file-select::before {
    content: "";
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--modified);
    flex: none;
  }

  .side.nav { width: calc(100% - 12px); }
  .side.nav .count { margin-left: auto; }

  .filter-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 28px;
    padding: 0 12px;
    color: var(--text-secondary);
    background: var(--sidebar);
    border-bottom: 1px solid var(--line);
  }

  .commit-side {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 12px;
    flex: none;
    color: var(--text-secondary);
  }

  .commit-author {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    width: 120px;
    min-width: 0;
    flex: none;
  }

  .commit-author .meta,
  .commit-author .author {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .commit-side .meta {
    line-height: 16px;
  }

  .commit-side .sha {
    width: 64px;
    flex: none;
    font-family: var(--mono);
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .commit-side .date {
    width: 120px;
    flex: none;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .commit-side .selector {
    width: 120px;
    flex: none;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .avatar.tile { border-radius: 3px; color: white; }
  .ref.compact { max-width: 88px; overflow: hidden; text-overflow: ellipsis; display: inline-block; vertical-align: bottom; }

  .detail {
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--line);
    background: var(--elevated);
    overflow: hidden;
  }

  .detail-tabs { display: flex; gap: 16px; padding: 0 16px; border-bottom: 1px solid var(--line); }
  .detail-tabs button { border: 0; background: transparent; font: inherit; padding: 8px 0; color: var(--text-secondary); }
  .detail-tabs button:hover { color: var(--text); }
  .detail-tabs button.on { color: var(--text); font-weight: 500; box-shadow: inset 0 -2px 0 var(--accent); }
  .detail-scroll { flex: 1; overflow: auto; padding: 12px 16px 16px; }
  .detail-people { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
  .person { display: flex; gap: 8px; align-items: flex-start; }
  .k { color: var(--text-secondary); font-size: 11px; letter-spacing: 0.04em; text-transform: uppercase; }
  .detail-line { display: flex; gap: 8px; align-items: center; margin-top: 8px; flex-wrap: wrap; }
  .detail-message { margin: 12px 0 4px; font-weight: 600; }
  .detail-body-text { margin: 0 0 12px; white-space: pre-wrap; font: inherit; }
  .issue { color: var(--accent); text-decoration: none; }

  .tree-dir { width: 100%; }
  .twist { width: 12px; flex: none; color: var(--text-secondary); }

  .summary-input,
  .composer textarea {
    width: 100%;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--elevated);
    color: inherit;
    font: inherit;
    padding: 6px 8px;
  }

  .summary-input { height: 32px; }
  .summary-input.warn { color: var(--modified); }
  .summary-input.over { color: var(--deleted); }

  .guided {
    font-family: var(--mono);
    background-image: linear-gradient(90deg, transparent calc(var(--guide) * 1ch), #d4d4d8 calc(var(--guide) * 1ch), #d4d4d8 calc(var(--guide) * 1ch + 1px), transparent calc(var(--guide) * 1ch + 1px));
    background-origin: content-box;
    background-attachment: local;
  }

  .composer-row .more { margin-left: auto; }
  .composer .commit { margin-left: 8px; }

  .pref-page { display: flex; flex-direction: column; gap: 10px; padding: 14px 0; min-height: 360px; }
  .pref-inline { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .pref-inline input[type="number"] { width: 72px; }
  .diff-sample {
    border: 1px solid var(--line);
    border-radius: 4px;
    overflow: auto;
    font-family: var(--mono);
    font-size: var(--diff-font, 13px);
    line-height: 1.45;
    padding: 8px;
  }
  .diff-sample .del { background: var(--diff-del); }
  .diff-sample .add { background: var(--diff-add); }
  .diff-sample.plain .tok-word { color: inherit; }
  .tool-grid { display: grid; grid-template-columns: 180px 1fr; gap: 12px; }
  .tool-list { display: flex; flex-direction: column; gap: 2px; padding: 4px; border: 1px solid var(--line); border-radius: var(--radius); overflow: auto; max-height: 140px; }
  .tool-list button { border: 0; background: transparent; text-align: left; padding: 6px 8px; font: inherit; color: inherit; border-radius: 6px; }
  .tool-list button:hover { background: var(--hover); }
  .tool-list button.on { background: var(--selection); color: var(--accent); font-weight: 500; }
  .image-toolbar button:hover:not(.on) { background: var(--hover); }
  .tool-fields { display: flex; flex-direction: column; gap: 8px; }

  .welcome-view {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 32px 20px;
    overflow-y: auto;
    overflow-x: hidden;
    background: var(--canvas);
  }

  .welcome-card {
    width: 640px;
    max-width: 100%;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .welcome-brand {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .welcome-logo {
    width: 44px;
    height: 44px;
    flex: none;
  }

  .welcome-titles h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .welcome-titles p {
    margin: 4px 0 0;
    color: var(--text-secondary);
    font-size: 13px;
  }

  .welcome-error {
    padding: 8px 12px;
    background: color-mix(in srgb, var(--deleted) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--deleted) 30%, transparent);
    border-radius: var(--radius);
    color: var(--deleted);
    font-size: 13px;
  }

  .welcome-actions {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
  }

  .welcome-btn {
    border: 1px solid var(--line);
    background: var(--elevated);
    border-radius: 10px;
    padding: 16px 14px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
    color: var(--text);
    cursor: pointer;
    text-align: left;
    transition: all 140ms ease;
  }

  .welcome-btn:hover {
    border-color: var(--accent);
    background: var(--selection);
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.04);
  }

  .welcome-btn svg {
    width: 22px;
    height: 22px;
    color: var(--accent);
  }

  .btn-meta strong {
    display: block;
    font-size: 14px;
    font-weight: 600;
  }

  .btn-meta span {
    display: block;
    margin-top: 2px;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .welcome-recent {
    border: 1px solid var(--line);
    background: var(--elevated);
    border-radius: 10px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .recent-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .recent-bar h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .recent-input {
    width: 200px;
    height: 26px;
    padding: 0 8px;
    font-size: 11px;
  }

  .recent-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 220px;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .recent-card {
    border: 0;
    background: transparent;
    padding: 8px 10px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    color: inherit;
    text-align: left;
    cursor: pointer;
    transition: background 120ms ease;
  }

  .recent-card:hover {
    background: var(--hover);
  }

  .recent-main {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .recent-title {
    font-weight: 600;
    font-size: 13px;
  }

  .recent-path {
    font-size: 11px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .recent-stats {
    font-size: 11px;
    color: var(--text-secondary);
    flex: none;
  }
</style>
