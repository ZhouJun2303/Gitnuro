import { onMount, tick } from "svelte";
import { invoke } from "../perf";
import { commits, diffs, sampleImages, sampleNotices, samplePulls, sampleReflog, staged as sampleStaged, unstaged as sampleUnstaged, type FileChange } from "../sample";
import { branchGroup, changedRanges, commitGraph, forkPath, formatWhen, gravatarUrl, laneColors, markTokens, numberedDiff, splitDiff, windowSlice, type GraphCell, type SplitRow } from "../view";
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
  } from "../status";
import type { ShellState } from "./state.svelte";

export function currentFilterRefs(shell: ShellState) {
    const current = shell.refs?.branches.find((branch) => branch.current);
    if (!current) return [];
    const names = [current.name];
    if (current.upstream && !names.includes(current.upstream)) names.push(current.upstream);
    return names;
  }

export async function loadCommits(shell: ShellState, path: string | undefined) {
    const order = shell.settings.commitSort === "topo" ? "topo" : "date";
    if (shell.historyFilter) return invoke<CommitRow[]>("file_history", { path, file: shell.historyFilter, limit: 200 });
    if (!shell.allBranches && shell.currentFilterRefs().length > 0) {
      try {
        return await invoke<CommitRow[]>("commit_log", { path, limit: shell.logLimit, order, revs: shell.currentFilterRefs() });
      } catch {
        return invoke<CommitRow[]>("commit_log", { path, limit: shell.logLimit, all: false, order });
      }
    }
    return invoke<CommitRow[]>("commit_log", { path, limit: shell.logLimit, all: shell.allBranches || shell.currentFilterRefs().length === 0, order });
  }

export async function loadContext(shell: ShellState) {
    const path = shell.repoPath();
    const gen = ++shell.contextGen;
    const nextRefs = await invoke<RefSnapshot>("repository_refs", { path });
    if (gen !== shell.contextGen) return;
    if (!shell.same(shell.refs, nextRefs)) shell.refs = nextRefs;
    const [rows, stats, state] = await Promise.all([
      shell.loadCommits(path),
      invoke<NonNullable<({ branch: string; ahead: number; behind: number; commits: number; branches: number; lastSummary: string } | null)>>("repository_summary", { path }).catch(() => shell.repoStats),
      invoke<InProgress | null>("in_progress", { path }),
    ]);
    if (gen !== shell.contextGen) return;
    if (!shell.same(shell.commitsLive, rows)) shell.commitsLive = rows;
    if (!shell.same(shell.repoStats, stats)) shell.repoStats = stats;
    shell.signOff = nextRefs.signOffSet ? !!nextRefs.signOff : shell.settings.signOff;
    shell.progress = state;
  }

export async function previewCherry(shell: ShellState, rev: string, record: boolean) {
    const text = await invoke<string>("cherry_preview", { path: shell.repoPath(), rev }).catch(() => "");
    if (text.trim()) shell.ask(shell.tr("menu.cherryPick"), text, () => void shell.mutate({ action: "cherryPick", rev, record }));
    else void shell.mutate({ action: "cherryPick", rev, record });
  }

export function toggleFold(shell: ShellState, id: string) {
    const roots = shell.foldRoots.includes(id) ? shell.foldRoots.filter((item) => item !== id) : [...shell.foldRoots, id];
    shell.foldRoots = roots;
    const source = shell.commitsLive.map((commit) => ({ id: commit.id, parents: commit.parents ?? [] }));
    shell.foldHidden = roots.flatMap((root) => {
      const start = source.findIndex((row) => row.id === root);
      if (start < 0) return [];
      const hide: string[] = [];
      for (let index = start + 1; index < source.length; index += 1) {
        const parents = source[index]?.parents ?? [];
        const prev = source[index - 1]?.id ?? "";
        if (!parents.some((parent) => parent === prev || prev.startsWith(parent) || parent.startsWith(prev))) break;
        hide.push(source[index].id);
      }
      return hide;
    });
  }

export async function searchContent(shell: ShellState, kind: string, needle: string) {
    shell.contentHits = await invoke<{ id: string; shortId: string; summary: string }[]>("search_commits", { path: shell.repoPath(), kind, needle }).catch((error) => {
      shell.actionError = shell.message(error);
      return [];
    });
  }

export function refKind(shell: ShellState, label: string) {
    if (label === "stash" || label.startsWith("stash")) return "stash";
    if (label.includes("/")) return "remote";
    if (/^v?\d/.test(label)) return "tag";
    return "local";
  }

export function branchRefLabels(shell: ShellState, labels: string[]) {
    return labels.filter((label) => {
      const kind = shell.refKind(label);
      return kind === "local" || kind === "remote";
    });
  }

export function plainRefLabels(shell: ShellState, labels: string[]) {
    return labels.filter((label) => {
      const kind = shell.refKind(label);
      return kind !== "local" && kind !== "remote";
    });
  }

export function laneTone(shell: ShellState, row: GraphCell[] | undefined) {
    const node = row?.find((cell) => cell?.role.split(" ").includes("node"));
    return node ? laneColors[node.color] : undefined;
  }

export function isCommitUnpushed(shell: ShellState, commit: { unpushed?: boolean; refs?: string[]; badges?: string[]; id?: string }) {
    if (commit.unpushed === true) return true;
    if (commit.refs?.includes("stash") || commit.badges?.includes("stash") || commit.id?.startsWith("stash@")) return true;
    return false;
  }

export async function showReflog(shell: ShellState) {
    shell.reflogOn = !shell.reflogOn;
    shell.treeOn = false;
    if (!shell.reflogOn || shell.mode !== "live") return;
    shell.reflogRows = await invoke("reflog", { path: shell.repoPath(), limit: 200 });
  }

export async function openTreeFile(shell: ShellState, file: string) {
    if (!shell.selectedCommit) return;
    shell.historyFile = file;
    shell.blobTop = 0;
    const view = await invoke<{ binary: boolean; text: string; preview: FilePreview | null }>("blob_view", { path: shell.repoPath(), rev: shell.selectedCommit, file });
    if (shell.historyFile !== file) return;
    shell.treeBinary = view.binary && !view.preview && !view.text;
    if (view.preview) {
      shell.imageOld = null;
      shell.imageNew = view.preview.dataUrl;
      shell.preview = view.preview;
      shell.treeText = "";
    } else {
      shell.treeText = view.text;
      shell.preview = null;
    }
  }

export async function compareDrops(shell: ShellState) {
    if (shell.drops.length !== 2) return;
    shell.comparePair = [shell.drops[0], shell.drops[1]];
    shell.resetCommitView();
    const gen = shell.patchGen;
    await shell.withWork(shell.tr("chrome.readingCommit"), async () => {
      try {
        shell.commitFiles = await invoke<StatusFile[]>("compare_files", { path: shell.repoPath(), from: shell.drops[0], to: shell.drops[1] });
        if (gen !== shell.patchGen) return;
        shell.selectedCommit = shell.drops[1];
        shell.historyFile = shell.commitFiles[0]?.path ?? null;
        if (shell.historyFile) {
          await shell.pickCommitFile(shell.historyFile);
          if (gen === shell.patchGen) shell.openPatches = [shell.historyFile];
        }
      } catch (error) {
        if (gen === shell.patchGen) shell.actionError = shell.message(error);
      }
    });
  }

export async function selectCommit(shell: ShellState, id: string, event?: MouseEvent) {
    if (event?.shiftKey && shell.selectedCommit) {
      const ids = shell.shownCommits.map((commit) => commit.id);
      const start = ids.indexOf(shell.selectedCommit);
      const end = ids.indexOf(id);
      if (start >= 0 && end >= 0) {
        const [from, to] = start < end ? [start, end] : [end, start];
        shell.drops = ids.slice(from, to + 1);
      }
    }
    shell.comparePair = null;
    shell.selectedCommit = id;
    shell.blameLines = null;
    shell.historyTab = "commit";
    shell.resetCommitView();
    const gen = shell.patchGen;
    const row = shell.commitsLive.find((commit) => commit.id === id);
    await shell.withWork(shell.tr("chrome.readingCommit"), async () => {
    shell.commitDetail = {
      id,
      body: "",
      summary: row?.summary ?? "",
      author: row?.author ?? "",
      authorEmail: row?.email ?? "",
      authorAt: row?.at ?? 0,
      committer: row?.author ?? "",
      committerEmail: row?.email ?? "",
      committerAt: row?.at ?? 0,
      parents: row?.parents ?? [],
    };
    try {
      shell.commitDetail = await invoke<CommitDetail>("commit_detail", { path: shell.repoPath(), id });
    } catch (error) {
      shell.actionError = shell.message(error);
    }
    try {
      shell.commitFiles = await invoke<StatusFile[]>("commit_files", { path: shell.repoPath(), id });
      if (gen !== shell.patchGen) return;
      shell.historyFile = shell.commitFiles[0]?.path ?? null;
      shell.historyDiff = shell.historyFile
        ? await invoke<FileDiff>("show_commit_file", {
            path: shell.repoPath(),
            id,
            file: shell.historyFile,
            unified: shell.settings.showEntireFile ? 100000 : 3,
          })
        : null;
      if (gen !== shell.patchGen) return;
      if (shell.historyFile && shell.historyDiff) {
        shell.patchCache = { [shell.historyFile]: shell.historyDiff };
        shell.openPatches = [shell.historyFile];
      }
    } catch (error) {
      if (gen === shell.patchGen) shell.actionError = shell.message(error);
    }
    });
  }

export async function pickCommitFile(shell: ShellState, file: string) {
    const gen = shell.patchGen;
    shell.historyFile = file;
    const cached = shell.patchCache[file];
    if (cached) {
      shell.historyDiff = cached;
      return;
    }
    shell.historyDiff = null;
    if (!shell.comparePair && !shell.selectedCommit) return;
    await shell.withWork(shell.tr("chrome.readingFile"), async () => {
      try {
        const diff = shell.comparePair
          ? await invoke<FileDiff>("compare_file", {
              path: shell.repoPath(),
              from: shell.comparePair[0],
              to: shell.comparePair[1],
              file,
              unified: shell.settings.showEntireFile ? 100000 : 3,
            })
          : await invoke<FileDiff>("show_commit_file", {
              path: shell.repoPath(),
              id: shell.selectedCommit ?? "",
              file,
              unified: shell.settings.showEntireFile ? 100000 : 3,
            });
        if (gen !== shell.patchGen) return;
        shell.historyDiff = diff;
        shell.patchCache = { ...shell.patchCache, [file]: diff };
      } catch (error) {
        if (gen === shell.patchGen) shell.actionError = shell.message(error);
      }
    });
  }

export async function ensurePatch(shell: ShellState, file: string) {
    if (shell.mode !== "live" || shell.patchCache[file] || (!shell.selectedCommit && !shell.comparePair)) return;
    await shell.pickCommitFile(file);
  }

export function togglePatch(shell: ShellState, path: string) {
    if (shell.openPatches.includes(path)) {
      shell.openPatches = shell.openPatches.filter((item) => item !== path);
      return;
    }
    shell.openPatches = [...shell.openPatches, path];
    shell.historyFile = path;
    void shell.ensurePatch(path);
  }

export async function toggleAllPatches(shell: ShellState) {
    const paths = shell.commitFiles.map((file) => file.path);
    if (paths.length > 0 && paths.every((path) => shell.openPatches.includes(path))) {
      shell.openPatches = [];
      return;
    }
    shell.openPatches = paths;
    const gen = shell.patchGen;
    const pending = paths.filter((path) => !shell.patchCache[path]);
    const workers = Array.from({ length: Math.min(4, pending.length) }, async () => {
      while (gen === shell.patchGen && pending.length > 0) {
        const path = pending.shift();
        if (path) await shell.ensurePatch(path);
      }
    });
    await Promise.all(workers);
  }

export async function showFileHistory(shell: ShellState, file: string) {
    shell.historyFilter = file;
    shell.section = "history";
    try {
      shell.commitsLive = await invoke<CommitRow[]>("file_history", { path: shell.repoPath(), file, limit: 200 });
    } catch (error) {
      shell.actionError = shell.message(error);
    }
  }

export function forkWhen(shell: ShellState, at: number) {
    if (!at) return "";
    const date = new Date(at * 1000);
    const hours = String(date.getHours()).padStart(2, "0");
    const minutes = String(date.getMinutes()).padStart(2, "0");
    if (shell.activeLocale === "zh") return `${date.getDate()} ${date.getMonth() + 1}月 ${date.getFullYear()} ${hours}:${minutes}`;
    const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    return `${date.getDate()} ${months[date.getMonth()]} ${date.getFullYear()} ${hours}:${minutes}`;
  }

export function forkWhenFull(shell: ShellState, at: number) {
    const clock = shell.forkWhen(at);
    if (!clock) return "";
    const date = new Date(at * 1000);
    const seconds = String(date.getSeconds()).padStart(2, "0");
    const offset = -date.getTimezoneOffset();
    const sign = offset >= 0 ? "+" : "-";
    const abs = Math.abs(offset);
    const hours = String(Math.floor(abs / 60)).padStart(2, "0");
    const minutes = String(abs % 60).padStart(2, "0");
    return `${clock}:${seconds} ${sign}${hours}:${minutes}`;
  }

export function commitSubject(shell: ShellState) {
    return shell.commitDetail?.summary || (shell.mode === "live" ? shell.commitsLive.find((row) => row.id === shell.selectedCommit)?.summary : shell.sampleRows.find((row) => row.id === shell.selectedCommit)?.summary) || "";
  }

export function commitAuthorName(shell: ShellState) {
    return shell.commitDetail?.author || (shell.mode === "live" ? shell.commitsLive.find((row) => row.id === shell.selectedCommit)?.author : shell.sampleRows.find((row) => row.id === shell.selectedCommit)?.author) || "";
  }

export function commitWhenShort(shell: ShellState) {
    if (shell.commitDetail?.authorAt) return shell.forkWhen(shell.commitDetail.authorAt);
    if (shell.mode === "live") {
      const row = shell.commitsLive.find((item) => item.id === shell.selectedCommit);
      return row?.at ? shell.forkWhen(row.at) : (row?.when ?? "");
    }
    return shell.sampleRows.find((row) => row.id === shell.selectedCommit)?.when ?? "";
  }

export function commitRefs(shell: ShellState) {
    if (shell.mode === "live") return shell.commitsLive.find((row) => row.id === shell.selectedCommit)?.refs ?? [];
    return shell.sampleRows.find((row) => row.id === shell.selectedCommit)?.badges ?? [];
  }

export function differentCommitter(shell: ShellState, detail: CommitDetail | null) {
    if (!detail?.committer && !detail?.committerEmail) return false;
    return detail.author.trim().toLowerCase() !== detail.committer.trim().toLowerCase() || detail.authorEmail.trim().toLowerCase() !== detail.committerEmail.trim().toLowerCase();
  }

export function resetCommitView(shell: ShellState) {
    shell.patchGen += 1;
    shell.patchCache = {};
    shell.openPatches = [];
    shell.treePaths = [];
    shell.treeText = "";
    shell.treeCollapsed = [];
    shell.treePrepared = null;
    shell.treeBinary = false;
    shell.preview = null;
    shell.blobTop = 0;
    shell.commitCollapsed = [];
    shell.historyFile = null;
    shell.historyDiff = null;
  }

export function avatarColor(shell: ShellState, name: string) {
    const colors = ["#5b8def", "#e07a3d", "#3aa76d", "#c45b8a", "#7a6ad8", "#c9a227", "#d16464"];
    let hash = 0;
    for (const ch of name) hash = (hash + ch.charCodeAt(0)) % colors.length;
    return colors[hash] ?? colors[0];
  }

export function issueHref(shell: ShellState, token: string) {
    if (token.startsWith("http")) return token;
    if (!token.startsWith("#")) return "";
    const remote = shell.refs?.remotes.find((item) => item.url)?.url ?? "";
    const number = token.slice(1);
    const github = remote.match(/github\.com[:/]([^/\s]+\/[^/\s.]+)/i);
    if (github) return `https://github.com/${github[1].replace(/\.git$/, "")}/issues/${number}`;
    const gitlab = remote.match(/gitlab[^/:]*[:/]([^/\s]+\/[^/\s.]+)/i);
    if (gitlab) return `https://gitlab.com/${gitlab[1].replace(/\.git$/, "")}/-/issues/${number}`;
    return "";
  }

export function linkParts(shell: ShellState, text: string) {
    if (!shell.settings.highlightIssues) return [{ text, href: "" }];
    const parts: { text: string; href: string }[] = [];
    const pattern = /https?:\/\/[^\s]+|#\d+/g;
    let cursor = 0;
    for (const match of text.matchAll(pattern)) {
      const index = match.index ?? 0;
      if (index > cursor) parts.push({ text: text.slice(cursor, index), href: "" });
      const token = match[0];
      parts.push({ text: token, href: shell.issueHref(token) });
      cursor = index + token.length;
    }
    if (cursor < text.length) parts.push({ text: text.slice(cursor), href: "" });
    return parts.length > 0 ? parts : [{ text, href: "" }];
  }

export async function openHistoryTab(shell: ShellState, tab: "commit" | "changes" | "tree") {
    shell.historyTab = tab;
    if (!shell.selectedCommit) return;
    if (tab === "changes" && shell.historyFile) shell.commitCollapsed = shell.expandParents(shell.historyFile, shell.commitCollapsed);
    if (tab === "changes") {
      shell.revealSelectedRow();
      return;
    }
    if (tab !== "tree") return;
    if (shell.mode === "live" && shell.treePaths.length === 0) {
      const rev = shell.selectedCommit;
      try {
        shell.treePaths = await invoke<string[]>("commit_tree", { path: shell.repoPath(), rev });
      } catch (error) {
        shell.actionError = shell.message(error);
        return;
      }
      if (shell.selectedCommit !== rev) return;
    }
    const paths = shell.mode === "live" ? shell.treePaths : shell.sampleTree;
    if (shell.treePrepared !== shell.selectedCommit) {
      shell.treeCollapsed = shell.collapseTreeExcept(paths, shell.historyFile);
      shell.treePrepared = shell.selectedCommit;
    } else if (shell.historyFile) {
      shell.treeCollapsed = shell.expandParents(shell.historyFile, shell.treeCollapsed);
    }
    if (shell.mode === "live" && shell.historyFile && !shell.treeText && !shell.preview) void shell.openTreeFile(shell.historyFile);
    shell.revealSelectedRow();
  }

export function selectSample(shell: ShellState, id: string) {
    const commit = shell.sampleRows.find((row) => row.id === id);
    shell.selectedCommit = id;
    shell.selectedPath = id;
    shell.historyTab = "commit";
    shell.resetCommitView();
    shell.historyFile = commit?.files[0]?.path ?? null;
    shell.commitFiles = (commit?.files ?? []).map((file) => ({ path: file.path, kind: file.kind === "A" ? "added" : file.kind === "D" ? "deleted" : "modified", previousPath: null }));
    const at = Math.floor(Date.now() / 1000) - 7200;
    shell.commitDetail = {
      id,
      summary: commit?.summary ?? "",
      body: "",
      author: commit?.author ?? "",
      authorEmail: "jun@example.com",
      authorAt: at,
      committer: commit?.author ?? "",
      committerEmail: "jun@example.com",
      committerAt: at,
      parents: commit?.parents ?? [],
    };
    shell.openPatches = shell.historyFile ? [shell.historyFile] : [];
  }

export function whenLabel(shell: ShellState, commit: CommitRow) {
    if (shell.settings.dateRelative) return commit.when;
    return formatWhen(commit.at, shell.settings.dateFormat.trim() || "dd MMM yyyy", shell.settings.date24h) || commit.when;
  }

export function scrollToCommit(shell: ShellState, id: string) {
    const index = shell.historyItems.findIndex((commit) => commit.id === id);
    if (index < 0) return;
    const view = shell.historyEl?.clientHeight ?? 0;
    const top = Math.max(0, index * shell.rowCommit - Math.max(0, view / 2 - shell.rowCommit));
    shell.historyTop = top;
    shell.historyEl?.scrollTo({ top });
  }

export function openHistory(shell: ShellState) {
    shell.section = "history";
    shell.reflogOn = false;
    shell.treeOn = false;
  }

export async function locateRef(shell: ShellState, name: string) {
    if (shell.mode !== "live") return;
    // A double click fires two clicks, then checkout. Only the newest locate keeps going.
    const seq = ++shell.locateSeq;
    shell.openHistory();
    if (shell.commitQuery.trim()) shell.commitQuery = "";
    if (shell.historyFilter) shell.historyFilter = "";
    shell.allBranches = true;
    shell.actionError = null;
    await shell.withWork(shell.tr("chrome.locating", { name }), async () => {
      try {
        const found = await invoke<{ id: string; commits: CommitRow[] }>("locate_ref", {
          path: shell.repoPath(),
          name,
          order: shell.settings.commitSort === "topo" ? "topo" : "date",
        });
        if (seq !== shell.locateSeq) return;
        shell.logLimit = Math.max(shell.logLimit, found.commits.length);
        if (!shell.same(shell.commitsLive, found.commits)) shell.commitsLive = found.commits;
        await tick();
        await shell.selectCommit(found.id);
        await tick();
        shell.scrollToCommit(found.id);
      } catch (error) {
        if (seq !== shell.locateSeq) return;
        shell.actionError = shell.message(error);
      }
    });
  }

export async function locateStash(shell: ShellState, stash: { name: string; id?: string }) {
    if (shell.mode !== "live") return;
    shell.openHistory();
    const id = stash.id || stash.name;
    await shell.withWork(shell.tr("chrome.locating", { name: stash.name }), async () => {
      await tick();
      await shell.selectCommit(id);
      await tick();
      shell.scrollToCommit(id);
    });
  }

export function revealHead(shell: ShellState) {
    shell.section = "history";
    const name = shell.snapshot?.branch;
    const index = shell.shownCommits.findIndex((commit) => (name ? commit.refs.includes(name) : commit.refs.length > 0));
    const top = Math.max(0, index) * shell.rowCommit;
    shell.historyTop = top;
    shell.historyEl?.scrollTo({ top });
  }

export function dropCommits(shell: ShellState) {
    const chosen = shell.shownCommits.filter((commit) => shell.drops.includes(commit.id));
    if (chosen.length === 0) return;
    const oldest = chosen[chosen.length - 1];
    const parent = oldest.parents[0];
    if (!parent) {
      shell.actionError = "The root commit cannot be dropped.";
      return;
    }
    shell.drops = [];
    void shell.mutate({ action: "rebaseInteractive", onto: parent, drop: chosen.map((commit) => commit.id), autostash: shell.settings.mergeAutostash });
  }

export function toggleDrop(shell: ShellState, id: string) {
    shell.drops = shell.drops.includes(id) ? shell.drops.filter((item) => item !== id) : [...shell.drops, id];
  }

function patchScroller(node: HTMLElement) {
  const detail = node.closest(".detail-scroll");
  if (detail instanceof HTMLElement) return detail;
  let parent = node.parentElement;
  while (parent) {
    const style = getComputedStyle(parent);
    const scrollable = style.overflowY === "auto" || style.overflowY === "scroll";
    if (scrollable && parent.scrollHeight > parent.clientHeight + 1) return parent;
    parent = parent.parentElement;
  }
  return node;
}

/** Keep only the rows of an open commit patch that sit in the detail viewport. */
export function trackPatch(shell: ShellState, node: HTMLElement, spec: { file: string; count: number; rowHeight: number }) {
  let current = spec;
  let scroller: HTMLElement | null = null;
  const update = () => {
    const nextScroller = patchScroller(node);
    if (scroller !== nextScroller) {
      scroller?.removeEventListener("scroll", update);
      scroller = nextScroller;
      scroller.addEventListener("scroll", update, { passive: true });
    }
    const rowHeight = current.rowHeight || 18;
    const count = current.count;
    const top = node.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
    const viewTop = scroller.scrollTop;
    const viewBottom = viewTop + (scroller.clientHeight || 600);
    let start = Math.max(0, Math.floor((viewTop - top) / rowHeight) - 30);
    let end = Math.min(count, Math.ceil((viewBottom - top) / rowHeight) + 30);
    if (end < 0) end = 0;
    if (start > count) start = count;
    if (end < start) end = start;
    const prev = shell.patchView[current.file];
    if (prev && prev.start === start && prev.end === end) return;
    shell.patchView = { ...shell.patchView, [current.file]: { start, end } };
  };
  update();
  const resize = new ResizeObserver(update);
  resize.observe(node);
  const host = node.closest(".detail-scroll") ?? node.parentElement;
  if (host instanceof HTMLElement && host !== node) resize.observe(host);
  return {
    update(next: { file: string; count: number; rowHeight: number }) {
      current = next;
      update();
    },
    destroy() {
      scroller?.removeEventListener("scroll", update);
      resize.disconnect();
      if (!(current.file in shell.patchView)) return;
      const next = { ...shell.patchView };
      delete next[current.file];
      shell.patchView = next;
    },
  };
}
