import type { ShellState } from "./state.svelte";
import { branchGroup, orderRefNames } from "../view";
import type { SubmoduleRow, WorktreeRow } from "../status";

export type SideAdd = "branch" | "remote" | "submodule" | "worktree";
type PagedSection = "tags" | "stashes";
export type SubmoduleTone = "ok" | "warn" | "bad" | "muted";

export type SideRow =
  | { key: string; kind: "section"; id: string; label: string; count: number | null; filter: boolean; add: SideAdd | null; showAll: boolean; empty: boolean }
  | { key: string; kind: "more"; section: PagedSection }
  | { key: string; kind: "folder"; folderKey: string; label: string; deep: boolean }
  | { key: string; kind: "local"; name: string; current: boolean; ahead: number; behind: number; upstream: string; group: string; deep: boolean }
  | { key: string; kind: "remote"; name: string; url: string; empty: boolean }
  | { key: string; kind: "remoteHead"; remote: string; head: string }
  | { key: string; kind: "remoteBranch"; remote: string; name: string; label: string; deeper: boolean }
  | { key: string; kind: "tag"; name: string; label: string; deep: boolean }
  | { key: string; kind: "stash"; name: string; summary: string; id?: string }
  | { key: string; kind: "submodule"; path: string; ready: boolean; tone: SubmoduleTone; title: string }
  | { key: string; kind: "submoduleNote"; path: string; tone: SubmoduleTone; note: string; title: string }
  | { key: string; kind: "worktree"; worktree: WorktreeRow };

type SubmoduleHealth = { tone: SubmoduleTone; note: string; title: string };

/**
 * Whether the submodule is used the way the superproject expects. Detached HEAD is normal when
 * `.gitmodules` names no branch: the gitlink pins one commit. A named branch must be checked out.
 */
function submoduleHealth(shell: ShellState, row: SubmoduleRow): SubmoduleHealth {
  const short = (id: string) => (id ? id.slice(0, 7) : "—");
  if (!row.ready) {
    const note = shell.tr("chrome.submoduleUninitialized");
    return { tone: "muted", note, title: `${note}\n${shell.tr("chrome.submoduleHealthUninitialized")}` };
  }
  // The superproject lists an unstaged submodule path for worktree changes only while the
  // checkout commit still matches the gitlink; a different commit is reported as its own state.
  const dirty = row.id === row.recorded && (shell.snapshot?.unstaged ?? []).some((file) => file.path === row.path);
  const suffix = dirty ? shell.tr("chrome.submoduleDirtySuffix") : "";
  if (row.configuredBranch && row.branch && row.branch !== row.configuredBranch) {
    const note = shell.tr("chrome.submoduleBranchMismatch", { configured: row.configuredBranch, actual: row.branch }) + suffix;
    return { tone: "bad", note, title: `${note}\n${shell.tr("chrome.submoduleHealthBranch", { configured: row.configuredBranch })}` };
  }
  if (row.configuredBranch && !row.branch) {
    const note = shell.tr("chrome.submoduleDetached", { branch: row.configuredBranch }) + suffix;
    return { tone: "warn", note, title: `${note}\n${shell.tr("chrome.submoduleHealthDetached", { branch: row.configuredBranch })}` };
  }
  if (row.id !== row.recorded) {
    const note = shell.tr("chrome.submoduleCommitMismatch", { recorded: short(row.recorded), actual: short(row.id) }) + suffix;
    return { tone: "warn", note, title: `${note}\n${shell.tr("chrome.submoduleHealthCommit")}` };
  }
  if (dirty) {
    const note = shell.tr("chrome.submoduleDirty");
    return { tone: "warn", note, title: `${note}\n${shell.tr("chrome.submoduleHealthDirty")}` };
  }
  const note = row.branch ? shell.tr("chrome.submoduleHealthyBranch", { branch: row.branch }) : shell.tr("chrome.submoduleHealthyPinned", { id: short(row.id) });
  return { tone: "ok", note, title: `${note}\n${shell.tr("chrome.submoduleHealthOk")}` };
}

/** Flat sidebar rows. The view draws only the ones inside the scroll window. */
export function liveSideRows(shell: ShellState, limits: Record<PagedSection, number>): SideRow[] {
  const refs = shell.refView;
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
          upstream: branch.upstream ?? "",
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
    let folderOpen = true;
    let shown = 0;
    for (const tag of shell.visibleTags) {
      const group = branchGroup(tag.name);
      const newFolder = group !== previous;
      if (newFolder) {
        folderOpen = shell.folderOpen(group ? `tag:${group}` : "");
        previous = group;
      }
      if (folderOpen && shown === limits.tags) {
        rows.push({ key: "more:tags", kind: "more", section: "tags" });
        break;
      }
      if (group && newFolder) {
        rows.push({ key: `tag-folder:${group}`, kind: "folder", folderKey: `tag:${group}`, label: group, deep: false });
      }
      if (!folderOpen) continue;
      rows.push({
        key: `tag:${tag.name}`,
        kind: "tag",
        name: tag.name,
        label: group ? tag.name.slice(group.length + 1) : tag.name,
        deep: group.length > 0,
      });
      shown++;
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
    let shown = 0;
    for (const stash of refs.stashes) {
      if (!shell.matchesQuery(stash.summary, query) && !shell.matchesQuery(stash.name, query)) continue;
      if (shown === limits.stashes) {
        rows.push({ key: "more:stashes", kind: "more", section: "stashes" });
        break;
      }
      rows.push({ key: `stash:${stash.name}`, kind: "stash", name: stash.name, summary: stash.summary, id: stash.id });
      shown++;
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
      const health = submoduleHealth(shell, row);
      rows.push({ key: `submodule:${row.path}`, kind: "submodule", path: row.path, ready: row.ready, tone: health.tone, title: health.title });
      rows.push({ key: `submodule-note:${row.path}`, kind: "submoduleNote", path: row.path, tone: health.tone, note: health.note, title: health.title });
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
        rows.push({ key: `worktree:${row.path}`, kind: "worktree", worktree: row });
      }
    }
  }

  return rows;
}
