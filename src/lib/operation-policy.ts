/** Refresh only the data a command can change. Keep this exhaustive with Mutation. */
export type RefreshArea = "worktree" | "index" | "refs" | "history" | "stashes" | "config" | "flow";
const work: RefreshArea[] = ["worktree", "index"];
const refs: RefreshArea[] = ["refs"];
const history: RefreshArea[] = [...work, "refs", "history", "flow"];
export const operationAreas = {
  fetch: refs, pull: history, push: refs, stash: [...work, "stashes"], stashPop: [...work, "stashes"],
  stashApply: work, stashDrop: ["stashes"], stashRename: ["stashes"], stashBranch: [...history, "stashes"], stashPatch: [], stashPaths: [...work, "stashes"],
  checkout: history, createBranch: refs, deleteBranch: refs, merge: history, rebase: history, rebaseInteractive: history,
  abort: history, continue: history, skip: history, reset: history, cherryPick: history, revert: history, reword: history,
  tag: refs, deleteTag: refs, deleteRemoteTag: refs, restoreFile: work, bisect: history,
  writeGitignore: [...work, "config"], appendIgnore: [...work, "config"], writeMailmap: ["worktree", "config", "history"],
  stageHunk: ["index"], stageLine: ["index"], stageLines: ["index"], stagePaths: ["index"],
  discardHunk: ["worktree"], discardLine: ["worktree"], discardLines: ["worktree"], resolve: [...work, "flow"], resolvePaths: [...work, "flow"],
  renameBranch: [...refs, "config"], setUpstream: [...refs, "config"], deleteRemoteBranch: refs, checkoutRemote: history,
  pushRef: refs, pullRef: history, discard: work, discardPaths: work, delete: work, deletePaths: work, applyPatch: work,
  addRemote: [...refs, "config"], removeRemote: [...refs, "config"], setRemoteUrl: [...refs, "config"],
  addWorktree: [...refs, "history"], removeWorktree: [...refs, "history"], pruneWorktrees: [...refs, "history"],
  setWorktreeLock: refs, moveWorktree: refs, repairWorktree: refs, submoduleUpdate: [...work, "refs"], submoduleUpdatePath: [...work, "refs"],
  submoduleAdd: [...work, "refs", "config"], submoduleInit: ["refs", "config"], submoduleSync: ["config"], submoduleRemove: [...work, "refs", "config"],
  gitFlowInit: ["config"], gitFlowStart: history, gitFlowFinish: history, squash: history, clone: history, init: history,
  lfsPull: ["worktree"], lfsPush: [], lfsLock: ["config"], lfsUnlock: ["config"],
  setHidden: ["config"], setExpanded: ["config"], setSignOff: ["config"], custom: history,
  commit: history,
} satisfies Record<string, readonly RefreshArea[]>;

export function areasFor(action: string): readonly RefreshArea[] {
  const commands: Record<string, readonly RefreshArea[]> = {
    commit_changes: history, stage_all: ["index"], unstage_all: ["index"], stage_path: ["index"], unstage_path: ["index"],
    save_hidden_refs: ["config"], save_expanded_groups: ["config"], save_sign_off: ["config"],
    set_repo_author: ["config"], save_remote_account: ["config"], approve_credential: [], remove_index_lock: work, remove_stale_lock: work,
  };
  return operationAreas[action as keyof typeof operationAreas] ?? commands[action] ?? history;
}

export const repositoryWrites = new Set([
  "mutate", "stage_all", "unstage_all", "stage_path", "unstage_path", "commit_changes",
  "save_hidden_refs", "save_expanded_groups", "save_sign_off", "set_repo_author",
  "remove_index_lock", "remove_stale_lock", "save_remote_account", "approve_credential",
]);

export const patchActions = new Set(["stageHunk", "stageLine", "stageLines", "discardHunk", "discardLine", "discardLines"]);
export const recoveryActions = new Set(["resolve", "resolvePaths", "stagePaths", "stageHunk", "stageLine", "stageLines", "abort", "continue", "skip"]);

export function repositoryKey(path: string) {
  const key = path.replace(/^\\\\\?\\/, "").replaceAll("\\", "/").replace(/\/+$/, "");
  return /^[a-z]:\//i.test(key) || key.startsWith("//") ? key.toLowerCase() : key;
}

export function requestTargets(args: Record<string, unknown>) {
  const request = (args.request ?? args) as Record<string, unknown>;
  const files = Array.isArray(request.files) ? request.files.filter((file): file is string => typeof file === "string") : [];
  const selection = Array.isArray(args.__selection) ? args.__selection.filter((file): file is string => typeof file === "string") : [];
  return [...new Set([...files, ...selection, ...["file", "name", "to", "branch", "rev", "onto", "destination", "remote", ...(args.request ? ["path"] : [])].flatMap((key) => typeof request[key] === "string" && request[key] ? [request[key] as string] : [])])];
}
