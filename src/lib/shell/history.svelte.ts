import { onMount, tick } from "svelte";
import { invoke } from "../perf";
import { commits, diffs, sampleImages, sampleNotices, samplePulls, sampleReflog, staged as sampleStaged, unstaged as sampleUnstaged, type FileChange } from "../sample";
import { branchGroup, changedRanges, formatWhen, gravatarUrl, laneColors, markTokens, numberedDiff, rowAtUnit, sameHistory, splitDiff, windowSlice, type GraphCell, type SplitRow } from "../view";
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
    type MergeState,
    type RefSnapshot,
    type Settings,
    type StatusFile,
    type StatusSnapshot,
    type Suggestion,
    type UpdateNotice,
  } from "../status";
import type { StoredError } from "./types";
import type { ShellState } from "./state.svelte";

const ERROR_KEY = "awegit-errors";
const PATCH_CACHE_LIMIT = 100;

function cachePatch(shell: ShellState, file: string, diff: FileDiff) {
    if (file in shell.patchCache || Object.keys(shell.patchCache).length < PATCH_CACHE_LIMIT) {
      shell.patchCache = { ...shell.patchCache, [file]: diff };
      return;
    }
    const evicted = Object.keys(shell.patchCache)[0];
    const next: Record<string, FileDiff> = {};
    for (const [key, value] of Object.entries(shell.patchCache)) {
      if (key === evicted) continue;
      next[key] = value;
    }
    next[file] = diff;
    shell.patchCache = next;
    if (evicted && shell.patchHeights[evicted] !== undefined) {
      const heights = { ...shell.patchHeights };
      delete heights[evicted];
      shell.patchHeights = heights;
    }
  }

export function readErrorLog(): StoredError[] {
    if (typeof localStorage === "undefined") return [];
    try {
      const parsed = JSON.parse(localStorage.getItem(ERROR_KEY) ?? "[]") as unknown;
      if (!Array.isArray(parsed)) return [];
      return parsed.flatMap((item) => {
        if (!item || typeof item !== "object") return [];
        const row = item as Partial<StoredError>;
        if (typeof row.at !== "number" || typeof row.text !== "string" || typeof row.id !== "number") return [];
        return [{ id: row.id, at: row.at, title: typeof row.title === "string" ? row.title : "", text: row.text, command: typeof row.command === "string" ? row.command : "" }];
      }).slice(0, 200);
    } catch {
      return [];
    }
  }

export function writeErrorLog(items: StoredError[]) {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(ERROR_KEY, JSON.stringify(items.slice(0, 200)));
  }

export function currentFilterRefs(shell: ShellState) {
    const current = shell.refs?.branches.find((branch) => branch.current);
    if (!current) return [];
    const names = [current.name];
    if (current.upstream && !names.includes(current.upstream)) names.push(current.upstream);
    return names;
  }

export async function loadCommits(shell: ShellState, path: string | undefined, operation?: string) {
    const order = shell.settings.commitSort === "topo" ? "topo" : "date";
    const query = shell.sideView === "search" ? shell.commitSearch.trim() : "";
    if (query && path) {
      return invoke<CommitRow[]>("search_log", {
        path,
        field: shell.commitSearchField,
        scope: shell.commitSearchScope,
        query,
        limit: shell.logLimit,
        order,
      }, undefined, operation);
    }
    if (shell.historyFilter) return invoke<CommitRow[]>("file_history", { path, file: shell.historyFilter, limit: 200 }, undefined, operation);
    const revs = shell.exactRefs();
    if (!shell.allBranches && revs.length > 0) {
      try {
        return await invoke<CommitRow[]>("commit_log", { path, limit: shell.logLimit, order, revs }, undefined, operation);
      } catch {
        return invoke<CommitRow[]>("commit_log", { path, limit: shell.logLimit, all: false, order }, undefined, operation);
      }
    }
    return invoke<CommitRow[]>("commit_log", { path, limit: shell.logLimit, all: shell.allBranches || revs.length === 0, order }, undefined, operation);
  }

const contextReads = new WeakMap<ShellState, { key: string; promise: Promise<void> }>();

export function loadContext(shell: ShellState, operation?: string): Promise<void> {
    // Only request parameters and a data generation, never serialize history/state.
    const key = JSON.stringify([shell.repoPath(), shell.statusGen, shell.settings.commitSort, shell.sideView, shell.commitSearch, shell.commitSearchField, shell.commitSearchScope, shell.historyFilter, shell.logLimit, shell.allBranches, shell.exactRefs()]);
    const current = contextReads.get(shell);
    if (current?.key === key) return current.promise;
    const promise = readContext(shell, operation).finally(() => {
      if (contextReads.get(shell)?.promise === promise) contextReads.delete(shell);
    });
    contextReads.set(shell, { key, promise });
    return promise;
}

async function readContext(shell: ShellState, operation?: string) {
    const path = shell.repoPath();
    const statusGen = shell.statusGen;
    const openGen = shell.openGen;
    const gen = ++shell.contextGen;
    const nextRefs = await invoke<RefSnapshot>("repository_refs", { path }, undefined, operation);
    if (gen !== shell.contextGen || statusGen !== shell.statusGen || openGen !== shell.openGen || !shell.viewing(path)) return;
    const { revision, ...visibleRefs } = nextRefs;
    if (!shell.same(shell.refs, visibleRefs)) shell.refs = visibleRefs;
    const [rows, state, merge] = await Promise.all([
      shell.loadCommits(path, operation),
      invoke<InProgress | null>("in_progress", { path }, undefined, operation),
      invoke<MergeState | null>("merge_state", { path }, undefined, operation).catch(() => null),
    ]);
    if (gen !== shell.contextGen || statusGen !== shell.statusGen || openGen !== shell.openGen || !shell.viewing(path)) return;
    shell.contextRevision = revision ?? 0;
    if (!sameHistory(shell.commitsLive, rows)) shell.commitsLive = rows;
    shell.signOff = nextRefs.signOffSet ? !!nextRefs.signOff : shell.settings.signOff;
    shell.progress = state;
    const nextMerge = state === "merge" ? merge : null;
    if (!sameMergeState(shell.mergeState, nextMerge)) shell.mergeState = nextMerge;
    if (nextMerge) fillMergeProgress(shell, nextMerge);
    else clearMergeProgress(shell);
  }

function sameMergeState(left: MergeState | null, right: MergeState | null) {
    if (left === right) return true;
    if (!left || !right) return false;
    if (left.source !== right.source || left.into !== right.into || left.files.length !== right.files.length) return false;
    return left.files.every((file, index) => file.status === right.files[index]?.status && file.path === right.files[index]?.path);
  }

/** Fill the composer for a paused merge without replacing a draft the user wrote. */
function fillMergeProgress(shell: ShellState, state: MergeState) {
    const summary = shell.tr("dialog.mergeTitleLine", { source: state.source, into: state.into });
    if (!shell.summary.trim() || shell.summary === shell.mergeProgressAutoSummary) shell.summary = summary;
    shell.mergeProgressAutoSummary = summary;
  }

function clearMergeProgress(shell: ShellState) {
    if (shell.mergeProgressAutoSummary && shell.summary === shell.mergeProgressAutoSummary) shell.summary = "";
    shell.mergeProgressAutoSummary = "";
  }


export async function previewCherry(shell: ShellState, rev: string, record: boolean, summary = "") {
    const text = await invoke<string>("cherry_preview", { path: shell.repoPath(), rev }).catch(() => "");
    const preview = text.trim().replace(/^[0-9a-f]{40,64}\s*/i, "").trim();
    const target = summary ? `${rev.slice(0, 7)} "${summary}"` : rev.slice(0, 7);
    const note = record ? ` ${shell.tr("dialog.cherryPickRecordNote")}` : "";
    const intro = `${shell.tr("dialog.cherryPickBody")}${note}`;
    const title = shell.tr(record ? "menu.cherryPickRecord" : "menu.cherryPick");
    shell.ask(title, intro, () => void shell.mutate({ action: "cherryPick", rev, record }), [], { subject: target, detail: preview });
  }

export function toggleFold(shell: ShellState, id: string) {
    const owner = (shell.mode === "sample" ? shell.sampleFold : shell.liveFold).owner.get(id);
    if (!owner) return;
    shell.expandedMerges = shell.expandedMerges.includes(owner) ? shell.expandedMerges.filter((item) => item !== owner) : [...shell.expandedMerges, owner];
  }

export function showSideRefs(shell: ShellState) {
    const restore = shell.sideView === "search" && shell.commitSearch.trim() !== "";
    shell.sideView = "refs";
    shell.commitSearchOpen = false;
    shell.commitSearchGen += 1;
    window.clearTimeout(shell.commitSearchTimer);
    void invoke("cancel_reads", { path: shell.repoPath() }).catch(() => {});
    void tick().then(() => shell.sideFilter?.focus());
    if (restore && shell.mode === "live") void shell.loadContext();
  }

export function showSideSearch(shell: ShellState) {
    shell.sideView = "search";
    shell.section = "history";
    shell.commitSearchOpen = false;
    void tick().then(() => shell.commitSearchEl?.focus());
    if (shell.commitSearch.trim() && shell.mode === "live") queueCommitSearch(shell);
  }

export function queueCommitSearch(shell: ShellState) {
    window.clearTimeout(shell.commitSearchTimer);
    const gen = ++shell.commitSearchGen;
    void invoke("cancel_reads", { path: shell.repoPath() }).catch(() => {});
    shell.commitSearchTimer = window.setTimeout(() => {
      if (gen !== shell.commitSearchGen || shell.sideView !== "search" || shell.mode !== "live") return;
      void refreshSearchedCommits(shell, gen);
    }, 200);
  }

export function pickCommitSearch(shell: ShellState, field: "message" | "author" | "path" | "diff") {
    shell.commitSearchField = field;
    shell.commitSearchOpen = false;
    queueCommitSearch(shell);
    void tick().then(() => shell.commitSearchEl?.focus());
  }

export function pickCommitScope(shell: ShellState, scope: "repository" | "branch") {
    shell.commitSearchScope = scope;
    shell.commitSearchOpen = false;
    queueCommitSearch(shell);
    void tick().then(() => shell.commitSearchEl?.focus());
  }

async function refreshSearchedCommits(shell: ShellState, gen: number) {
    const path = shell.repoPath();
    try {
      const rows = await loadCommits(shell, path);
      if (gen !== shell.commitSearchGen) return;
      if (!sameHistory(shell.commitsLive, rows)) shell.commitsLive = rows;
    } catch (error) {
      if (gen !== shell.commitSearchGen) return;
      shell.actionError = shell.message(error);
    }
  }

export async function searchContent(shell: ShellState, kind: string, needle: string) {
    shell.contentHits = await invoke<{ id: string; shortId: string; summary: string }[]>("search_commits", { path: shell.repoPath(), kind, needle }).catch((error) => {
      shell.actionError = shell.message(error);
      return [];
    });
  }

/** `On <branch>: …` and `WIP on <branch>: …` split into a branch plate and a stash plate. */
export function stashParts(summary: string, refs: string[] = []): { branch: string; message: string } | null {
  const match = /^(?:WIP )?on ([^:]+): (.*)$/i.exec(summary.trim());
  if (match) return { branch: match[1].trim(), message: match[2] };
  if (refs.some((label) => label === "stash" || label.startsWith("stash"))) return { branch: "", message: summary };
  return null;
}

let refKindCache: { refs: RefSnapshot | null; branches: Set<string>; tags: Set<string> } = {
  refs: null,
  branches: new Set(),
  tags: new Set(),
};

export function refKind(shell: ShellState, label: string) {
    if (label === "stash" || label.startsWith("stash")) return "stash";
    if (refKindCache.refs !== shell.refs) {
      refKindCache = {
        refs: shell.refs,
        branches: new Set((shell.refs?.branches ?? []).map((branch) => branch.name)),
        tags: new Set((shell.refs?.tags ?? []).map((tag) => tag.name)),
      };
    }
    if (refKindCache.branches.has(label)) return "local";
    if (refKindCache.tags.has(label)) return "tag";
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

function nodeColor(row: GraphCell[] | undefined) {
    const node = row?.find((cell) => cell?.role.split(" ").includes("node"));
    return node ? node.color : null;
  }

/** Lane color of the selected commit, or of the current branch tip when nothing is selected. */
export function activeLaneColor(shell: ShellState): number | null {
    const sample = shell.mode === "sample";
    const rows = sample ? shell.sampleShownGraph.rows : shell.liveGraph.rows;
    const items = sample ? shell.sampleShown : shell.historyItems;
    let index = shell.selectedCommit ? items.findIndex((row) => row.id === shell.selectedCommit) : -1;
    if (index < 0) {
      if (sample) index = shell.sampleShown.findIndex((row) => row.badges.includes("main"));
      else if (shell.currentBranch?.name) {
        const name = shell.currentBranch.name;
        index = shell.historyItems.findIndex((row) => row.refs.includes(name));
      }
    }
    if (index < 0 || index >= rows.length) return null;
    return nodeColor(rows[index]);
  }

/** Revisions the history should walk while “checked branches only” is on. */
export function exactRefs(shell: ShellState): string[] {
    const visible = (name: string, forced: boolean) => forced || !shell.refHidden(name);
    if (shell.graphSolo) {
      const names = [shell.graphSolo];
      const local = shell.refs?.branches.find((branch) => branch.name === shell.graphSolo);
      if (local?.upstream && !names.includes(local.upstream)) names.push(local.upstream);
      return names.filter((name, index) => visible(name, index === 0));
    }
    return shell.currentFilterRefs().filter((name, index) => visible(name, index === 0));
  }

export function soloBranch(shell: ShellState, name: string) {
    shell.graphSolo = shell.graphSolo === name ? null : name;
    shell.allBranches = false;
    if (shell.mode === "live") void shell.loadContext();
  }

export function toggleHistoryScope(shell: ShellState) {
    shell.allBranches = !shell.allBranches;
    if (shell.allBranches) shell.graphSolo = null;
    if (shell.mode === "live") void shell.loadContext();
  }

export function clearHistoryFilter(shell: ShellState) {
    shell.allBranches = true;
    shell.graphSolo = null;
    shell.historyFilter = "";
    if (shell.mode === "live") void shell.loadContext();
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
    const view = await invoke<{ binary: boolean; text: string; preview: FilePreview | null; truncated?: boolean }>("blob_view", { path: shell.repoPath(), rev: shell.selectedCommit, file });
    if (shell.historyFile !== file) return;
    shell.treeBinary = view.binary && !view.preview && !view.text;
    shell.treeTruncated = view.truncated === true;
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
        if (shell.historyFile) await shell.pickCommitFile(shell.historyFile);
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
    const rowAt = shell.historyItems.findIndex((row) => row.id === id);
    if (rowAt >= 0) shell.selectedIndex = rowAt;
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
      const detailTask = invoke<CommitDetail>("commit_detail", { path: shell.repoPath(), id })
        .then((detail) => {
          if (gen !== shell.patchGen || shell.selectedCommit !== id) return;
          shell.commitDetail = detail;
        })
        .catch((error) => {
          if (gen === shell.patchGen && shell.selectedCommit === id) shell.actionError = shell.message(error);
        });
      const files = await invoke<StatusFile[]>("commit_files", { path: shell.repoPath(), id });
      if (gen !== shell.patchGen || shell.selectedCommit !== id) {
        await detailTask;
        return;
      }
      shell.commitFiles = files;
      const file = files[0]?.path ?? null;
      shell.historyFile = file;
      const diff = file
        ? await invoke<FileDiff>("show_commit_file", {
            path: shell.repoPath(),
            id,
            file,
            unified: shell.settings.showEntireFile ? 100000 : (shell.settings.diffContext ?? 3),
          })
        : null;
      if (gen !== shell.patchGen || shell.selectedCommit !== id || shell.historyFile !== file) {
        await detailTask;
        return;
      }
      shell.historyDiff = diff;
      if (file && diff) {
        shell.patchCache = { [file]: diff };
        shell.patchVersion += 1;
        shell.patchHeights = {};
        shell.openPatches = [];
      }
      await detailTask;
    } catch (error) {
      if (gen === shell.patchGen && shell.selectedCommit === id) shell.actionError = shell.message(error);
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
              unified: shell.settings.showEntireFile ? 100000 : (shell.settings.diffContext ?? 3),
            })
          : await invoke<FileDiff>("show_commit_file", {
              path: shell.repoPath(),
              id: shell.selectedCommit ?? "",
              file,
              unified: shell.settings.showEntireFile ? 100000 : (shell.settings.diffContext ?? 3),
            });
        if (gen !== shell.patchGen) return;
        cachePatch(shell, file, diff);
        shell.patchVersion += 1;
        if (shell.historyFile === file) {
          shell.historyDiff = diff;
          // The placeholder height must not clip the loaded patch.
          if (shell.patchHeights[file] !== undefined) {
            const heights = { ...shell.patchHeights };
            delete heights[file];
            shell.patchHeights = heights;
          }
        }
      } catch (error) {
        if (gen === shell.patchGen && shell.historyFile === file) shell.actionError = shell.message(error);
      }
    });
  }

export async function ensurePatch(shell: ShellState, file: string) {
    if (shell.mode !== "live" || shell.patchCache[file] || (!shell.selectedCommit && !shell.comparePair)) return;
    await shell.pickCommitFile(file);
  }

export function togglePatch(shell: ShellState, path: string) {
    if (shell.openPatchSet.has(path)) {
      shell.openPatches = shell.openPatches.filter((item) => item !== path);
      return;
    }
    shell.openPatches = [...shell.openPatches, path];
    shell.historyFile = path;
    shell.requestPatch(path);
  }

export async function toggleAllPatches(shell: ShellState) {
    const paths = shell.commitShown.map((file) => file.path);
    if (paths.length > 0 && paths.every((path) => shell.openPatchSet.has(path))) {
      shell.openPatches = [];
      return;
    }
    // Opening every file only mounts the visible patches; each body loads as it scrolls in.
    shell.openPatches = paths;
    const first = shell.historyFile ?? paths[0];
    if (first) shell.requestPatch(first);
  }

export async function showFileHistory(shell: ShellState, file: string) {
    shell.historyFilter = file;
    shell.section = "history";
    try {
      const rows = await invoke<CommitRow[]>("file_history", { path: shell.repoPath(), file, limit: 200 });
      shell.commitsLive = rows;
      shell.historyTop = 0;
      const first = rows[0];
      if (first) {
        await shell.selectCommit(first.id);
        if (shell.historyFilter === file) await shell.pickCommitFile(file);
      } else {
        shell.selectedCommit = null;
        shell.historyDiff = null;
      }
    } catch (error) {
      shell.actionError = shell.message(error);
    }
  }

export function formatCommitDate(shell: ShellState, at: number, fallback = "") {
    if (!at) return fallback;
    if (shell.settings.dateRelative && fallback) return fallback;
    let base = shell.settings.dateFormat.trim() || (shell.activeLocale === "zh" ? "yyyy年MM月dd日 HH:mm" : "dd MMM yyyy HH:mm");
    if (!/[Hh]{2}[:.]?mm/.test(base)) base += shell.settings.date24h ? " HH:mm" : " hh:mm A";
    const pattern = shell.settings.date24h ? base : base.replace(/HH:mm/g, "hh:mm A");
    return formatWhen(at, pattern, shell.settings.date24h) || fallback;
  }

export function forkWhen(shell: ShellState, at: number) {
    return formatCommitDate(shell, at);
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
    return shell.commitDetail?.summary || (shell.mode === "live" ? shell.selectedCommitRow?.summary : shell.sampleRows.find((row) => row.id === shell.selectedCommit)?.summary) || "";
  }

export function commitAuthorName(shell: ShellState) {
    return shell.commitDetail?.author || (shell.mode === "live" ? shell.selectedCommitRow?.author : shell.sampleRows.find((row) => row.id === shell.selectedCommit)?.author) || "";
  }

export function commitWhenShort(shell: ShellState) {
    if (shell.commitDetail?.authorAt) return shell.forkWhen(shell.commitDetail.authorAt);
    if (shell.mode === "live") {
      const row = shell.selectedCommitRow;
      return row?.at ? shell.forkWhen(row.at) : (row?.when ?? "");
    }
    return shell.sampleRows.find((row) => row.id === shell.selectedCommit)?.when ?? "";
  }

export function commitRefs(shell: ShellState) {
    if (shell.mode === "live") return shell.selectedCommitRow?.refs ?? [];
    return shell.sampleRows.find((row) => row.id === shell.selectedCommit)?.badges ?? [];
  }

/** Source and target branch names for a merge commit, when its message names them. */
export function mergeRoute(shell: ShellState) {
    const detail = shell.commitDetail;
    if (!detail || detail.parents.length < 2) return null;
    const summary = detail.summary.trim();
    let match = /^Merge (?:remote-tracking )?branch '([^']+)'(?:\s+of\s+\S+)?(?:\s+into\s+(.+))?$/i.exec(summary);
    if (!match) match = /^Merge pull request #\d+ from (.+?)(?:\s+into\s+(.+))?$/i.exec(summary);
    if (!match) match = /^(?:合并|压缩合并)\s+(.+?)\s+到\s+(.+)$/.exec(summary);
    if (!match) match = /^Merge\s+(.+?)\s+into\s+(.+)$/i.exec(summary);
    if (!match) return null;
    const source = (match[1] ?? "").trim();
    if (!source) return null;
    const into = (match[2] ?? "").trim() || shell.commitRefs()[0] || "";
    return { source, into };
  }

export function differentCommitter(shell: ShellState, detail: CommitDetail | null) {
    if (!detail?.committer && !detail?.committerEmail) return false;
    return detail.author.trim().toLowerCase() !== detail.committer.trim().toLowerCase() || detail.authorEmail.trim().toLowerCase() !== detail.committerEmail.trim().toLowerCase();
  }

export function resetCommitView(shell: ShellState) {
    shell.patchGen += 1;
    shell.patchCache = {};
    shell.patchVersion += 1;
    shell.patchHeights = {};
    shell.openPatches = [];
    shell.clearPatchQueue();
    shell.treePaths = [];
    shell.treeText = "";
    shell.treeCollapsed = [];
    shell.treePrepared = null;
    shell.treeBinary = false;
    shell.treeTruncated = false;
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
    shell.openPatches = [];
  }

export function whenLabel(shell: ShellState, commit: CommitRow) {
    return formatCommitDate(shell, commit.at, commit.when);
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
    shell.actionError = null;
    // A branch or tag resolves to a filtered log page. `locate_ref` would otherwise
    // walk and materialise every commit above it just to scroll to one row.
    const namedRef =
      (shell.refs?.branches.some((branch) => branch.name === name) ?? false) ||
      (shell.refs?.tags.some((tag) => tag.name === name) ?? false) ||
      (shell.refs?.remotes.some((remote) => name === remote.name || name.startsWith(`${remote.name}/`)) ?? false);
    if (!shell.allBranches || namedRef) {
      shell.allBranches = false;
      shell.graphSolo = name;
      await shell.withWork(shell.tr("chrome.locating", { name }), async () => {
        try {
          const rows = await invoke<CommitRow[]>("commit_log", {
            path: shell.repoPath(),
            limit: shell.logLimit,
            order: shell.settings.commitSort === "topo" ? "topo" : "date",
            revs: shell.exactRefs(),
          });
          if (seq !== shell.locateSeq) return;
          if (!sameHistory(shell.commitsLive, rows)) shell.commitsLive = rows;
          await tick();
          const tip = rows.find((commit) => commit.refs.includes(name)) ?? rows[0];
          if (!tip) return;
          await shell.selectCommit(tip.id);
          await tick();
          shell.scrollToCommit(tip.id);
        } catch (error) {
          if (seq !== shell.locateSeq) return;
          shell.actionError = shell.message(error);
        }
      });
      return;
    }
    await shell.withWork(shell.tr("chrome.locating", { name }), async () => {
      try {
        const found = await invoke<{ id: string; commits: CommitRow[] }>("locate_ref", {
          path: shell.repoPath(),
          name,
          order: shell.settings.commitSort === "topo" ? "topo" : "date",
        });
        if (seq !== shell.locateSeq) return;
        shell.logLimit = Math.min(20_000, Math.max(shell.logLimit, found.commits.length));
        if (!sameHistory(shell.commitsLive, found.commits)) shell.commitsLive = found.commits;
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
    void shell.mutate({ action: "rebaseInteractive", onto: parent, drop: chosen.map((commit) => commit.id), autostash: true });
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
export function trackPatch(shell: ShellState, node: HTMLElement, spec: { file: string; count: number; rowHeight: number; prefix?: Uint32Array | null }) {
  let current = spec;
  let scroller: HTMLElement | null = null;
  let frame = 0;
  const measure = () => {
    const nextScroller = patchScroller(node);
    if (scroller !== nextScroller) {
      scroller?.removeEventListener("scroll", schedule);
      scroller = nextScroller;
      scroller.addEventListener("scroll", schedule, { passive: true });
    }
    const rowHeight = current.rowHeight || 18;
    const count = current.count;
    const prefix = current.prefix;
    const top = node.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
    const viewTop = scroller.scrollTop;
    const viewBottom = viewTop + (scroller.clientHeight || 600);
    let start: number;
    let end: number;
    if (prefix && prefix.length === count + 1) {
      const unitTop = Math.floor((viewTop - top) / rowHeight);
      const unitBottom = Math.ceil((viewBottom - top) / rowHeight);
      start = Math.max(0, rowAtUnit(prefix, Math.max(0, unitTop)) - 30);
      end = Math.min(count, rowAtUnit(prefix, Math.max(0, unitBottom)) + 31);
    } else {
      start = Math.max(0, Math.floor((viewTop - top) / rowHeight) - 30);
      end = Math.min(count, Math.ceil((viewBottom - top) / rowHeight) + 30);
    }
    if (end < 0) end = 0;
    if (start > count) start = count;
    if (end < start) end = start;
    const prev = shell.patchView[current.file];
    if (prev && prev.start === start && prev.end === end) return;
    shell.patchView = { ...shell.patchView, [current.file]: { start, end } };
  };
  const schedule = () => {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      measure();
    });
  };
  measure();
  const resize = new ResizeObserver(schedule);
  resize.observe(node);
  const host = node.closest(".detail-scroll") ?? node.parentElement;
  if (host instanceof HTMLElement && host !== node) resize.observe(host);
  return {
    update(next: { file: string; count: number; rowHeight: number; prefix?: Uint32Array | null }) {
      current = next;
      measure();
    },
    destroy() {
      if (frame) cancelAnimationFrame(frame);
      scroller?.removeEventListener("scroll", schedule);
      resize.disconnect();
      if (!(current.file in shell.patchView)) return;
      const next = { ...shell.patchView };
      delete next[current.file];
      shell.patchView = next;
    },
  };
}
