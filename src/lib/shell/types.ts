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
    | "features"
    | "submodule"
    | "worktree"
    | "output"
    | "mergeConflict"
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

export type MergePick = "ours" | "theirs" | "both" | "none";

export type MergePiece =
    | { kind: "text"; text: string }
    | { kind: "hunk"; ours: string; theirs: string; pick: MergePick };

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

export type ConfirmAsk = { title: string; body: string; files?: string[]; run: () => void };

export type TreeEntry = { key: string; kind: "dir" | "file"; path: string; file?: Row; depth: number };

export type RepoEntry =
    | { kind: "folder"; path: string; name: string; repos: string[] }
    | { kind: "repo"; path: string };

export type CommandFilter = "all" | "user" | "background" | "errors" | "notices";

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
    /** When set, this row is a live status notice, not a git command. */
    noticeId?: StatusNoticeId;
  };

export type StatusNoticeId = "conflicts" | "progress" | "bisect" | "error" | "update" | "clone-done";

export type StatusTone = "conflict" | "progress" | "busy" | "error" | "info";

export type StatusNoticeAction = {
    id: string;
    label: string;
    disabled?: boolean;
  };

export type StatusNotice = {
    id: StatusNoticeId;
    /** Lower wins the identity floor. */
    priority: number;
    tone: StatusTone;
    title: string;
    detail: string;
    dataLayer: string;
    /** True when this notice may occupy the identity floor. */
    floor: boolean;
    actions: StatusNoticeAction[];
  };

/** Stable negative ids so notice rows do not collide with command log ids. */
export const STATUS_NOTICE_ENTRY: Record<StatusNoticeId, number> = {
    conflicts: -101,
    progress: -102,
    bisect: -103,
    error: -104,
    update: -105,
    "clone-done": -106,
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
