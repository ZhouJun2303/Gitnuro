import type { ShellState } from "./state.svelte";
import { branchGroup, orderRefNames } from "../view";
import type { SubmoduleRemoteCommit, SubmoduleRow, WorktreeRow } from "../status";

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

export function submoduleHealth(shell: ShellState, row: SubmoduleRow): SubmoduleHealth {
  const short = (id: string) => (id ? id.slice(0, 7) : "—");
  const details = [
    shell.tr("chrome.submodulePathValue", { path: row.path }),
    shell.tr("chrome.submoduleIndexValue", { id: row.recorded || "—" }),
    shell.tr("chrome.submoduleCheckoutValue", { id: row.id || "—", branch: row.branch || (row.ready ? "HEAD" : "—") }),
  ];
  const issues: { tone: SubmoduleTone; note: string }[] = [];
  const add = (tone: SubmoduleTone, note: string, detail?: string) => {
    issues.push({ tone, note });
    if (detail) details.push(detail);
  };
  if (row.conflict) {
    add("bad", shell.tr("chrome.submoduleConflict"), shell.tr("chrome.submoduleConflictDetail", {
      base: row.conflict[0] ?? "—", ours: row.conflict[1] ?? "—", theirs: row.conflict[2] ?? "—",
    }));
  }
  if (row.brokenCheckout) {
    add("bad", shell.tr("chrome.submoduleBroken"), shell.tr("chrome.submoduleBrokenDetail"));
  } else if (!row.ready) {
    add("muted", shell.tr("chrome.submoduleUninitialized"), shell.tr("chrome.submoduleHealthUninitialized"));
  }
  if (row.configError) {
    add("bad", shell.tr("chrome.submoduleConfigIssue"), row.configError);
  }
  const remote = shell.submoduleRemote.repo === shell.jobKey(shell.repoPath()) ? shell.submoduleRemote : undefined;
  const result = remote?.rows.find((item) => item.path === row.path && item.configKey === row.configKey);
  const target = result?.target || row.configuredBranch;
  details.push(shell.tr("chrome.submoduleTargetValue", { branch: target || "—" }));
  if (row.inheritsBranch) {
    if (row.configuredBranch) details.push(shell.tr("chrome.submoduleInherited"));
    else add("warn", shell.tr("chrome.submoduleInheritedDetached"), shell.tr("chrome.submoduleInheritedDetachedDetail"));
  }
  if (result?.url) details.push(shell.tr("chrome.submoduleRemoteValue", { url: result.url }));
  const showRemote = (commit: SubmoduleRemoteCommit | undefined, id: string, role: string) => {
    if (!id) return;
    if (commit?.id !== id) {
      if (!remote?.pending && !result?.error && !remote?.error) {
        details.push(shell.tr("chrome.submoduleRemoteUnchecked", { role, id }));
      }
      return;
    }
    const values = { role, id, branch: target || "—", ref: commit.ref ? ` (${commit.ref})` : "" };
    switch (commit.state) {
      case "inTarget":
        add("ok", shell.tr("chrome.submoduleRemoteInTarget", values), shell.tr("chrome.submoduleRemoteInTargetDetail", values));
        break;
      case "outsideTarget":
        add("warn", shell.tr("chrome.submoduleRemoteOutside", values), shell.tr("chrome.submoduleRemoteOutsideDetail", values));
        break;
      case "targetMissing":
        add("warn", shell.tr("chrome.submoduleRemoteTargetMissing", values), shell.tr("chrome.submoduleRemoteTargetMissingDetail", values));
        break;
      case "reachable":
        add("ok", shell.tr("chrome.submoduleRemoteReachable", values), shell.tr("chrome.submoduleRemoteReachableDetail", values));
        break;
      case "unreachable":
        add("bad", shell.tr("chrome.submoduleRemoteUnreachable", values), shell.tr("chrome.submoduleRemoteUnreachableDetail", values));
        break;
    }
  };
  if (remote?.pending) details.push(shell.tr("chrome.submoduleRemoteChecking"));
  if (result?.error || remote?.error) {
    add("warn", shell.tr("chrome.submoduleRemoteFailed"), shell.tr("chrome.submoduleRemoteFailedDetail", { error: result?.error || remote?.error || "" }));
  }
  showRemote(result?.recorded, row.recorded, shell.tr("chrome.submoduleIndexRole"));
  if (row.id !== row.recorded) showRemote(result?.checkout, row.id, shell.tr("chrome.submoduleCheckoutRole"));

  if (row.ready && row.configuredBranch && row.branch && row.branch !== row.configuredBranch) {
    add("warn", shell.tr("chrome.submoduleBranchMismatch", { configured: row.configuredBranch, actual: row.branch }), shell.tr("chrome.submoduleHealthBranch"));
  } else if (row.ready && !row.branch && row.configuredBranch) {
    details.push(shell.tr("chrome.submoduleHealthDetached", { branch: row.configuredBranch }));
  }
  if (row.ready && row.recorded && row.id !== row.recorded) {
    add("warn", shell.tr("chrome.submoduleCommitMismatch", { recorded: short(row.recorded), actual: short(row.id) }), shell.tr("chrome.submoduleHealthCommit"));
  }
  const dirty = shell.snapshot?.unstaged.find((file) => file.path === row.path)?.submoduleDirty ?? 0;
  if (dirty > 0) {
    add("warn", shell.tr("chrome.submoduleDirty", { count: String(dirty) }), shell.tr("chrome.submoduleHealthDirty"));
  }
  const tone = (["bad", "warn", "muted", "ok"] as const).find((tone) => issues.some((issue) => issue.tone === tone)) ?? "ok";
  const local = row.branch ? shell.tr("chrome.submoduleHealthyBranch", { branch: row.branch }) : shell.tr("chrome.submoduleHealthyPinned", { id: short(row.id) });
  const notes = issues.map((issue) => issue.note);
  const note = notes.join(" · ") || local;
  return { tone, note, title: [note, ...details].join("\n") };
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
