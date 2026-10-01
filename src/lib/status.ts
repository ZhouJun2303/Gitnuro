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
  lines: DiffLine[];
  hunks: number;
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

export type BlameLine = {
  id: string;
  shortId: string;
  summary: string;
  author: string;
  line: number;
  text: string;
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
  mergeAutostash: boolean;
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
};

export type WorkspaceRecord = {
  id: string;
  name: string;
  repositories: string[];
  openTabs: string[];
  selectedTab: number;
};

export type CommandRecord = {
  id: string;
  name: string;
  target: string;
  command: string;
};

export type ConflictSides = {
  ours: string;
  theirs: string;
  working: string;
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
};

export type Suggestion = {
  summary: string;
  description: string;
};
