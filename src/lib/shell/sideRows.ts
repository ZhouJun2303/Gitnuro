import type { ShellState } from "./state.svelte";
import { branchGroup, orderRefNames } from "../view";

export type SideAdd = "branch" | "remote" | "submodule" | "worktree";

export type SideRow =
  | { key: string; kind: "section"; id: string; label: string; count: number | null; filter: boolean; add: SideAdd | null; showAll: boolean; empty: boolean }
  | { key: string; kind: "folder"; folderKey: string; label: string; deep: boolean }
  | { key: string; kind: "local"; name: string; current: boolean; ahead: number; behind: number; group: string; deep: boolean }
  | { key: string; kind: "remote"; name: string; url: string; empty: boolean }
  | { key: string; kind: "remoteHead"; remote: string; head: string }
  | { key: string; kind: "remoteBranch"; remote: string; name: string; label: string; deeper: boolean }
  | { key: string; kind: "tag"; name: string; label: string; deep: boolean }
  | { key: string; kind: "stash"; name: string; summary: string; id?: string }
  | { key: string; kind: "submodule"; path: string; ready: boolean }
  | { key: string; kind: "worktree"; path: string; branch: string | null };

/** Flat sidebar rows. The view draws only the ones inside the scroll window. */
export function liveSideRows(shell: ShellState): SideRow[] {
  const refs = shell.refs;
  if (!refs) return [];
  const rows: SideRow[] = [];
  const query = shell.sideQuery;

  rows.push({
    key: "section:branches",
    kind: "section",
    id: "branches",
    label: shell.tr("chrome.localBranches"),
    count: null,
    filter: true,
    add: "branch",
    showAll: true,
    empty: shell.visibleBranches.length === 0,
  });
  if (shell.sideOpen("branches")) {
    let previous = "";
    for (const branch of shell.visibleBranches) {
      const group = branchGroup(branch.name);
      if (group && group !== previous) {
        rows.push({ key: `group:${group}`, kind: "folder", folderKey: group, label: group, deep: false });
      }
      previous = group;
      if (shell.groupOpen(branch.name)) {
        rows.push({
          key: `local:${branch.name}`,
          kind: "local",
          name: branch.name,
          current: branch.current,
          ahead: branch.ahead,
          behind: branch.behind,
          group,
          deep: group.length > 0,
        });
      }
    }
  }

  rows.push({
    key: "section:remotes",
    kind: "section",
    id: "remotes",
    label: shell.tr("chrome.remotes"),
    count: null,
    filter: false,
    add: "remote",
    showAll: false,
    empty: refs.remotes.length === 0,
  });
  if (shell.sideOpen("remotes")) {
    for (const remote of refs.remotes) {
      const remoteKey = `remote:${remote.name}`;
      const named = shell.matchesQuery(remote.name, query);
      let branchMatch = false;
      if (!named) {
        for (const branch of remote.branches) {
          if (shell.matchesQuery(branch, query)) {
            branchMatch = true;
            break;
          }
        }
      }
      if (!named && !branchMatch) continue;
      const empty = remote.branches.length === 0 && !remote.head;
      rows.push({ key: remoteKey, kind: "remote", name: remote.name, url: remote.url ?? "", empty });
      if (!shell.sideOpen(remoteKey)) continue;
      const ordered = orderRefNames(remote.branches);
      const groups = new Set<string>();
      for (const branch of ordered) {
        const folder = branchGroup(branch);
        if (folder && shell.matchesQuery(branch, query)) groups.add(folder);
      }
      if (remote.head && query.trim() === "") {
        rows.push({ key: `${remoteKey}:HEAD`, kind: "remoteHead", remote: remote.name, head: remote.head });
      }
      let previous = "";
      for (const branch of ordered) {
        const folder = branchGroup(branch);
        const folderKey = `${remoteKey}/${folder}`;
        if (folder && folder !== previous && groups.has(folder)) {
          rows.push({ key: `folder:${folderKey}`, kind: "folder", folderKey, label: folder, deep: true });
        }
        previous = folder;
        if (!shell.matchesQuery(branch, query) || shell.refHidden(`${remote.name}/${branch}`)) continue;
        if (folder && !shell.folderOpen(folderKey)) continue;
        rows.push({
          key: `${remoteKey}:${branch}`,
          kind: "remoteBranch",
          remote: remote.name,
          name: branch,
          label: folder ? branch.slice(folder.length + 1) : branch,
          deeper: folder.length > 0,
        });
      }
    }
  }

  rows.push({
    key: "section:tags",
    kind: "section",
    id: "tags",
    label: shell.tr("chrome.tags"),
    count: shell.visibleTags.length,
    filter: false,
    add: null,
    showAll: false,
    empty: shell.visibleTags.length === 0,
  });
  if (shell.sideOpen("tags")) {
    let previous = "";
    for (const tag of shell.visibleTags) {
      const group = branchGroup(tag.name);
      const folderKey = group ? `tag:${group}` : "";
      if (group && group !== previous) {
        rows.push({ key: `tag-folder:${group}`, kind: "folder", folderKey, label: group, deep: false });
      }
      previous = group;
      if (!shell.folderOpen(folderKey)) continue;
      rows.push({
        key: `tag:${tag.name}`,
        kind: "tag",
        name: tag.name,
        label: group ? tag.name.slice(group.length + 1) : tag.name,
        deep: group.length > 0,
      });
    }
  }

  rows.push({
    key: "section:stashes",
    kind: "section",
    id: "stashes",
    label: shell.tr("chrome.stashes"),
    count: refs.stashes.length,
    filter: false,
    add: null,
    showAll: false,
    empty: refs.stashes.length === 0,
  });
  if (shell.sideOpen("stashes")) {
    for (const stash of refs.stashes) {
      if (!shell.matchesQuery(stash.summary, query) && !shell.matchesQuery(stash.name, query)) continue;
      rows.push({ key: `stash:${stash.name}`, kind: "stash", name: stash.name, summary: stash.summary, id: stash.id });
    }
  }

  rows.push({
    key: "section:submodules",
    kind: "section",
    id: "submodules",
    label: shell.tr("chrome.submodules"),
    count: refs.submodules.length,
    filter: false,
    add: "submodule",
    showAll: false,
    empty: refs.submodules.length === 0,
  });
  if (shell.sideOpen("submodules")) {
    for (const row of refs.submodules) {
      if (!shell.matchesQuery(row.path, query)) continue;
      rows.push({ key: `submodule:${row.path}`, kind: "submodule", path: row.path, ready: row.ready });
    }
  }

  if (shell.settings.showWorktrees) {
    rows.push({
      key: "section:worktrees",
      kind: "section",
      id: "worktrees",
      label: shell.tr("chrome.worktrees"),
      count: refs.worktrees.length,
      filter: false,
      add: "worktree",
      showAll: false,
      empty: refs.worktrees.length === 0,
    });
    if (shell.sideOpen("worktrees")) {
      for (const row of refs.worktrees) {
        if (!shell.matchesQuery(row.path, query) && !shell.matchesQuery(row.branch ?? "", query)) continue;
        rows.push({ key: `worktree:${row.path}`, kind: "worktree", path: row.path, branch: row.branch });
      }
    }
  }

  return rows;
}
