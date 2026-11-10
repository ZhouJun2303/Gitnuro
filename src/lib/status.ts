/** A changed path returned by the Rust `workspace_status` command. */
export type ChangeKind = "added" | "modified" | "deleted" | "renamed" | "conflict" | "untracked";

export type StatusFile = {
  path: string;
  kind: ChangeKind;
  previousPath: string | null;
};

export type StatusSnapshot = {
  path: string;
  branch: string | null;
  staged: StatusFile[];
  unstaged: StatusFile[];
};

export type DiffLineKind = "add" | "delete" | "hunk" | "context" | "meta";

export type DiffLine = {
  kind: DiffLineKind;
  text: string;
  stageAt?: number;
  workAt?: number;
};

export type FileDiff = {
  path: string;
  staged: boolean;
  binary: boolean;
  truncated: boolean;
  loadable: boolean;
  lines: DiffLine[];
  hunks: number;
  /** Bytes on the old side; null when the file is new. */
  oldBytes: number | null;
  /** Bytes on the new side; null when the file was deleted. */
  newBytes: number | null;
};

export type BadgeTone = "added" | "modified" | "deleted";

/** Single-letter badge used in the file list. */
export function badge(kind: ChangeKind): { letter: string; tone: BadgeTone } {
  switch (kind) {
    case "added":
      return { letter: "A", tone: "added" };
    case "modified":
      return { letter: "M", tone: "modified" };
    case "deleted":
      return { letter: "D", tone: "deleted" };
    case "renamed":
      return { letter: "R", tone: "modified" };
    case "conflict":
      return { letter: "C", tone: "deleted" };
    case "untracked":
      return { letter: "?", tone: "added" };
  }
}

export type CommitDetail = {
  id: string;
  summary: string;
  body: string;
  author: string;
  authorEmail: string;
  authorAt: number;
  committer: string;
  committerEmail: string;
  committerAt: number;
  parents: string[];
};

export type CommitRow = {
  id: string;
  shortId: string;
  summary: string;
  author: string;
  when: string;
  parents: string[];
  refs: string[];
  lane: number;
  at: number;
  email?: string;
  unpushed?: boolean;
  remoteOnly?: boolean;
};

export type BranchRow = {
  name: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  current: boolean;
};

export type RemoteRow = {
  name: string;
  url: string | null;
  head: string | null;
  branches: string[];
};

export type TagRow = { name: string; id: string };
export type StashRow = { name: string; summary: string; id?: string; parent?: string };
export type SubmoduleRow = { path: string; id: string; ready: boolean };
export type WorktreeRow = { path: string; branch: string | null; detached: boolean };

export type RefSnapshot = {
  head?: string | null;
  branches: BranchRow[];
  remotes: RemoteRow[];
  tags: TagRow[];
  stashes: StashRow[];
  submodules: SubmoduleRow[];
  worktrees: WorktreeRow[];
  hiddenRefs?: string[];
  expandedGroups?: string[];
  signOffSet?: boolean;
  signOff?: boolean;
  signOffFormat?: string;
};

export type InProgress = "merge" | "rebase" | "cherryPick" | "revert";

export type MergeState = {
  source: string;
  into: string;
  files: { status: string; path: string }[];
};

export type BlameLine = {
  id: string;
  shortId: string;
  summary: string;
  author: string;
  line: number;
  text: string;
  truncated?: boolean;
};

/** One entry in the commit composer's type picker. */
export type CommitTypeRecord = {
  /** Lowercase name before the colon, such as `feat`. */
  name: string;
  /** `#rrggbb` swatch. */
  color: string;
  /** Custom description. Empty keeps the translated description of a built-in type. */
  label: string;
};

export type ToolRecord = {
  id: string;
  name: string;
  path: string;
  /** Diff: `{left}` `{right}` `{file}`. Merge: `{ours}` `{theirs}` `{base}` `{working}` `{file}`. */
  args: string;
  /** Lowercase suffixes without the dot. Empty means every file. */
  extensions: string[];
  /** Used for files that match no explicit extension rule. */
  fallback: boolean;
};

export type Settings = {
  theme: string;
  pullRebase: boolean;
  fetchPrune: boolean;
  diffStyle: string;
  proxy: string;
  aiBaseUrl: string;
  aiModel: string;
  aiApiKey: string;
  terminal: string;
  swapPanes: boolean;
  showEntireFile: boolean;
  linesHeight: string;
  uiScale: number;
  dateRelative: boolean;
  dateFormat: string;
  date24h: boolean;
  hiddenRefs: string;
  authorName: string;
  authorEmail: string;
  sslVerify: boolean;
  sslCaFile: string;
  proxyUser: string;
  proxyPassword: string;
  proxyEnabled: boolean;
  proxyType: string;
  proxyHost: string;
  proxyPort: number;
  signCommits: boolean;
  mergeNoFf: boolean;
  cloneDirectory: string;
  windowX: number;
  windowY: number;
  windowWidth: number;
  windowHeight: number;
  treeFiles: boolean;
  gravatar: boolean;
  signOff: boolean;
  signOffFormat: string;
  forceWithLease: boolean;
  aiEnabled: boolean;
  aiLanguage: string;
  aiMaxChars: number;
  aiPrompt: string;
  aiPromptStyle: string;
  aiTemperature: number;
  logDirectory: string;
  recent: string[];
  workspaces: WorkspaceRecord[];
  currentWorkspace: string;
  commands: CommandRecord[];
  flowMaster: string;
  flowDevelop: string;
  flowFeature: string;
  flowRelease: string;
  flowHotfix: string;
  flowSupport: string;
  expandedGroups: string[];
  locale: string;
  diffTool: string;
  mergeTool: string;
  githubToken: string;
  gitlabToken: string;
  gitlabHost: string;
  showDiffMarks: boolean;
  diffFontSize: number;
  disableSyntaxHighlight: boolean;
  commitSort: string;
  fetchAutomatically: boolean;
  fetchTags: boolean;
  tabIndicator: boolean;
  updateSubmodulesOnCheckout: boolean;
  branchSpace: string;
  pushOnCommit: boolean;
  compactBranchLabels: boolean;
  errorLogDays: number;
  messageLow: number;
  messageHigh: number;
  spellChecking: string;
  pageGuide: number;
  highlightIssues: boolean;
  shellKind: string;
  shellPath: string;
  shellArgs: string;
  diffToolName: string;
  diffToolPath: string;
  diffToolArgs: string;
  mergeToolName: string;
  mergeToolPath: string;
  mergeToolArgs: string;
  diffTools: ToolRecord[];
  mergeTools: ToolRecord[];
  ignoreSpace: boolean;
  verticalTabs: boolean;
  pinnedRefs: string[];
  sourceDirectories: string[];
  groupNames: string[];
  showWhitespace: boolean;
  fileLayout: string;
  fakeIgnore: boolean;
  tabLabels: string[];
  tabColors: string[];
  accounts: Account[];
  bitbucketToken: string;
  azureToken: string;
  azureOrg: string;
  oauthClientId: string;
  fetchAllRemotes: boolean;
  pushAllTags: boolean;
  checkoutAfterCreate: boolean;
  stashIncludeUntracked: boolean;
  diffWordWrap: boolean;
  diffContext: number;
  statusInterval: number;
  showWorktrees: boolean;
  maxCommits: number;
  rebaseUpdateRefs: boolean;
  applyStashDelete: boolean;
  pushTagOnCreate: boolean;
  workspaceShowTitle: boolean;
  scanDepth: number;
  /** Last commit-message type chosen in the composer. */
  commitType: string;
  /** Types offered by the composer. Empty keeps the built-in list. */
  commitTypes: CommitTypeRecord[];
  /** Update banner stays hidden for this version until a newer one is published. */
  dismissedUpdate: string;
  /** `pending` on a brand-new install, otherwise `seen`. */
  featuresIntro: string;
  /** `recovered-backup` when the last load restored a damaged settings file. */
  recoveryNotice?: string;
};

export type ForkImport = {
  settings: Settings;
  preferences: number;
  repositories: number;
  workspaces: number;
};

export type Account = {
  id: string;
  forge: string;
  label: string;
  token: string;
  host: string;
  clientId: string;
};

export type WorkspaceRecord = {
  id: string;
  name: string;
  repositories: string[];
  openTabs: string[];
  selectedTab: number;
  tabsSeeded?: boolean;
  /** Count-based width kept with the workspace. The strip paints from `fillWorkspaceTabs` when dirty. */
  tabWidth?: number;
};

/** Built-in workspace that always holds the most recently opened repositories. */
export const recentWorkspaceId = "awegit-recent";
export const recentWorkspaceLimit = 5;

/** Narrowest project tab. Past this, the strip scrolls instead of shrinking further. */
export const workspaceTabFloor = 96;

/** Width to store for a workspace. More repositories get a narrower tab. */
export function workspaceTabWidth(count: number) {
  if (count <= 1) return 220;
  if (count === 2) return 200;
  if (count <= 4) return 168;
  if (count <= 8) return 140;
  if (count <= 12) return 112;
  return workspaceTabFloor;
}

/**
 * Split `available` pixels across `count` tabs so the row is full.
 * The last tab receives the leftover pixels. Below the floor, every tab stays
 * at the floor and the strip scrolls.
 */
export function fillWorkspaceTabs(available: number, count: number) {
  const n = Math.max(1, Math.trunc(count));
  const space = Math.max(0, Math.trunc(available));
  const share = Math.floor(space / n);
  if (share < workspaceTabFloor) {
    if (n === 1) return { width: space, extra: 0 };
    return { width: workspaceTabFloor, extra: 0 };
  }
  return { width: share, extra: space - share * n };
}

export type CommandRecord = {
  id: string;
  name: string;
  target: string;
  command: string;
  prompt: string;
  shared: boolean;
  repo: string;
};

export type ConflictSides = {
  ours: string;
  theirs: string;
  working: string;
};

export type ConflictCommit = {
  id: string;
  short: string;
  subject: string;
  author: string;
  email: string;
  /** Unix seconds from `%at`. */
  at: number;
};

export type ConflictParty = {
  label: string;
  state: "added" | "modified" | "deleted" | "unchanged";
  bytes: number;
  commit?: ConflictCommit | null;
};

export type ConflictView = {
  ours: ConflictParty;
  theirs: ConflictParty;
  binary: boolean;
};

export type FilePreview = {
  mime: string;
  dataUrl: string;
  animated: boolean;
};

export type UpdateNotice = {
  appVersion: string;
  appCode: number;
  downloadUrl: string;
  notes: string;
  installable: boolean;
  sha256?: string;
  releaseUrl?: string;
  fileName?: string;
};

export type UpdateProgress = {
  phase: "connecting" | "downloading" | "verifying" | "done";
  percent: number | null;
  received: number;
  total: number;
};

export type GitRuntime = {
  program: string;
  version: string;
  mergeTree: boolean;
  lfs: boolean;
  minimum: boolean;
};

export type RepoScan = {
  paths: string[];
  truncated: boolean;
};

export type Suggestion = {
  summary: string;
  description: string;
};
