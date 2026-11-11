import { onMount, tick } from "svelte";
import { pushState } from "$app/navigation";
import { invoke } from "../perf";
import { listen } from "@tauri-apps/api/event";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commits, diffs, sampleConflictViews, sampleImages, sampleNotices, samplePulls, sampleReflog, staged as sampleStaged, unstaged as sampleUnstaged, type FileChange } from "../sample";
import { highlight } from "../highlight";
import { clipboardRemote, cloneTarget, cloneUrl, remoteFromText, remoteNameFromUrl, repoNameFromUrl, safeFolderName } from "../clonePath";
import { featuresPageUrl } from "../featuresPage";
import { fileAccessReport } from "../fileAccess";
import { resolveLocale, translate } from "../i18n";
import { broadcastLocale, onLocaleChange } from "../i18n/sync";
import { commitTypesOf, isBuiltinCommitType, isCommentary, isCommitType, joinSubject, splitSubject, subjectFitsConvention, typeMessageKey, type CommitType } from "./commitMessage";
import { shortcut, shortcutLabel, typing } from "../keys";
import { branchGroup, changedRanges, commitGraph, foldedMerges, formatWhen, gravatarUrl, laneColors, laneCommits, markTokens, numberedDiff, orderRefNames, splitDiff, virtualLines, virtualSplit, windowSlice, type GraphCell, type SplitRow } from "../view";
import {
    badge,
    type BlameLine,
    type CommandRecord,
    type CommitDetail,
    type CommitRow,
    type ConflictSides,
    type ConflictView,
    type DiffLine,
    type FileDiff,
    type FilePreview,
    type InProgress,
    type MergeState,
    type RefSnapshot,
    type ForkImport,
    type Settings,
    type StatusFile,
    type StatusSnapshot,
    type Suggestion,
    type GitRuntime,
    type RepoScan,
    type ToolRecord,
    type UpdateNotice,
    type UpdateProgress,
    type WorkspaceRecord,
    recentWorkspaceId,
    recentWorkspaceLimit,
    workspaceTabWidth,
  } from "../status";
import { branchItems, fileViewMenu, fileMenu, commitMenu, reflogMenu, viewItems, stashItems, appearanceItems, repositoryItems, windowItems, helpItems, commitExtras, dirMenu, commandItems, workspaceItems, diffLineMenu, revisionFileMenu, historyFileMenu, externalItems } from "./menus";
import { textColumns, cachedSplit, splitColumnsOf, sampleSplitOf, pickSplitSide, splitScroller, loadDiff, moveHunk, copyPatch, markedLine, patchOf, lineText, splitLineAt, selectDiffRange, toggleDiffLine, clickDiffLine, dragDiffLine, chosenDiffLines, hunkOf, hunkRanges, editorLineNo, removesOnApply, applyLineOps, showLineHistory, toggleAllDiffLines, fitImage, paintPixel, paint, selectFile, selectFolder, fileTargetRows, selectAllFiles, collapseFileSelection, selectAllHere, pathsForCopy, fullPath, diffSide, blockText, activeTextRoot, clearTextSelection, rememberPointer, discardableEntries, stageChosen, flipStage, stagePicked, unstagePicked, resolvePicked, pickedConflicts, discardPicked, deletePicked, confirmPicked, discardChosen, deleteChosen, pickedIn, showBlame, selectChangeFile, selectTreeFile, toggleDir, toggleCommitDir, setFolderFold, setLayout, setConflictsOnly, fileKind, expandParents, collapseTreeExcept, flatEntries, asTree, toRow, sampleRow, groupLabel, ignoreTargets, addIgnore, stageFolder, filesInDir, openStash, fileName, parentDir, folderName, normalPath, copyText, toggleFakeIgnore, openFileView, sampleBlob, tabText, tabColor, pairSetting } from "./diff.svelte";
import { loadCommits, loadContext, selectCommit, pickCommitFile, ensurePatch, togglePatch, toggleAllPatches, toggleFold, showReflog, openHistoryTab, selectSample, showFileHistory, scrollToCommit, locateRef, locateStash, openHistory, revealHead, dropCommits, toggleDrop, trackPatch, whenLabel, commitSubject, commitAuthorName, commitWhenShort, commitRefs, mergeRoute, differentCommitter, resetCommitView, openTreeFile, laneTone, activeLaneColor, exactRefs, soloBranch, toggleHistoryScope, clearHistoryFilter, isCommitUnpushed, branchRefLabels, plainRefLabels, refKind, stashParts, forkWhen, forkWhenFull, linkParts, issueHref, avatarColor, searchContent, currentFilterRefs, compareDrops, previewCherry, showSideRefs, showSideSearch, queueCommitSearch, pickCommitSearch, pickCommitScope, readErrorLog, writeErrorLog } from "./history.svelte";
import { selectPrefTab, openLaunch, closeLaunch, runLaunch, onLaunchKey, ask, savePrefs, browseFolder, withSourceRoots, sourceRootList, addSourceRoot, removeSourceRoot, setSourceAlias, beginClone, onCloneUrl, onCloneFolder, rememberCloneRoot, pathsMatch, finishCloneOpen, cancelClone, browseCloneRoot, startClone, startInit, openResolve, openTextMerge, loadTextMerge, closeTextMerge, applyTextMerge, applyFileMerge, takeMergeSide, toggleMergeSide, hunkIncludes, chosenHunkText, openRebase, submitCommit, suggestMessage, testAiEndpoint, loadAiModels, togglePromptStyle, fillFromMessage, fillTemplate, splitConflicts, joinedPieces, pickHunk, saveAccount, saveCommand, removeCommand, runCommand, sendCommand, fillCommand, openCommandLog, showCommandFilter, placeCommandLog, confirmSync, beginOpen, loadMergePreview, refreshMergeSummary, readClipboardText, pastePatch, requestCheckout, quickCheckout, quick, openForge, pullRequestUrl, commitWeb, bisectMark, setRebaseVerb, moveStep, hardReset, removeBranch, cancelOperation, keepPassphrase, saveWindow, openRepoSettings, createWorkspace, renameWorkspace, deleteWorkspace, switchWorkspace, selectWorkspace, loadExternalTools, applyToolSettings, applyCommitTypes, addCommitType, renameCommitType, setCommitTypeField, removeCommitType, resetCommitTypes, openEditorImport, detectEditors, toggleEditorCandidate, importEditorCandidates, closeEditorImport, type EditorTool, type ExternalTool, type ToolKind, composeTool, browseSourceRoot, openSourceRoot, openLocal, reopenClosed, liveOnly, toolsOf, applicableTools, resolvedTool, defaultTool, selectedTool, selectTool, addTool, removeTool, moveTool, setToolExtensions, setToolField, toolTypeRows, assignToolType, removeToolType, renameToolType, setFallbackTool, browseToolPath, importTools, toolTemplate, toolLabel, toolExtensionLabel, openToolSettings, type ToolSource } from "./dialogs.svelte";
import type { Mode, Side, Row, Dialog, MergePiece, MergePick, RepoFacts, MenuItem, MenuItems, ConfirmAsk, TreeEntry, RepoEntry, CommandEntry, CommandFilter, RepoJob, PrefTab, LaunchItem, Place, SearchKey, StoredError, StatusNotice, StatusNoticeId, StatusNoticeAction, StatusTone } from "./types";
import { STATUS_NOTICE_ENTRY } from "./types";

const initialErrors = readErrorLog();

/** Ids below this belong only to error rows. Notices use small negatives (-101…-106). */
const ERROR_ENTRY_ID = -1_000_000;

/** Histories up to this length are drawn in one pass. Longer ones continue across frames. */
const GRAPH_SYNC_LIMIT = 1500;

function standaloneView(): "" | "history" | "blob" | "merge" {
  if (typeof window === "undefined") return "";
  const view = new URLSearchParams(window.location.search).get("view");
  return view === "history" || view === "blob" || view === "merge" ? view : "";
}

function sameStrings(left: readonly string[], right: readonly string[]): boolean {
  if (left === right) return true;
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return false;
  }
  return true;
}

function sameCommitRow(left: CommitRow, right: CommitRow): boolean {
  return (
    left.id === right.id &&
    left.shortId === right.shortId &&
    left.summary === right.summary &&
    left.author === right.author &&
    left.when === right.when &&
    left.lane === right.lane &&
    left.at === right.at &&
    left.email === right.email &&
    left.unpushed === right.unpushed &&
    left.remoteOnly === right.remoteOnly &&
    sameStrings(left.parents, right.parents) &&
    sameStrings(left.refs, right.refs)
  );
}

function sameCommitRows(left: readonly CommitRow[], right: readonly CommitRow[]): boolean {
  if (left === right) return true;
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    if (!sameCommitRow(left[index]!, right[index]!)) return false;
  }
  return true;
}

function sameStatusFile(left: StatusFile, right: StatusFile): boolean {
  return left.path === right.path && left.kind === right.kind && left.previousPath === right.previousPath;
}

function sameStatusFiles(left: readonly StatusFile[], right: readonly StatusFile[]): boolean {
  if (left === right) return true;
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    if (!sameStatusFile(left[index]!, right[index]!)) return false;
  }
  return true;
}

/** Directories whose every visible file is picked, keyed by the same `/`-joined path the tree uses. */
function fullyPickedDirs(files: readonly Row[], picked: readonly string[]) {
  const pickedSet = new Set(picked);
  const totals = new Map<string, number>();
  const hits = new Map<string, number>();
  for (const file of files) {
    const parts = file.path.split(/[/\\]/);
    let acc = "";
    for (let index = 0; index < parts.length - 1; index += 1) {
      acc = acc ? `${acc}/${parts[index]}` : (parts[index] ?? "");
      totals.set(acc, (totals.get(acc) ?? 0) + 1);
      if (pickedSet.has(file.path)) hits.set(acc, (hits.get(acc) ?? 0) + 1);
    }
  }
  const dirs = new Set<string>();
  for (const [dir, total] of totals) {
    if (total > 0 && hits.get(dir) === total) dirs.add(dir);
  }
  return dirs;
}

function sameSnapshot(left: StatusSnapshot | null, right: StatusSnapshot | null): boolean {
  if (left === right) return true;
  if (!left || !right) return false;
  return left.path === right.path && left.branch === right.branch && sameStatusFiles(left.staged, right.staged) && sameStatusFiles(left.unstaged, right.unstaged);
}

function sameValue(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) return true;
  if (typeof left !== "object" || typeof right !== "object" || left === null || right === null) return false;
  if (Array.isArray(left) || Array.isArray(right)) {
    if (!Array.isArray(left) || !Array.isArray(right) || left.length !== right.length) return false;
    const first = left[0] as Record<string, unknown> | undefined;
    const other = right[0] as Record<string, unknown> | undefined;
    if (first && other && typeof first === "object" && typeof other === "object") {
      if ("shortId" in first && "parents" in first && "shortId" in other) {
        return sameCommitRows(left as CommitRow[], right as CommitRow[]);
      }
      if ("kind" in first && "path" in first && "kind" in other) {
        return sameStatusFiles(left as StatusFile[], right as StatusFile[]);
      }
    }
    for (let index = 0; index < left.length; index += 1) {
      if (!sameValue(left[index], right[index])) return false;
    }
    return true;
  }
  const a = left as Record<string, unknown>;
  const b = right as Record<string, unknown>;
  const aKeys = Object.keys(a);
  if (aKeys.length !== Object.keys(b).length) return false;
  for (const key of aKeys) {
    if (!Object.prototype.hasOwnProperty.call(b, key) || !sameValue(a[key], b[key])) return false;
  }
  return true;
}

export class ShellState {


  defaultSettings: Settings = {
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
    aiLanguage: "zh",
    aiMaxChars: 12000,
    aiPrompt: "",
    aiPromptStyle: "short",
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
    errorLogDays: 7,
    flowHistoryDays: 3,
    flowAutoOpen: true,
    flowAutoClose: false,
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
    diffTools: [],
    mergeTools: [],
    editorTools: [],
    ignoreSpace: false,
    verticalTabs: false,
    pinnedRefs: [],
    sourceDirectories: [],
    groupNames: [],
    showWhitespace: false,
    fileLayout: "",
    fakeIgnore: true,
    tabLabels: [],
    tabColors: [],
    accounts: [],
    bitbucketToken: "",
    azureToken: "",
    azureOrg: "",
    oauthClientId: "",
    fetchAllRemotes: false,
    pushAllTags: false,
    checkoutAfterCreate: true,
    stashIncludeUntracked: false,
    diffWordWrap: false,
    diffContext: 3,
    statusInterval: 0,
    showWorktrees: true,
    maxCommits: 500,
    rebaseUpdateRefs: false,
    applyStashDelete: false,
    pushTagOnCreate: false,
    workspaceShowTitle: false,
    scanDepth: 5,
    commitType: "feat",
    commitTypes: [],
    dismissedUpdate: "",
    silentInstall: true,
    featuresIntro: "seen",
    recoveryNotice: "",
  };



  desktopApp = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;


  /** Browser scene page. The desktop app keeps using Tauri. */
  labPage = typeof window !== "undefined" && !this.desktopApp && new URLSearchParams(window.location.search).has("scene");


  section = $state<"changes" | "history">("changes");


  selectedPath = $state<string | null>(this.desktopApp || this.labPage ? null : (sampleUnstaged[0]?.path ?? null));


  summary = $state("");


  commitType = $state<CommitType>("feat");


  description = $state("");


  amend = $state(false);


  signOff = $state(false);


  skipHooks = $state(false);


  repoSignOff = $state(false);


  selectedSide = $state<Side>("unstaged");


  mode = $state<Mode>(this.desktopApp || this.labPage ? "loading" : "sample");


  labSceneId = $state("");


  snapshot = $state.raw<StatusSnapshot | null>(null);


  loadError = $state<string | null>(null);


  actionErrorText = $state<string | null>(null);

  get actionError() {
    return this.actionErrorText;
  }

  set actionError(value: string | null) {
    this.actionErrorText = value;
    if (value) this.noteError(this.tr("chrome.errorTitle"), value);
  }


  /** The write a dirty work tree blocked, so the error banner can stash the changes and retry it. */
  stashRetry = $state<{ request: Record<string, unknown>; source: "user" | "background" } | null>(null);


  jobs = $state<Record<string, RepoJob>>({});


  /** Repositories with an automatic fetch in flight. This does not count as busy. */
  backgroundFetch = new Set<string>();


  /** Set as soon as a tab is chosen, before that repository's status returns. */
  focusPath = $state<string | null>(null);


  openGen = 0;


  errorCopied = $state(false);


  diff = $state.raw<FileDiff | null>(null);


  diffError = $state<string | null>(null);


  diffToken = 0;

  /** Diffs the user agreed to load past the large-file cutoff, keyed by repo and side. */
  largeLoaded = new Set<string>();


  statusGen = 0;


  contextGen = 0;


  commitSearchGen = 0;


  commitSearchTimer = 0;


  refreshing = false;


  refreshQueued: "status" | "full" | null = null;


  /** A watcher event that arrived while a write was running. */
  watchQueued: "status" | "full" | null = null;


  refs = $state<RefSnapshot | null>(null);


  commitsLive = $state.raw<CommitRow[]>([]);


  progress = $state<InProgress | null>(null);


  selectedCommit = $state<string | null>(null);


  selectedCommitRow = $derived(this.mode === "live" && this.selectedCommit ? (this.commitsLive.find((row) => row.id === this.selectedCommit) ?? null) : null);


  commitFiles = $state.raw<StatusFile[]>([]);


  historyFile = $state<string | null>(null);


  historyDiff = $state.raw<FileDiff | null>(null);


  patchCache = $state.raw<Record<string, FileDiff>>({});
  /** Bumped when a patch is added in place, so readers do not copy the whole map. */
  patchVersion = $state(0);


  openPatches = $state.raw<string[]>([]);


  treeCollapsed = $state<string[]>([]);


  treePrepared = $state<string | null>(null);


  treeBinary = $state(false);
  treeTruncated = $state(false);


  blobTop = $state(0);


  patchGen = 0;


  historyFilter = $state("");


  historyTop = $state(0);


  commitNavTimer = 0;


  historyHeight = $state(0);


  blameLines = $state.raw<BlameLine[] | null>(null);


  settingsPath = $state("");


  forkImportNote = $state("");


  settings = $state<Settings>(
    this.desktopApp
      ? { ...this.defaultSettings }
      : {
          ...this.defaultSettings,
          cloneDirectory: "D:\\work",
          sourceDirectories: ["D:\\work", "D:\\personal"],
          groupNames: ["D:\\work" + "\t" + "工作", "D:\\personal" + "\t" + "个人"],
        },
  );


  repos = $state<string[]>([]);


  repoPickerOpen = $state(false);


  repoPickerRecentOpen = $state(true);


  repoPickerReposOpen = $state(true);


  repoPickerExpanded = $state<string[]>([]);


  repoPickerButton = $state<HTMLButtonElement | null>(null);


  repoPickerPos = $state({ x: 12, y: 72, maxHeight: 560 });


  scannedRepos = $state<string[]>([]);


  repoScanTruncated = $state(false);


  repoScanning = $state(false);


  gitRuntime = $state<GitRuntime | null>(null);


  repoScanToken = 0;


  commandLog = $state<CommandEntry[]>([]);


  commandLogOpen = $state(false);


  commandFilter = $state<CommandFilter>("all");


  errorLog = $state<StoredError[]>(initialErrors);


  errorSeq = initialErrors.reduce((max, item) => Math.max(max, item.id), 0);


  unseenErrors = $state(0);


  recentErrors = $derived.by(() => {
    const cutoff = Date.now() - this.errorDays() * 86_400_000;
    return this.errorLog.filter((item) => item.at >= cutoff);
  });


  commandPick = $state(0);


  commandButton = $state<HTMLButtonElement | null>(null);


  commandPos = $state({ x: 12, y: 80, width: 860, maxHeight: 460 });


  commandSeq = 0;


  dialog = $state<Dialog>(null);


  prefsSave = 0;


  draft = $state("");


  draftExtra = $state("");


  /** The remote dialog edits an existing remote's URL instead of adding one. */
  remoteEdit = $state(false);


  mergeInto = $state("");


  mergeSummary = $state("");


  mergeBody = $state("");


  mergeAutoSummary = $state("");


  mergeFiles = $state<{ status: string; path: string }[]>([]);


  mergeState = $state<MergeState | null>(null);


  /** True when the conflict dialog explains an autostash that could not return cleanly. */
  mergeConflictRestore = $state(false);


  /** Commit being replayed while a rebase is paused, shown in the continue dialog. */
  continueDetail = $state<{ author: string; summary: string } | null>(null);


  /** The continue dialog hides its message field until the user asks to edit it. */
  editContinue = $state(false);


  mergeProgressAutoSummary = $state("");


  /** Fetch, pull, and push dialog options. */
  syncAll = $state(false);


  syncTags = $state(false);


  syncRebase = $state(false);


  syncForce = $state(false);


  pushBranch = $state("");


  pushTo = $state("");


  cloneRoot = $state("");


  cloneFolder = $state("");


  cloneNameManual = $state(false);


  cloneIntent = $state<"clone" | "init">("clone");


  cloneJob = $state<{ name: string; destination: string; workspace: string } | null>(null);


  cloneDone = $state<string | null>(null);


  cloneCancel = false;


  cloneTargetPath = $derived(cloneTarget(this.cloneRoot, this.cloneFolder, this.draft));


  draftUser = $state("");


  draftSecret = $state("");


  launch = $state("");


  launchOpen = $state(false);


  launchIndex = $state(0);


  launchMode = $state<"commands" | "files">("commands");


  launchFiles = $state<string[]>([]);


  standalone = $state<"" | "history" | "blob" | "merge">(standaloneView());


  blobFile = $state("");


  blobRev = $state("");


  blobText = $state("");


  blobLoaded = $state(false);


  blobGen = 0;
  blobTruncated = $state(false);


  blobError = $state("");


  blobBinary = $state(false);


  blobPreview = $state<FilePreview | null>(null);


  launchList = $state<HTMLElement | null>(null);


  allBranches = $state(false);


  /** Branch whose history is shown alone. Empty means the current branch and its upstream. */
  graphSolo = $state<string | null>(null);


  commitQuery = $state("");


  drops = $state<string[]>([]);


  fileTop = $state(0);


  /** Combined staged+unstaged list scrolls as one virtual list. */
  combinedTop = $state(0);


  combinedHeight = $state(0);


  /** Commit-detail file list: scroll offset and measured patch heights. */
  commitScrollTop = $state(0);


  commitViewport = $state(0);


  commitListTop = $state(0);


  patchHeights = $state.raw<Record<string, number>>({});


  sideTop = $state(0);


  stagedTop = $state(0);


  patchView = $state<Record<string, { start: number; end: number }>>({});


  patchSplits = new Map<string, { lines: DiffLine[]; rows: SplitRow[]; columns: { left: number; right: number } }>();


  graphResume: { ids: string[]; columns: Array<string | null>; rows: GraphCell[][]; width: number } | null = null;


  /** Plain cache. The row arrays stay off `$state` so a long graph is not deeply proxied. */
  graphItems: CommitRow[] | null = null;


  graphRows: GraphCell[][] = [];


  graphWidth = 1;


  graphColumns: Array<string | null> = [];


  /** Plain counter: the graph effect must not read the signal it writes. */
  graphEpoch = 0;


  graphVersion = $state(0);


  commitNeedles: { rows: CommitRow[]; text: string[] } | null = null;


  diffTop = $state(0);


  diffHeight = $state(0);


  blameTop = $state(0);


  preview = $state<FilePreview | null>(null);


  updateNotice = $state<UpdateNotice | null>(null);

  pendingUpdate: UpdateNotice | null = null;

  updateBusy = $state(false);

  updatePhase = $state<"idle" | "checking" | "current" | "available" | "downloading" | "verifying" | "ready" | "cancelled" | "error">("idle");

  updateMessage = $state("");

  updatePercent = $state<number | null>(null);

  updateReceived = $state(0);

  updateTotal = $state(0);

  updateSpeed = $state<number | null>(null);

  updateCancelling = $state(false);

  updateCheckedAt = $state(0);

  updateReleaseUrl = $state("");

  updateSpeedAt = 0;

  updateSpeedBytes = 0;

  updateCancelRequested = false;

  appVersion = $state("2.0.7");


  passphrase = $state("");


  passUser = $state("");


  searchEl = $state<HTMLInputElement | null>(null);


  launchEl = $state<HTMLInputElement | null>(null);


  historyEl = $state<HTMLElement | null>(null);


  selectedIndex = $state(0);


  sideQuery = $state("");


  sideView = $state<"refs" | "search">("refs");


  commitSearch = $state("");


  commitSearchField = $state<"message" | "author" | "path" | "diff">("message");


  commitSearchScope = $state<"repository" | "branch">("repository");


  commitSearchOpen = $state(false);


  commitSearchEl = $state<HTMLInputElement | null>(null);


  sideFilter = $state<HTMLInputElement | null>(null);


  fileQuery = $state("");


  detailQuery = $state("");


  recentQuery = $state("");


  searchOpen = $state({ recent: false, side: false, commit: false, file: false, detail: false });


  historyDiffTop = $state(0);


  picked = $state<string[]>([]);


  keyTarget: "unstaged" | "staged" | "combined" | "diff" | "blob" | "detail" | "commits" = "unstaged";


  textWhere = $state<null | "changes" | "blob" | "detail">(null);


  textFile = $state<string | null>(null);


  textSide = $state<"left" | "right" | "all">("all");


  selectRoot: HTMLElement | null = null;


  heldText: string | null = null;


  checkoutNewBranch = $state(true);


  pendingCheckout = $state("");


  expandedMerges = $state<string[]>([]);


  contentHits = $state<{ id: string; shortId: string; summary: string }[] | null>(null);


  activityLines = $state<string[]>([]);


  staleNames = $state<string[]>([]);


  stalePicked = $state<string[]>([]);


  compareWith = $state("");


  pendingCommand = $state<CommandRecord | null>(null);


  commandPrompt = $state("");


  hostForge = $state("github");


  editingAccount = $state("");


  commandShared = $state(true);


  sessionLog = $state<string[]>([]);


  fieldValues = $state<Record<string, string>>({});


  includeUntracked = $state(false);


  stashFiles = $state<string[]>([]);


  flowKind = $state("feature");


  privateRepo = $state(false);


  mergeResult = $state("");


  mergePieces = $state<MergePiece[]>([]);


  textMerge = $state(false);

  mergeKind = $state<"text" | "file">("text");

  /** Text merge above the auto-load limit waits for the user to ask for it. */
  mergeLarge = $state(false);


  mergeFile = $state("");
  mergeSource = $state.raw<MergePiece[]>([]);
  mergeDirty = $state(false);
  mergeEdited = $state(false);
  mergeEditing = $state(false);
  mergeError = $state("");
  mergeLoading = $state(false);
  mergeApplying = $state(false);
  mergeGeneration = 0;


  mergeLeft = $state("");


  mergeRight = $state("");


  closedTabs = $state<string[]>([]);


  facts = $state<RepoFacts | null>(null);


  factsPath = "";


  showIgnored = $state(false);


  conflictsOnly = $state(false);


  hunkAt = $state(0);


  diffAnchor = 0;


  diffSelected = $state<number[]>([]);


  diffEl = $state<HTMLElement | null>(null);


  diffPicked = $derived(new Set(this.diffSelected));


  menu = $state<{ x: number; y: number; items: MenuItems; anchorTop?: number } | null>(null);


  submenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);


  menuOpener: HTMLElement | null = null;


  confirmAsk = $state<ConfirmAsk | null>(null);


  imageMode = $state<"side" | "swipe" | "onion" | "pixel">("side");


  imagePos = $state(50);


  imageOld = $state<string | null>(null);


  imageNew = $state<string | null>(null);


  pixelUrl = $state("");


  reflogOn = $state(false);


  treeOn = $state(false);


  reflogRows = $state<{ id: string; shortId: string; selector: string; summary: string }[]>([]);


  treePaths = $state.raw<string[]>([]);


  treeText = $state("");


  notices = $state<{ id: string; title: string; url: string; unread: boolean }[]>([]);


  pulls = $state<{ title: string; author: string; branch: string; url: string }[]>([]);


  forgeOpen = $state<"notes" | "pulls" | null>(null);


  repoStats = $state<{ branch: string; ahead: number; behind: number; commits: number; branches: number; lastSummary: string } | null>(null);


  rebaseConflictFiles = $state<string[]>([]);
  rebaseConflictKnown = $state(false);


  updateRefs = $state(false);


  lfsAvailable = $state(false);


  prefTab = $state<PrefTab>("general");


  prefPage = $state<HTMLElement | null>(null);



  async selectPrefTab(tab: PrefTab) {
    return selectPrefTab(this, tab);
  }


  commitDetail = $state<CommitDetail | null>(null);


  historyTab = $state<"commit" | "changes" | "tree">("commit");


  collapsedDirs = $state<string[]>([]);


  collapsedSide = $state<string[]>(["tags", "stashes", "submodules", "worktrees"]);


  sideSelected = $state<string | null>(null);


  commitCollapsed = $state<string[]>([]);



  aiModels = $state<string[]>([]);


  aiEndpointNote = $state("");


  aiEndpointBusy = $state<"" | "test" | "models">("");


  dragStep = $state<number | null>(null);


  rebasePicked = $state<string[]>([]);


  recentStats = $state<Record<string, { branch: string; ahead: number; behind: number; lastSummary: string }>>({});
  recentAsked = new Set<string>();


  commandOutput = $state("");


  conflict = $state<ConflictSides | null>(null);


  conflictView = $state<ConflictView | null>(null);


  logLimit = $state(500);


  rebaseSteps = $state<{ verb: string; rev: string; summary: string; message: string }[]>([]);


  comparePair = $state<[string, string] | null>(null);


  drafts = new Map<string, { summary: string; description: string; commitType?: CommitType }>();



  LAYOUT_KEY = "awegit.layout.settings";


  getStoredLayout(): {
    sidebarWidth?: number;
    sidebarCollapsed?: boolean;
    changesWidth?: number;
    unstagedHeight?: number;
    composerHeight?: number;
    historyDetailHeight?: number;
    detailTreeWidth?: number;
  } {
    try {
      const raw = typeof localStorage !== "undefined" ? localStorage.getItem(this.LAYOUT_KEY) : null;
      if (raw) return JSON.parse(raw);
    } catch {}
    return {};
  }



  saveLayoutState(partial: Record<string, unknown>) {
    try {
      const cur = this.getStoredLayout();
      localStorage.setItem(this.LAYOUT_KEY, JSON.stringify({ ...cur, ...partial }));
    } catch {}
  }



  initialLayout = this.getStoredLayout();


  sidebarWidth = $state<number>(this.initialLayout.sidebarWidth ?? 240);


  sidebarCollapsed = $state<boolean>(this.initialLayout.sidebarCollapsed ?? false);


  changesWidth = $state<number>(this.initialLayout.changesWidth ?? 300);


  unstagedHeight = $state<number>(this.initialLayout.unstagedHeight ?? 280);


  composerHeight = $state<number>(this.initialLayout.composerHeight ?? 170);


  historyDetailHeight = $state<number>(this.initialLayout.historyDetailHeight ?? 280);


  detailTreeWidth = $state<number>(this.initialLayout.detailTreeWidth ?? 280);



  handleLayoutChange() {
    this.saveLayoutState({
      sidebarWidth: this.sidebarWidth,
      sidebarCollapsed: this.sidebarCollapsed,
      changesWidth: this.changesWidth,
      unstagedHeight: this.unstagedHeight,
      composerHeight: this.composerHeight,
      historyDetailHeight: this.historyDetailHeight,
      detailTreeWidth: this.detailTreeWidth,
    });
  }



  messageHigh = $derived(this.settings.messageHigh > 0 ? this.settings.messageHigh : 70);


  messageLow = $derived(this.settings.messageLow > 0 ? this.settings.messageLow : 50);


  summaryRoom = $derived(Math.max(1, 200 - this.commitType.length - 2));


  fullSummary = $derived(joinSubject(this.commitType, this.summary));


  summaryTooLong = $derived(this.fullSummary.length > this.messageHigh);


  summaryTone = $derived(this.fullSummary.length > this.messageHigh ? "over" : this.fullSummary.length >= this.messageLow && this.summary.trim().length > 0 ? "warn" : "");


  summaryFitsConvention = $derived(subjectFitsConvention(this.fullSummary));


  descriptionIsGuide = $derived(isCommentary(this.description));


  sampleRow(file: FileChange): Row {
    return sampleRow(this, file);
  }


  selectedDiff = $derived(this.mode === "sample" && this.selectedPath ? (diffs[this.selectedPath] ?? []) : []);


  staged = $derived<Row[]>(
    this.mode === "live" ? (this.snapshot?.staged ?? []).map(this.toRow.bind(this)) : this.mode === "sample" ? sampleStaged.map(this.sampleRow.bind(this)) : [],
  );


  unstaged = $derived<Row[]>(this.mode === "live" ? (this.snapshot?.unstaged ?? []).map(this.toRow.bind(this)) : sampleUnstaged.map(this.sampleRow.bind(this)));


  branchLabel = $derived(
    this.mode === "sample" ? "main" : (this.snapshot?.branch ?? (this.mode === "loading" ? "…" : "HEAD")),
  );


  tabLabel = $derived(this.mode === "live" && this.snapshot ? this.folderName(this.snapshot.path) : "AweGit");


  changeCount = $derived(
    this.mode === "live" || this.mode === "sample" ? this.staged.length + this.unstaged.length : 0,
  );


  conflictCount = $derived.by(() => {
    let count = 0;
    for (const file of this.snapshot?.staged ?? []) if (file.kind === "conflict") count += 1;
    for (const file of this.snapshot?.unstaged ?? []) if (file.kind === "conflict") count += 1;
    return count;
  });


  conflicted = $derived(this.conflictCount > 0);


  selectedConflict = $derived.by(() => {
    if (!this.selectedPath) return false;
    if (this.mode === "sample") return this.selectedSide === "unstaged" && !!sampleConflictViews[this.selectedPath];
    return (
      this.mode === "live" &&
      (this.selectedSide === "staged" ? this.snapshot?.staged : this.snapshot?.unstaged)?.some((file) => file.path === this.selectedPath && file.kind === "conflict") === true
    );
  });


  activeConflict = $derived<ConflictView | null>(
    this.mode === "sample" ? (this.selectedPath ? (sampleConflictViews[this.selectedPath] ?? null) : null) : this.conflictView,
  );


  jobKey(path: string | null | undefined) {
    return (path ?? "").replace(/^\\\\\?\\/, "").replace(/[\\/]+$/, "").replaceAll("\\", "/").toLowerCase();
  }


  viewing(path: string | null | undefined) {
    const key = this.jobKey(path);
    return key !== "" && key === this.jobKey(this.snapshot?.path);
  }


  /** Counts user writes in flight; repeated clicks while one runs are ignored. */
  userWrites = $state(0);


  switching = $derived(!!this.focusPath && this.jobKey(this.focusPath) !== this.jobKey(this.snapshot?.path));


  meterPath = $derived(this.switching ? this.focusPath : (this.snapshot?.path ?? null));


  currentJob = $derived(this.meterPath ? this.jobs[this.jobKey(this.meterPath)] : undefined);


  busy = $derived(this.switching || (this.jobs[this.jobKey(this.snapshot?.path)]?.depth ?? 0) > 0);


  /** A user write is running; further clicks must not queue another one. */
  writing = $derived(this.userWrites > 0);


  syncDisabled(kind: "fetch" | "pull" | "push") {
    return this.busy || this.writing || this.mode !== "live" || !this.snapshot
      || (kind !== "fetch" && (!!this.progress || this.conflicted));
  }


  busyLabel = $derived(this.currentJob?.label ?? "");


  showingClone = $derived(
    !!this.cloneJob &&
      (this.cloneJob.workspace || "") === (this.settings.currentWorkspace || "") &&
      (this.currentJob?.depth ?? 0) === 0,
  );


  workDepth = $derived.by(() => {
    if ((this.currentJob?.depth ?? 0) > 0) return this.currentJob?.depth ?? 0;
    if (!this.showingClone || !this.cloneJob) return 0;
    const depth = this.jobs[this.jobKey(this.cloneJob.destination)]?.depth ?? 0;
    return depth > 0 ? depth : 1;
  });


  workPercent = $derived.by(() => {
    if ((this.currentJob?.depth ?? 0) > 0) return this.currentJob?.percent ?? null;
    if (!this.showingClone || !this.cloneJob) return null;
    return this.jobs[this.jobKey(this.cloneJob.destination)]?.percent ?? null;
  });


  /** Live banners formerly in notice-stack, ordered by priority. */
  statusNotices = $derived.by((): StatusNotice[] => {
    const items: StatusNotice[] = [];
    const live = this.mode === "live";
    const route = this.mergeRouteLabel();
    if (live && this.conflictCount > 0) {
      const actions: StatusNoticeAction[] = [];
      if (this.progress && this.progress !== "merge") {
        actions.push({ id: "continue", label: this.tr("chrome.continueAction"), disabled: this.busy || this.conflicted });
        actions.push({ id: "skip", label: this.tr("chrome.skip"), disabled: this.busy });
      }
      actions.push({ id: "abort", label: this.tr("chrome.abort"), disabled: this.busy });
      const unmerged = this.conflictCount === 1 ? this.tr("chrome.unmergedOne") : this.tr("chrome.unmergedMany", { count: String(this.conflictCount) });
      items.push({
        id: "conflicts",
        priority: 0,
        tone: "conflict",
        title: route ? `${unmerged} · ${route}` : unmerged,
        detail: route || (this.progress ? this.tr("chrome.inProgress", { name: this.progressLabel(this.progress) }) : ""),
        dataLayer: "conflicts",
        floor: true,
        actions,
      });
    } else if (live && this.progress) {
      const progress = this.tr("chrome.inProgress", { name: this.progressLabel(this.progress) });
      items.push({
        id: "progress",
        priority: 1,
        tone: "progress",
        title: route ? `${progress} · ${route}` : progress,
        detail: route,
        dataLayer: "progress",
        floor: true,
        actions: [
          { id: "abort", label: this.tr("chrome.abort"), disabled: this.busy },
          { id: "continue", label: this.tr("chrome.continueAction"), disabled: this.busy || this.conflicted },
          { id: "skip", label: this.tr("chrome.skip"), disabled: this.busy || this.progress === "merge" },
        ],
      });
    }
    if (live && this.facts?.bisectActive) {
      items.push({
        id: "bisect",
        priority: 1,
        tone: "progress",
        title: this.tr("chrome.bisect"),
        detail: "",
        dataLayer: "bisect",
        floor: true,
        actions: [
          { id: "good", label: this.tr("chrome.good"), disabled: this.busy },
          { id: "bad", label: this.tr("chrome.bad"), disabled: this.busy },
          { id: "skip", label: this.tr("chrome.skip"), disabled: this.busy },
          { id: "abort", label: this.tr("chrome.abort"), disabled: this.busy },
        ],
      });
    }
    if (this.actionError) {
      const accessReport = fileAccessReport(this.actionError);
      const actions: StatusNoticeAction[] = [];
      if (this.stashRetry) actions.push({ id: "stash", label: this.tr("chrome.stashAndRetry"), disabled: this.busy });
      actions.push({ id: "close", label: this.tr("chrome.close") });
      items.push({
        id: "error",
        priority: 3,
        tone: "error",
        title: accessReport ? this.tr("chrome.fileAccessError") : this.actionError,
        detail: accessReport ? this.actionError : "",
        dataLayer: "error",
        floor: true,
        actions,
      });
    }
    if (this.cloneDone) {
      items.push({
        id: "clone-done",
        priority: 4,
        tone: "info",
        title: this.tr("chrome.cloneReady", { name: this.folderName(this.cloneDone) }),
        detail: this.cloneDone,
        dataLayer: "clone-done",
        floor: false,
        actions: [{ id: "open", label: this.tr("chrome.open") }],
      });
    }
    if (this.updateNotice) {
      items.push({
        id: "update",
        priority: 4,
        tone: "info",
        title: this.tr("chrome.updateAvailable", { name: `AweGit ${this.updateNotice.appVersion}` }),
        detail: this.updateNotice.notes,
        dataLayer: "update",
        floor: false,
        actions: [{ id: "open", label: this.tr("chrome.updateOpenPanel") }],
      });
    }
    return items.sort((left, right) => left.priority - right.priority || left.id.localeCompare(right.id));
  });


  /** Highest-priority notice that may occupy the identity floor (P0–P3). Busy workDepth stays separate. */
  floorNotice = $derived(this.statusNotices.find((item) => item.floor) ?? null);


  identityTone = $derived.by((): StatusTone | null => {
    if (this.floorNotice) return this.floorNotice.tone;
    if (this.workDepth > 0) return "busy";
    return null;
  });


  floorNoticeAction(notice: StatusNotice): StatusNoticeAction | null {
    const prefer =
      notice.id === "conflicts" || notice.id === "bisect"
        ? "abort"
        : notice.id === "progress"
          ? "continue"
          : notice.id === "error"
            ? this.stashRetry
              ? "stash"
              : "close"
            : notice.actions[0]?.id;
    if (!prefer) return null;
    return notice.actions.find((item) => item.id === prefer) ?? notice.actions[0] ?? null;
  }


  canCommit = $derived(
    this.summary.trim().length > 0 &&
      (this.skipHooks || this.summaryFitsConvention) &&
      !this.summaryTooLong &&
      !this.busy &&
      !this.conflicted &&
      !this.progress &&
      (this.mode !== "live" || this.staged.length > 0 || this.amend),
  );


  rowCommit = $derived(this.rowHeight(this.settings.linesHeight === "spaced" ? 36 : 28, 2.1));


  rowFile = $derived(this.rowHeight(this.settings.linesHeight === "spaced" ? 32 : 28, 2));


  rowTree = $derived(this.rowHeight(this.settings.linesHeight === "spaced" ? 28 : 24, 1.8));


  rowSide = $derived(this.rowHeight(this.settings.linesHeight === "spaced" ? 32 : 28, 2));


  rowHeight(base: number, factor: number) {
    return Math.max(base, Math.ceil((this.settings.uiScale || 13) * factor));
  }


  activeLocale = $derived(resolveLocale(this.settings.locale));


  tr(key: string, vars: Record<string, string> = {}) {
    return translate(this.activeLocale, key, vars);
  }


  sampleRows = $derived([
    { id: "stash@{0}", summary: "WIP before the palette", author: "", when: "", badges: ["stash"], parents: [commits[0]?.id ?? ""], files: [] as (typeof commits)[number]["files"], unpushed: true },
    ...commits,
  ]);


  sampleFold = $derived(foldedMerges(this.sampleRows, this.expandedMerges));


  sampleGraph = $derived(commitGraph(laneCommits(this.sampleRows, this.sampleFold.straight)));


  sampleShown = $derived.by(() => {
    const hidden = this.sampleFold.hidden;
    const open = hidden.size === 0 ? this.sampleRows : this.sampleRows.filter((row) => !hidden.has(row.id));
    const query = this.sideView === "search" ? this.commitSearch.trim().toLowerCase() : "";
    if (!query) return open;
    const rows = this.commitSearchScope === "branch" ? open.filter((row) => row.badges.includes("main")) : open;
    return rows.filter((row) => {
      if (this.commitSearchField === "author") return row.author.toLowerCase().includes(query);
      if (this.commitSearchField === "path") return row.files.some((file) => file.path.toLowerCase().includes(query));
      if (this.commitSearchField === "diff") return false;
      return row.summary.toLowerCase().includes(query) || row.id.toLowerCase().startsWith(query);
    });
  });


  sampleShownGraph = $derived(commitGraph(laneCommits(this.sampleShown, this.sampleFold.straight)));


  sampleTree = $derived([...new Set(commits.flatMap((commit) => commit.files.map((file) => file.path)))]);


  detailFiltering = $derived(this.detailQuery.trim().length > 0);


  commitShown = $derived(this.detailFiltering ? this.commitFiles.filter((file) => this.matchesQuery(file.path, this.detailQuery)) : this.commitFiles);


  changeTree = $derived(this.asTree(this.commitShown.map((file) => this.toRow(file)), this.detailFiltering ? [] : this.commitCollapsed));


  repoTree = $derived(this.asTree((this.mode === "live" ? this.treePaths : this.sampleTree).filter((path) => this.matchesQuery(path, this.detailQuery)).map((path) => ({ path, letter: "", tone: "modified" as const })), this.detailFiltering ? [] : this.treeCollapsed));


  openPatchSet = $derived(new Set(this.openPatches));


  allPatchesOpen = $derived(this.commitShown.length > 0 && this.commitShown.every((file) => this.openPatchSet.has(file.path)));


  /** Measured height wins; the estimate only positions rows before they first mount. */
  commitPatchHeight(path: string) {
    this.patchVersion;
    const measured = this.patchHeights[path];
    if (measured) return measured;
    const cached = this.patchCache[path];
    if (!cached || cached.binary || cached.lines.length === 0) return 34;
    const stride = this.settings.diffStyle === "split" ? this.splitRowHeight : this.diffRowHeight;
    return Math.max(34, cached.lines.length * stride + (cached.truncated ? 22 : 2));
  }


  commitRows = $derived.by(() => {
    const open = this.openPatchSet;
    const rows: { key: string; kind: "file" | "patch"; path: string; file: StatusFile; top: number; height: number }[] = [];
    let top = 0;
    for (const file of this.commitShown) {
      rows.push({ key: `f:${file.path}`, kind: "file", path: file.path, file, top, height: 30 });
      top += 30;
      if (open.has(file.path)) {
        const height = this.commitPatchHeight(file.path);
        rows.push({ key: `p:${file.path}`, kind: "patch", path: file.path, file, top, height });
        top += height;
      }
    }
    return { rows, total: top };
  });


  commitWindow = $derived.by(() => {
    const { rows, total } = this.commitRows;
    if (rows.length <= 80) return { rows, topSpacer: 0, bottomSpacer: 0 };
    const offset = Math.max(0, this.commitScrollTop - this.commitListTop);
    const viewport = this.commitViewport || 600;
    const from = Math.max(0, offset - 200);
    const to = offset + viewport + 400;
    let low = 0;
    let high = rows.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      const row = rows[middle]!;
      if (row.top + row.height < from) low = middle + 1;
      else high = middle;
    }
    const start = Math.min(low, rows.length - 1);
    low = start;
    high = rows.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if (rows[middle]!.top < to) low = middle + 1;
      else high = middle;
    }
    const end = Math.min(rows.length, Math.max(low, start + 1));
    const first = rows[start]!;
    const last = rows[end - 1]!;
    return {
      rows: rows.slice(start, end),
      topSpacer: first.top,
      bottomSpacer: Math.max(0, total - (last.top + last.height)),
    };
  });


  trackCommitScroll(node: HTMLElement) {
    let frame = 0;
    const read = () => {
      frame = 0;
      this.commitScrollTop = node.scrollTop;
      this.commitViewport = node.clientHeight;
    };
    const schedule = () => {
      if (frame) return;
      frame = requestAnimationFrame(read);
    };
    node.addEventListener("scroll", schedule, { passive: true });
    const resize = new ResizeObserver(schedule);
    resize.observe(node);
    read();
    return {
      destroy() {
        if (frame) cancelAnimationFrame(frame);
        node.removeEventListener("scroll", schedule);
        resize.disconnect();
      },
    };
  }


  trackCommitList(node: HTMLElement) {
    const scroller = node.closest(".detail-scroll");
    if (!(scroller instanceof HTMLElement)) return;
    const measure = () => {
      this.commitListTop = node.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
    };
    const resize = new ResizeObserver(measure);
    resize.observe(node);
    if (node.parentElement) {
      resize.observe(node.parentElement);
      // The commit card above the list grows when its body arrives; observe it too.
      for (const sibling of Array.from(node.parentElement.children)) {
        if (sibling !== node) resize.observe(sibling);
      }
    }
    resize.observe(scroller);
    measure();
    return {
      destroy() {
        resize.disconnect();
      },
    };
  }


  measureCommitPatch(node: HTMLElement, path: string) {
    let current = path;
    let last = 0;
    const measure = () => {
      const height = Math.ceil(node.getBoundingClientRect().height);
      if (height <= 0 || Math.abs(height - last) <= 1) return;
      last = height;
      this.patchHeights = { ...this.patchHeights, [current]: height };
    };
    const resize = new ResizeObserver(measure);
    resize.observe(node);
    measure();
    return {
      update(next: string) {
        current = next;
        last = 0;
        measure();
      },
      destroy() {
        resize.disconnect();
      },
    };
  }


  loadCommitPatch(node: HTMLElement, path: string) {
    let current = path;
    const load = (file: string) => {
      if (!this.patchCache[file]) this.requestPatch(file);
    };
    load(current);
    return {
      update(next: string) {
        if (next === current) return;
        current = next;
        load(current);
      },
    };
  }


  /** Patches loading for on-screen rows. Four at a time keeps a fast scroll from
   * spawning dozens of git processes. */
  private patchQueue: string[] = [];
  private patchLoading = new Set<string>();

  requestPatch(file: string) {
    if (this.patchCache[file] || this.patchLoading.has(file) || this.patchQueue.includes(file)) return;
    this.patchQueue.push(file);
    this.pumpPatches();
  }

  private pumpPatches() {
    while (this.patchLoading.size < 4 && this.patchQueue.length > 0) {
      const file = this.patchQueue.shift();
      if (!file || this.patchCache[file]) continue;
      this.patchLoading.add(file);
      void this.ensurePatch(file).finally(() => {
        this.patchLoading.delete(file);
        this.pumpPatches();
      });
    }
  }

  clearPatchQueue() {
    this.patchQueue = [];
    this.patchLoading.clear();
  }


  blobLines = $derived.by(() => {
    const text = this.mode === "live" ? this.treeText : this.sampleBlob(this.historyFile ?? "");
    return text ? text.split("\n") : [];
  });


  blobWindow = $derived.by(() => {
    if (this.blobLines.length <= 400) return { start: 0, rows: this.blobLines };
    const start = Math.max(0, Math.floor(this.blobTop / 18) - 30);
    return { start, rows: this.blobLines.slice(start, start + 140) };
  });


  shownCommits = $derived.by(() => {
    const query = this.commitQuery.trim().toLowerCase();
    const rows = this.commitsLive;
    if (!query) return rows;
    if (this.commitNeedles?.rows !== rows) {
      this.commitNeedles = {
        rows,
        text: rows.map((commit) => `${commit.summary}\n${commit.author}\n${commit.email ?? ""}\n${commit.refs.join("\n")}`.toLowerCase()),
      };
    }
    const text = this.commitNeedles.text;
    const hex = /^[0-9a-f]{4,}$/.test(query);
    const matched: CommitRow[] = [];
    for (let index = 0; index < rows.length; index += 1) {
      const row = rows[index];
      if (row && (text[index]?.includes(query) || (hex && (row.id.startsWith(query) || row.shortId.startsWith(query))))) matched.push(row);
    }
    return matched;
  });


  liveFold = $derived(foldedMerges(this.commitsLive, this.expandedMerges));


  historyItems = $derived.by(() => {
    const stashes = this.refs?.stashes ?? [];
    const hidden = this.liveFold.hidden;
    let rows = hidden.size === 0 ? this.shownCommits : this.shownCommits.filter((row) => !hidden.has(row.id));
    if (stashes.length === 0 && hidden.size === 0) return rows;
    if (stashes.length > 0) {
      rows = rows.slice();
      for (const stash of stashes) {
        const parent = stash.parent;
        if (!parent) continue;
        const extra: CommitRow = { id: stash.id || stash.name, shortId: stash.name, summary: stash.summary, author: "", when: "", parents: [parent], refs: ["stash"], lane: 0, at: 0, email: "", unpushed: true };
        const at = rows.findIndex((row) => row.id === parent || row.id.startsWith(parent));
        if (at >= 0) rows.splice(at, 0, extra);
      }
    }
    return rows;
  });


  historyWindow = $derived(windowSlice(this.historyItems, this.historyTop, this.rowCommit, 80, this.windowRows(this.historyHeight, this.rowCommit)));


  historyStart = $derived(this.historyWindow.start);


  historyRows = $derived(this.historyWindow.rows);


  shownUnstaged = $derived(this.unstaged.filter((file) => this.matchesQuery(file.path, this.fileQuery) && (!this.conflictsOnly || file.letter === "C")));


  shownStaged = $derived(this.staged.filter((file) => this.matchesQuery(file.path, this.fileQuery) && (!this.conflictsOnly || file.letter === "C")));


  pickedUnstagedDirs = $derived(fullyPickedDirs(this.shownUnstaged, this.picked));


  pickedStagedDirs = $derived(fullyPickedDirs(this.shownStaged, this.picked));


  combinedRows = $derived.by(() => [
    ...this.shownStaged.map((file) => ({ key: `s:${file.path}`, file, side: "staged" as Side })),
    ...this.shownUnstaged.map((file) => ({ key: `u:${file.path}`, file, side: "unstaged" as Side })),
  ]);


  combinedWindow = $derived(windowSlice(this.combinedRows, this.combinedTop, this.rowFile, 80, this.windowRows(this.combinedHeight, this.rowFile)));


  fileLayout = $derived(this.settings.fileLayout || (this.settings.treeFiles ? "tree" : "list"));


  unstagedTree = $derived(this.fileLayout === "tree" ? this.asTree(this.shownUnstaged, this.collapsedDirs) : this.flatEntries(this.shownUnstaged));


  fileStride = $derived(this.fileLayout === "tree" ? this.rowTree : this.rowFile);


  fileWindow = $derived(windowSlice(this.unstagedTree, this.fileTop, this.fileStride));


  stagedTree = $derived(this.fileLayout === "tree" ? this.asTree(this.shownStaged) : this.flatEntries(this.shownStaged));


  stagedWindow = $derived(windowSlice(this.stagedTree, this.stagedTop, this.fileStride));


  ignoredWindow = $derived(windowSlice(this.facts?.ignored ?? [], this.fileTop, this.fileStride));


  diffLines = $derived(this.diff?.lines ?? []);


  /** Wrapped diffs draw every line, so large ones stay plain text instead of highlighting each one. */
  heavyWrap = $derived.by(() => {
    if (!this.settings.diffWordWrap) return false;
    this.patchVersion;
    if (this.section === "changes") return this.diffLines.length > 300;
    if (this.openPatches.length > 500) return true;
    for (const file of this.openPatches) {
      const lines = this.patchCache[file]?.lines.length ?? (this.historyDiff?.path === file ? this.historyDiff.lines.length : 0);
      if (lines > 300) return true;
    }
    return false;
  });



  discardableEntries(indexes: number[]) {
    return discardableEntries(this, indexes);
  }



  discardableSelected = $derived(this.discardableEntries(this.diffSelected));


  shownDiffLines = $derived(numberedDiff(this.diffLines));


  liveGraph = $derived.by(() => {
    const items = this.historyItems;
    if (items.length > GRAPH_SYNC_LIMIT && this.graphVersion > 0 && this.graphItems === items) {
      return { rows: this.graphRows, width: this.graphWidth, columns: this.graphColumns };
    }
    const limit = items.length > GRAPH_SYNC_LIMIT ? Math.min(items.length, 400) : items.length;
    const built = commitGraph(laneCommits(items.slice(0, limit), this.liveFold.straight));
    return { rows: built.rows, width: built.width, columns: built.columns };
  });


  minimapMarks = $derived.by(() => {
    const lines = this.diff?.lines ?? [];
    const total = lines.length;
    if (total === 0) return [];
    const slots = Math.min(120, total);
    const marks: { index: number; kind: string; top: number }[] = [];
    for (let slot = 0; slot < slots; slot++) {
      const from = Math.floor((slot * total) / slots);
      const to = slot === slots - 1 ? total : Math.floor(((slot + 1) * total) / slots);
      for (let index = from; index < to; index++) {
        const line = lines[index];
        if (line && (line.kind === "add" || line.kind === "delete" || line.kind === "hunk")) {
          marks.push({ index, kind: line.kind, top: (index / total) * 100 });
          break;
        }
      }
    }
    return marks;
  });


  diffRowHeight = $derived(Math.max(18, Math.ceil((this.settings.diffFontSize || 13) * 1.45)));


  diffWindow = $derived(virtualLines(this.diffLines, this.diffTop, this.diffRowHeight, this.windowRows(this.diffHeight, this.diffRowHeight), this.settings.diffWordWrap));


  blameWindow = $derived(windowSlice(this.blameLines ?? [], this.blameTop, this.diffRowHeight, 80, this.windowRows(this.diffHeight, this.diffRowHeight)));


  historyDiffWindow = $derived(virtualLines(this.historyDiff?.lines ?? [], this.historyDiffTop, this.diffRowHeight, this.windowRows(this.historyDetailHeight, this.diffRowHeight), this.settings.diffWordWrap));


  splitRows = $derived(this.settings.diffStyle === "split" ? splitDiff(this.diffLines) : []);


  splitRowHeight = $derived(this.diffRowHeight);


  splitWindow = $derived(virtualSplit(this.splitRows, this.diffTop, this.splitRowHeight, this.diffHeight, this.settings.diffWordWrap));


  splitColumns = $derived.by(() => {
    let left = 0;
    let right = 0;
    for (const row of this.splitRows) {
      left = Math.max(left, this.textColumns(row.left));
      right = Math.max(right, this.textColumns(row.right));
    }
    const mark = this.settings.showDiffMarks ? 1 : 0;
    return { left: left + mark, right: right + mark };
  });


  textColumns(text: string) {
    return textColumns(this, text);
  }


  cachedSplit(file: string, lines: DiffLine[]) {
    return cachedSplit(this, file, lines);
  }


  splitColumnsOf(rows: SplitRow[]) {
    return splitColumnsOf(this, rows);
  }


  trackPatch(node: HTMLElement, spec: { file: string; count: number; rowHeight: number }) {
    return trackPatch(this, node, spec);
  }


  sampleSplitOf(lines: string[]) {
    return sampleSplitOf(this, lines);
  }


  sampleSplit = $derived(this.sampleSplitOf(this.selectedDiff));


  splitPick = $state<"left" | "right">("left");


  pickSplitSide(event: MouseEvent) {
    return pickSplitSide(this, event);
  }


  /** `columns` is the widest line per side, for a virtual list that only renders some of its rows. */
  splitScroller(node: HTMLElement, columns?: { left: number; right: number }) {
    return splitScroller(this, node, columns);
  }


  filterNames = $derived.by(() => {
    if (this.historyFilter) return [this.historyFilter];
    if (this.allBranches) return [];
    if (this.mode === "sample") return this.graphSolo ? [this.graphSolo] : ["main", "origin/main"];
    return this.exactRefs();
  });


  pinnedHere = $derived(
    new Set(
      this.settings.pinnedRefs
        .filter((item) => item.startsWith(`${this.snapshot?.path ?? ""}\t`))
        .map((item) => item.slice(item.indexOf("\t") + 1)),
    ),
  );


  visibleBranches = $derived.by(() => {
    const rows = (this.refs?.branches ?? []).filter((branch) => (branch.current || !this.refHidden(branch.name)) && this.matchesQuery(branch.name, this.sideQuery));
    const order = new Map(orderRefNames(rows.map((branch) => branch.name), this.pinnedHere).map((name, index) => [name, index]));
    return rows.slice().sort((a, b) => (order.get(a.name) ?? 0) - (order.get(b.name) ?? 0));
  });


  currentBranch = $derived(this.refs?.branches.find((branch) => branch.current) ?? null);


  localBranchNames = $derived(new Set((this.refs?.branches ?? []).map((branch) => branch.name)));


  remoteBranchNames = $derived.by(() => {
    const names = new Set<string>();
    for (const remote of this.refs?.remotes ?? []) for (const branch of remote.branches) names.add(branch);
    return names;
  });


  visibleTags = $derived.by(() => {
    const rows = (this.refs?.tags ?? []).filter((tag) => !this.refHidden(tag.name) && this.matchesQuery(tag.name, this.sideQuery));
    if (!rows.some((tag) => tag.name.includes("/"))) return rows;
    // Git returns newest first. Keep that order within folders and place each folder
    // at its newest tag, in one linear pass instead of sorting names on the UI thread.
    const groups = new Map<string, typeof rows>();
    for (const tag of rows) {
      const group = branchGroup(tag.name);
      const key = group ? `${group}/` : tag.name;
      const bucket = groups.get(key);
      if (bucket) bucket.push(tag);
      else groups.set(key, [tag]);
    }
    return Array.from(groups.values()).flat();
  });


  navBack: Place[] = [];


  navForward: Place[] = [];


  navLock = 0;


  lastPlace: Place | null = null;


  navStamp = 0;



  matches = $derived.by(() => {
    const query = this.launch.trim().toLowerCase();
    const action = this.tr("chrome.groupAction");
    const repoGroup = this.tr("chrome.groupRepo");
    if (query.startsWith("!")) {
      const command = query.slice(1);
      return [{ label: command ? `! ${command}` : "!", group: action, run: () => this.mutate({ action: "custom", command }) }];
    }
    if (this.launchMode === "files") {
      const files: LaunchItem[] = [];
      for (const path of this.launchFiles) {
        if (query && !path.toLowerCase().includes(query)) continue;
        const name = path.split("/").pop() || path;
        files.push({
          label: name,
          group: path,
          run: () => void invoke("open_window", { view: "history", repo: this.repoPath(), file: path }),
        });
        if (files.length >= 40) break;
      }
      return files;
    }
    const items: LaunchItem[] = [
      { label: this.tr("chrome.fetch"), group: action, run: () => void this.doFetch() },
      { label: this.tr("chrome.fetchAll"), group: action, run: () => void this.mutate({ action: "fetch", remote: null, prune: this.settings.fetchPrune, tags: this.settings.fetchTags }) },
      { label: this.tr("chrome.pull"), group: action, run: () => void this.doPull() },
      { label: this.tr("dialog.fastForward"), group: action, run: () => { this.settings.pullRebase = false; void this.doPull(); } },
      { label: this.tr("chrome.push"), group: action, run: () => void this.doPush() },
      { label: this.tr("menu.createTag"), group: action, run: () => { this.draft = ""; this.draftExtra = "HEAD"; this.draftUser = ""; this.dialog = "tag"; } },
      { label: this.tr("chrome.pushTags"), group: action, run: () => void this.mutate({ action: "push", remote: null, setUpstream: true, tags: true, forceWithLease: this.settings.forceWithLease }) },
      { label: this.tr("chrome.stash"), group: action, run: () => this.openStash([]) },
      { label: this.tr("chrome.pop"), group: action, run: () => this.confirmPopStash() },
      { label: this.tr("chrome.refresh"), group: action, run: () => void this.refresh() },
      { label: this.tr("chrome.stageAll"), group: action, run: () => void this.runChange("stage_all") },
      { label: this.tr("chrome.commit"), group: action, run: () => void this.submitCommit(false) },
      { label: this.tr("chrome.commitAndPush"), group: action, run: () => void this.submitCommit(true) },
      { label: this.tr("chrome.preferences"), group: action, run: () => { this.dialog = "prefs"; } },
      { label: this.tr("chrome.terminal"), group: action, run: () => void invoke("open_terminal", { path: this.repoPath() }) },
      { label: this.tr("chrome.explorer"), group: action, run: () => { if (this.snapshot) void this.openLocal(this.snapshot.path); } },
      { label: this.tr("chrome.flow"), group: action, run: () => { this.draft = ""; this.dialog = "flow"; } },
      { label: this.tr("chrome.about"), group: action, run: () => { this.dialog = "about"; } },
      { label: this.tr("chrome.open"), group: action, run: () => this.beginOpen() },
      { label: this.tr("chrome.workspace"), group: action, run: () => { this.draft = ""; this.dialog = "workspace"; } },
      { label: this.tr("chrome.clone"), group: action, run: () => void this.beginClone("clone") },
      { label: this.tr("chrome.newRepo"), group: action, run: () => void this.beginClone("init") },
      { label: this.tr("chrome.notifications"), group: action, run: () => this.openForge("notes") },
      { label: this.tr("chrome.pulls"), group: action, run: () => this.openForge("pulls") },
      { label: this.tr("chrome.changes"), group: action, run: () => { this.section = "changes"; } },
      { label: this.tr("chrome.commits"), group: action, run: () => { this.section = "history"; } },
      ...this.settings.workspaces.map((workspace) => ({
        label: this.workspaceName(workspace),
        group: this.tr("chrome.workspace"),
        run: () => void this.switchWorkspace(workspace.id),
      })),
      ...[...new Set([...this.repos, ...this.settings.recent])].map((path) => ({
        label: this.folderName(path),
        group: repoGroup,
        run: () => void this.openRepo(path),
      })),
    ];
    if (this.mode === "live") {
      items.push(
        { label: this.tr("chrome.fileHistory"), group: action, stay: true, run: () => { this.launchMode = "files"; this.launch = ""; this.launchIndex = 0; void this.loadLaunchFiles(); } },
        ...this.settings.commands.map((command) => ({ label: command.name, group: action, run: () => this.runCommand(command) })),
        ...(this.refs?.branches ?? []).map((branch) => ({
          label: `${this.tr("menu.checkout")} ${branch.name}`,
          group: this.tr("chrome.groupBranch"),
          run: () => void this.mutate({ action: "checkout", name: branch.name }),
        })),
        ...(this.refs?.tags ?? []).map((tag) => ({
          label: tag.name,
          group: this.tr("chrome.groupTag"),
          run: () => void this.mutate({ action: "checkout", name: tag.name }),
        })),
        ...(this.refs?.remotes ?? []).map((remote) => ({
          label: remote.name,
          group: this.tr("chrome.groupRemote"),
          run: () => void this.mutate({ action: "fetch", remote: remote.name, prune: this.settings.fetchPrune, tags: this.settings.fetchTags }),
        })),
        ...(this.refs?.stashes ?? []).map((stash) => ({
          label: stash.summary || stash.name,
          group: this.tr("chrome.groupStash"),
          run: () => this.confirmPopStash(stash.name),
        })),
      );
    }
    const seen = new Set<string>();
    const unique: LaunchItem[] = [];
    for (const item of items) {
      const key = `${item.group}\0${item.label}`;
      if (seen.has(key)) continue;
      if (query && !this.fuzzy(query, item.label)) continue;
      seen.add(key);
      unique.push(item);
    }
    return unique.slice(0, query ? 30 : 60);
  });



  fuzzy(query: string, label: string) {
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



  async openLaunch() {
    return openLaunch(this);
  }


  async loadLaunchFiles() {
    if (this.mode !== "live") {
      this.launchFiles = [];
      return;
    }
    try {
      this.launchFiles = await invoke<string[]>("commit_tree", { path: this.repoPath(), rev: "HEAD" });
    } catch (error) {
      this.launchFiles = [];
      this.actionError = this.message(error);
    }
  }


  commitSearchEnter() {
    const query = this.commitQuery.trim();
    if (query.startsWith("S:") || query.startsWith("G:")) {
      void this.searchContent(query[0] ?? "S", query.slice(2).trim());
      return;
    }
    if (/^[0-9a-f]{4,}$/i.test(query) && this.shownCommits.length === 0) void this.locateRef(query);
  }


  revisionFileMenu(file: string): MenuItem[] {
    return revisionFileMenu(this, file);
  }


  historyFileMenu(commit: CommitRow): MenuItem[] {
    return historyFileMenu(this, commit);
  }


  async loadBlobPane() {
    if (!this.blobFile) return;
    const file = this.blobFile;
    const rev = this.blobRev || "HEAD";
    const path = this.repoPath();
    const gen = ++this.blobGen;
    const current = () => gen === this.blobGen && this.blobFile === file && (!path || this.viewing(path));
    this.blobLoaded = false;
    this.blobError = "";
    try {
      const view = await invoke<{ binary: boolean; text: string; preview: FilePreview | null; truncated?: boolean }>("blob_view", { path, rev, file });
      if (!current()) return;
      this.blobBinary = view.binary && !view.preview && !view.text;
      this.blobPreview = view.preview;
      this.blobText = view.text;
      this.blobTruncated = view.truncated === true;
      if (this.inApp()) await getCurrentWindow().setTitle(this.fileName(file)).catch(() => {});
    } catch (error) {
      if (!current()) return;
      this.actionError = this.message(error);
      this.blobError = this.message(error);
      this.blobText = "";
      this.blobPreview = null;
      this.blobBinary = false;
      this.blobTruncated = false;
    } finally {
      if (current()) this.blobLoaded = true;
    }
  }


  async applyWindowNav(nav: { view: string; repo: string; file: string; rev: string }) {
    const label = this.inApp() ? getCurrentWindow().label : "";
    if (nav.view === "history") {
      if (label && label !== "history") return;
      this.standalone = "history";
      if (nav.file) this.historyFilter = nav.file;
      if (nav.repo && nav.repo !== this.repoPath()) await this.openRepo(nav.repo);
      if (nav.file) await this.showFileHistory(nav.file);
      return;
    }
    if (nav.view === "blob") {
      if (label && label !== "blob") return;
      this.standalone = "blob";
      this.blobFile = nav.file;
      this.blobRev = nav.rev;
      if (nav.repo && nav.repo !== this.repoPath()) await this.openRepo(nav.repo);
      await this.loadBlobPane();
      return;
    }
    if (nav.view === "merge") {
      if (label && label !== "merge") return;
      if (this.textMerge && nav.file === this.mergeFile && (!nav.repo || this.pathsMatch(nav.repo, this.repoPath() ?? ""))) return;
      if (this.busy) return;
      this.guardMerge(() => { void (async () => {
        this.textMerge = false;
        this.standalone = "merge";
        if (nav.repo && !this.pathsMatch(nav.repo, this.repoPath() ?? "")) {
          await this.openRepo(nav.repo);
          if (!this.pathsMatch(nav.repo, this.repoPath() ?? "")) return;
        }
        if (nav.file) await this.openTextMerge(nav.file);
      })().catch((error) => { this.mergeError = this.message(error); }); });
    }
  }



  closeLaunch() {
    return closeLaunch(this);
  }



  runLaunch(item: LaunchItem) {
    return runLaunch(this, item);
  }



  onLaunchKey(event: KeyboardEvent) {
    return onLaunchKey(this, event);
  }



  snapshotPlace(): Place {
    return {
      section: this.section,
      path: this.selectedPath,
      side: this.selectedSide,
      commit: this.selectedCommit,
      blame: this.blameLines != null,
      tree: this.treeOn,
      historyFilter: this.historyFilter,
    };
  }



  samePlace(a: Place, b: Place) {
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



  async restorePlace(place: Place) {
    this.navLock += 1;
    this.section = place.section;
    this.selectedSide = place.side;
    this.selectedPath = place.path;
    this.selectedCommit = place.commit;
    this.treeOn = place.tree;
    const filterChanged = this.historyFilter !== place.historyFilter;
    this.historyFilter = place.historyFilter;
    if (!place.blame) this.blameLines = null;
    try {
      if (this.mode === "live" && filterChanged) {
        if (place.historyFilter) {
          this.commitsLive = await invoke<CommitRow[]>("file_history", { path: this.repoPath(), file: place.historyFilter, limit: 200 });
        } else {
          await this.loadContext();
        }
      }
      if (this.mode === "live" && place.section === "history" && place.commit) await this.selectCommit(place.commit);
      if (place.blame && place.path && this.mode === "live") await this.showBlame(place.path);
      if (place.tree && place.commit && this.mode === "live") {
        this.treePaths = await invoke<string[]>("commit_tree", { path: this.repoPath(), rev: place.commit });
      }
      if (this.mode === "live" && place.section === "changes" && place.path) await this.loadDiff();
    } finally {
      this.lastPlace = this.snapshotPlace();
      this.navLock -= 1;
    }
  }



  goBack() {
    const now = Date.now();
    if (now - this.navStamp < 150) return;
    this.navStamp = now;
    if (this.navBack.length === 0) return;
    const previous = this.navBack[this.navBack.length - 1];
    this.navForward = [this.snapshotPlace(), ...this.navForward].slice(0, 50);
    this.navBack = this.navBack.slice(0, -1);
    void this.restorePlace(previous);
  }



  goForward() {
    const now = Date.now();
    if (now - this.navStamp < 150) return;
    this.navStamp = now;
    if (this.navForward.length === 0) return;
    const next = this.navForward[0];
    this.navBack = [...this.navBack, this.snapshotPlace()].slice(-50);
    this.navForward = this.navForward.slice(1);
    void this.restorePlace(next);
  }



  inApp() {
    return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  }



  /** Desktop commands, or the browser lab when a scene is open. */
  backend() {
    return this.inApp() || this.labPage;
  }



  async bootLab() {
    const params = new URLSearchParams(window.location.search);
    const id = params.get("scene") ?? "";
    try {
      const [{ sceneById }, { applyLabScene }] = await Promise.all([import("../lab/scenes"), import("../lab/apply")]);
      const item = sceneById(id);
      if (!item) throw new Error(`未知场景 ${id}`);
      const layout = await invoke<Record<string, string>>("lab_layout");
      const settings = await invoke<Settings>("load_settings");
      const shelfRecent = (layout.recentList ?? "").split(/\r?\n/).map((path) => path.trim()).filter(Boolean);
      const recent = shelfRecent.length > 0
        ? shelfRecent
        : [layout.full, layout.ahead, layout.conflict, layout.even].filter((path) => !!path);
      const sourceDirectories = layout.repos ? [layout.repos] : settings.sourceDirectories;
      const commands = Array.from({ length: 24 }, (_, index) => {
        const n = String(index + 1).padStart(2, "0");
        return {
          id: `lab-${n}`,
          name: `实验室命令 ${n}`,
          target: "repository",
          command: `git status --short # ${n}`,
          prompt: "",
          shared: true,
          repo: "",
        };
      });
      const accounts = Array.from({ length: 24 }, (_, index) => {
        const n = String(index + 1).padStart(2, "0");
        return {
          id: `lab-acct-${n}`,
          forge: index % 2 === 0 ? "github" : "gitlab",
          label: `lab-${n}`,
          token: `lab-token-${n}`,
          host: index % 2 === 0 ? "github.com" : "gitlab.com",
          clientId: `lab-client-${n}`,
        };
      });
      const workspaces = Array.from({ length: 24 }, (_, index) => {
        const n = String(index + 1).padStart(2, "0");
        return {
          id: `lab-ws-${n}`,
          name: `实验室工作区 ${n}`,
          repositories: recent.slice(0, Math.min(3, recent.length)),
          openTabs: [],
          selectedTab: 0,
          tabsSeeded: true,
        };
      });
      this.adoptSettings({
        ...settings,
        locale: params.get("locale") === "en" ? "en" : params.get("locale") === "zh" ? "zh" : settings.locale,
        theme: params.get("theme") === "dark" ? "dark" : params.get("theme") === "light" ? "light" : settings.theme,
        authorName: settings.authorName || "AweGit Lab",
        authorEmail: settings.authorEmail || "lab@example.com",
        recent,
        sourceDirectories,
        groupNames: sourceDirectories.map((path) => `${path}\t实验室仓库`),
        cloneDirectory: layout.repos || settings.cloneDirectory,
        commands,
        accounts,
        workspaces,
        currentWorkspace: "",
        fetchAutomatically: false,
        statusInterval: 0,
        fileLayout: "list",
      });
      this.collapsedSide = [];
      this.allBranches = true;
      this.mergeTools = Array.from({ length: 24 }, (_, index) => {
        const n = String(index + 1).padStart(2, "0");
        return { name: `Merge Tool ${n}`, path: `C:\\Tools\\merge-${n}.exe`, args: "$LOCAL $REMOTE $MERGED", primary: index === 0 };
      });
      this.diffTools = Array.from({ length: 24 }, (_, index) => {
        const n = String(index + 1).padStart(2, "0");
        return { name: `Diff Tool ${n}`, path: `C:\\Tools\\diff-${n}.exe`, args: "$LOCAL $REMOTE", primary: index === 0 };
      });
      this.aiModels = Array.from({ length: 24 }, (_, index) => `lab-model-${String(index + 1).padStart(2, "0")}`);
      void invoke<string>("app_version").then((value) => { if (value) this.appVersion = value; }).catch(() => {});
      void invoke<string>("update_release_page").then((value) => { if (value) this.updateReleaseUrl = value; }).catch(() => {});
      if (item.repo) {
        const path = layout[item.repo];
        if (!path) throw new Error(`场景 ${item.id} 没有仓库 ${item.repo}`);
        if (["conflict", "merge", "rebase", "cherry", "revert", "bisect"].includes(item.repo)) {
          const progress = await invoke<string | null>("in_progress", { path }).catch(() => null);
          const facts = await invoke<{ bisectActive?: boolean }>("repo_facts", { path, full: false }).catch(() => null);
          const status = await invoke<{ staged?: { kind: string }[]; unstaged?: { kind: string }[] }>("workspace_status", { path }).catch(() => null);
          const conflicts = [...(status?.staged ?? []), ...(status?.unstaged ?? [])].some((file) => file.kind === "conflict");
          const need = item.repo === "conflict"
            ? !conflicts
            : item.repo === "bisect"
              ? !facts?.bisectActive
              : !progress;
          if (need) await invoke("lab_restore", { name: item.repo });
        }
        const file = params.get("file");
        if (item.id === "file-history" && file) this.historyFilter = file;
        await this.openRepo(path);
        if (!this.snapshot) throw new Error(this.actionError || `打不开 ${item.repo}`);
      } else {
        this.snapshot = null;
        this.mode = "error";
        this.loadError = "";
      }
      const runtime = await invoke<GitRuntime>("git_runtime").catch(() => null);
      if (runtime) this.gitRuntime = runtime;
      await applyLabScene(this, item);
      if (params.get("locale") === "en") this.settings.locale = "en";
      this.labSceneId = item.id;
    } catch (error) {
      const text = this.message(error);
      this.loadError = text;
      if (!this.snapshot) this.mode = "error";
      console.error(text);
    }
  }



  repoPath() {
    return this.snapshot?.path;
  }



  startWork(label: string, path = this.repoPath() ?? "") {
    const key = this.jobKey(path);
    if (!key) return;
    const current = this.jobs[key];
    if (current && current.depth > 0) {
      this.jobs[key] = { ...current, depth: current.depth + 1 };
      return;
    }
    this.jobs[key] = { label, percent: null, depth: 1 };
  }



  stopWork(path = this.repoPath() ?? "") {
    const key = this.jobKey(path);
    const current = key ? this.jobs[key] : undefined;
    if (!current) return;
    if (current.depth <= 1) delete this.jobs[key];
    else this.jobs[key] = { ...current, depth: current.depth - 1 };
    this.releaseWriteWaiters();
  }


  /** User writes wait for background reads instead of being dropped. */
  writeWaiters: Array<() => void> = [];


  async waitForWriteSlot() {
    while (this.busy) {
      await new Promise<void>((resolve) => this.writeWaiters.push(resolve));
    }
  }


  releaseWriteWaiters() {
    if (this.busy || this.writeWaiters.length === 0) return;
    const waiting = this.writeWaiters;
    this.writeWaiters = [];
    for (const resolve of waiting) resolve();
  }



  aliasJob(from: string, to: string) {
    const left = this.jobKey(from);
    const right = this.jobKey(to);
    if (!left || left === right) return;
    const job = this.jobs[left];
    if (!job) return;
    if (!this.jobs[right]) this.jobs[right] = job;
    delete this.jobs[left];
  }



  noteProgress(path: string, percent: number) {
    const key = this.jobKey(path);
    const current = this.jobs[key];
    if (current) {
      if (current.percent !== percent) this.jobs[key] = { ...current, percent };
      return;
    }
    if (!this.cloneJob) return;
    const dest = this.jobKey(this.cloneJob.destination);
    const cloning = this.jobs[dest];
    if (cloning && cloning.percent !== percent) this.jobs[dest] = { ...cloning, percent };
  }



  message(error: unknown) {
    const raw = (error instanceof Error ? error.message : String(error)).trim();
    const coded = /^awegit:([a-z]+):(.*)$/i.exec(raw);
    if (coded) {
      const code = (coded[1] ?? "").toLowerCase();
      const detail = (coded[2] ?? "").trim();
      const known = code === "network" || code === "auth" || code === "update" || code === "git" || code === "cancelled" || code === "settings";
      const sentence = known ? this.tr(`error.${code}`) : raw;
      return detail && known ? `${sentence} ${detail}` : sentence;
    }
    if (/^could not start git/i.test(raw)) return this.tr("chrome.gitMissing");
    const open = /^could not open repository at (.+)$/i.exec(raw);
    if (open) return this.tr("dialog.errorOpenRepo", { path: open[1] ?? "" });
    if (/^not a git repository/i.test(raw)) return this.tr("dialog.errorNotRepo");
    return raw;
  }



  /** Unchanged data keeps its old object so lists and diffs are not rendered again. */
  same(left: unknown, right: unknown) {
    return sameValue(left, right);
  }


  windowRows(height: number, rowHeight: number) {
    return Math.max(72, Math.ceil(Math.max(height, 0) / Math.max(rowHeight, 1)) + 48);
  }



  applySnapshot(value: StatusSnapshot) {
    this.statusGen += 1;
    if (!sameSnapshot(this.snapshot, value)) this.snapshot = value;
    this.mode = "live";
    this.loadError = "";
    if (value.path && !this.repos.includes(value.path)) this.repos = [...this.repos, value.path];
    const sameSide = (this.selectedSide === "staged" ? value.staged : value.unstaged).some(
      (file) => file.path === this.selectedPath,
    );
    if (this.selectedPath && sameSide) return;
    const otherSide: Side = this.selectedSide === "staged" ? "unstaged" : "staged";
    const other = otherSide === "staged" ? value.staged : value.unstaged;
    if (this.selectedPath && other.some((file) => file.path === this.selectedPath)) {
      this.selectedSide = otherSide;
      return;
    }
    const conflict = value.unstaged.find((file) => file.kind === "conflict");
    if (conflict) {
      this.selectedSide = "unstaged";
      this.selectedPath = conflict.path;
    } else if (value.unstaged[0]) {
      this.selectedSide = "unstaged";
      this.selectedPath = value.unstaged[0].path;
    } else if (value.staged[0]) {
      this.selectedSide = "staged";
      this.selectedPath = value.staged[0].path;
    } else {
      this.selectedPath = null;
    }
  }



  async loadDiff(loadLarge = false) {
    return loadDiff(this, loadLarge);
  }



  currentFilterRefs() {
    return currentFilterRefs(this);
  }



  async loadCommits(path: string | undefined) {
    return loadCommits(this, path);
  }



  async loadContext() {
    return loadContext(this);
  }



  /** `status` reads the work tree and the open diff only; `full` also reloads branches and history. */
  async refresh(scope: "status" | "full" = "full", silent = false) {
    const path = this.repoPath();
    if (!this.backend() || !path || this.busy || this.backgroundFetch.has(this.jobKey(path))) return;
    if (this.refreshing) {
      if (this.refreshQueued !== "full") this.refreshQueued = scope;
      return;
    }
    this.refreshing = true;
    if (!silent) this.startWork(this.tr("chrome.refresh"), path);
    try {
      const gen = this.statusGen;
      const value = await invoke<StatusSnapshot>("workspace_status", { path });
      if (gen === this.statusGen && this.viewing(path)) {
        this.applySnapshot(value);
        if (scope === "full") await this.loadContext();
        if (this.viewing(path)) await this.loadDiff();
      }
    } catch (error) {
      if (this.viewing(path)) {
        const text = this.message(error);
        this.actionError = text;
        if (this.missingRepoPath(text)) await this.dropMissingRepo(path, text);
      }
    } finally {
      this.refreshing = false;
      if (!silent) this.stopWork(path);
      const next = this.refreshQueued;
      this.refreshQueued = null;
      if (next && this.viewing(path)) await this.refresh(next, silent);
    }
  }



  async withWork<T>(label: string, body: () => Promise<T>): Promise<T> {
    const path = this.repoPath() ?? "";
    this.startWork(label, path);
    try {
      return await body();
    } finally {
      this.stopWork(path);
    }
  }



  async runChange(command: string, extra: Record<string, unknown> = {}, duringRead = false) {
    const path = this.repoPath();
    const reading = duringRead && !this.switching && this.currentJob?.label === this.tr("chrome.readingFile");
    if (this.mode !== "live" || !path) return undefined;
    if (this.busy && !reading) {
      await this.waitForWriteSlot();
      if (this.mode !== "live" || this.repoPath() !== path) return undefined;
    }
    this.actionError = null;
    this.stashRetry = null;
    this.startWork(this.tr("chrome.working"), path);
    try {
      const value = await invoke<StatusSnapshot & { log?: string }>(command, { path, ...extra });
      if (this.viewing(path)) {
        this.applySnapshot(value);
        await this.loadDiff();
      }
      return value;
    } catch (error) {
      if (this.viewing(path)) this.actionError = this.message(error);
      return null;
    } finally {
      this.stopWork(path);
    }
  }



  quietGit = new Set(["stageHunk", "stageLine", "stageLines", "discardHunk", "discardLine", "discardLines", "setExpanded", "setHidden", "setSignOff"]);


  /** Writes that can move refs or history. Everything else only needs the status snapshot. */
  contextActions = new Set([
    "fetch", "pull", "push", "checkout", "checkoutRemote", "createBranch", "deleteBranch", "renameBranch",
    "merge", "rebase", "rebaseInteractive", "abort", "continue", "skip", "reset", "cherryPick", "revert",
    "reword", "tag", "deleteTag", "deleteRemoteTag", "stash", "stashPop", "stashApply", "stashDrop",
    "stashRename", "stashBranch", "stashPaths", "pushRef", "pullRef", "setUpstream", "deleteRemoteBranch",
    "addRemote", "removeRemote", "setRemoteUrl", "submoduleAdd", "submoduleInit", "submoduleSync", "submoduleRemove",
    "gitFlowInit", "gitFlowStart", "gitFlowFinish", "squash", "clone", "init", "setHidden", "setExpanded",
  ]);


  needsContext(action: string) {
    return this.contextActions.has(action);
  }



  shellArg(value: string) {
    return /^[A-Za-z0-9_./:@+=,-]+$/.test(value) ? value : `'${value.replaceAll("'", "'\\''")}'`;
  }



  gitLine(args: string[]) {
    return `git ${args.filter(Boolean).map(this.shellArg.bind(this)).join(" ")}`;
  }



  commandExcerpt(text: string) {
    const line = text.split(/\r?\n/).map((item) => item.trim()).find((item) => item.length > 0) ?? "";
    return line.length > 140 ? `${line.slice(0, 137)}…` : line;
  }



  describeRequest(request: Record<string, unknown>) {
    const action = String(request.action ?? "");
    const text = (key: string) => (typeof request[key] === "string" ? String(request[key]) : "");
    const remote = text("remote");
    const branch = this.snapshot?.branch || "HEAD";
    switch (action) {
      case "fetch":
        return {
          title: this.tr("chrome.commandFetchFrom", { name: remote || this.tr("dialog.allRemotes") }),
          command: this.gitLine(["fetch", request.prune ? "--prune" : "", request.tags ? "--tags" : "", remote || "--all"]),
        };
      case "pull":
        return { title: this.tr("chrome.pull"), command: this.gitLine(["pull", request.autostash ? "--autostash" : "", request.rebase ? "--rebase" : "--no-rebase"]) };
      case "push":
        return {
          title: remote ? this.tr("chrome.commandPushTo", { branch, remote }) : this.tr("chrome.push"),
          command: this.gitLine(["push", request.setUpstream ? "--set-upstream" : "", request.tags ? "--tags" : "", request.force ? "--force" : request.forceWithLease ? "--force-with-lease" : "", remote, request.setUpstream ? "HEAD" : ""]),
        };
      case "pullRef":
        return { title: `${this.tr("chrome.pull")} ${remote}/${text("branch")}`, command: this.gitLine(["pull", request.autostash ? "--autostash" : "", request.rebase ? "--rebase" : "--no-rebase", remote, text("branch")]) };
      case "pushRef": {
        const target = text("remoteBranch") || text("branch");
        return {
          title: this.tr("chrome.commandPushTo", { branch: text("branch"), remote }),
          command: this.gitLine(["push", request.setUpstream ? "--set-upstream" : "", request.tags ? "--tags" : "", request.force ? "--force" : request.forceWithLease ? "--force-with-lease" : "", remote, text("remoteBranch") ? `${text("branch")}:${target}` : `HEAD:${target}`]),
        };
      }
      case "checkout":
      case "checkoutRemote":
        return { title: `${this.tr("menu.checkout")} ${text("name")}`, command: this.gitLine(["checkout", text("name")]) };
      case "merge":
        return { title: `${this.tr("menu.merge")} ${text("name")}`, command: this.gitLine(["merge", request.squash ? "--squash" : "", request.noFf ? "--no-ff" : "", text("summary") ? "-m" : "", text("summary"), text("name")]) };
      case "discard":
        return { title: this.tr("chrome.discardSelected"), command: this.gitLine(["restore", "--worktree", "--source=HEAD", "--", text("file")]) };
      case "discardPaths": {
        const files = Array.isArray(request.files) ? request.files.map((file) => String(file)) : [];
        return { title: this.tr("chrome.discardSelected"), command: this.gitLine(["restore", "--worktree", "--source=HEAD", "--", ...files.slice(0, 8)]) };
      }
      case "resolvePaths": {
        const files = Array.isArray(request.files) ? request.files.map((file) => String(file)) : [];
        const ours = text("side") === "ours";
        return {
          title: `${this.tr(ours ? "dialog.useOurs" : "dialog.useTheirs")} (${files.length})`,
          command: this.gitLine(["checkout", ours ? "--ours" : "--theirs", "--", ...files.slice(0, 8)]),
        };
      }
      case "stash":
        return { title: this.tr("chrome.stash"), command: this.gitLine(["stash", "push", text("message") ? "-m" : "", text("message")]) };
      case "stashPop":
        return { title: this.tr("chrome.pop"), command: this.gitLine(["stash", "pop", text("name")]) };
      case "clone":
        return { title: `${this.tr("chrome.clone")} ${text("url")}`, command: this.gitLine(["clone", text("url"), text("destination")]) };
      case "init":
        return { title: this.tr("chrome.newRepo"), command: this.gitLine(["init", text("destination")]) };
      case "custom":
        return { title: text("command").split("\n")[0] || this.tr("chrome.activity"), command: text("command") };
      case "submoduleUpdate":
        return { title: this.tr("chrome.submodules"), command: this.gitLine(["submodule", "update", "--init", "--recursive"]) };
      case "abort": {
        const command = this.progress === "cherryPick" ? "cherry-pick" : this.progress;
        return this.progress
          ? { title: this.tr("chrome.abort"), command: this.gitLine([command ?? "", "--abort"]) }
          : { title: this.tr("chrome.abort"), command: this.gitLine(["reset", "--merge"]) };
      }
      default:
        return { title: action || this.tr("chrome.activity"), command: this.gitLine([action, text("name"), text("rev")]) };
    }
  }



  recordCommand(action: string, described: { title: string; command: string }, output: string, ok: boolean, source: "user" | "background") {
    if (this.quietGit.has(action)) return;
    const text = output.trim();
    const item: CommandEntry = {
      id: ++this.commandSeq,
      title: described.title,
      command: described.command,
      output: text,
      summary: this.commandExcerpt(text) || (ok ? this.tr("chrome.commandSucceeded") : this.tr("chrome.commandFailed")),
      ok,
      at: Date.now(),
      source,
    };
    this.commandLog = [item, ...this.commandLog].slice(0, 80);
    if (!this.commandLogOpen) this.commandPick = item.id;
    if (!ok) this.noteError(described.title, text || this.tr("chrome.commandFailed"), described.command);
  }


  errorDays() {
    const days = Number(this.settings.errorLogDays);
    if (!Number.isFinite(days)) return 7;
    return Math.min(90, Math.max(1, Math.round(days)));
  }


  noteError(title: string, text: string, command = "") {
    const body = text.trim();
    if (!body) return;
    const latest = this.errorLog[0];
    if (latest && Date.now() - latest.at < 2000 && (latest.text === body || latest.text.startsWith(body) || body.startsWith(latest.text))) {
      if (body.length > latest.text.length || (command && !latest.command)) {
        const revised = { ...latest, title: title || latest.title, text: body.length >= latest.text.length ? body : latest.text, command: command || latest.command, at: Date.now() };
        this.errorLog = this.keptErrors([revised, ...this.errorLog.slice(1)]);
        writeErrorLog(this.errorLog);
      }
      return;
    }
    const entry: StoredError = { id: ++this.errorSeq, at: Date.now(), title, text: body, command };
    this.errorLog = this.keptErrors([entry, ...this.errorLog]);
    writeErrorLog(this.errorLog);
    if (!this.commandLogOpen) this.unseenErrors += 1;
  }


  keptErrors(items: StoredError[]) {
    const cutoff = Date.now() - this.errorDays() * 86_400_000;
    return items.filter((item) => item.at >= cutoff).slice(0, 200);
  }


  pruneErrorLog() {
    const next = this.keptErrors(this.errorLog);
    if (next.length !== this.errorLog.length) {
      this.errorLog = next;
      writeErrorLog(next);
    }
    if (next.length === 0) this.unseenErrors = 0;
  }


  copyNoted(text: string) {
    if (!text) return;
    this.copyText(text);
    this.errorCopied = true;
    window.setTimeout(() => (this.errorCopied = false), 1500);
  }


  visibleCommands(): CommandEntry[] {
    if (this.commandFilter === "notices") {
      return this.statusNotices.map((item) => this.noticeAsCommand(item));
    }
    if (this.commandFilter === "errors") {
      return this.recentErrors.map((item): CommandEntry => ({
        // Saved errors count from 1 on their own, so they take a separate id band.
        // Otherwise a stale command id could select an old error and open scrolled down.
        id: ERROR_ENTRY_ID - item.id,
        title: item.title || this.tr("chrome.errorTitle"),
        command: item.command,
        output: item.text,
        summary: this.commandExcerpt(item.text) || this.tr("chrome.commandFailed"),
        ok: false,
        at: item.at,
        source: "user",
      }));
    }
    return this.commandFilter === "all" ? this.commandLog : this.commandLog.filter((item) => item.source === this.commandFilter);
  }


  noticeAsCommand(item: StatusNotice): CommandEntry {
    return {
      id: STATUS_NOTICE_ENTRY[item.id],
      title: item.title,
      summary: item.detail || item.actions.map((action) => action.label).join(" · ") || this.tr("chrome.commandNotices"),
      command: "",
      output: item.detail,
      ok: item.tone !== "error",
      at: Date.now(),
      source: "user",
      noticeId: item.id,
    };
  }


  pickedCommand() {
    return this.visibleCommands().find((item) => item.id === this.commandPick);
  }


  pickedNotice(): StatusNotice | null {
    const pick = this.pickedCommand();
    if (!pick?.noticeId) return null;
    return this.statusNotices.find((item) => item.id === pick.noticeId) ?? null;
  }


  statusNotice(id: StatusNoticeId) {
    return this.statusNotices.find((item) => item.id === id) ?? null;
  }


  runStatusNoticeAction(id: StatusNoticeId, action: string) {
    if (id === "conflicts" || id === "progress") {
      if (action === "abort") void this.mutate({ action: "abort" });
      else if (action === "skip") void this.mutate({ action: "skip" });
      else if (action === "continue") {
        if (this.progress === "rebase") void this.openContinue();
        else void this.mutate({ action: "continue" });
      }
      return;
    }
    if (id === "bisect") {
      const verb = action === "abort" ? "reset" : action;
      if (verb === "good" || verb === "bad" || verb === "skip" || verb === "reset") {
        void this.mutate({ action: "bisect", verb, rev: "" }).then(() => this.loadFacts(true));
      }
      return;
    }
    if (id === "error") {
      if (action === "close") this.closeError();
      else if (action === "stash") void this.stashAndRetry();
      return;
    }
    if (id === "clone-done" && action === "open") {
      const path = this.cloneDone;
      this.cloneDone = null;
      if (path) void this.openRepo(path);
      return;
    }
    if (id === "update" && action === "open") {
      this.openUpdatePanel(false);
    }
  }



  commandClock(at: number) {
    return new Date(at).toLocaleTimeString(this.activeLocale === "zh" ? "zh-CN" : "en-GB", { hour: "2-digit", minute: "2-digit", hour12: false });
  }



  commandStamp(at: number) {
    return new Date(at).toLocaleString(this.activeLocale === "zh" ? "zh-CN" : "en-GB", { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });
  }



  seedSampleCommands() {
    if (this.inApp() || this.labPage || this.commandLog.length > 0) return;
    const now = Date.now();
    const samples: Array<Omit<CommandEntry, "id">> = [
      { title: this.tr("chrome.commandFetchFrom", { name: "origin" }), summary: "Already up to date", command: "git fetch --prune origin", output: "Already up to date", ok: true, at: now - 4 * 60 * 1000, source: "user" },
      { title: this.tr("chrome.commandPushTo", { branch: "rewrite/tauri", remote: "origin" }), summary: "Updated 'origin/rewrite/tauri'", command: "git push --force-with-lease origin", output: "To github.com:example/awegit.git\n   abc1234..def5678  rewrite/tauri -> rewrite/tauri", ok: true, at: now - 12 * 60 * 1000, source: "user" },
      { title: this.tr("chrome.commandFetchFrom", { name: "origin" }), summary: "Already up to date", command: "git fetch --prune origin", output: "Already up to date", ok: true, at: now - 40 * 60 * 1000, source: "background" },
    ];
    this.commandLog = samples.map((item) => ({ ...item, id: ++this.commandSeq }));
    this.commandPick = this.commandLog[0]?.id ?? 0;
  }



  placeCommandLog() {
    return placeCommandLog(this);
  }



  branchItems(): MenuItem[] {
    return branchItems(this);
  }



  openCommandLog(prefer?: CommandFilter | StatusNoticeId) {
    return openCommandLog(this, prefer);
  }



  showCommandFilter(next: CommandFilter) {
    return showCommandFilter(this, next);
  }



  async mutate(request: Record<string, unknown>, source: "user" | "background" = "user", retryStash = false) {
    const path = this.repoPath();
    if (!this.backend() || !path) return;
    const background = source === "background";
    const action = String(request.action ?? "");
    // Menus and shortcuts obey the same restrictions as the sync toolbar.
    if (["fetch", "pull", "pullRef", "push", "pushRef"].includes(action)) {
      const kind = action === "pullRef" ? "pull" : action === "pushRef" ? "push" : action as "fetch" | "pull" | "push";
      if (this.syncDisabled(kind)) return;
    }
    if (!background && this.progress && this.startsFlow(action)) {
      // A paused rebase, merge, cherry-pick, or revert owns HEAD until it ends.
      this.actionError = this.tr("chrome.finishFlowFirst");
      return;
    }
    if (!background) {
      if (this.userWrites > 0) return;
      this.userWrites += 1;
    }
    let started = false;
    try {
      if (this.busy) {
        if (background) return;
        await this.waitForWriteSlot();
        if (this.repoPath() !== path) return;
      }
      if (!background) this.actionError = null;
      if (!background) this.stashRetry = null;
      if (!background) this.mergeConflictRestore = false;
      const described = this.describeRequest(request);
      const dialogBefore = this.dialog;
      const refreshContext = this.needsContext(action);
      const followSubmodules = this.settings.updateSubmodulesOnCheckout && (action === "checkout" || action === "checkoutRemote") && (this.refs?.submodules.length ?? 0) > 0;
      if (!background) {
        this.startWork(described.title, path);
        started = true;
      }
      try {
        const value = await invoke<StatusSnapshot & import("../operation-flow").MutationResult>("mutate", { path, request, retryStash });
        const output = value.output ?? "";
        // A foreground write started while this was running will paint the later status itself.
        const superseded = background && this.busy;
        if (this.viewing(path) && !superseded) {
          this.applySnapshot(value);
          if (refreshContext) await this.loadContext();
          if (!background && this.viewing(path)) await this.loadDiff();
        }
        if (value.outcome === "failed" || value.outcome === "conflict") throw new Error(value.error || value.log || "Git conflict");
        if (followSubmodules) {
          const follow = await invoke<StatusSnapshot & import("../operation-flow").MutationResult>("mutate", { path, request: { action: "submoduleUpdate" } });
          if (follow.outcome === "failed") throw new Error(follow.error || follow.log);
          if (this.viewing(path)) this.applySnapshot(follow);
          this.recordCommand("submoduleUpdate", this.describeRequest({ action: "submoduleUpdate" }), follow.log ?? "", true, "background");
        }
        this.noteActivity(`${action}${output ? " output" : " ok"}`);
        this.recordCommand(action, described, value.log || output, true, source);
        const restoreConflict = !background && this.viewing(path) && !superseded && this.conflictCount > 0 && this.flowConflictAction(action);
        if (!background && this.viewing(path) && this.dialog === dialogBefore) {
          if (restoreConflict) {
            // The operation finished, but the local changes it kept conflict with the result.
            this.mergeConflictRestore = true;
            this.commandOutput = (value.log ?? "").trim();
            this.dialog = "mergeConflict";
            this.actionError = null;
            this.stashRetry = null;
          } else if (dialogBefore === "prefs") {
            // Preferences stay open for LFS/author actions; the operation is recorded below.
          } else if (output) {
            this.commandOutput = output;
            this.dialog = "output";
          } else {
            this.dialog = null;
          }
        }
        return "completed" as const;
      } catch (error) {
        const text = this.message(error);
        this.noteActivity(`${String(request.action ?? "")} failed`);
        this.recordCommand(String(request.action ?? ""), described, text, false, source);
        if (this.viewing(path) && !(background && this.busy)) {
          try {
            this.applySnapshot(await invoke<StatusSnapshot>("workspace_status", { path }));
            if (refreshContext) await this.loadContext();
          } catch {
            /* keep the action error */
          }
          if (this.shouldShowMergeConflict(action, text)) {
            this.mergeConflictRestore = false;
            this.commandOutput = text.trim();
            this.dialog = "mergeConflict";
            this.actionError = null;
            this.stashRetry = null;
          } else {
            this.actionError = text;
            this.stashRetry = this.stashBlocked(action, text) ? { request, source } : null;
            if (action === "stageHunk" && this.settings.ignoreSpace) this.actionError = `${this.actionError} ${this.tr("dialog.spaceHint")}`;
          }
        }
        return this.conflictCount > 0 ? "conflict" as const : "failed" as const;
      } finally {
        if (started) this.stopWork(path);
      }
    } finally {
      if (!background) this.userWrites -= 1;
    }
  }



  closeError() {
    this.actionError = null;
    this.stashRetry = null;
    this.errorCopied = false;
  }



  copyError() {
    if (!this.actionError) return;
    this.copyText(this.actionError);
    this.errorCopied = true;
    window.setTimeout(() => (this.errorCopied = false), 1500);
  }


  /** True when git refused a write because the work tree has changes it would overwrite. */
  stashBlocked(action: string, text: string) {
    const retryable = ["checkout", "checkoutRemote", "merge", "pull", "pullRef", "rebase", "rebaseInteractive", "cherryPick", "revert", "stashApply", "stashPop", "applyPatch", "continue", "gitFlowFinish"];
    if (!retryable.includes(action)) return false;
    const lower = text.toLowerCase();
    return lower.includes("would be overwritten") || lower.includes("please commit your changes") || lower.includes("please move or remove");
  }


  /** Stash what blocked the last write, then run that write again. */
  async stashAndRetry() {
    const pending = this.stashRetry;
    if (!pending) return;
    this.stashRetry = null;
    const request = { ...pending.request };
    // A new stash renumbers stash@{n}, so pin the entry this write targeted by commit id.
    const name = typeof request.name === "string" ? request.name : "";
    if (name.startsWith("stash@{") && (request.action === "stashApply" || request.action === "stashPop")) {
      const stash = (this.refs?.stashes ?? []).find((item) => item.name === name);
      if (stash?.id) request.name = stash.id;
    }
    await this.mutate(request, pending.source, true);
  }



  doFetch(source: "user" | "background" = "user") {
    const remote = this.settings.fetchAllRemotes ? null : this.refs?.remotes[0]?.name ?? null;
    const request = { action: "fetch", remote, prune: this.settings.fetchPrune, tags: this.settings.fetchTags };
    if (source !== "background") return this.mutate(request, source);
    const key = this.jobKey(this.repoPath());
    if (!key || this.backgroundFetch.has(key)) return;
    this.backgroundFetch.add(key);
    return this.mutate(request, source).finally(() => this.backgroundFetch.delete(key));
  }



  doPull() {
    return this.mutate({ action: "pull", rebase: this.settings.pullRebase, autostash: true });
  }


  /** Pull/merge-style writes that leave unmerged paths get Fork-style conflict dialog. */
  shouldShowMergeConflict(action: string, text: string) {
    const ops = new Set(["pull", "pullRef", "merge", "rebase", "rebaseInteractive", "cherryPick", "revert", "stashPop", "stashApply", "continue", "skip", "gitFlowFinish"]);
    if (!ops.has(action)) return false;
    if (this.conflictCount > 0) return true;
    const lower = text.toLowerCase();
    return /conflict|fix conflicts|merge failed|could not apply|needs merge|unmerged (paths|files)|merging is not possible|自动合并失败/.test(lower);
  }


  /** Label for the composer button while an operation is paused. */
  flowLabel() {
    if (this.progress === "rebase") return this.tr("chrome.continueRebase");
    if (this.progress === "merge") return this.tr("chrome.finishMerge");
    if (this.progress === "cherryPick") return this.tr("chrome.continueCherryPick");
    if (this.progress === "revert") return this.tr("chrome.continueRevert");
    return this.tr("chrome.commit");
  }


  /** Continue the paused operation instead of creating a separate commit. */
  continueFlow() {
    if (!this.progress) return;
    if (this.progress === "rebase") return this.openContinue();
    return this.mutate({ action: "continue" });
  }


  /** Show which commit the paused rebase is replaying before asking for a message. */
  async openContinue() {
    this.draft = "";
    this.editContinue = false;
    this.continueDetail = null;
    this.dialog = "continue";
    if (this.mode !== "live") return;
    const path = this.repoPath();
    if (!path) return;
    try {
      const detail = await invoke<CommitDetail>("commit_detail", { path, id: "REBASE_HEAD" });
      if (this.dialog === "continue" && this.viewing(path)) {
        this.continueDetail = { author: detail.author, summary: detail.summary };
      }
    } catch {
      /* a rebase without REBASE_HEAD has no commit to show */
    }
  }


  /** Writes that start a new flow; a paused flow owns HEAD until it ends. */
  startsFlow(action: string) {
    return ["pull", "pullRef", "merge", "rebase", "rebaseInteractive", "cherryPick", "revert", "checkout", "checkoutRemote", "reset", "reword", "squash", "stashBranch", "gitFlowStart", "gitFlowFinish"].includes(action);
  }


  /** Writes whose autostash can conflict after the operation itself finished. */
  flowConflictAction(action: string) {
    return ["pull", "pullRef", "merge", "rebase", "rebaseInteractive", "checkout", "checkoutRemote", "cherryPick", "revert", "continue", "skip", "abort", "squash", "gitFlowFinish"].includes(action);
  }


  doPush() {
    const current = this.refs?.branches.find((branch) => branch.current);
    const remote = this.refs?.remotes[0]?.name ?? null;
    return this.mutate({
      action: "push",
      remote,
      setUpstream: !current?.upstream,
      tags: this.settings.pushAllTags,
      forceWithLease: this.settings.forceWithLease,
    });
  }



  async applyStash(name: string) {
    const pinned = this.refs?.stashes.find((stash) => stash.name === name || stash.id === name)?.id ?? name;
    // Git pop drops only after a successful apply; a conflicted entry stays intact.
    return this.mutate({ action: this.settings.applyStashDelete ? "stashPop" : "stashApply", name: pinned });
  }


  confirmApplyStash(name: string, summary: string) {
    const note = this.settings.applyStashDelete
      ? this.tr("dialog.applyStashDeleteNote")
      : this.tr("dialog.applyStashKeepNote");
    this.ask(this.tr("dialog.applyStash"), this.tr("dialog.applyStashBody"), () => void this.applyStash(name), [], { subject: summary, detail: note });
  }


  confirmPopStash(name?: string) {
    const stash = name ? this.refs?.stashes.find((item) => item.name === name) : this.refs?.stashes[0];
    if (!stash && !name) {
      void this.mutate({ action: "stashPop" });
      return;
    }
    const summary = stash?.summary || name || "";
    this.ask(this.tr("dialog.popStash"), this.tr("dialog.popStashBody"), () => void this.mutate({ action: "stashPop", name: name ?? stash?.name }), [], { subject: summary });
  }


  confirmReset(rev: string, mode: "soft" | "mixed", summary = "") {
    const short = rev.slice(0, 7);
    const target = summary ? `${short} "${summary}"` : short;
    const title = this.tr(mode === "soft" ? "dialog.softReset" : "dialog.mixedReset");
    const body = this.tr(mode === "soft" ? "dialog.softResetBody" : "dialog.mixedResetBody");
    this.ask(title, body, () => void this.mutate({ action: "reset", rev, mode }), [], { subject: target });
  }



  confirmRevert(id: string, summary: string) {
    const subject = `${id.slice(0, 7)} ${summary}`.trim();
    this.ask(this.tr("dialog.revert"), this.tr("dialog.revertBody"), () => void this.mutate({ action: "revert", rev: id }), [], { subject });
  }



  adoptSettings(value: Settings, resetHistory = false) {
    const recovered = value.recoveryNotice === "recovered-backup";
    this.settings = this.withSourceRoots({
      ...this.defaultSettings,
      ...value,
      recent: value.recent ?? [],
      workspaces: (value.workspaces ?? []).map((item) => ({
        ...item,
        tabWidth: item.tabWidth || workspaceTabWidth(item.repositories?.length || item.openTabs?.length || 0),
      })),
      commands: (value.commands ?? []).map((item) => ({ ...item, prompt: item.prompt ?? "", shared: item.shared !== false, repo: item.repo ?? "" })),
      expandedGroups: value.expandedGroups ?? [],
      accounts: value.accounts ?? [],
      tabLabels: value.tabLabels ?? [],
      tabColors: value.tabColors ?? [],
      pinnedRefs: value.pinnedRefs ?? [],
      sourceDirectories: value.sourceDirectories ?? [],
      groupNames: value.groupNames ?? [],
    });
    applyCommitTypes(this);
    this.syncRecentWorkspace();
    if (recovered) this.settings = { ...this.settings, recoveryNotice: "" };
    if (!this.settings.dateFormat.trim()) this.settings = { ...this.settings, dateFormat: "dd MMM yyyy" };
    if (!this.settings.proxyType) this.settings = { ...this.settings, proxyType: "http" };
    if (this.settings.aiLanguage !== "zh" && this.settings.aiLanguage !== "en") {
      const legacy = this.settings.aiLanguage.trim();
      this.settings = { ...this.settings, aiLanguage: /^(en|english)$/i.test(legacy) ? "en" : "zh" };
    }
    if (this.settings.aiPromptStyle !== "long") this.settings = { ...this.settings, aiPromptStyle: "short" };
    if (!isCommitType(this.settings.commitType)) this.settings = { ...this.settings, commitType: this.preferredCommitType() };
    if (!this.settings.errorLogDays) this.settings = { ...this.settings, errorLogDays: 7 };
    this.pruneErrorLog();
    if (!this.snapshot && isCommitType(this.settings.commitType)) this.commitType = this.settings.commitType;
    const limit = Math.min(100000, Math.max(1, this.settings.maxCommits || 500));
    if (resetHistory || this.logLimit === 500) this.logLimit = limit;
    this.signOff = this.settings.signOff;
    this.showPendingUpdate(false);
    if (recovered && !this.actionError) this.actionError = this.tr("chrome.settingsRecovered");
  }

  showPendingUpdate(force: boolean) {
    const value = this.pendingUpdate;
    if (!value) {
      if (force) this.updateNotice = null;
      return;
    }
    if (!force && value.appVersion && value.appVersion === this.settings.dismissedUpdate) {
      this.updateNotice = null;
      if (this.updatePhase === "available") this.updatePhase = "idle";
      return;
    }
    this.updateNotice = value;
    if (this.updatePhase === "idle") this.updatePhase = "available";
  }

  updateStatusLine = $derived.by(() => {
    if (this.updatePhase === "checking") return this.tr("chrome.updateChecking");
    if (this.updatePhase === "current") return this.tr("chrome.updateCurrent", { version: this.appVersion });
    if (this.updatePhase === "available" && this.updateNotice) return this.tr("chrome.updateAvailable", { name: `AweGit ${this.updateNotice.appVersion}` });
    if (this.updatePhase === "downloading") return this.updatePercent == null ? this.tr("chrome.updateDownloading") : this.tr("chrome.updateProgress", { percent: String(this.updatePercent) });
    if (this.updatePhase === "verifying") return this.tr("chrome.updateVerifying");
    if (this.updatePhase === "ready") return this.tr("chrome.updateStarted");
    if (this.updatePhase === "cancelled") return this.tr("chrome.updateCancelled");
    if (this.updatePhase === "error") return this.updateMessage;
    return "";
  });

  openUpdatePanel(check = true) {
    this.dialog = "update";
    if (!check || this.updateBusy || this.updatePhase === "checking") return;
    if (this.updateNotice) return;
    void this.checkForUpdate();
  }

  async checkForUpdate() {
    if (this.updatePhase === "checking" || this.updateBusy) return;
    this.updatePhase = "checking";
    this.updateMessage = this.tr("chrome.updateChecking");
    this.updatePercent = null;
    this.updateReceived = 0;
    this.updateTotal = 0;
    this.updateSpeed = null;
    try {
      const value = await invoke<UpdateNotice | null>("check_for_update");
      this.updateCheckedAt = Date.now();
      this.pendingUpdate = value;
      if (!value) {
        this.updateNotice = null;
        this.updatePhase = "current";
        this.updateMessage = this.tr("chrome.updateCurrent", { version: this.appVersion });
        return;
      }
      this.updateNotice = value;
      this.updatePhase = "available";
      this.updateMessage = "";
    } catch (error) {
      this.updatePhase = "error";
      this.updateMessage = this.message(error);
    }
  }

  /**
   * A brand-new install asks once. Mark it seen before later saves, and show the
   * prompt only from the main window after startup finishes.
   */
  takeFeaturesIntro() {
    if (!this.inApp() || this.labPage || this.standalone) return false;
    if (this.settings.featuresIntro !== "pending") return false;
    this.settings = { ...this.settings, featuresIntro: "seen" };
    return true;
  }

  openFeaturesPage() {
    if (this.inApp()) void openUrl(featuresPageUrl);
    else window.open(featuresPageUrl, "_blank", "noopener");
  }

  dismissUpdate() {
    const version = this.updateNotice?.appVersion ?? "";
    this.updateNotice = null;
    this.updatePhase = "idle";
    this.updateMessage = "";
    this.updatePercent = null;
    this.updateReceived = 0;
    this.updateTotal = 0;
    this.updateSpeed = null;
    if (this.dialog === "update") this.dialog = null;
    if (!version) return;
    this.settings = { ...this.settings, dismissedUpdate: version };
    void this.savePrefs(false);
  }

  async installUpdate() {
    const notice = this.updateNotice;
    if (!notice || this.updateBusy) return;
    if (!notice.installable || !notice.downloadUrl) {
      const url = notice.downloadUrl || notice.releaseUrl;
      if (url) void openUrl(url);
      return;
    }
    this.updateBusy = true;
    this.updatePhase = "downloading";
    this.updatePercent = null;
    this.updateReceived = 0;
    this.updateTotal = 0;
    this.updateSpeed = null;
    this.updateSpeedAt = 0;
    this.updateSpeedBytes = 0;
    this.updateCancelling = false;
    this.updateCancelRequested = false;
    this.updateMessage = this.tr("chrome.updateDownloading");
    try {
      await invoke("download_update", { url: notice.downloadUrl, sha256: notice.sha256 ?? "", silent: this.settings.silentInstall !== false });
      this.updatePhase = "ready";
      this.updatePercent = 100;
      this.updateMessage = this.tr("chrome.updateStarted");
    } catch (error) {
      if (this.updateCancelRequested) {
        this.updatePhase = "cancelled";
        this.updatePercent = null;
        this.updateMessage = this.tr("chrome.updateCancelled");
      } else {
        this.updatePhase = "error";
        this.updateMessage = this.message(error);
      }
    } finally {
      this.updateBusy = false;
      this.updateCancelling = false;
      this.updateCancelRequested = false;
    }
  }

  cancelUpdate() {
    if (this.updatePhase !== "downloading" || this.updateCancelling) return;
    this.updateCancelling = true;
    invoke("cancel_update").catch(() => {});
  }

  noteUpdateProgress(progress: UpdateProgress) {
    if (progress.phase === "verifying" && this.updatePhase === "downloading") {
      this.updatePhase = "verifying";
      this.updatePercent = 100;
    }
    if (progress.phase === "downloading") this.updatePercent = progress.percent;
    if (progress.total > 0) this.updateTotal = progress.total;
    this.updateReceived = progress.received;
    const now = performance.now();
    if (!this.updateSpeedAt) {
      this.updateSpeedAt = now;
      this.updateSpeedBytes = progress.received;
      return;
    }
    const elapsed = now - this.updateSpeedAt;
    if (elapsed >= 500) {
      this.updateSpeed = Math.max(0, ((progress.received - this.updateSpeedBytes) * 1000) / elapsed);
      this.updateSpeedAt = now;
      this.updateSpeedBytes = progress.received;
    }
  }



  async importFork(path?: string | null) {
    if (!this.inApp()) return;
    try {
      const result = await invoke<ForkImport>("import_fork_settings", { path: path || null });
      this.adoptSettings(result.settings, true);
      this.forkImportNote = this.tr("dialog.forkImported", {
        preferences: String(result.preferences),
        repositories: String(result.repositories),
        workspaces: String(result.workspaces),
      });
      if (this.settings.currentWorkspace) await this.selectWorkspace();
      if (this.mode === "live") void this.loadContext();
    } catch (error) {
      const text = this.message(error);
      this.forkImportNote = text.includes("fork-settings-missing") ? this.tr("dialog.forkMissing") : text;
    }
  }



  async importForkFile() {
    if (!this.inApp()) return;
    try {
      const file = await invoke<string | null>("pick_file", { initial: this.settingsPath || null });
      if (file) await this.importFork(file);
    } catch (error) {
      this.forkImportNote = this.message(error);
    }
  }



  /** `full` adds the ignored files, statistics, and LFS list, which take seconds in a large work tree. */
  async loadFacts(force = false, full = false) {
    if (!this.backend() || this.mode !== "live") return null;
    const path = this.repoPath() ?? "";
    if (!force && this.facts && this.factsPath === path) return this.facts;
    try {
      const value = await invoke<RepoFacts>("repo_facts", { path, full });
      if (!this.viewing(path)) return null;
      const kept = !full && this.facts && this.factsPath === path ? { ignored: this.facts.ignored, weekday: this.facts.weekday, hour: this.facts.hour, lfs: this.facts.lfs, hot: this.facts.hot } : {};
      this.facts = { ...value, ...kept };
      this.factsPath = path;
      return this.facts;
    } catch (error) {
      if (this.viewing(path)) this.actionError = this.message(error);
      return null;
    }
  }



  preferredCommitType(): CommitType {
    if (isCommitType(this.settings.commitType)) return this.settings.commitType;
    return commitTypesOf(this.settings.commitTypes)[0]?.name ?? "feat";
  }


  /** Types offered by the composer, in display order. Empty settings mean the built-in list. */
  commitTypeList() {
    return commitTypesOf(this.settings.commitTypes);
  }


  commitTypeColor(type: string) {
    return this.commitTypeList().find((item) => item.name === type)?.color ?? commitTypesOf(undefined)[0].color;
  }


  /** Custom description, or the translated built-in one. Custom types without a label have none. */
  commitTypeLabel(type: string) {
    const label = this.commitTypeList().find((item) => item.name === type)?.label;
    if (label) return label;
    return isBuiltinCommitType(type) ? this.tr(typeMessageKey(type)) : "";
  }


  rememberCommitType(type: CommitType) {
    this.commitType = type;
    if (this.settings.commitType === type) return;
    this.settings = { ...this.settings, commitType: type };
    void this.savePrefs(false);
  }


  addCommitType(name: string, color: string, label = "") {
    return addCommitType(this, name, color, label);
  }


  renameCommitType(from: string, to: string) {
    return renameCommitType(this, from, to);
  }


  setCommitTypeField(name: string, field: "color" | "label", value: string) {
    return setCommitTypeField(this, name, field, value);
  }


  removeCommitType(name: string) {
    return removeCommitType(this, name);
  }


  resetCommitTypes() {
    return resetCommitTypes(this);
  }


  applySubject(text: string) {
    const split = splitSubject(text);
    if (split.type) this.rememberCommitType(split.type);
    this.summary = split.text;
    this.fitSummary();
  }


  normalizeSummary() {
    const split = splitSubject(this.summary);
    if (!split.peeled) return;
    if (split.type) this.rememberCommitType(split.type);
    this.summary = split.text;
  }


  fitSummary() {
    if (this.summary.length > this.summaryRoom) this.summary = this.summary.slice(0, this.summaryRoom);
  }


  async fillFromMessage(text: string) {
    return fillFromMessage(this, text);
  }



  async fillTemplate() {
    return fillTemplate(this);
  }



  splitConflicts(working: string): MergePiece[] {
    return splitConflicts(this, working);
  }



  joinedPieces(pieces: MergePiece[]) {
    return joinedPieces(this, pieces);
  }



  pickHunk(index: number, pick: MergePick) {
    return pickHunk(this, index, pick);
  }

  toggleMergeSide(index: number, side: "ours" | "theirs") {
    return toggleMergeSide(this, index, side);
  }

  hunkIncludes(pick: MergePick, side: "ours" | "theirs") {
    return hunkIncludes(pick, side);
  }

  chosenHunkText(piece: Extract<MergePiece, { kind: "hunk" }>) {
    return chosenHunkText(piece);
  }



  bisectMark(id: string) {
    return bisectMark(this, id);
  }



  pullRequestUrl(branch: string) {
    return pullRequestUrl(this, branch);
  }



  commitWeb(id: string) {
    return commitWeb(this, id);
  }



  requestCheckout(name: string) {
    return requestCheckout(this, name);
  }



  /** Double-click checkout: stash local work first, switch, then tell the user. */
  async quickCheckout(name: string, remote?: { remote: string; branch: string }) {
    return quickCheckout(this, name, remote);
  }



  reopenClosed() {
    return reopenClosed(this);
  }



  async previewCherry(rev: string, record: boolean, summary = "") {
    return previewCherry(this, rev, record, summary);
  }



  async pastePatch() {
    return pastePatch(this);
  }



  toggleFold(id: string) {
    return toggleFold(this, id);
  }


  mergeOwner(id: string) {
    return (this.mode === "sample" ? this.sampleFold : this.liveFold).owner.get(id) ?? "";
  }


  mergeOpen(id: string) {
    const owner = this.mergeOwner(id);
    return owner !== "" && this.expandedMerges.includes(owner);
  }



  tabText(path: string) {
    return tabText(this, path);
  }



  tabColor(path: string) {
    return tabColor(this, path);
  }



  pairSetting(list: string[], path: string, value: string) {
    return pairSetting(this, list, path, value);
  }



  setLayout(next: "list" | "tree" | "combined") {
    return setLayout(this, next);
  }


  setConflictsOnly(on: boolean) {
    return setConflictsOnly(this, on);
  }


  showSideRefs() {
    return showSideRefs(this);
  }


  showSideSearch() {
    return showSideSearch(this);
  }


  queueCommitSearch() {
    return queueCommitSearch(this);
  }


  pickCommitSearch(field: "message" | "author" | "path" | "diff") {
    return pickCommitSearch(this, field);
  }


  pickCommitScope(scope: "repository" | "branch") {
    return pickCommitScope(this, scope);
  }



  async toggleFakeIgnore() {
    return toggleFakeIgnore(this);
  }



  fileViewMenu(): MenuItem[] {
    return fileViewMenu(this);
  }



  openFileView(event: MouseEvent) {
    return openFileView(this, event);
  }



  async searchContent(kind: string, needle: string) {
    return searchContent(this, kind, needle);
  }



  togglePin(name: string) {
    const key = `${this.snapshot?.path ?? ""}\t${name}`;
    const pinnedRefs = this.settings.pinnedRefs.includes(key) ? this.settings.pinnedRefs.filter((item) => item !== key) : [...this.settings.pinnedRefs, key];
    this.settings = { ...this.settings, pinnedRefs };
    void this.savePrefs(false);
  }



  moveHunk(delta: number) {
    return moveHunk(this, delta);
  }



  copyPatch(indexes?: number[]) {
    return copyPatch(this, indexes);
  }



  groupLabel(folder: string) {
    return groupLabel(this, folder);
  }



  confirmSync(kind: "fetch" | "pull" | "push") {
    if (this.syncDisabled(kind)) return;
    return confirmSync(this, kind);
  }



  /** The remote branch a pull should default to after the remote changes. */
  pullBranchFor(remoteName: string) {
    const remote = this.refs?.remotes.find((item) => item.name === remoteName);
    const branches = remote?.branches ?? [];
    const upstream = this.refs?.branches.find((branch) => branch.current)?.upstream ?? "";
    const fromUpstream = upstream.startsWith(`${remoteName}/`) ? upstream.slice(remoteName.length + 1) : "";
    const same = this.snapshot?.branch ?? "";
    return [fromUpstream, same].find((name) => name && branches.includes(name)) ?? branches[0] ?? same;
  }



  /** The name an added remote gets: derived from its URL and kept unique. */
  remoteName() {
    const base = remoteNameFromUrl(this.draftExtra);
    const taken = new Set((this.refs?.remotes ?? []).map((remote) => remote.name));
    let name = base;
    for (let index = 2; taken.has(name); index += 1) name = `${base}-${index}`;
    return name;
  }



  splitRemoteRef(ref: string) {
    const remote = this.refs?.remotes.find((item) => ref.startsWith(`${item.name}/`));
    return remote ? { remote: remote.name, branch: ref.slice(remote.name.length + 1) } : null;
  }



  /** Where a push goes: the picked remote branch, the upstream, or a new branch on the first remote. */
  pushTarget(local: string, to: string) {
    if (to) {
      const picked = this.splitRemoteRef(to);
      if (!picked) return null;
      // A remote-only target means "the local branch with the same name".
      return { ...picked, setUpstream: !picked.branch };
    }
    const upstream = this.refs?.branches.find((branch) => branch.name === local)?.upstream ?? "";
    const known = upstream ? this.splitRemoteRef(upstream) : null;
    if (known) return { ...known, setUpstream: false };
    const first = this.refs?.remotes[0]?.name;
    return first ? { remote: first, branch: local, setUpstream: true } : null;
  }



  /** The remote tab the target belongs to. */
  pushRemoteName() {
    return this.splitRemoteRef(this.pushTo)?.remote ?? this.refs?.remotes[0]?.name ?? "";
  }



  /** Target on one remote: its upstream branch, else the same local name, else a new branch. */
  pushTargetOn(remote: string) {
    const known = this.splitRemoteRef(this.refs?.branches.find((branch) => branch.name === this.pushBranch)?.upstream ?? "");
    if (known?.remote === remote) return `${remote}/${known.branch}`;
    const branches = this.refs?.remotes.find((item) => item.name === remote)?.branches ?? [];
    return branches.includes(this.pushBranch) ? `${remote}/${this.pushBranch}` : `${remote}/`;
  }



  /** Branches offered under the selected remote tab; a same-name new branch when the remote has none. */
  pushBranchOptions() {
    const remotes = this.refs?.remotes ?? [];
    const remote = remotes.find((item) => item.name === this.pushRemoteName());
    if (!remote) return [];
    const options = remote.branches.map((branch) => ({ value: `${remote.name}/${branch}`, label: branch }));
    if (!remote.branches.includes(this.pushBranch)) options.unshift({ value: `${remote.name}/`, label: this.tr("dialog.pushNewBranch", { name: this.pushBranch }) });
    return options;
  }



  selectPushRemote(remote: string) {
    this.pushTo = this.pushTargetOn(remote);
  }



  /** Reset the target to the upstream branch, or the same name on the first remote. */
  resetPushTarget() {
    const known = this.splitRemoteRef(this.refs?.branches.find((branch) => branch.name === this.pushBranch)?.upstream ?? "");
    const remote = known?.remote ?? this.refs?.remotes[0]?.name ?? "";
    this.pushTo = remote ? this.pushTargetOn(remote) : "";
  }



  matchesQuery(value: string, query: string) {
    const needle = query.trim().toLowerCase();
    return !needle || value.toLowerCase().includes(needle);
  }



  searchQuery(key: SearchKey) {
    return { recent: this.recentQuery, side: this.sideQuery, commit: this.commitQuery, file: this.fileQuery, detail: this.detailQuery }[key];
  }



  closeSearch(key: SearchKey) {
    this.searchOpen[key] = false;
    if (key === "recent") this.recentQuery = "";
    else if (key === "side") this.sideQuery = "";
    else if (key === "commit") this.commitQuery = "";
    else if (key === "detail") this.detailQuery = "";
    else this.fileQuery = "";
  }



  toggleSearch(key: SearchKey) {
    if (this.searchOpen[key]) this.closeSearch(key);
    else this.searchOpen[key] = true;
  }



  searchKeys(key: SearchKey, event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    this.closeSearch(key);
  }



  searchBlur(key: SearchKey) {
    if (!this.searchQuery(key).trim()) this.searchOpen[key] = false;
  }



  focusOnMount(node: HTMLInputElement) {
    node.focus();
  }



  focusWhen(node: HTMLInputElement, on: boolean) {
    if (on) node.focus();
  }



  fitMenu(node: HTMLElement, spot: { x: number; y: number; anchorTop?: number }) {
    const place = (next: { x: number; y: number; anchorTop?: number }) => {
      const margin = 8;
      const { width, height } = node.getBoundingClientRect();
      let left = next.x;
      let top = next.y;
      if (left + width > window.innerWidth - margin) left = window.innerWidth - width - margin;
      if (top + height > window.innerHeight - margin) {
        top = next.anchorTop !== undefined && next.anchorTop - height - 2 >= margin ? next.anchorTop - height - 2 : window.innerHeight - height - margin;
      }
      node.style.left = `${Math.max(margin, left)}px`;
      node.style.top = `${Math.max(margin, top)}px`;
    };
    place(spot);
    return { update: place };
  }



  openMenu(event: MouseEvent, items: MenuItem[]) {
    event.preventDefault();
    event.stopPropagation();
    this.submenu = null;
    this.menuOpener = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
    this.menu = { x: Math.min(event.clientX, window.innerWidth - 240), y: Math.min(event.clientY, window.innerHeight - 40), items };
  }



  dismissMenu(restore = true) {
    const opener = this.menuOpener;
    this.menu = null;
    this.submenu = null;
    this.menuOpener = null;
    if (!restore || !opener) return;
    requestAnimationFrame(() => {
      if (!opener.isConnected) return;
      const active = document.activeElement;
      if (!active || active === document.body) opener.focus();
    });
  }



  ask(title: string, body: string, run: () => void, files: string[] = [], extras: Pick<ConfirmAsk, "subject" | "detail" | "danger" | "confirmLabel"> = {}) {
    return ask(this, title, body, run, files, extras);
  }



  liveOnly(run: () => void) {
    return liveOnly(this, run);
  }



  refKind(label: string) {
    return refKind(this, label);
  }



  branchRefLabels(labels: string[]) {
    return branchRefLabels(this, labels);
  }



  plainRefLabels(labels: string[]) {
    return plainRefLabels(this, labels);
  }



  stashParts(summary: string, refs: string[] = []) {
    return stashParts(summary, refs);
  }



  /** Color of the commit dot, so a branch chip matches the line it sits on. */
  laneTone(row: GraphCell[] | undefined) {
    return laneTone(this, row);
  }


  activeLaneColor() {
    return activeLaneColor(this);
  }


  exactRefs() {
    return exactRefs(this);
  }


  soloBranch(name: string) {
    return soloBranch(this, name);
  }


  toggleHistoryScope() {
    return toggleHistoryScope(this);
  }


  clearHistoryFilter() {
    return clearHistoryFilter(this);
  }



  isCommitUnpushed(commit: { unpushed?: boolean; refs?: string[]; badges?: string[]; id?: string }) {
    return isCommitUnpushed(this, commit);
  }



  async showReflog() {
    return showReflog(this);
  }



  async openTreeFile(file: string) {
    return openTreeFile(this, file);
  }



  async openForge(kind: "notes" | "pulls") {
    return openForge(this, kind);
  }



  fitImage(event: Event) {
    return fitImage(this, event);
  }



  async paintPixel(left: string, right: string) {
    return paintPixel(this, left, right);
  }



  fileMenu(file: Row, side: Side): MenuItem[] {
    return fileMenu(this, file, side);
  }



  commitMenu(commit: { id: string; summary: string }): MenuItem[] {
    return commitMenu(this, commit);
  }



  reflogMenu(row: { id: string; selector: string }): MenuItem[] {
    return reflogMenu(this, row);
  }



  setRebaseVerb(rev: string, verb: string) {
    return setRebaseVerb(this, rev, verb);
  }



  async rememberRepo(path: string) {
    const recent = [path, ...this.settings.recent.filter((item) => item !== path)].slice(0, 12);
    const currentId = this.settings.currentWorkspace;
    const workspaces = this.settings.workspaces.map((workspace) => {
      if (!currentId || workspace.id !== currentId) return workspace;
      const openTabs = workspace.openTabs.includes(path) ? workspace.openTabs : [...workspace.openTabs, path];
      const repositories = workspace.repositories.includes(path) ? workspace.repositories : [...workspace.repositories, path];
      return {
        ...workspace,
        repositories,
        openTabs,
        selectedTab: Math.max(0, openTabs.indexOf(path)),
        tabWidth: workspaceTabWidth(repositories.length),
      };
    });
    this.settings = { ...this.settings, recent, workspaces };
    this.syncRecentWorkspace();
    await invoke("save_settings", { values: this.settings }).catch(() => {});
  }

  /** Path inside `could not open repository at {path}: missing`. Other open errors stay put. */
  missingRepoPath(text: string) {
    const marker = "could not open repository at ";
    const suffix = ": missing";
    if (!text.startsWith(marker) || !text.endsWith(suffix)) return null;
    const path = text.slice(marker.length, -suffix.length).trim();
    return path || null;
  }

  /** Drop a repository that is gone from the saved lists. Custom commands stay. */
  forgetRepo(path: string) {
    const match = (item: string) => this.pathsMatch(item, path);
    const dropKeyed = (items: string[]) => items.filter((item) => {
      const tab = item.indexOf("\t");
      const head = tab >= 0 ? item.slice(0, tab) : item;
      return !match(head);
    });
    const recent = this.settings.recent.filter((item) => !match(item));
    const workspaces = this.settings.workspaces.map((workspace) => {
      const current = workspace.openTabs[workspace.selectedTab];
      const repositories = workspace.repositories.filter((item) => !match(item));
      const openTabs = workspace.openTabs.filter((item) => !match(item));
      const kept = current && !match(current) ? openTabs.findIndex((item) => this.pathsMatch(item, current)) : -1;
      return {
        ...workspace,
        repositories,
        openTabs,
        selectedTab: kept >= 0 ? kept : 0,
        tabWidth: workspaceTabWidth(repositories.length),
      };
    });
    this.settings = {
      ...this.settings,
      recent,
      workspaces,
      tabLabels: dropKeyed(this.settings.tabLabels),
      tabColors: dropKeyed(this.settings.tabColors),
      pinnedRefs: dropKeyed(this.settings.pinnedRefs),
    };
    this.repos = this.repos.filter((item) => !match(item));
    this.closedTabs = this.closedTabs.filter((item) => !match(item));
    for (const key of [...this.drafts.keys()]) {
      if (match(key)) this.drafts.delete(key);
    }
    if (Object.keys(this.recentStats).some((key) => match(key))) {
      const stats = { ...this.recentStats };
      for (const key of Object.keys(stats)) {
        if (!match(key)) continue;
        delete stats[key];
        this.recentAsked.delete(key);
      }
      this.recentStats = stats;
    }
    this.syncRecentWorkspace();
    void this.savePrefs(false);
  }

  /** Show the missing-repository error, then drop that path. Open another tab when this one was on screen. */
  async dropMissingRepo(path: string, text: string) {
    this.forgetRepo(path);
    const named = this.missingRepoPath(text);
    if (named && !this.pathsMatch(named, path)) this.forgetRepo(named);
    const viewingFailed = !this.snapshot || this.pathsMatch(this.snapshot.path, path);
    if (!viewingFailed) return;
    this.snapshot = null;
    this.refs = null;
    this.mode = "error";
    this.loadError = text;
    const next = this.repos.find((item) => !this.pathsMatch(item, path));
    if (next) await this.openRepo(next);
  }



  quick(event: MouseEvent, dialogName: Dialog, run: () => void) {
    return quick(this, event, dialogName, run);
  }



  async submitCommit(pushAfter = false) {
    return submitCommit(this, pushAfter);
  }



  async suggestMessage() {
    return suggestMessage(this);
  }

  async testAiEndpoint() {
    return testAiEndpoint(this);
  }

  async loadAiModels() {
    return loadAiModels(this);
  }

  togglePromptStyle(event: MouseEvent) {
    return togglePromptStyle(this, event);
  }



  async compareDrops() {
    return compareDrops(this);
  }



  async selectCommit(id: string, event?: MouseEvent) {
    window.clearTimeout(this.commitNavTimer);
    this.commitNavTimer = 0;
    return selectCommit(this, id, event);
  }

  /** Move the highlight now, load the expensive detail once the key repeat settles. */
  selectCommitSoon(id: string) {
    const path = this.repoPath();
    this.comparePair = null;
    this.selectedCommit = id;
    const rowAt = this.historyItems.findIndex((row) => row.id === id);
    if (rowAt >= 0) this.selectedIndex = rowAt;
    this.blameLines = null;
    this.historyTab = "commit";
    window.clearTimeout(this.commitNavTimer);
    this.commitNavTimer = window.setTimeout(() => {
      this.commitNavTimer = 0;
      if (this.viewing(path)) void this.selectCommit(id);
    }, 90);
  }



  async pickCommitFile(file: string) {
    return pickCommitFile(this, file);
  }



  async ensurePatch(file: string) {
    return ensurePatch(this, file);
  }



  togglePatch(path: string) {
    return togglePatch(this, path);
  }



  async toggleAllPatches() {
    return toggleAllPatches(this);
  }



  revealSelectedRow() {
    void tick().then(() => document.querySelector<HTMLElement>(".detail-tree .tree-row.selected")?.scrollIntoView({ block: "nearest" }));
  }



  selectChangeFile(path: string) {
    return selectChangeFile(this, path);
  }



  selectTreeFile(path: string) {
    return selectTreeFile(this, path);
  }



  toggleRepoDir(path: string) {
    this.treeCollapsed = this.treeCollapsed.includes(path) ? this.treeCollapsed.filter((item) => item !== path) : [...this.treeCollapsed, path];
  }



  async showFileHistory(file: string) {
    return showFileHistory(this, file);
  }



  async showBlame(file: string) {
    return showBlame(this, file);
  }



  async openRepo(path: string) {
    if (!this.backend() || !path.trim()) return;
    this.dialog = null;
    this.textMerge = false;
    this.confirmAsk = null;
    this.dismissMenu(false);
    this.launchOpen = false;
    this.commandLogOpen = false;
    this.repoPickerOpen = false;
    this.forgeOpen = null;
    const gen = ++this.openGen;
    if (this.snapshot?.path && !this.viewing(path)) void invoke("cancel_reads", { path: this.snapshot.path }).catch(() => {});
    this.focusPath = path;
    if (this.snapshot?.path && !this.viewing(path)) this.drafts.set(this.snapshot.path, { summary: this.summary, description: this.description, commitType: this.commitType });
    const running = (this.jobs[this.jobKey(path)]?.depth ?? 0) > 0;
    let stopAt = path;
    this.actionError = null;
    this.stashRetry = null;
    if (!running) this.startWork(this.tr("chrome.open"), path);
    try {
      const value = await invoke<StatusSnapshot>("workspace_status", { path });
      if (gen !== this.openGen) return;
      // Reopening the repository already on screen keeps the message being written.
      const staying = this.viewing(path) || this.viewing(value.path);
      this.aliasJob(path, value.path);
      stopAt = value.path;
      this.applySnapshot(value);
      if (!staying) {
        this.sideTop = 0;
        this.resetCommitView();
        this.selectedCommit = null;
        this.commitDetail = null;
        this.commitFiles = [];
        this.historyDiff = null;
        this.graphSolo = null;
        this.drops = [];
        this.contentHits = null;
        this.comparePair = null;
        this.blameLines = null;
        this.expandedMerges = [];
        this.historyTab = "commit";
        this.detailQuery = "";
        this.searchOpen.detail = false;
        this.historyTop = 0;
        this.fileTop = 0;
        this.stagedTop = 0;
        this.combinedTop = 0;
        this.commitScrollTop = 0;
        this.commitListTop = 0;
        this.selectedIndex = 0;
        this.diffTop = 0;
        this.blameTop = 0;
        this.historyDiffTop = 0;
        const saved = this.drafts.get(path) ?? this.drafts.get(value.path);
        const savedType = saved?.commitType ?? "";
        this.commitType = isCommitType(savedType) ? savedType : this.preferredCommitType();
        this.summary = saved?.summary ?? "";
        this.description = saved?.description ?? "";
        this.normalizeSummary();
      }
      if (this.standalone !== "history") this.historyFilter = "";
      await this.loadContext();
      if (gen !== this.openGen) return;
      await this.loadDiff();
      if (gen !== this.openGen) return;
      // Arming the watcher walks every work-tree directory; on a huge repository that
      // must not hold up the first paint. Changes before it arms are rare and recover
      // on the next refresh.
      void invoke("watch_repository", { path: this.repoPath() }).catch(() => {});
      void invoke<{ available: boolean }>("lfs_locks", { path: this.repoPath(), list: false })
        .then((locks) => {
          if (gen === this.openGen) this.lfsAvailable = locks.available;
        })
        .catch(() => {
          if (gen === this.openGen) this.lfsAvailable = false;
        });
      await this.rememberRepo(path);
      this.dialog = null;
      this.facts = null;
      this.factsPath = "";
      void this.fillTemplate();
    } catch (error) {
      if (gen !== this.openGen) return;
      const text = this.message(error);
      this.actionError = text;
      if (this.missingRepoPath(text)) await this.dropMissingRepo(path, text);
    } finally {
      if (!running) this.stopWork(stopAt);
      if (gen === this.openGen) this.focusPath = null;
    }
    if (gen === this.openGen && this.settings.fetchAutomatically && this.viewing(path) && !this.busy) void this.doFetch("background");
  }



  composeTool(path: string, args: string) {
    return composeTool(this, path, args);
  }



  applyToolSettings() {
    return applyToolSettings(this);
  }



  async savePrefs(close = true) {
    return savePrefs(this, close);
  }

  switchLanguage() {
    this.settings.locale = this.activeLocale === "zh" ? "en" : "zh";
    broadcastLocale(this.settings.locale);
    void this.savePrefs(false);
  }



  async browseFolder(initial = "") {
    return browseFolder(this, initial);
  }



  withSourceRoots(value: Settings): Settings {
    return withSourceRoots(this, value);
  }



  sourceRootList() {
    return sourceRootList(this);
  }



  addSourceRoot(path: string) {
    return addSourceRoot(this, path);
  }



  removeSourceRoot(path: string) {
    return removeSourceRoot(this, path);
  }



  setSourceAlias(path: string, alias: string) {
    return setSourceAlias(this, path, alias);
  }



  beginOpen() {
    return beginOpen(this);
  }



  async readClipboardText() {
    return readClipboardText(this);
  }



  async beginClone(intent: "clone" | "init" = "clone") {
    return beginClone(this, intent);
  }



  onCloneUrl(value: string) {
    return onCloneUrl(this, value);
  }



  onCloneFolder(value: string) {
    return onCloneFolder(this, value);
  }



  rememberCloneRoot() {
    return rememberCloneRoot(this);
  }



  pathsMatch(left: string, right: string) {
    return pathsMatch(this, left, right);
  }



  async finishCloneOpen(destination: string) {
    return finishCloneOpen(this, destination);
  }



  cancelClone() {
    return cancelClone(this);
  }



  async browseCloneRoot() {
    return browseCloneRoot(this);
  }



  async startClone() {
    return startClone(this);
  }



  async startInit() {
    return startInit(this);
  }



  async openSourceRoot(path: string) {
    return openSourceRoot(this, path);
  }



  async browseSourceRoot() {
    return browseSourceRoot(this);
  }



  async openLocal(path: string) {
    return openLocal(this, path);
  }



  branchName(name: string) {
    const space = this.settings.branchSpace ?? "-";
    const trimmed = name.trim();
    return space ? trimmed.replaceAll(" ", space) : trimmed;
  }



  groupOpen(name: string) {
    const group = branchGroup(name);
    if (!group) return true;
    if (this.sideQuery.trim() !== "") return true;
    return (this.refs?.expandedGroups ?? []).includes(group);
  }



  toggleGroup(group: string) {
    const saved = this.refs?.expandedGroups ?? [];
    const next = saved.includes(group) ? saved.filter((item) => item !== group) : [...saved, group];
    if (this.refs) this.refs = { ...this.refs, expandedGroups: next };
    // This only writes `.git/awegit`; a status scan and history reload would both be wasted.
    void invoke("save_expanded_groups", { path: this.repoPath(), names: next }).catch((error) => {
      this.actionError = this.message(error);
    });
  }



  folderOpen(key: string) {
    if (!key || this.sideQuery.trim() !== "") return true;
    return (this.refs?.expandedGroups ?? []).includes(key);
  }



  toggleFolder(key: string) {
    this.toggleGroup(key);
  }



  sideOpen(key: string) {
    return this.sideQuery.trim() !== "" || !this.collapsedSide.includes(key);
  }



  toggleSide(key: string) {
    this.collapsedSide = this.collapsedSide.includes(key) ? this.collapsedSide.filter((item) => item !== key) : [...this.collapsedSide, key];
  }



  pickSide(key: string) {
    this.sideSelected = key;
    this.toggleSide(key);
  }



  private hiddenRefSource: RefSnapshot | null = null;
  private hiddenRefExact = new Set<string>();
  private hiddenRefPrefix: string[] = [];

  refHidden(name: string) {
    if (this.hiddenRefSource !== this.refs) {
      const tokens = this.refs?.hiddenRefs ?? [];
      this.hiddenRefSource = this.refs;
      this.hiddenRefExact = new Set(tokens.filter((token) => !token.endsWith("/")));
      this.hiddenRefPrefix = tokens.filter((token) => token.endsWith("/"));
    }
    if (this.hiddenRefExact.has(name)) return true;
    return this.hiddenRefPrefix.some((token) => name.startsWith(token));
  }



  hideRef(name: string) {
    const names = [...(this.refs?.hiddenRefs ?? [])];
    if (!names.includes(name)) names.push(name);
    if (this.refs) this.refs = { ...this.refs, hiddenRefs: names };
    void invoke("save_hidden_refs", { path: this.repoPath(), names }).catch((error) => {
      this.actionError = this.message(error);
    });
  }



  showOnly(name: string) {
    this.soloBranch(name);
  }



  showAllRefs() {
    if (this.refs) this.refs = { ...this.refs, hiddenRefs: [] };
    void invoke("save_hidden_refs", { path: this.repoPath(), names: [] }).catch((error) => {
      this.actionError = this.message(error);
    });
  }



  flatEntries(files: Row[]): TreeEntry[] {
    return flatEntries(this, files);
  }



  asTree(files: Row[], collapsed = this.collapsedDirs): TreeEntry[] {
    return asTree(this, files, collapsed);
  }



  toggleDir(path: string) {
    return toggleDir(this, path);
  }



  setFolderFold(mode: "open-all" | "close-all" | "open-first" | "close-first") {
    return setFolderFold(this, mode);
  }



  toggleCommitDir(path: string) {
    return toggleCommitDir(this, path);
  }



  paint(text: string, path: string) {
    return paint(this, text, path);
  }



  forkWhen(at: number) {
    return forkWhen(this, at);
  }



  forkWhenFull(at: number) {
    return forkWhenFull(this, at);
  }



  sampleBlob(path: string) {
    return sampleBlob(this, path);
  }



  commitSubject() {
    return commitSubject(this);
  }



  commitAuthorName() {
    return commitAuthorName(this);
  }



  commitWhenShort() {
    return commitWhenShort(this);
  }



  commitRefs() {
    return commitRefs(this);
  }



  mergeRoute() {
    return mergeRoute(this);
  }



  differentCommitter(detail: CommitDetail | null) {
    return differentCommitter(this, detail);
  }



  fileKind(path: string): "image" | "json" | "markup" | "text" | "code" | "file" {
    return fileKind(this, path);
  }



  expandParents(path: string, collapsed: string[]) {
    return expandParents(this, path, collapsed);
  }



  collapseTreeExcept(paths: string[], focus: string | null) {
    return collapseTreeExcept(this, paths, focus);
  }



  markedLine(text: string, other: string, path: string) {
    return markedLine(this, text, other, path);
  }



  patchOf(file: string) {
    return patchOf(this, file);
  }



  resetCommitView() {
    return resetCommitView(this);
  }



  avatarColor(name: string) {
    return avatarColor(this, name);
  }



  issueHref(token: string) {
    return issueHref(this, token);
  }



  linkParts(text: string) {
    return linkParts(this, text);
  }



  openBar(event: MouseEvent, items: MenuItems) {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    this.submenu = null;
    this.menuOpener = event.currentTarget as HTMLElement;
    this.menu = { x: rect.left, y: rect.bottom + 2, items, anchorTop: rect.top };
  }



  viewItems(): MenuItem[] {
    return viewItems(this);
  }



  stashItems(): MenuItem[] {
    return stashItems(this);
  }



  appearanceItems(): MenuItem[] {
    return appearanceItems(this);
  }



  repositoryItems(): MenuItem[] {
    return repositoryItems(this);
  }



  windowItems(): MenuItem[] {
    return windowItems(this);
  }



  helpItems(): MenuItem[] {
    return helpItems(this);
  }



  commitExtras(): MenuItem[] {
    return commitExtras(this);
  }



  mergeTools = $state<ExternalTool[]>([]);



  diffTools = $state<ExternalTool[]>([]);

  toolKind = $state<ToolKind>("diff");

  toolPick = $state("");

  editorCandidates = $state<EditorTool[]>([]);

  editorPicked = $state<string[]>([]);

  editorDetecting = $state(false);



  async loadExternalTools() {
    return loadExternalTools(this);
  }

  toolsOf(kind: ToolKind) {
    return toolsOf(this, kind);
  }

  applicableTools(kind: ToolKind, file: string) {
    return applicableTools(this, kind, file);
  }

  resolvedTool(kind: ToolKind, file: string) {
    return resolvedTool(this, kind, file);
  }

  defaultTool(kind: ToolKind) {
    return defaultTool(this, kind);
  }

  selectedTool(kind: ToolKind) {
    return selectedTool(this, kind);
  }

  selectTool(toolId: string) {
    selectTool(this, toolId);
  }

  addTool(kind: ToolKind) {
    return addTool(this, kind);
  }

  removeTool(kind: ToolKind, toolId: string) {
    removeTool(this, kind, toolId);
  }

  moveTool(kind: ToolKind, toolId: string, delta: number) {
    moveTool(this, kind, toolId, delta);
  }

  setToolExtensions(kind: ToolKind, toolId: string, text: string) {
    setToolExtensions(this, kind, toolId, text);
  }

  setToolField(kind: ToolKind, toolId: string, field: "name" | "path" | "args", value: string) {
    setToolField(this, kind, toolId, field, value);
  }

  toolTypeRows(kind: ToolKind) {
    return toolTypeRows(this, kind);
  }

  assignToolType(kind: ToolKind, suffix: string, toolId: string) {
    assignToolType(this, kind, suffix, toolId);
  }

  removeToolType(kind: ToolKind, suffix: string) {
    removeToolType(this, kind, suffix);
  }

  renameToolType(kind: ToolKind, oldSuffix: string, newSuffix: string) {
    renameToolType(this, kind, oldSuffix, newSuffix);
  }

  setFallbackTool(kind: ToolKind, toolId: string) {
    setFallbackTool(this, kind, toolId);
  }

  browseToolPath(kind: ToolKind, toolId: string) {
    return browseToolPath(this, kind, toolId);
  }

  importTools(kind: ToolKind, source: ToolSource) {
    return importTools(this, kind, source);
  }

  toolTemplate(kind: ToolKind, file: string) {
    return toolTemplate(this, kind, file);
  }

  toolLabel(tool: { name: string; path: string }) {
    return toolLabel(tool);
  }

  toolExtensionLabel(tool: ToolRecord) {
    return toolExtensionLabel(tool);
  }

  openToolSettings(kind: ToolKind, toolId = "") {
    openToolSettings(this, kind, toolId);
  }

  openEditorImport() {
    openEditorImport(this);
  }

  detectEditors() {
    return detectEditors(this);
  }

  toggleEditorCandidate(path: string) {
    toggleEditorCandidate(this, path);
  }

  importEditorCandidates() {
    importEditorCandidates(this);
  }

  closeEditorImport() {
    closeEditorImport(this);
  }

  openExternalToolMenu(event: MouseEvent, kind: "diff" | "merge", file: string) {
    this.openMenu(event, externalItems(this, kind, file));
  }



  async openHistoryTab(tab: "commit" | "changes" | "tree") {
    return openHistoryTab(this, tab);
  }



  selectSample(id: string) {
    return selectSample(this, id);
  }



  pickedIn(rows: Row[]) {
    return pickedIn(this, rows);
  }



  async discardPicked(files = [...this.picked]) {
    return discardPicked(this, files);
  }


  branchRemoteState(name: string): "missing" | "mirrored" | null {
    if (!this.localBranchNames.has(name)) return null;
    return this.remoteBranchNames.has(name) ? "mirrored" : "missing";
  }


  async loadMergePreview() {
    return loadMergePreview(this);
  }


  refreshMergeSummary() {
    refreshMergeSummary(this);
  }



  async unstagePicked(files = [...this.picked]) {
    return unstagePicked(this, files);
  }



  async stagePicked(files = [...this.picked]) {
    return stagePicked(this, files);
  }



  pickedConflicts(files = [...this.picked]) {
    return pickedConflicts(this, files);
  }



  async resolvePicked(side: "ours" | "theirs", files = [...this.picked]) {
    return resolvePicked(this, side, files);
  }



  stageChosen(side: Side, files: string[]) {
    return stageChosen(this, side, files);
  }



  flipStage(path: string, side: Side) {
    return flipStage(this, path, side);
  }



  async deletePicked(files = [...this.picked]) {
    return deletePicked(this, files);
  }



  confirmPicked(files: string[], title: string, run: (files: string[]) => Promise<void>) {
    return confirmPicked(this, files, title, run);
  }



  discardChosen(files: string[]) {
    return discardChosen(this, files);
  }



  deleteChosen(files: string[]) {
    return deleteChosen(this, files);
  }



  openStash(files: string[]) {
    return openStash(this, files);
  }



  ignoreTargets(path: string, side: Side) {
    return ignoreTargets(this, path, side);
  }



  async addIgnore(paths: string[], fake: boolean, directory: boolean) {
    return addIgnore(this, paths, fake, directory);
  }



  dirMenu(dir: string, side: Side): MenuItem[] {
    return dirMenu(this, dir, side);
  }



  filesInDir(dir: string, side: Side) {
    return filesInDir(this, dir, side);
  }



  stageFolder(dir: string, unstage: boolean) {
    return stageFolder(this, dir, unstage);
  }



  noteActivity(text: string) {
    const at = new Date().toLocaleTimeString();
    this.sessionLog = [`${at} ${text}`, ...this.sessionLog].slice(0, 100);
  }



  fillCommand(template: string, answers: Record<string, string>) {
    return fillCommand(this, template, answers);
  }



  sendCommand(command: string) {
    return sendCommand(this, command);
  }



  runCommand(command: Pick<CommandRecord, "command" | "prompt">) {
    return runCommand(this, command);
  }



  saveCommand() {
    return saveCommand(this);
  }



  removeCommand(id: string) {
    return removeCommand(this, id);
  }



  commandItems(target: string): MenuItem[] {
    return commandItems(this, target);
  }



  saveAccount() {
    return saveAccount(this);
  }



  copyText(text: string) {
    return copyText(this, text);
  }



  clearTextSelection() {
    return clearTextSelection(this);
  }



  rememberPointer(event: MouseEvent) {
    return rememberPointer(this, event);
  }



  diffSide(root: HTMLElement): "left" | "right" | "all" {
    return diffSide(this, root);
  }



  blockText(root: HTMLElement, side: "left" | "right" | "all") {
    return blockText(this, root, side);
  }



  activeTextRoot() {
    return activeTextRoot(this);
  }



  fileTargetRows() {
    return fileTargetRows(this);
  }



  selectAllFiles() {
    return selectAllFiles(this);
  }



  collapseFileSelection(rows: Row[]) {
    return collapseFileSelection(this, rows);
  }



  toggleAllDiffLines() {
    return toggleAllDiffLines(this);
  }



  selectAllHere() {
    return selectAllHere(this);
  }



  pathsForCopy() {
    return pathsForCopy(this);
  }



  fullPath(file: string) {
    return fullPath(this, file);
  }



  async openResolve(file: string) {
    return openResolve(this, file);
  }


  async openTextMerge(file: string) {
    return openTextMerge(this, file);
  }

  async loadTextMerge() {
    return loadTextMerge(this);
  }


  guardMerge(run: () => void) {
    if (this.mergeApplying) return;
    if (!this.mergeDirty) { run(); return; }
    if (this.confirmAsk) return;
    this.ask(this.tr("dialog.mergeDiscardTitle"), this.tr("dialog.mergeDiscardBody"), () => {
      this.mergeDirty = false;
      run();
    }, [], { danger: true });
  }

  closeTextMerge() {
    return closeTextMerge(this);
  }


  async applyTextMerge(text?: string) {
    return applyTextMerge(this, text);
  }

  async applyFileMerge(side: "ours" | "theirs") {
    return applyFileMerge(this, side);
  }


  takeMergeSide(side: "ours" | "theirs", on: boolean) {
    return takeMergeSide(this, side, on);
  }



  openRebase() {
    return openRebase(this);
  }



  moveStep(index: number, delta: number) {
    return moveStep(this, index, delta);
  }



  selectFile(path: string, side: Side, event?: MouseEvent) {
    return selectFile(this, path, side, event);
  }



  selectFolder(dir: string, side: Side, event?: MouseEvent) {
    return selectFolder(this, dir, side, event);
  }



  removeBranch(name: string) {
    return removeBranch(this, name);
  }


  cancelOperation(path: string) {
    return cancelOperation(this, path);
  }



  hardReset() {
    return hardReset(this);
  }



  toRow(file: StatusFile): Row {
    return toRow(this, file);
  }



  fileName(path: string) {
    return fileName(this, path);
  }



  parentDir(path: string) {
    return parentDir(this, path);
  }



  folderName(path: string) {
    return folderName(this, path);
  }



  sampleRecent = [
    "D:\\work\\MyTest",
    "D:\\work\\VPNCloudRelay",
    "D:\\personal\\TinyCar",
    "D:\\personal\\LearnAgent",
    "D:\\personal\\AabInstallHelp",
  ];


  sampleCatalog = [
    ...this.sampleRecent,
    "D:\\work\\ExcelMergeDiffTools_B\\left",
    "D:\\work\\ExcelMergeDiffTools_B\\right",
    "D:\\work\\H_FrameWork\\core",
    "D:\\work\\H_FrameWork\\ui",
    "D:\\work\\TinyCar\\app",
    "D:\\work\\TinyCar\\firmware",
    "D:\\work\\.cursor",
    "D:\\work\\AgentRelay",
    "D:\\work\\AITools",
    "D:\\work\\Ant_Miner",
    "D:\\work\\Async",
    "D:\\work\\AutoBuildUnity",
    "D:\\work\\AutoBuildUnity_Build",
    "D:\\work\\Log",
    "D:\\work\\CAO",
    "D:\\work\\CodeAnalyzer",
    "D:\\personal\\CursorHelper",
  ];



  normalPath(path: string) {
    return normalPath(this, path);
  }



  dedupeRepos(paths: string[]) {
    const seen = new Set<string>();
    const unique: string[] = [];
    for (const path of paths) {
      const key = this.normalPath(path);
      if (!key || seen.has(key)) continue;
      seen.add(key);
      unique.push(path);
    }
    return unique;
  }



  previewRepos() {
    return !this.backend() && this.settings.recent.length === 0 && this.repos.length === 0;
  }



  recentPaths() {
    if (this.settings.recent.length > 0) return this.settings.recent;
    return this.previewRepos() ? this.sampleRecent : [];
  }



  catalogPaths() {
    return this.dedupeRepos([
      ...this.settings.recent,
      ...this.repos,
      ...this.settings.workspaces.flatMap((workspace) => [...workspace.repositories, ...workspace.openTabs]),
      ...this.scannedRepos,
      ...(this.previewRepos() ? this.sampleCatalog : []),
    ]);
  }



  sourceOf(path: string) {
    const target = this.normalPath(path);
    let best = "";
    for (const root of this.sourceRootList()) {
      const base = this.normalPath(root.path);
      if (!base) continue;
      if (target === base || target.startsWith(`${base}/`)) {
        if (base.length > this.normalPath(best).length) best = root.path;
      }
    }
    return best;
  }



  relativeParts(root: string, path: string) {
    const base = root.replace(/[\\/]+$/, "").replaceAll("\\", "/");
    const target = path.replace(/[\\/]+$/, "").replaceAll("\\", "/");
    if (target.toLowerCase() === base.toLowerCase()) return [];
    if (!target.toLowerCase().startsWith(`${base.toLowerCase()}/`)) return [];
    return target.slice(base.length + 1).split("/").filter(Boolean);
  }



  repositoryEntries(): RepoEntry[] {
    const folders = new Map<string, { name: string; repos: string[] }>();
    const loose: string[] = [];
    const outside = new Map<string, string[]>();
    for (const path of this.catalogPaths()) {
      const root = this.sourceOf(path);
      const parts = root ? this.relativeParts(root, path) : [];
      if (root && parts.length > 1) {
        const sep = root.includes("\\") ? "\\" : "/";
        const folderPath = `${root.replace(/[\\/]+$/, "")}${sep}${parts[0]}`;
        const key = this.normalPath(folderPath);
        const group = folders.get(key) ?? { name: parts[0], repos: [] };
        group.repos.push(path);
        folders.set(key, group);
        continue;
      }
      if (root) {
        loose.push(path);
        continue;
      }
      const parent = this.parentDir(path);
      const key = this.normalPath(parent);
      const list = outside.get(key) ?? [];
      list.push(path);
      outside.set(key, list);
    }
    for (const [key, items] of outside) {
      if (key && items.length > 1) folders.set(key, { name: this.folderName(this.parentDir(items[0] ?? "")), repos: items });
      else loose.push(...items);
    }
    const byName = (left: string, right: string) => this.folderName(left).localeCompare(this.folderName(right), undefined, { sensitivity: "base" });
    const entries: RepoEntry[] = [...folders.entries()]
      .map(([path, group]) => ({ kind: "folder" as const, path, name: group.name, repos: [...group.repos].sort(byName) }))
      .sort((left, right) => left.name.localeCompare(right.name, undefined, { sensitivity: "base" }));
    for (const path of [...loose].sort(byName)) entries.push({ kind: "repo", path });
    return entries;
  }



  placeRepoPicker() {
    const rect = this.repoPickerButton?.getBoundingClientRect();
    const width = 292;
    const margin = 8;
    if (!rect) {
      this.repoPickerPos = { x: 12, y: 72, maxHeight: 560 };
      return;
    }
    const below = window.innerHeight - rect.bottom - margin;
    const above = rect.top - margin;
    const openUp = below < 280 && above > below;
    const maxHeight = Math.max(180, Math.min(640, (openUp ? above : below) - 6));
    this.repoPickerPos = {
      x: Math.max(margin, Math.min(rect.left, window.innerWidth - width - margin)),
      y: openUp ? Math.max(margin, rect.top - 6 - maxHeight) : rect.bottom + 6,
      maxHeight,
    };
  }



  async toggleRepoPicker() {
    if (this.repoPickerOpen) {
      this.repoPickerOpen = false;
      return;
    }
    this.closeLaunch();
    this.commandLogOpen = false;
    this.dismissMenu(false);
    this.placeRepoPicker();
    this.repoPickerOpen = true;
    if (!this.backend()) return;
    const token = ++this.repoScanToken;
    this.repoScanning = true;
    this.repoScanTruncated = false;
    const found: string[] = [];
    let truncated = false;
    for (const root of this.sourceRootList()) {
      try {
        const hit = await invoke<RepoScan>("scan_repositories", { root: root.path, depth: this.settings.scanDepth || 5 });
        found.push(...(hit.paths ?? []));
        if (hit.truncated) truncated = true;
      } catch {
        /* a missing source folder should not hide the ones that can be read */
      }
    }
    if (token !== this.repoScanToken) return;
    this.scannedRepos = this.dedupeRepos(found);
    this.repoScanTruncated = truncated;
    this.repoScanning = false;
  }



  toggleRepoFolder(path: string) {
    this.repoPickerExpanded = this.repoPickerExpanded.includes(path)
      ? this.repoPickerExpanded.filter((item) => item !== path)
      : [...this.repoPickerExpanded, path];
  }



  pickRepo(path: string) {
    this.repoPickerOpen = false;
    void this.openRepo(path);
  }



  currentRepo(path: string) {
    return !!this.snapshot?.path && this.pathsMatch(this.snapshot.path, path);
  }



  whenLabel(commit: CommitRow) {
    return whenLabel(this, commit);
  }



  lineText(line: DiffLine) {
    return lineText(this, line);
  }



  splitLineAt(row: SplitRow, side: "left" | "right") {
    return splitLineAt(this, row, side);
  }



  selectDiffRange(from: number, to: number) {
    return selectDiffRange(this, from, to);
  }



  toggleDiffLine(index: number) {
    return toggleDiffLine(this, index);
  }



  clickDiffLine(index: number, event: MouseEvent) {
    return clickDiffLine(this, index, event);
  }



  dragDiffLine(index: number, event: MouseEvent) {
    return dragDiffLine(this, index, event);
  }



  chosenDiffLines(clicked: number) {
    return chosenDiffLines(this, clicked);
  }



  hunkOf(index: number) {
    return hunkOf(this, index);
  }

  hunkRanges() {
    return hunkRanges(this);
  }



  editorLineNo(index: number) {
    return editorLineNo(this, index);
  }



  removesOnApply(line: DiffLine, action: "stageLine" | "discardLine", unstage: boolean) {
    return removesOnApply(this, line, action, unstage);
  }



  async applyLineOps(action: "stageLine" | "discardLine", entries: { line: DiffLine; index: number }[]) {
    return applyLineOps(this, action, entries);
  }



  async showLineHistory(line: DiffLine | undefined) {
    return showLineHistory(this, line);
  }



  diffLineMenu(clicked: number): MenuItem[] {
    return diffLineMenu(this, clicked);
  }



  moveSelection(delta: number) {
    if (this.section === "history") {
      const rows = this.historyItems;
      if (rows.length === 0) return;
      this.selectedIndex = Math.min(rows.length - 1, Math.max(0, this.selectedIndex + delta));
      const row = rows[this.selectedIndex];
      if (row) this.selectCommitSoon(row.id);
      return;
    }
    const { target, rows } = this.fileTargetRows();
    if (rows.length === 0) return;
    const current = rows.findIndex((row) => row.path === this.selectedPath);
    const base = current >= 0 ? current : this.selectedIndex;
    this.selectedIndex = Math.min(rows.length - 1, Math.max(0, base + delta));
    const row = rows[this.selectedIndex];
    if (!row) return;
    const side: Side = target === "staged" ? "staged" : target === "unstaged" ? "unstaged" : this.shownStaged.some((file) => file.path === row.path) ? "staged" : "unstaged";
    this.selectFile(row.path, side);
  }



  cycleTab(delta: number) {
    if (this.repos.length < 2 || !this.snapshot) return;
    const index = this.repos.indexOf(this.snapshot.path);
    const next = this.repos[(index + delta + this.repos.length) % this.repos.length];
    void this.openRepo(next);
  }



  closeTab(path: string) {
    this.closedTabs = [path, ...this.closedTabs.filter((item) => item !== path)].slice(0, 12);
    const next = this.repos.filter((repo) => repo !== path);
    this.repos = next;
    const currentId = this.settings.currentWorkspace;
    if (currentId) {
      this.settings = {
        ...this.settings,
        workspaces: this.settings.workspaces.map((workspace) => {
          if (workspace.id !== currentId) return workspace;
          const openTabs = workspace.openTabs.filter((item) => item !== path);
          return { ...workspace, openTabs, selectedTab: Math.min(workspace.selectedTab, Math.max(0, openTabs.length - 1)) };
        }),
      };
    }
    if (this.snapshot?.path !== path) {
      if (currentId) void this.savePrefs(false);
      return;
    }
    if (next[0]) void this.openRepo(next[0]);
    else {
      this.snapshot = null;
      this.refs = null;
      this.mode = "error";
      this.loadError = "";
      if (currentId) void this.savePrefs(false);
    }
  }



  async selectWorkspace() {
    return selectWorkspace(this);
  }



  currentWorkspace = $derived(this.settings.workspaces.find((item) => item.id === this.settings.currentWorkspace) ?? null);



  workspaceName(workspace: Pick<WorkspaceRecord, "id" | "name">) {
    return workspace.id === recentWorkspaceId ? this.tr("chrome.recentWorkspace") : workspace.name;
  }



  /** Keep the built-in recent workspace on the newest five repositories, without reshuffling an unchanged set. */
  syncRecentWorkspace() {
    const recent = this.dedupeRepos(this.settings.recent).slice(0, recentWorkspaceLimit);
    const workspaces = this.settings.workspaces;
    const fixed = workspaces.find((item) => item.id === recentWorkspaceId);
    const same = !!fixed
      && fixed.repositories.length === recent.length
      && recent.every((path) => fixed.repositories.some((item) => this.pathsMatch(item, path)));
    if (same) {
      if (workspaces[0]?.id !== recentWorkspaceId) {
        this.settings = { ...this.settings, workspaces: [fixed, ...workspaces.filter((item) => item.id !== recentWorkspaceId)] };
      }
      return;
    }
    const selected = fixed?.openTabs[fixed.selectedTab] ?? "";
    const selectedIndex = selected ? recent.findIndex((path) => this.pathsMatch(path, selected)) : -1;
    const next: WorkspaceRecord = {
      id: recentWorkspaceId,
      name: fixed?.name || this.tr("chrome.recentWorkspace"),
      repositories: recent,
      openTabs: recent,
      selectedTab: selectedIndex >= 0 ? selectedIndex : 0,
      tabsSeeded: true,
      tabWidth: workspaceTabWidth(recent.length),
    };
    this.settings = {
      ...this.settings,
      workspaces: [next, ...workspaces.filter((item) => item.id !== recentWorkspaceId)],
    };
    if (this.settings.currentWorkspace === recentWorkspaceId) this.repos = [...recent];
  }



  async switchWorkspace(id: string) {
    return switchWorkspace(this, id);
  }



  async createWorkspace(name: string) {
    return createWorkspace(this, name);
  }



  renameWorkspace(id: string, name: string) {
    return renameWorkspace(this, id, name);
  }



  async deleteWorkspace(id: string) {
    return deleteWorkspace(this, id);
  }



  workspaceItems(): MenuItem[] {
    return workspaceItems(this);
  }



  scrollToCommit(id: string) {
    return scrollToCommit(this, id);
  }



  openHistory() {
    return openHistory(this);
  }



  /** Select the commit a branch or tag points at in the history list. */
  locateSeq = 0;



  async locateRef(name: string) {
    return locateRef(this, name);
  }



  /** Show what a stash changed, using the stash commit against the commit it was taken from. */
  async locateStash(stash: { name: string; id?: string }) {
    return locateStash(this, stash);
  }



  openRepoSettings() {
    return openRepoSettings(this);
  }



  revealHead() {
    return revealHead(this);
  }



  dropCommits() {
    return dropCommits(this);
  }



  toggleDrop(id: string) {
    return toggleDrop(this, id);
  }



  async keepPassphrase() {
    return keepPassphrase(this);
  }



  async saveWindow() {
    return saveWindow(this);
  }



  activateLayer(event: KeyboardEvent) {
    if (event.key === "Escape" && this.confirmAsk) {
      event.preventDefault();
      this.confirmAsk = null;
      return true;
    }
    if (event.key === "Escape" && this.textMerge) {
      event.preventDefault();
      this.closeTextMerge();
      return true;
    }
    if (event.key === "Escape" && this.dialog && !this.busy) {
      event.preventDefault();
      if (this.dialog === "prefs") void this.savePrefs(true);
      this.dialog = null;
      return true;
    }
    const bare = event.key === "Enter" && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey && !event.isComposing;
    if (!bare || this.launchOpen || this.menu) return false;
    if (this.confirmAsk) {
      const target = event.target;
      if (target instanceof Element && target.closest("[data-confirm]")) return false;
      event.preventDefault();
      const run = this.confirmAsk.run;
      this.confirmAsk = null;
      run();
      return true;
    }
    if (this.textMerge) {
      const target = event.target;
      const inside = target instanceof Element && !!target.closest(".merge-panel");
      if (inside && (typing(event) || target instanceof HTMLButtonElement)) return false;
      const button = document.querySelector<HTMLButtonElement>(".merge-panel .commit");
      if (button && !button.disabled) {
        event.preventDefault();
        button.click();
        return true;
      }
    }
    if (this.dialog) {
      const target = event.target;
      const inside = target instanceof Element && !!target.closest(".dialog");
      if (inside && (typing(event) || target instanceof HTMLButtonElement)) return false;
      if (target instanceof HTMLInputElement && ["checkbox", "radio", "range", "color", "file"].includes(target.type)) return false;
      const form = document.querySelector(".scrim .dialog form");
      if (form instanceof HTMLFormElement) {
        event.preventDefault();
        form.requestSubmit();
        return true;
      }
    }
    return false;
  }



  onShortcut(event: KeyboardEvent) {
    if (event.defaultPrevented) return;
    const action = shortcut(event);
    if (action === "switchLanguage") {
      event.preventDefault();
      if (!event.repeat && !event.isComposing) this.switchLanguage();
      return;
    }
    if ((event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === "c") {
      if (typing(event) || this.dialog || this.launchOpen) return;
      if (this.heldText) {
        event.preventDefault();
        this.copyText(this.heldText);
        return;
      }
      if ((window.getSelection?.()?.toString() ?? "").length > 0) return;
      const paths = this.pathsForCopy();
      if (paths) {
        event.preventDefault();
        this.copyText(paths);
      }
      return;
    }
    if (event.key === " " && !event.ctrlKey && !event.metaKey && !event.altKey && !typing(event) && this.section === "history" && this.selectedCommit) {
      const target = event.target as HTMLElement | null;
      if (!target?.closest("button, a, input, textarea")) {
        event.preventDefault();
        void this.selectCommit(this.selectedCommit);
        return;
      }
    }
    if (this.activateLayer(event)) return;
    if (this.dialog || this.textMerge || this.confirmAsk) return;
    if (!action) return;
    // Command log, repository picker, menus, quick launch and forge are modal enough
    // that shortcuts must not write behind them. Escape still closes the top layer.
    const overlay = this.menu || this.launchOpen || this.commandLogOpen || this.repoPickerOpen || this.forgeOpen;
    const launchKeys = this.launchOpen && (action === "up" || action === "down");
    if (overlay && action !== "exit" && !launchKeys) {
      if (action === "launch" && this.launchOpen) this.closeLaunch();
      return;
    }
    const editor = typing(event);
    if (this.launchOpen && (action === "up" || action === "down" || action === "exit")) {
      event.preventDefault();
      if (action === "exit") this.closeLaunch();
      else if (action === "down") this.launchIndex = Math.min(this.matches.length - 1, this.launchIndex + 1);
      else this.launchIndex = Math.max(0, this.launchIndex - 1);
      return;
    }
    if ((action === "up" || action === "down") && (event.target as HTMLElement | null)?.closest(".sidebar")) {
      event.preventDefault();
      this.moveSide(action === "down" ? 1 : -1);
      return;
    }
    if (editor && action !== "commit" && action !== "commitPush" && action !== "exit" && action !== "launch") return;
    if (action === "exit") {
      event.preventDefault();
      if (this.menu) {
        this.dismissMenu();
        return;
      }
      if (this.textMerge) {
        this.closeTextMerge();
        return;
      }
      if (this.forgeOpen) {
        this.forgeOpen = null;
        return;
      }
      if (this.dialog) {
        this.dialog = null;
        return;
      }
      if (this.launchOpen) {
        this.closeLaunch();
        return;
      }
      if (this.repoPickerOpen) {
        this.repoPickerOpen = false;
        return;
      }
      if (this.commandLogOpen) {
        this.commandLogOpen = false;
        return;
      }
      this.blameLines = null;
      return;
    }
    event.preventDefault();
    if (action === "selectAll") {
      if (this.dialog || this.launchOpen || this.menu) return;
      this.selectAllHere();
      return;
    }
    if (this.mode !== "live" && action !== "settings" && action !== "zoomIn" && action !== "zoomOut" && action !== "launch" && action !== "back" && action !== "forward" && action !== "open" && action !== "clone" && action !== "init") return;
    if (action === "refresh") void this.refresh();
    else if (action === "commit") void this.submitCommit(false);
    else if (action === "commitPush") void this.submitCommit(true);
    else if (action === "up") this.moveSelection(-1);
    else if (action === "down") this.moveSelection(1);
    else if (action === "pull" || action === "quickPull") void this.doPull();
    else if (action === "push" || action === "quickPush") void this.doPush();
    else if (action === "fetch" || action === "quickFetch") void this.doFetch();
    else if (action === "branch") {
      this.draft = "";
      this.draftExtra = "";
      this.dialog = "branch";
    } else if (action === "stash") void this.mutate({ action: "stash", message: "" });
    else if (action === "open" || action === "newTab") this.beginOpen();
    else if (action === "closeTab" && this.snapshot) this.closeTab(this.snapshot.path);
    else if (action === "tabLeft") this.cycleTab(-1);
    else if (action === "tabRight") this.cycleTab(1);
    else if (action === "settings") this.dialog = "prefs";
    else if (action === "launch") void this.openLaunch();
    else if (action === "back") this.goBack();
    else if (action === "forward") this.goForward();
    else if (action === "tag") {
      this.draft = "";
      this.draftExtra = this.snapshot?.branch ?? "HEAD";
      this.dialog = "tag";
    } else if (action === "clone" || action === "init") void this.beginClone(action === "init" ? "init" : "clone");
    else if (action === "changes") this.section = "changes";
    else if (action === "commits") this.section = "history";
    else if (action === "reveal") this.revealHead();
    else if (action === "zoomIn") {
      this.settings.uiScale = Math.min(20, this.settings.uiScale + 1);
      void this.savePrefs(false);
    } else if (action === "zoomOut") {
      this.settings.uiScale = Math.max(11, this.settings.uiScale - 1);
      void this.savePrefs(false);
    } else if (action === "search") {
      this.section = "history";
      this.searchOpen.commit = true;
      queueMicrotask(() => this.searchEl?.focus());
    } else if (action === "stageToggle" && this.selectedPath) {
      const rows = this.selectedSide === "staged" ? this.shownStaged : this.shownUnstaged;
      const chosen = this.pickedIn(rows);
      this.stageChosen(this.selectedSide, chosen.length > 1 && chosen.includes(this.selectedPath) ? chosen : [this.selectedPath]);
    } else if (action === "stageAll") void this.runChange(this.selectedSide === "staged" ? "unstage_all" : "stage_all");
    else if (action === "discard") {
      const bareDelete = event.key === "Delete";
      const inUnstaged = (event.target as HTMLElement | null)?.closest(".unstaged-pane");
      if ((bareDelete && !inUnstaged) || this.dialog || this.menu || this.launchOpen || !this.selectedPath || this.selectedSide !== "unstaged") return;
      const chosen = this.pickedIn(this.shownUnstaged);
      this.discardChosen(chosen.length > 1 && chosen.includes(this.selectedPath) ? chosen : [this.selectedPath]);
    } else if (action === "explorer" && this.snapshot) void this.openLocal(this.snapshot.path);
    else if (action === "terminal") void invoke("open_terminal", { path: this.repoPath() });
    else if (action === "filterBranch") this.toggleHistoryScope(); else if (action === "reopen") this.reopenClosed();
    else if (action === "pastePatch") void this.pastePatch();
  }



  moveSide(delta: number) {
    const names = this.visibleBranches.map((branch) => branch.name);
    if (names.length === 0) return;
    const current = (this.sideSelected ?? "").startsWith("local:") ? (this.sideSelected ?? "").slice("local:".length) : "";
    const index = Math.max(0, names.indexOf(current));
    const next = names[Math.min(names.length - 1, Math.max(0, index + delta))] ?? names[0];
    this.sideSelected = `local:${next}`;
  }



  progressLabel(value: InProgress) {
    if (value === "cherryPick") return "Cherry-pick";
    if (value === "rebase") return "Rebase";
    if (value === "revert") return "Revert";
    return "Merge";
  }



  mergeRouteLabel() {
    if (this.progress !== "merge" || !this.mergeState) return "";
    return `${this.mergeState.source} → ${this.mergeState.into}`;
  }

  constructor() {
    $effect(() => {
      if (!this.busy) this.releaseWriteWaiters();
    });
    $effect(() => {
      // Measured heights belong to one diff style; drop them when the layout can have changed.
      this.settings.diffStyle;
      this.settings.diffWordWrap;
      this.settings.diffFontSize;
      this.settings.linesHeight;
      this.patchHeights = {};
    });
    $effect(() => {
      // Leaving History cleared the diff; opening Changes again must start the read,
      // otherwise the pane sits on "Reading…" until the file is clicked again.
      if (this.section !== "changes" || this.mode !== "live" || !this.selectedPath || this.busy) return;
      if (!this.diff && !this.diffError && !this.conflictView) void this.loadDiff();
    });
    $effect(() => {
      if (this.busy || !this.watchQueued) return;
      const scope = this.watchQueued;
      this.watchQueued = null;
      void this.refresh(scope, true);
    });
    $effect(() => {
      const visible = this.section === "history" || this.standalone === "history";
      if (!visible) return;
      const straight = this.liveFold.straight;
      const items = this.historyItems;
      if (items.length <= GRAPH_SYNC_LIMIT) {
        this.graphItems = null;
        this.graphRows = [];
        return;
      }
      const commits = laneCommits(items, straight);
      const prev = this.graphResume;
      const prefix = !!prev && prev.ids.length <= commits.length && prev.ids.every((id, index) => id === commits[index].id);
      let count = prefix && prev ? prev.ids.length : 0;
      let resume: { columns: Array<string | null>; count: number; rows: GraphCell[][] } | undefined =
        prefix && prev ? { columns: prev.columns, count: prev.ids.length, rows: prev.rows } : undefined;
      const publish = (rows: GraphCell[][], width: number, columns: Array<string | null>) => {
        this.graphRows = rows;
        this.graphWidth = width;
        this.graphColumns = columns;
        this.graphItems = items;
        this.graphVersion = ++this.graphEpoch;
      };
      if (count >= commits.length && prev) {
        publish(prev.rows, prev.width, prev.columns);
        return;
      }
      let frame = 0;
      let first = true;
      const pump = () => {
        if (count >= commits.length) return;
        const next = Math.min(commits.length, count + (first ? 2000 : 5000));
        first = false;
        const built = commitGraph(commits.slice(0, next), resume);
        count = next;
        resume = { columns: built.columns, count, rows: built.rows };
        this.graphResume = { ids: commits.slice(0, count).map((commit) => commit.id), columns: built.columns, rows: built.rows, width: built.width };
        publish(built.rows, built.width, built.columns);
        if (count < commits.length) frame = requestAnimationFrame(pump);
      };
      pump();
      return () => cancelAnimationFrame(frame);
    });
    $effect(() => {
    if (!this.settings.fetchAutomatically) return;
    const timer = window.setInterval(() => {
    if (!this.busy && this.snapshot) void this.doFetch("background");
    }, 10 * 60 * 1000);
    return () => window.clearInterval(timer);
    });
    $effect(() => {
    const seconds = this.settings.statusInterval;
    if (!seconds || seconds < 1) return;
    const timer = window.setInterval(() => {
    if (!this.busy && this.snapshot && this.mode === "live") void this.refresh("status", true);
    }, seconds * 1000);
    return () => window.clearInterval(timer);
    });
    $effect(() => {
    const parts: string[] = [];
    if (this.standalone === "merge") {
      parts.push(this.tr("dialog.mergeTitle"));
      if (this.mergeFile) parts.push(`${this.mergeDirty ? "* " : ""}${this.mergeFile}`);
    }
    if (this.settings.workspaceShowTitle && this.currentWorkspace) parts.push(this.workspaceName(this.currentWorkspace));
    if (this.snapshot?.path) parts.push(this.folderName(this.snapshot.path));
    parts.push("AweGit");
    const title = parts.join(" — ");
    if (typeof document !== "undefined") document.title = title;
    if (this.inApp()) void getCurrentWindow().setTitle(title).catch(() => {});
    });
    $effect(() => {
    if (typeof document === "undefined") return;
    const root = document.documentElement;
    const media = typeof window === "undefined" ? null : window.matchMedia("(prefers-color-scheme: dark)");
    const resolved = () => (this.settings.theme === "dark" || (this.settings.theme !== "light" && (media?.matches ?? false)) ? "dark" : "light");
    const paint = () => {
    const theme = resolved();
    root.setAttribute("data-theme", theme);
    root.lang = this.activeLocale === "zh" ? "zh-CN" : "en";
    if (this.inApp()) void getCurrentWindow().setTheme(theme).catch(() => {});
    };
    paint();
    media?.addEventListener("change", paint);
    root.style.setProperty("--ui-scale", `${this.settings.uiScale || 13}px`);
    root.style.setProperty("--row-file", `${this.rowFile}px`);
    root.style.setProperty("--row-commit", `${this.rowCommit}px`);
    root.style.setProperty("--row-side", `${this.rowSide}px`);
    root.style.setProperty("--row-tree", `${this.rowTree}px`);
    root.style.setProperty("--diff-row", `${this.diffRowHeight}px`);
    return () => media?.removeEventListener("change", paint);
    });
    $effect(() => {
    const next = this.snapshotPlace();
    if (this.navLock > 0) {
    this.lastPlace = next;
    return;
    }
    if (!this.lastPlace) {
    this.lastPlace = next;
    return;
    }
    if (this.samePlace(this.lastPlace, next)) return;
    this.navBack = [...this.navBack, this.lastPlace].slice(-50);
    this.navForward = [];
    this.lastPlace = next;
    });
    $effect(() => {
    const max = Math.max(0, this.matches.length - 1);
    if (this.launchIndex > max) this.launchIndex = max;
    });
    $effect(() => {
    if (!this.launchOpen) return;
    this.launchIndex;
    queueMicrotask(() => this.launchList?.querySelector(".on")?.scrollIntoView({ block: "nearest" }));
    });
    $effect(() => {
    if (this.mode !== "error" || !this.backend()) return;
    for (const path of this.settings.recent.slice(0, 12)) {
    if (this.recentStats[path] || this.recentAsked.has(path)) continue;
    this.recentAsked.add(path);
    void invoke<{ branch: string; ahead: number; behind: number; lastSummary: string }>("repository_summary", { path })
    .then((value) => {
    this.recentStats = { ...this.recentStats, [path]: value };
    })
    .catch(() => {
    this.recentAsked.delete(path);
    });
    }
    });
    onMount(() => {
    const onMouseNav = (event: MouseEvent) => {
    if (event.button !== 3 && event.button !== 4) return;
    event.preventDefault();
    if (event.button === 3) this.goBack();
    else this.goForward();
    };
    const addHistoryEntry = () => pushState("", { awegit: 1 });
    const onPop = () => {
    addHistoryEntry();
    this.goBack();
    };
    const onShortcut = this.onShortcut.bind(this);
    const stopLocale = onLocaleChange((locale) => { this.settings.locale = locale; });
    const onRememberPointer = this.rememberPointer.bind(this);
    window.addEventListener("keydown", onShortcut);
    window.addEventListener("mousedown", onMouseNav);
    window.addEventListener("mousedown", onRememberPointer, true);
    window.addEventListener("mouseup", onMouseNav);
    // SvelteKit's router is initialized after hydration; wait a task before touching history.
    const historyTimer = window.setTimeout(addHistoryEntry, 0);
    window.addEventListener("popstate", onPop);
    const detachNav = () => {
    stopLocale();
    window.clearTimeout(historyTimer);
    window.removeEventListener("keydown", onShortcut);
    window.removeEventListener("mousedown", onMouseNav);
    window.removeEventListener("mousedown", onRememberPointer, true);
    window.removeEventListener("mouseup", onMouseNav);
    window.removeEventListener("popstate", onPop);
    };
    if (this.labPage) {
    void this.bootLab();
    return detachNav;
    }
    if (!this.inApp()) {
    this.notices = sampleNotices;
    this.pulls = samplePulls;
    this.repoStats = { branch: "main", ahead: 5, behind: 0, commits: commits.length, branches: 2, lastSummary: commits[0]?.summary ?? "" };
    return detachNav;
    }
    let unlisten = () => {};
    let unprogress = () => {};
    let unmoved = () => {};
    let unresized = () => {};
    let unnav = () => {};
    let unreveal = () => {};
    let unupdate = () => {};
    let placeTimer = 0;
    let stopped = false;
    this.mode = "loading";
    this.selectedPath = null;
    void invoke<string>("app_version").then((value) => { if (value) this.appVersion = value; }).catch(() => {});
    void invoke<string>("update_release_page").then((value) => { if (value) this.updateReleaseUrl = value; }).catch(() => {});
    void invoke<GitRuntime>("git_runtime").then((value) => {
      this.gitRuntime = value;
      if (this.actionError) return;
      if (!value.version) this.actionError = this.tr("chrome.gitMissing");
      else if (!value.minimum) this.actionError = this.tr("chrome.gitOld", { version: value.version });
    }).catch(() => {});
    invoke<UpdateNotice | null>("check_for_update")
    .then((value) => {
    this.pendingUpdate = value;
    this.showPendingUpdate(false);
    })
    .catch(() => {});
    let targeted = false;
    let introduce = false;
    void (async () => {
    try {
    const value = await invoke<Settings>("load_settings");
    if (stopped) return;
    this.adoptSettings(value);
    introduce = this.takeFeaturesIntro();
    if (introduce) void this.savePrefs(false);
    void invoke<string>("settings_path").then((path) => (this.settingsPath = path)).catch(() => {});
    await this.loadExternalTools();
    if (value.windowWidth > 200 && value.windowHeight > 200 && !this.standalone) {
    try {
    const win = getCurrentWindow();
    await win.setPosition(new LogicalPosition(value.windowX, value.windowY));
    await win.setSize(new LogicalSize(value.windowWidth, value.windowHeight));
    } catch {
    /* a saved position can sit off the current desktop */
    }
    }
    try {
    unlisten = await listen<{ refs?: boolean; path?: string } | null>("repo-changed", (event) => {
      const path = event.payload?.path;
      if (path && !this.viewing(path)) return;
      const scope = event.payload?.refs === false ? "status" : "full";
      if (this.busy) {
        // A write ignores refreshes for its whole duration; replay the newest event when it ends.
        if (this.watchQueued !== "full") this.watchQueued = scope;
        return;
      }
      void this.refresh(scope, true);
    });
    unprogress = await listen<{ path: string; percent: number }>("git-progress", (event) => {
    const percent = event.payload?.percent;
    const path = event.payload?.path;
    if (typeof percent === "number" && percent >= 0 && percent <= 100 && path) this.noteProgress(path, percent);
    });
    unupdate = await listen<UpdateProgress>("update-progress", (event) => {
    if (event.payload) this.noteUpdateProgress(event.payload);
    });
    } catch {
    /* the watcher is optional until a repository is open */
    }
    try {
    if (!this.standalone) {
    const win = getCurrentWindow();
    const remember = () => {
    window.clearTimeout(placeTimer);
    placeTimer = window.setTimeout(() => void this.saveWindow(), 400);
    };
    unmoved = await win.onMoved(remember);
    unresized = await win.onResized(remember);
    }
    } catch {
    /* placement events are optional */
    }
    try {
    unnav = await listen<{ view: string; repo: string; file: string; rev: string }>("awegit-navigate", (event) => {
      void this.applyWindowNav(event.payload);
    });
    unreveal = await listen<{ repo: string; id: string }>("awegit-reveal", (event) => {
      if (this.standalone) return;
      const repo = event.payload?.repo;
      const id = event.payload?.id;
      if (!id || (repo && repo !== this.repoPath())) return;
      void this.locateRef(id);
    });
    } catch {
    /* navigation events are optional outside the desktop shell */
    }
    const params = new URLSearchParams(window.location.search);
    const queryRepo = params.get("repo");
    const queryFile = params.get("file");
    const queryView = params.get("view");
    if (queryRepo) {
    targeted = true;
    if (queryView === "history" && queryFile) this.historyFilter = queryFile;
    await this.openRepo(queryRepo);
    if (queryView === "blame" && queryFile) await this.showBlame(queryFile);
    if (queryView === "history" && queryFile) await this.showFileHistory(queryFile);
    if (queryView === "blob" && queryFile) {
      this.blobFile = queryFile;
      this.blobRev = params.get("rev") ?? "";
      await this.loadBlobPane();
    }
    if (queryView === "merge" && queryFile) await this.openTextMerge(queryFile);
    return;
    }
    const workspace = this.settings.workspaces.find((item) => item.id === this.settings.currentWorkspace);
    if (workspace) {
    const tabs = workspace.openTabs.length > 0 ? [...workspace.openTabs] : [...workspace.repositories];
    this.repos = tabs;
    const target = tabs[Math.min(workspace.selectedTab, Math.max(0, tabs.length - 1))] ?? tabs[0];
    if (!target) {
    this.mode = "error";
    this.loadError = "";
    return;
    }
    await this.openRepo(target);
    if (this.snapshot) return;
    }
    const recent = this.settings.recent[0];
    if (!recent) {
    this.mode = "error";
    if (!this.loadError) this.loadError = this.actionError || "";
    return;
    }
    await this.openRepo(recent);
    if (!this.snapshot) {
    this.loadError = this.actionError || "Could not open the last repository.";
    this.mode = "error";
    }
    } catch (error) {
    if (!stopped) {
    this.loadError = this.message(error);
    this.mode = "error";
    }
    } finally {
    if (!stopped && !targeted && introduce) this.dialog = "features";
    }
    })();
    return () => {
    stopped = true;
    detachNav();
    window.clearTimeout(placeTimer);
    unlisten();
    unprogress();
    unmoved();
    unresized();
    unnav();
    unreveal();
    unupdate();
    };
    });
  }
}
