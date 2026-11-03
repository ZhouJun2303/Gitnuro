export type Mode = "sample" | "loading" | "live" | "error";

export type Side = "staged" | "unstaged";

export type Row = { path: string; letter: string; tone: "added" | "modified" | "deleted" };

export type Dialog =
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
    | "gitignore"
    | "stats"
    | "lfs"
    | "github"
    | "stashRename"
    | "stashBranch"
    | "checkoutDirty"
    | "checkoutNotice"
    | "activity"
    | "mailmap"
    | "accounts"
    | "ssh"
    | "stale"
    | "pushAs"
    | "askInput"
    | "renameTab"
    | "colorTab"
    | "groupRename"
    | null;

export type MergePiece =
    | { kind: "text"; text: string }
    | { kind: "hunk"; ours: string; theirs: string; pick: "ours" | "theirs" | "both" };

export type RepoFacts = {
    ignored: string[];
    bisectActive: boolean;
    bisectLog: string;
    template: string;
    headMessage: string;
    weekday: number[];
    hour: number[];
    lfs: string;
    hot: { path: string; count: number }[];
  };

export type MenuItem = { label: string; run?: () => void; disabled?: boolean; shortcut?: string; children?: MenuItem[]; sep?: boolean; checked?: boolean };

export type ConfirmAsk = { title: string; body: string; run: () => void };

export type TreeEntry = { key: string; kind: "dir" | "file"; path: string; file?: Row; depth: number };

export type RepoEntry =
    | { kind: "folder"; path: string; name: string; repos: string[] }
    | { kind: "repo"; path: string };

export type CommandFilter = "all" | "user" | "background" | "errors";

export type StoredError = {
    id: number;
    at: number;
    title: string;
    text: string;
    command: string;
  };

export type CommandEntry = {
    id: number;
    title: string;
    summary: string;
    command: string;
    output: string;
    ok: boolean;
    at: number;
    source: "user" | "background";
  };

export type RepoJob = { label: string; percent: number | null; depth: number };

export type PrefTab = "general" | "commit" | "git" | "integration" | "commands" | "updates";

export type LaunchItem = { label: string; group: string; run: () => void; stay?: boolean };

export type Place = {
    section: "changes" | "history";
    path: string | null;
    side: Side;
    commit: string | null;
    blame: boolean;
    tree: boolean;
    historyFilter: string;
  };

export type SearchKey = "recent" | "side" | "commit" | "file"
