import { onMount, tick } from "svelte";
import { invoke } from "../perf";
import { broadcastLocale } from "../i18n/sync";
import { broadcastAppearance } from "../appearance";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commits, diffs, sampleConflictText, sampleConflictViews, sampleImages, sampleNotices, samplePulls, sampleReflog, staged as sampleStaged, unstaged as sampleUnstaged, type FileChange } from "../sample";
import { clipboardRemote, cloneTarget, cloneUrl, remoteFromText, repoNameFromUrl, safeFolderName } from "../clonePath";
import {
    badge,
    type BlameLine,
    type CommandRecord,
    type CommitDetail,
    type CommitRow,
    type CommitTypeRecord,
    type ConflictSides,
    type ConflictView,
    type DiffLine,
    type FileDiff,
    type FilePreview,
    type InProgress,
    type RefSnapshot,
    type Settings,
    type StatusFile,
    type StatusSnapshot,
    type Suggestion,
    type ToolRecord,
    type UpdateNotice,
    recentWorkspaceId,
    workspaceTabWidth,
  } from "../status";
import type { Mode, Side, Row, Dialog, MergePiece, MergePick, RepoFacts, MenuItem, ConfirmAsk, TreeEntry, RepoEntry, CommandEntry, CommandFilter, RepoJob, PrefTab, LaunchItem, Place, SearchKey, StatusNoticeId } from "./types";
import { STATUS_NOTICE_ENTRY } from "./types";
import type { ShellState } from "./state.svelte";
import { alignCommitTemplate, commitTypeName, commitTypesOf, isCommentary, isCommitType, setCommitTypes, splitSubject, validCommitTypeName, withoutRepeatedSubject } from "./commitMessage";

export async function selectPrefTab(shell: ShellState, tab: PrefTab) {
    shell.prefTab = tab;
    if (tab === "integration" && !shell.labPage) void shell.loadExternalTools();
    await tick();
    shell.prefPage?.scrollTo(0, 0);
  }

export async function openLaunch(shell: ShellState) {
    if (shell.dialog || shell.textMerge || shell.confirmAsk || shell.forgeOpen) return;
    shell.repoPickerOpen = false;
    shell.commandLogOpen = false;
    shell.launch = "";
    shell.launchIndex = 0;
    shell.launchOpen = true;
    await tick();
    shell.launchEl?.focus();
  }

export function closeLaunch(shell: ShellState) {
    shell.launchOpen = false;
    shell.launch = "";
    shell.launchIndex = 0;
    shell.launchMode = "commands";
  }

export function runLaunch(shell: ShellState, item: LaunchItem) {
    if (!item.stay) shell.closeLaunch();
    item.run();
  }

export function onLaunchKey(shell: ShellState, event: KeyboardEvent) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      event.stopPropagation();
      shell.launchIndex = Math.min(shell.matches.length - 1, shell.launchIndex + 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      shell.launchIndex = Math.max(0, shell.launchIndex - 1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      const item = shell.matches[shell.launchIndex] ?? shell.matches[0];
      if (item) shell.runLaunch(item);
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      shell.closeLaunch();
    } else if (event.key === "Backspace" && shell.launchMode === "files" && shell.launch === "") {
      event.preventDefault();
      event.stopPropagation();
      shell.launchMode = "commands";
      shell.launchIndex = 0;
    }
  }

export function placeCommandLog(shell: ShellState) {
    const rect = shell.commandButton?.getBoundingClientRect();
    const width = Math.min(860, window.innerWidth - 16);
    const y = rect ? rect.bottom + 8 : 76;
    const preferred = rect ? rect.left - 280 : 12;
    shell.commandPos = {
      x: Math.max(8, Math.min(preferred, window.innerWidth - width - 8)),
      y,
      width,
      maxHeight: Math.max(280, Math.min(480, window.innerHeight - y - 12)),
    };
  }

export function openCommandLog(shell: ShellState, prefer?: CommandFilter | StatusNoticeId) {
    shell.pruneErrorLog();
    if (prefer === "notices" || (typeof prefer === "string" && prefer in STATUS_NOTICE_ENTRY)) {
      shell.commandFilter = "notices";
      if (prefer !== "notices") shell.commandPick = STATUS_NOTICE_ENTRY[prefer as StatusNoticeId];
    } else if (prefer === "errors" || prefer === "all" || prefer === "user" || prefer === "background") {
      shell.commandFilter = prefer;
    } else if (shell.unseenErrors > 0) {
      shell.commandFilter = "errors";
    } else if (shell.statusNotices.length > 0) {
      shell.commandFilter = "notices";
      if (shell.floorNotice) shell.commandPick = STATUS_NOTICE_ENTRY[shell.floorNotice.id];
    }
    shell.unseenErrors = 0;
    shell.seedSampleCommands();
    if (!shell.visibleCommands().some((item) => item.id === shell.commandPick)) shell.commandPick = shell.visibleCommands()[0]?.id ?? 0;
    shell.repoPickerOpen = false;
    shell.closeLaunch();
    shell.dismissMenu(false);
    shell.placeCommandLog();
    shell.commandLogOpen = true;
  }

export function showCommandFilter(shell: ShellState, next: CommandFilter) {
    shell.commandFilter = next;
    if (!shell.visibleCommands().some((item) => item.id === shell.commandPick)) shell.commandPick = shell.visibleCommands()[0]?.id ?? 0;
  }

export async function fillFromMessage(shell: ShellState, text: string) {
    const [first, ...rest] = text.replace(/\r\n/g, "\n").split("\n");
    shell.applySubject(first ?? "");
    shell.description = rest.join("\n").replace(/^\n/, "").trim();
  }

export async function fillTemplate(shell: ShellState) {
    const value = await shell.loadFacts(true);
    if (!value?.template.trim()) return;
    const aligned = alignCommitTemplate(value.template);
    // `feat:` with an empty subject is the template placeholder. Keep the chosen type.
    if (!shell.summary.trim() && splitSubject(aligned.subject).text.trim()) shell.applySubject(aligned.subject);
    if (!shell.description.trim() || isCommentary(shell.description)) shell.description = aligned.description;
  }

const MERGE_CONFLICT_MARK = "--- Merge Conflict ---";

export function hunkIncludes(pick: MergePick, side: "ours" | "theirs") {
    if (pick === "both") return true;
    if (pick === "none") return false;
    return pick === side;
  }

export function pickFromIncludes(theirs: boolean, ours: boolean): MergePick {
    if (theirs && ours) return "both";
    if (theirs) return "theirs";
    if (ours) return "ours";
    return "none";
  }

export function chosenHunkText(piece: Extract<MergePiece, { kind: "hunk" }>, emptyMark = MERGE_CONFLICT_MARK) {
    const left = hunkIncludes(piece.pick, "theirs");
    const right = hunkIncludes(piece.pick, "ours");
    if (!left && !right) return emptyMark;
    const parts: string[] = [];
    if (left && (piece.theirs.length > 0 || piece.theirsLines === 1)) parts.push(piece.theirs);
    if (right && (piece.ours.length > 0 || piece.oursLines === 1)) parts.push(piece.ours);
    return parts.join("\n");
  }

export function splitConflicts(shell: ShellState, working: string) {
    const lines = working.split("\n");
    const pieces: MergePiece[] = [];
    let text: string[] = [];
    let ours: string[] = [];
    let theirs: string[] = [];
    let mode: "text" | "ours" | "theirs" = "text";
    const flush = () => {
      if (text.length > 0) pieces.push({ kind: "text", text: text.join("\n") });
      text = [];
    };
    for (const line of lines) {
      if (line.startsWith("<<<<<<<")) {
        flush();
        mode = "ours";
        ours = [];
        theirs = [];
        continue;
      }
      if (mode === "ours" && line.startsWith("=======")) {
        mode = "theirs";
        continue;
      }
      if (mode === "theirs" && line.startsWith(">>>>>>>")) {
        pieces.push({ kind: "hunk", ours: ours.join("\n"), theirs: theirs.join("\n"), oursLines: ours.length, theirsLines: theirs.length, pick: "none" });
        mode = "text";
        continue;
      }
      if (mode === "ours") ours.push(line);
      else if (mode === "theirs") theirs.push(line);
      else text.push(line);
    }
    flush();
    return pieces;
  }

export function joinedPieces(shell: ShellState, pieces: MergePiece[]) {
    return pieces
      .flatMap((piece) => {
        if (piece.kind === "text") return [piece.text];
        const text = chosenHunkText(piece);
        const blankLine = (hunkIncludes(piece.pick, "ours") && piece.oursLines === 1)
          || (hunkIncludes(piece.pick, "theirs") && piece.theirsLines === 1);
        return piece.pick !== "none" && text === "" && !blankLine ? [] : [text];
      })
      .join("\n");
  }

export function pickHunk(shell: ShellState, index: number, pick: MergePick) {
    shell.mergeDirty = true;
    shell.mergePieces = shell.mergePieces.map((piece, at) => (piece.kind === "hunk" && at === index ? { ...piece, pick } : piece));
  }

/** Toggle one side of a conflict hunk (Fork Select / Remove). */
export function toggleMergeSide(shell: ShellState, index: number, side: "ours" | "theirs") {
    const piece = shell.mergePieces[index];
    if (!piece || piece.kind !== "hunk") return;
    const theirs = side === "theirs" ? !hunkIncludes(piece.pick, "theirs") : hunkIncludes(piece.pick, "theirs");
    const ours = side === "ours" ? !hunkIncludes(piece.pick, "ours") : hunkIncludes(piece.pick, "ours");
    pickHunk(shell, index, pickFromIncludes(theirs, ours));
  }

/** Set or clear one side across every hunk (header checkboxes). */
export function takeMergeSide(shell: ShellState, side: "ours" | "theirs", on: boolean) {
    shell.mergeDirty = true;
    shell.mergePieces = shell.mergePieces.map((piece) => {
      if (piece.kind !== "hunk") return piece;
      const theirs = side === "theirs" ? on : hunkIncludes(piece.pick, "theirs");
      const ours = side === "ours" ? on : hunkIncludes(piece.pick, "ours");
      return { ...piece, pick: pickFromIncludes(theirs, ours) };
    });
  }

function piecesFromSides(shell: ShellState, sides: { ours: string; theirs: string; working: string }) {
    const pieces = shell.splitConflicts(sides.working);
    if (pieces.some((piece) => piece.kind === "hunk")) return pieces;
    return [{ kind: "hunk" as const, ours: sides.ours, theirs: sides.theirs, pick: "none" as const }];
  }

/** Past this, moving the whole text through the window costs more than a merge view is worth. */
const MERGE_TEXT_LIMIT = 8_000_000;
const MERGE_LINE_LIMIT = 200_000;
/** Above this, opening the merge panel offers a load button instead of building the view. */
const MERGE_AUTO_LIMIT = 200_000;
const MERGE_AUTO_LINES = 8_000;

function textTooBig(text: string) {
    if (text.length > MERGE_TEXT_LIMIT) return true;
    let lines = 1;
    for (let index = 0; index < text.length; index += 1) {
      if (text.charCodeAt(index) === 10 && ++lines > MERGE_LINE_LIMIT) return true;
    }
    return false;
  }

function textNeedsLoad(text: string) {
    if (text.length > MERGE_AUTO_LIMIT) return true;
    let lines = 1;
    for (let index = 0; index < text.length; index += 1) {
      if (text.charCodeAt(index) === 10 && ++lines > MERGE_AUTO_LINES) return true;
    }
    return false;
  }

function sidesTooBig(sides: { ours: string; theirs: string; working: string }) {
    return textTooBig(sides.working) || textTooBig(sides.ours) || textTooBig(sides.theirs);
  }

function sidesNeedLoad(sides: { ours: string; theirs: string; working: string }) {
    return textNeedsLoad(sides.working) || textNeedsLoad(sides.ours) || textNeedsLoad(sides.theirs);
  }

function showFileMerge(shell: ShellState, file: string) {
    shell.mergeFile = file;
    shell.mergeSource = [];
    shell.mergeKind = "file";
    shell.mergeLarge = false;
    shell.mergePieces = [];
    shell.mergeResult = "";
    shell.conflict = null;
    shell.textMerge = true;
  }

/** Build the merge view once the user asks for a file too large to draw on open. */
export async function loadTextMerge(shell: ShellState) {
    const file = shell.mergeFile;
    const generation = shell.mergeGeneration;
    if (!file || shell.busy || shell.mergeLoading) return;
    shell.mergeLoading = true;
    shell.mergeError = "";
    try {
      const sides = shell.mode === "sample" ? sampleConflictText[file] : await invoke<ConflictSides>("conflict_sides", { path: shell.repoPath(), file });
      if (!shell.textMerge || shell.mergeFile !== file || shell.mergeGeneration !== generation) return;
      if (!sides || sidesTooBig(sides)) {
        showFileMerge(shell, file);
        return;
      }
      shell.conflict = sides;
      shell.mergePieces = piecesFromSides(shell, sides);
      shell.mergeSource = shell.mergePieces;
      shell.mergeResult = shell.joinedPieces(shell.mergePieces);
      shell.mergeLarge = false;
    } catch (error) {
      if (shell.mergeGeneration === generation) shell.mergeError = shell.message(error);
    } finally {
      if (shell.mergeGeneration === generation) shell.mergeLoading = false;
    }
  }

export async function openTextMerge(shell: ShellState, file: string) {
    if (shell.inApp() && shell.standalone !== "merge") {
      try {
        let size: { width?: number; height?: number } = {};
        try { size = JSON.parse(localStorage.getItem("awegit.mergeWindowSize") ?? "{}"); } catch { /* native defaults */ }
        await invoke("open_window", { view: "merge", repo: shell.repoPath(), file,
          width: Number.isFinite(size?.width) ? size.width : null,
          height: Number.isFinite(size?.height) ? size.height : null });
      } catch (error) { shell.actionError = shell.message(error); }
      return;
    }
    if (shell.textMerge && shell.mergeFile === file) return;
    shell.standalone = "merge";
    const generation = ++shell.mergeGeneration;
    shell.mergeLoading = true;
    try { await readTextMerge(shell, file, generation); }
    finally { if (generation === shell.mergeGeneration) shell.mergeLoading = false; }
  }

async function readTextMerge(shell: ShellState, file: string, generation: number) {
    shell.mergeFile = file;
    shell.mergeDirty = false;
    shell.mergeEdited = false;
    shell.mergeEditing = false;
    shell.mergeError = "";
    shell.section = "changes";
    const already = shell.selectedPath === file;
    shell.selectedPath = file;
    const path = shell.repoPath();
    const stillOpen = () => generation === shell.mergeGeneration && (shell.mode !== "live" || (shell.viewing(path) && shell.selectedPath === file));
    let view = shell.mode === "sample" ? sampleConflictViews[file] : already ? shell.activeConflict : null;
    if (shell.mode === "live" && !view) {
      try {
        view = await invoke<ConflictView>("conflict_view", { path, file });
        if (!stillOpen()) return;
        shell.conflictView = view;
      } catch (error) {
        if (stillOpen()) shell.mergeError = shell.message(error);
        return;
      }
    }
    shell.mergeLeft = view?.theirs.label || shell.tr("dialog.useTheirs");
    shell.mergeRight = view?.ours.label || shell.branchLabel || shell.tr("dialog.useOurs");
    const editable = shell.mode === "sample" ? !!sampleConflictText[file] : !view?.binary;
    if (!editable) {
      showFileMerge(shell, file);
      return;
    }
    try {
      const sides = shell.mode === "sample" ? sampleConflictText[file] : await invoke<ConflictSides>("conflict_sides", { path, file });
      if (!stillOpen()) return;
      if (!sides || sidesTooBig(sides)) {
        showFileMerge(shell, file);
        return;
      }
      shell.conflict = sides;
      shell.mergeFile = file;
      shell.mergeKind = "text";
      shell.mergeLarge = sidesNeedLoad(sides);
      shell.mergePieces = shell.mergeLarge ? [] : piecesFromSides(shell, sides);
      shell.mergeSource = shell.mergePieces;
      shell.mergeResult = shell.mergeLarge ? "" : shell.joinedPieces(shell.mergePieces);
      shell.textMerge = true;
    } catch (error) {
      if (stillOpen()) shell.mergeError = shell.message(error);
    }
  }

export function closeTextMerge(shell: ShellState) {
    if (shell.mergeApplying) return;
    shell.guardMerge(() => {
      shell.mergeGeneration++;
      shell.textMerge = false;
      shell.mergeLarge = false;
      shell.mergeLoading = false;
      if (shell.inApp() && shell.standalone === "merge") void getCurrentWindow().close();
      else shell.standalone = "";
    });
  }

export type RebaseStep = { verb: string; rev: string; summary: string; message: string };

/** Only checked commits are sent to the backend; unchecked rows are excluded. */
export function buildRebaseSteps(steps: RebaseStep[], picked: string[]): RebaseStep[] {
    if (picked.length === 0) return [];
    return steps
      .filter((step) => picked.includes(step.rev))
      .map((step) => ({ ...step, message: step.verb === "reword" || step.verb === "squash" || step.verb === "exec" ? step.message : "" }));
  }

export async function applyTextMerge(shell: ShellState, text?: string): Promise<boolean> {
    const file = shell.mergeFile;
    if (!file || shell.busy || shell.mergeLarge || shell.mergePieces.length === 0) return false;
    if (shell.mergePieces.some((piece) => piece.kind === "hunk" && piece.pick === "none")) return false;
    return applyMerge(shell, { action: "resolve", file, side: "text", text: text ?? shell.mergeResult });
  }

export async function applyFileMerge(shell: ShellState, side: "ours" | "theirs"): Promise<boolean> {
    if (!shell.mergeFile || shell.busy) return false;
    return applyMerge(shell, { action: "resolve", file: shell.mergeFile, side, text: "" });
  }

async function applyMerge(shell: ShellState, request: Record<string, unknown>): Promise<boolean> {
    shell.mergeError = "";
    shell.mergeApplying = true;
    try {
      const outcome = shell.mode === "live" ? await shell.mutate(request) : "completed";
      if (outcome !== "completed") {
        shell.mergeError = shell.actionError || shell.tr("dialog.mergeApplyFailed");
        return false;
      }
    } catch (error) {
      shell.mergeError = shell.message(error);
      return false;
    } finally {
      shell.mergeApplying = false;
    }
    shell.mergeDirty = false;
    shell.closeTextMerge();
    return true;
  }

export async function openResolve(shell: ShellState, file: string) {
    await openTextMerge(shell, file);
  }

export function bisectMark(shell: ShellState, id: string) {
    const log = shell.facts?.bisectLog ?? "";
    if (!shell.facts?.bisectActive || !id) return "";
    const short = id.slice(0, 9);
    for (const line of log.split("\n")) {
      if (!line.includes(short) && !line.includes(id)) continue;
      if (/\bgood\b/.test(line)) return "good";
      if (/\bbad\b/.test(line)) return "bad";
    }
    return "";
  }

export function pullRequestUrl(shell: ShellState, branch: string) {
    const remote = shell.refs?.remotes.find((item) => item.url)?.url ?? "";
    const github = remote.match(/github\.com[:/]([^/\s]+\/[^/\s.]+)/i);
    if (github) {
      const project = github[1].replace(/\.git$/, "");
      return `https://github.com/${project}/compare/${encodeURIComponent(branch)}?expand=1`;
    }
    const gitlab = remote.match(/([^@/:]+)[:/]([^/\s]+\/[^/\s.]+)/i);
    if (gitlab && gitlab[1].includes("gitlab")) {
      const project = gitlab[2].replace(/\.git$/, "");
      return `https://${gitlab[1]}/${project}/-/merge_requests/new?merge_request[source_branch]=${encodeURIComponent(branch)}`;
    }
    const bitbucket = remote.match(/bitbucket\.org[:/]([^/\s]+\/[^/\s.]+)/i);
    if (bitbucket) return `https://bitbucket.org/${bitbucket[1].replace(/\.git$/, "")}/pull-requests/new?source=${encodeURIComponent(branch)}`;
    const azure = remote.match(/dev\.azure\.com\/([^/\s]+)\/([^/\s]+)\/_git\/([^/\s.]+)/i);
    if (azure) return `https://dev.azure.com/${azure[1]}/${azure[2]}/_git/${azure[3].replace(/\.git$/, "")}/pullrequestcreate?sourceRef=${encodeURIComponent(branch)}`;
    return "";
  }

export function commitWeb(shell: ShellState, id: string) {
    const remote = shell.refs?.remotes.find((item) => item.url)?.url ?? "";
    const github = remote.match(/github\.com[:/]([^/\s]+\/[^/\s.]+)/i);
    if (github) return `https://github.com/${github[1].replace(/\.git$/, "")}/commit/${id}`;
    const gitlab = remote.match(/([^@/:]+)[:/]([^/\s]+\/[^/\s.]+)/i);
    if (gitlab && gitlab[1].includes("gitlab")) return `https://${gitlab[1]}/${gitlab[2].replace(/\.git$/, "")}/-/commit/${id}`;
    const bitbucket = remote.match(/bitbucket\.org[:/]([^/\s]+\/[^/\s.]+)/i);
    if (bitbucket) return `https://bitbucket.org/${bitbucket[1].replace(/\.git$/, "")}/commits/${id}`;
    return "";
  }

export function requestCheckout(shell: ShellState, name: string) {
    const dirty = (shell.snapshot?.staged.length ?? 0) + (shell.snapshot?.unstaged.length ?? 0) > 0;
    if (!dirty) {
      void shell.mutate({ action: "checkout", name });
      return;
    }
    void checkoutWithStash(shell, name);
  }

async function checkoutWithStash(shell: ShellState, name: string, remote?: { remote: string; branch: string }) {
    await shell.mutate({ action: "stash", message: shell.tr("dialog.checkoutStashMessage", { name }), includeUntracked: true });
    if (shell.actionError) return;
    if (remote) await shell.mutate({ action: "checkoutRemote", remote: remote.remote, branch: remote.branch });
    else await shell.mutate({ action: "checkout", name });
    if (shell.actionError) {
      const failure = shell.actionError;
      await shell.mutate({ action: "stashPop", name: "stash@{0}" });
      shell.actionError = shell.actionError ? `${failure}\n${shell.actionError}` : failure;
      return;
    }
    await shell.mutate({ action: "stashPop", name: "stash@{0}" });
    if (shell.actionError) {
      shell.pendingCheckout = name;
      shell.dialog = "checkoutNotice";
    }
  }

export async function quickCheckout(shell: ShellState, name: string, remote?: { remote: string; branch: string }) {
    if (shell.mode !== "live") return;
    const dirty = (shell.snapshot?.staged.length ?? 0) + (shell.snapshot?.unstaged.length ?? 0) > 0;
    if (dirty) {
      await checkoutWithStash(shell, name, remote);
      return;
    }
    if (remote) await shell.mutate({ action: "checkoutRemote", remote: remote.remote, branch: remote.branch });
    else await shell.mutate({ action: "checkout", name });
  }

export function reopenClosed(shell: ShellState) {
    const path = shell.closedTabs[0];
    if (!path) return;
    shell.closedTabs = shell.closedTabs.slice(1);
    void shell.openRepo(path);
  }

export async function pastePatch(shell: ShellState) {
    const text = await navigator.clipboard?.readText().catch(() => "");
    if (!text?.includes("diff")) return;
    void shell.mutate({ action: "applyPatch", patch: text });
  }

export function confirmSync(shell: ShellState, kind: "fetch" | "pull" | "push") {
    const current = shell.refs?.branches.find((branch) => branch.current);
    const upstream = current?.upstream ?? "";
    const firstRemote = shell.refs?.remotes[0]?.name ?? "";
    const upstreamRemote = shell.refs?.remotes.find((remote) => upstream.startsWith(`${remote.name}/`))?.name ?? "";
    shell.syncAll = shell.settings.fetchAllRemotes;
    shell.syncTags = kind === "fetch" ? shell.settings.fetchTags : shell.settings.pushAllTags;
    shell.syncRebase = shell.settings.pullRebase;
    shell.syncForce = false;
    shell.pushBranch = current?.name ?? shell.snapshot?.branch ?? "";
    shell.resetPushTarget();
    shell.draft = upstreamRemote || firstRemote;
    shell.draftExtra = kind === "pull" ? (upstreamRemote ? upstream.slice(upstreamRemote.length + 1) : shell.snapshot?.branch ?? "") : "";
    shell.dialog = kind;
  }

export function ask(
    shell: ShellState,
    title: string,
    body: string,
    run: () => void,
    files: string[] = [],
    extras: Pick<ConfirmAsk, "subject" | "detail" | "danger" | "confirmLabel"> = {},
  ) {
    shell.confirmAsk = { title, body, files, ...extras, run };
  }

export function liveOnly(shell: ShellState, run: () => void) {
    return shell.mode === "live" ? run : undefined;
  }

export async function openForge(shell: ShellState, kind: "notes" | "pulls") {
    shell.forgeOpen = shell.forgeOpen === kind ? null : kind;
    if (shell.mode !== "live") {
      shell.notices = sampleNotices;
      shell.pulls = samplePulls;
      return;
    }
    const path = shell.repoPath();
    if (kind === "notes") {
      const notices = await invoke<({ id: string; title: string; url: string; unread: boolean }[])>("forge_notifications").catch(() => []);
      if (shell.viewing(path)) shell.notices = notices;
    } else {
      const pulls = await invoke<({ title: string; author: string; branch: string; url: string }[])>("forge_pulls", { path }).catch(() => []);
      if (shell.viewing(path)) shell.pulls = pulls;
    }
  }

export function setRebaseVerb(shell: ShellState, rev: string, verb: string) {
    shell.rebaseSteps = shell.rebaseSteps.map((step) => (step.rev === rev ? { ...step, verb } : step));
  }

export function quick(shell: ShellState, event: MouseEvent, dialogName: Dialog, run: () => void) {
    if (event.ctrlKey || event.metaKey) void run();
    else {
      shell.draft = "";
      shell.draftExtra = "";
      shell.dialog = dialogName;
    }
  }

export async function submitCommit(shell: ShellState, pushAfter = false) {
    if (shell.mode !== "live") return;
    if (shell.progress) {
      // A rebase, merge, cherry-pick, or revert owns the next commit.
      void shell.continueFlow();
      return;
    }
    if (!shell.canCommit) return;
    const messageText = shell.fullSummary.trim();
    if (!messageText) return;
    const description = isCommentary(shell.description) ? "" : shell.description;
    const path = shell.repoPath();
    const value = await shell.runChange("commit_changes", { summary: messageText, description, amend: shell.amend, signOff: shell.signOff, skipHooks: shell.skipHooks });
    if (value === undefined) return;
    const failed = value ? "" : (shell.actionError ?? "");
    shell.recordCommand("commit", { title: shell.tr("chrome.commandCommit", { name: messageText || "HEAD" }), command: shell.gitLine(["commit", "-m", messageText]) }, (value?.log ?? failed).trim(), !!value, "user");
    if (value && !shell.actionError && shell.viewing(path)) {
      shell.summary = "";
      shell.description = "";
      shell.amend = false;
      shell.skipHooks = false;
      if (path) shell.drafts.delete(path);
      await shell.loadContext();
      void shell.fillTemplate();
      // A commit can end a paused merge; the autostash it kept may come back as conflicts.
      const restoreConflict = shell.conflictCount > 0;
      if (restoreConflict) {
        shell.mergeConflictRestore = true;
        shell.commandOutput = (value.log ?? "").trim();
        shell.dialog = "mergeConflict";
        shell.actionError = null;
      }
      if (!restoreConflict && (pushAfter || shell.settings.pushOnCommit)) await shell.doPush();
    }
  }

export function togglePromptStyle(shell: ShellState, event: MouseEvent) {
    event.preventDefault();
    const aiPromptStyle = shell.settings.aiPromptStyle === "long" ? "short" : "long";
    shell.settings = { ...shell.settings, aiPromptStyle };
    void shell.savePrefs(false);
  }

function explainSuggest(shell: ShellState, error: unknown) {
    const text = shell.message(error);
    if (text.startsWith("Stage changes")) return shell.tr("chrome.suggestNeedStaged");
    if (text.startsWith("AI commit messages are turned off")) return shell.tr("chrome.suggestOff");
    if (text.startsWith("Add an API key")) return shell.tr("chrome.suggestNeedKey");
    if (text.startsWith("Set an API base URL")) return shell.tr("chrome.suggestNeedUrl");
    if (text.startsWith("The model returned an empty")) return shell.tr("chrome.suggestEmpty");
    if (text.startsWith("The model did not reply in")) return shell.tr("chrome.suggestLanguage");
    if (text.startsWith("The model reply does not match")) {
      const detail = text.slice("The model reply does not match the commit format:".length).trim();
      const reason = shell.tr("chrome.suggestFormat");
      return detail ? `${reason}\n${detail}` : reason;
    }
    return text;
  }

function explainAiEndpoint(shell: ShellState, error: unknown) {
    const text = shell.message(error);
    if (text.startsWith("Set an API base URL")) return shell.tr("chrome.suggestNeedUrl");
    if (text.startsWith("The API base URL")) return shell.tr("dialog.endpointScheme");
    if (text.startsWith("curl is not available")) return shell.tr("dialog.curlMissing");
    return text;
  }

export async function testAiEndpoint(shell: ShellState) {
    if (shell.aiEndpointBusy) return;
    const baseUrl = shell.settings.aiBaseUrl;
    const apiKey = shell.settings.aiApiKey;
    if (!baseUrl.trim()) {
      shell.aiEndpointNote = shell.tr("chrome.suggestNeedUrl");
      return;
    }
    shell.aiEndpointBusy = "test";
    shell.aiEndpointNote = "";
    try {
      const count = await invoke<number>("test_ai_endpoint", { baseUrl, apiKey });
      if (shell.settings.aiBaseUrl !== baseUrl) return;
      shell.aiEndpointNote = shell.tr("dialog.endpointOk", { count: String(count) });
    } catch (error) {
      if (shell.settings.aiBaseUrl !== baseUrl) return;
      shell.aiEndpointNote = explainAiEndpoint(shell, error);
    } finally {
      shell.aiEndpointBusy = "";
    }
  }

export async function loadAiModels(shell: ShellState) {
    if (shell.aiEndpointBusy) return;
    const baseUrl = shell.settings.aiBaseUrl;
    const apiKey = shell.settings.aiApiKey;
    if (!baseUrl.trim()) {
      shell.aiEndpointNote = shell.tr("chrome.suggestNeedUrl");
      return;
    }
    shell.aiEndpointBusy = "models";
    shell.aiEndpointNote = "";
    try {
      const models = await invoke<string[]>("list_ai_models", { baseUrl, apiKey });
      if (shell.settings.aiBaseUrl !== baseUrl) return;
      shell.aiModels = models;
      shell.aiEndpointNote = models.length ? shell.tr("dialog.modelsLoaded", { count: String(models.length) }) : shell.tr("dialog.noModels");
    } catch (error) {
      if (shell.settings.aiBaseUrl !== baseUrl) return;
      shell.aiModels = [];
      shell.aiEndpointNote = explainAiEndpoint(shell, error);
    } finally {
      shell.aiEndpointBusy = "";
    }
  }

async function openAiSettings(shell: ShellState, field: "enabled" | "baseUrl" | "apiKey") {
    shell.prefTab = "ai";
    shell.dialog = "prefs";
    await tick();
    const target = shell.prefPage?.querySelector<HTMLInputElement>(`[data-ai-field="${field}"]`);
    target?.focus({ preventScroll: true });
    target?.scrollIntoView({ block: "center" });
  }

export async function suggestMessage(shell: ShellState) {
    const path = shell.repoPath();
    if (shell.busy) return;
    shell.actionError = null;
    if (shell.mode !== "live" || !path) {
      shell.actionError = shell.tr("chrome.suggestNeedRepo");
      return;
    }
    if (!shell.settings.aiEnabled) {
      await openAiSettings(shell, "enabled");
      return;
    }
    if (!shell.settings.aiBaseUrl.trim()) {
      await openAiSettings(shell, "baseUrl");
      return;
    }
    if (shell.staged.length === 0) {
      shell.actionError = shell.tr("chrome.suggestNeedStaged");
      return;
    }
    const style = shell.settings.aiPromptStyle === "long" ? "long" : "short";
    const styleName = shell.tr(style === "long" ? "chrome.promptLong" : "chrome.promptShort");
    shell.startWork(shell.tr("chrome.suggestGenerate", { style: styleName }), path);
    try {
      const value = await invoke<Suggestion>("suggest_commit_message", { path, style });
      if (shell.viewing(path)) {
        shell.applySubject(value.summary);
        shell.description = withoutRepeatedSubject(value.description, shell.commitType, shell.summary);
      }
    } catch (error) {
      if (shell.viewing(path)) {
        const text = shell.message(error);
        if (text.startsWith("Add an API key")) await openAiSettings(shell, "apiKey");
        else if (text.startsWith("Set an API base URL")) await openAiSettings(shell, "baseUrl");
        else if (text.startsWith("AI commit messages are turned off")) await openAiSettings(shell, "enabled");
        else shell.actionError = explainSuggest(shell, error);
      }
    } finally {
      shell.stopWork(path);
    }
  }

export function composeTool(shell: ShellState, path: string, args: string) {
    const file = path.trim();
    const rest = args.trim();
    if (!file) return rest;
    const quoted = file.includes(" ") ? `"${file}"` : file;
    return rest ? `${quoted} ${rest}` : quoted;
  }

export function applyToolSettings(shell: ShellState) {
    for (const kind of toolKinds) {
      if (kind === "editor") continue;
      const tool = defaultTool(shell, kind);
      const name = tool?.name ?? "";
      const path = tool?.path ?? "";
      const args = tool?.args ?? "";
      const command = composeTool(shell, path, args);
      if (kind === "merge") {
        shell.settings.mergeToolName = name;
        shell.settings.mergeToolPath = path;
        shell.settings.mergeToolArgs = args;
        shell.settings.mergeTool = command;
      } else {
        shell.settings.diffToolName = name;
        shell.settings.diffToolPath = path;
        shell.settings.diffToolArgs = args;
        shell.settings.diffTool = command;
      }
    }
    if (shell.settings.shellKind === "custom" && shell.settings.shellPath.trim()) {
      shell.settings.terminal = shell.composeTool(shell.settings.shellPath, shell.settings.shellArgs);
    }
  }

export async function savePrefs(shell: ShellState, close = true) {
    shell.applyToolSettings();
    if (close) shell.dialog = null;
    if (!shell.backend()) return;
    const ticket = ++shell.prefsSave;
    const values = shell.settings;
    const locale = values.locale;
    const theme = values.theme;
    try {
      const saved = await invoke<Settings>("save_settings", { values });
      if (ticket === shell.prefsSave) {
        const localeChanged = shell.settings.locale !== locale;
        const themeChanged = shell.settings.theme !== theme;
        shell.settings = { ...saved, locale: shell.settings.locale, theme: shell.settings.theme };
        broadcastLocale(shell.settings.locale);
        broadcastAppearance(shell.settings.theme);
        applyCommitTypes(shell);
        if (localeChanged || themeChanged) void shell.savePrefs(false);
      }
    } catch (error) {
      if (ticket === shell.prefsSave) shell.actionError = shell.message(error);
    }
  }

/** Point the composer helpers at the saved types and repair a type that was removed. */
export function applyCommitTypes(shell: ShellState) {
    setCommitTypes(shell.settings.commitTypes);
    if (!isCommitType(shell.settings.commitType)) shell.settings = { ...shell.settings, commitType: shell.preferredCommitType() };
    if (!isCommitType(shell.commitType)) shell.commitType = shell.preferredCommitType();
  }

function syncCommitTypes(shell: ShellState, next: CommitTypeRecord[]) {
    const list = commitTypesOf(next);
    shell.settings = { ...shell.settings, commitTypes: next.length ? list : [] };
    setCommitTypes(shell.settings.commitTypes);
    if (!isCommitType(shell.commitType)) shell.commitType = list[0]?.name ?? "";
    if (!isCommitType(shell.settings.commitType)) shell.settings = { ...shell.settings, commitType: list[0]?.name ?? "" };
  }

export function addCommitType(shell: ShellState, value: string, color: string, label = "") {
    const name = commitTypeName(value);
    const list = commitTypesOf(shell.settings.commitTypes);
    if (!validCommitTypeName(name) || list.some((item) => item.name === name)) return false;
    syncCommitTypes(shell, [...list, { name, color, label: label.trim() }]);
    return true;
  }

export function renameCommitType(shell: ShellState, from: string, value: string) {
    const name = commitTypeName(value);
    const list = commitTypesOf(shell.settings.commitTypes);
    const taken = name !== from && list.some((item) => item.name === name);
    if (!validCommitTypeName(name) || taken) return false;
    if (name === from) return true;
    if (shell.commitType === from) shell.commitType = name;
    if (shell.settings.commitType === from) shell.settings = { ...shell.settings, commitType: name };
    syncCommitTypes(shell, list.map((item) => (item.name === from ? { ...item, name } : item)));
    return true;
  }

export function setCommitTypeField(shell: ShellState, name: string, field: "color" | "label", value: string) {
    const list = commitTypesOf(shell.settings.commitTypes);
    syncCommitTypes(shell, list.map((item) => (item.name === name ? { ...item, [field]: value } : item)));
  }

export function removeCommitType(shell: ShellState, name: string) {
    const list = commitTypesOf(shell.settings.commitTypes);
    if (list.length <= 1) return;
    const next = list.filter((item) => item.name !== name);
    if (shell.commitType === name) shell.commitType = next[0].name;
    if (shell.settings.commitType === name) shell.settings = { ...shell.settings, commitType: next[0].name };
    syncCommitTypes(shell, next);
  }

export function resetCommitTypes(shell: ShellState) {
    shell.settings = { ...shell.settings, commitTypes: [] };
    applyCommitTypes(shell);
  }

export async function browseFolder(shell: ShellState, initial = "") {
    if (!shell.inApp()) return null;
    try {
      return await invoke<string | null>("pick_directory", { initial: initial || null });
    } catch (error) {
      shell.actionError = shell.message(error);
      return null;
    }
  }

export function withSourceRoots(shell: ShellState, value: Settings) {
    const folder = value.cloneDirectory.trim();
    if (!folder || value.sourceDirectories.includes(folder)) return value;
    const groupNames = value.groupNames.some((item) => item.startsWith(`${folder}\t`))
      ? value.groupNames
      : [...value.groupNames, `${folder}\t${shell.folderName(folder)}`];
    return { ...value, sourceDirectories: [folder, ...value.sourceDirectories], groupNames };
  }

export function sourceRootList(shell: ShellState) {
    const paths = [...shell.settings.sourceDirectories];
    const extra = shell.settings.cloneDirectory.trim();
    if (extra && !paths.includes(extra)) paths.unshift(extra);
    return paths.map((path) => ({ path, alias: shell.groupLabel(path) }));
  }

export function addSourceRoot(shell: ShellState, path: string) {
    const folder = path.trim();
    if (!folder) return;
    if (shell.settings.sourceDirectories.includes(folder)) {
      if (shell.settings.cloneDirectory !== folder) shell.settings = { ...shell.settings, cloneDirectory: folder };
      return;
    }
    const groupNames = shell.settings.groupNames.some((item) => item.startsWith(`${folder}\t`))
      ? shell.settings.groupNames
      : [...shell.settings.groupNames, `${folder}\t${shell.folderName(folder)}`];
    shell.settings = {
      ...shell.settings,
      sourceDirectories: [...shell.settings.sourceDirectories, folder],
      groupNames,
      cloneDirectory: shell.settings.cloneDirectory.trim() || folder,
    };
  }

export function removeSourceRoot(shell: ShellState, path: string) {
    const sourceDirectories = shell.settings.sourceDirectories.filter((item) => item !== path);
    const groupNames = shell.settings.groupNames.filter((item) => !item.startsWith(`${path}\t`));
    const cloneDirectory = shell.settings.cloneDirectory === path ? (sourceDirectories[0] ?? "") : shell.settings.cloneDirectory;
    shell.settings = { ...shell.settings, sourceDirectories, groupNames, cloneDirectory };
    void shell.savePrefs(false);
  }

export function setSourceAlias(shell: ShellState, path: string, alias: string) {
    const label = alias.trim() || shell.folderName(path);
    const groupNames = [...shell.settings.groupNames.filter((item) => !item.startsWith(`${path}\t`)), `${path}\t${label}`];
    shell.settings = { ...shell.settings, groupNames };
  }

export function beginOpen(shell: ShellState) {
    shell.draft = "";
    shell.dialog = "open";
  }

export async function readClipboardText(shell: ShellState) {
    if (shell.inApp()) {
      try {
        const text = await invoke<string>("clipboard_text");
        if (text.trim()) return text;
      } catch {
        /* the webview clipboard is the fallback */
      }
    }
    try {
      return (await navigator.clipboard?.readText()) ?? "";
    } catch {
      return "";
    }
  }

export async function beginClone(shell: ShellState, intent: "clone" | "init" = "clone") {
    const roots = shell.sourceRootList();
    shell.cloneIntent = intent;
    shell.cloneRoot = roots.find((root) => root.path === shell.settings.cloneDirectory)?.path ?? roots[0]?.path ?? "";
    shell.cloneNameManual = false;
    const copied = intent === "clone" ? clipboardRemote(await shell.readClipboardText()) : "";
    shell.draft = copied;
    shell.cloneFolder = repoNameFromUrl(copied);
    shell.actionError = null;
    shell.dialog = "clone";
  }

export function onCloneUrl(shell: ShellState, value: string) {
    const remote = remoteFromText(value);
    shell.draft = remote || value;
    if (!shell.cloneNameManual) shell.cloneFolder = repoNameFromUrl(shell.draft);
  }

export function onCloneFolder(shell: ShellState, value: string) {
    shell.cloneFolder = value;
    shell.cloneNameManual = safeFolderName(value) !== repoNameFromUrl(remoteFromText(shell.draft) || shell.draft);
  }

export function rememberCloneRoot(shell: ShellState) {
    const folder = shell.cloneRoot.trim();
    if (!folder || shell.settings.cloneDirectory === folder) return;
    shell.settings = { ...shell.settings, cloneDirectory: folder };
    void shell.savePrefs(false);
  }

export function pathsMatch(shell: ShellState, left: string, right: string) {
    const norm = (value: string) => value.replace(/[\\/]+$/, "").replaceAll("\\", "/");
    return norm(left).toLowerCase() === norm(right).toLowerCase();
  }

export async function finishCloneOpen(shell: ShellState, destination: string) {
    if (!shell.busy) await shell.openRepo(destination);
    shell.cloneDone = shell.snapshot?.path && shell.pathsMatch(shell.snapshot.path, destination) ? null : destination;
  }

export function cancelClone(shell: ShellState) {
    shell.cloneCancel = true;
    void cancelOperation(shell, shell.cloneJob?.destination ?? "");
  }

export async function browseCloneRoot(shell: ShellState) {
    const folder = await shell.browseFolder(shell.cloneRoot);
    if (!folder) return;
    shell.addSourceRoot(folder);
    shell.cloneRoot = folder;
    void shell.savePrefs(false);
  }

export async function startClone(shell: ShellState) {
    const url = cloneUrl(shell.draft);
    const destination = shell.cloneTargetPath;
    if (!shell.backend()) return;
    if (shell.cloneJob) {
      shell.actionError = shell.tr("dialog.cloneBusy");
      return;
    }
    if (!url || !destination) {
      shell.actionError = shell.tr("dialog.cloneMissing");
      return;
    }
    shell.rememberCloneRoot();
    const name = safeFolderName(shell.cloneFolder.trim() || repoNameFromUrl(url));
    shell.cloneCancel = false;
    shell.dialog = null;
    shell.actionError = null;
    shell.cloneDone = null;
    shell.cloneJob = { name, destination, workspace: shell.settings.currentWorkspace };
    shell.startWork(shell.tr("chrome.clone"), destination);
    try {
      await invoke("mutate", { path: shell.repoPath(), request: { action: "clone", url, destination } });
      shell.noteActivity(`clone ${name}`);
      shell.cloneJob = null;
      await shell.finishCloneOpen(destination);
    } catch (error) {
      shell.cloneJob = null;
      if (!shell.cloneCancel) shell.actionError = shell.message(error);
      shell.cloneCancel = false;
    } finally {
      shell.stopWork(destination);
    }
  }

export async function startInit(shell: ShellState) {
    const destination = shell.cloneTargetPath;
    if (!shell.backend()) return;
    if (shell.cloneJob) {
      shell.actionError = shell.tr("dialog.cloneBusy");
      return;
    }
    if (!destination) {
      shell.actionError = shell.tr("dialog.cloneMissing");
      return;
    }
    shell.rememberCloneRoot();
    const name = shell.folderName(destination);
    shell.cloneCancel = false;
    shell.dialog = null;
    shell.actionError = null;
    shell.cloneDone = null;
    shell.cloneJob = { name, destination, workspace: shell.settings.currentWorkspace };
    try {
      await invoke("mutate", { path: shell.repoPath(), request: { action: "init", destination } });
      shell.noteActivity(`init ${name}`);
      shell.cloneJob = null;
      await shell.finishCloneOpen(destination);
    } catch (error) {
      shell.cloneJob = null;
      if (!shell.cloneCancel) shell.actionError = shell.message(error);
      shell.cloneCancel = false;
    }
  }

export async function openSourceRoot(shell: ShellState, path: string) {
    if (shell.settings.cloneDirectory !== path) {
      shell.settings = { ...shell.settings, cloneDirectory: path };
      void shell.savePrefs(false);
    }
    const folder = await shell.browseFolder(path);
    if (folder) void shell.openRepo(folder);
  }

export async function browseSourceRoot(shell: ShellState) {
    const folder = await shell.browseFolder();
    if (!folder) return;
    shell.addSourceRoot(folder);
    void shell.savePrefs(false);
  }

export async function openLocal(shell: ShellState, path: string) {
    if (!shell.backend()) return;
    try {
      await invoke("reveal_path", { path });
    } catch (error) {
      shell.actionError = shell.message(error);
    }
  }

/** Open a work-tree file in a configured editor; without a template the backend picks one. */
export async function openInEditor(shell: ShellState, file: string, line = 0, template?: string) {
    if (!file) return;
    try {
      await invoke("open_editor", { path: shell.repoPath(), file, line, tool: template ?? null });
    } catch (error) {
      shell.actionError = shell.message(error);
    }
  }

export type ExternalTool = {
    name: string;
    path: string;
    args: string;
    primary: boolean;
  };

export type CommonTool = {
    name: string;
    path: string;
    diffArgs: string;
    mergeArgs: string;
  };

export type EditorTool = {
    name: string;
    path: string;
    args: string;
  };

export type ToolKind = "diff" | "merge" | "editor";

export type ToolSource = "detected" | "common";

const toolKinds: ToolKind[] = ["diff", "merge", "editor"];

function legacyToolName(shell: ShellState, kind: ToolKind) {
    return kind === "merge" ? shell.settings.mergeToolName : shell.settings.diffToolName;
  }

function legacyToolPath(shell: ShellState, kind: ToolKind) {
    return kind === "merge" ? shell.settings.mergeToolPath : shell.settings.diffToolPath;
  }

function legacyToolArgs(shell: ShellState, kind: ToolKind) {
    return kind === "merge" ? shell.settings.mergeToolArgs : shell.settings.diffToolArgs;
  }

export function toolsOf(shell: ShellState, kind: ToolKind): ToolRecord[] {
    if (kind === "editor") return shell.settings.editorTools;
    return kind === "merge" ? shell.settings.mergeTools : shell.settings.diffTools;
  }

function setTools(shell: ShellState, kind: ToolKind, tools: ToolRecord[]) {
    const next = ensureFallback(tools);
    if (kind === "editor") shell.settings = { ...shell.settings, editorTools: next };
    else if (kind === "merge") shell.settings = { ...shell.settings, mergeTools: next };
    else shell.settings = { ...shell.settings, diffTools: next };
  }

/** One tool owns the fallback for files that match no extension rule. */
function ensureFallback(tools: ToolRecord[]) {
    if (tools.length === 0 || tools.some((tool) => tool.fallback)) return tools;
    const empty = tools.findIndex((tool) => tool.extensions.length === 0);
    const at = empty >= 0 ? empty : 0;
    return tools.map((tool, index) => (index === at ? { ...tool, fallback: true } : tool));
  }

export function newToolId() {
    return `tool-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
  }

export function normalizeToolExtension(value: string) {
    const text = value.trim();
    if (text === "*") return "*";
    return text.replace(/^\*\./, "").replace(/^\.+/, "").toLowerCase();
  }

export function toolFileSuffix(file: string) {
    const leaf = file.split(/[/\\]/).pop() ?? file;
    const dot = leaf.lastIndexOf(".");
    return dot > 0 && dot < leaf.length - 1 ? leaf.slice(dot + 1).toLowerCase() : "";
  }

export function toolIsGeneric(tool: ToolRecord) {
    return tool.fallback || tool.extensions.length === 0 || tool.extensions.every((item) => normalizeToolExtension(item) === "*");
  }

function toolMatches(tool: ToolRecord, file: string) {
    const leaf = (file.split(/[/\\]/).pop() ?? file).toLowerCase();
    return tool.extensions.some((item) => {
      const value = normalizeToolExtension(item);
      if (value === "*") return true;
      return value.length > 0 && (leaf === value || leaf.endsWith(`.${value}`));
    });
  }

/** Exact suffix matches first, then all-files tools. List order is the priority. */
export function applicableTools(shell: ShellState, kind: ToolKind, file: string) {
    const tools = toolsOf(shell, kind);
    const exact = tools.filter((tool) => toolMatches(tool, file));
    const generic = tools.filter((tool) => toolIsGeneric(tool) && !exact.includes(tool));
    const ordered = [...exact, ...generic];
    return ordered.length > 0 ? ordered : tools;
  }

export function defaultTool(shell: ShellState, kind: ToolKind) {
    const tools = toolsOf(shell, kind);
    return tools.find((tool) => tool.fallback) ?? tools.find(toolIsGeneric) ?? tools[0] ?? null;
  }

export function resolvedTool(shell: ShellState, kind: ToolKind, file: string) {
    return applicableTools(shell, kind, file)[0] ?? defaultTool(shell, kind);
  }

export function toolLabel(tool: Pick<ToolRecord, "name" | "path"> | ExternalTool) {
    const named = tool.name.trim();
    if (named) return named;
    const leaf = tool.path.split(/[/\\]/).pop() ?? "";
    return leaf.replace(/\.(exe|bat|cmd|com)$/i, "") || leaf;
  }

export function toolExtensionLabel(tool: ToolRecord) {
    const parts = tool.extensions
      .map(normalizeToolExtension)
      .filter((item) => item.length > 0 && item !== "*")
      .map((item) => `.${item}`);
    if (tool.fallback) parts.unshift("*");
    return parts.length > 0 ? parts.join(" ") : "*";
  }

export function toolTemplate(shell: ShellState, kind: ToolKind, file: string) {
    const tool = resolvedTool(shell, kind, file);
    if (tool?.path.trim()) return composeTool(shell, tool.path, tool.args);
    return kind === "merge" ? shell.settings.mergeTool : shell.settings.diffTool;
  }

export function openToolSettings(shell: ShellState, kind: ToolKind, toolId = "") {
    shell.prefTab = "integration";
    shell.toolKind = kind;
    shell.toolPick = toolId;
    shell.dialog = "prefs";
  }

export function selectedTool(shell: ShellState, kind: ToolKind) {
    const tools = toolsOf(shell, kind);
    return tools.find((tool) => tool.id === shell.toolPick) ?? tools[0] ?? null;
  }

export function selectTool(shell: ShellState, toolId: string) {
    shell.toolPick = toolId;
  }

export function addTool(shell: ShellState, kind: ToolKind) {
    const tool: ToolRecord = { id: newToolId(), name: "", path: "", args: "", extensions: [], fallback: false };
    setTools(shell, kind, [...toolsOf(shell, kind), tool]);
    shell.toolPick = tool.id;
    void shell.savePrefs(false);
    return tool;
  }

export function removeTool(shell: ShellState, kind: ToolKind, toolId: string) {
    const tools = toolsOf(shell, kind);
    if (!tools.some((tool) => tool.id === toolId)) return;
    setTools(shell, kind, tools.filter((tool) => tool.id !== toolId));
    if (shell.toolPick === toolId) shell.toolPick = "";
    applyToolSettings(shell);
    void shell.savePrefs(false);
  }

export function moveTool(shell: ShellState, kind: ToolKind, toolId: string, delta: number) {
    const tools = [...toolsOf(shell, kind)];
    const at = tools.findIndex((tool) => tool.id === toolId);
    const next = at + delta;
    if (at < 0 || next < 0 || next >= tools.length) return;
    const [tool] = tools.splice(at, 1);
    if (tool) tools.splice(next, 0, tool);
    setTools(shell, kind, tools);
    applyToolSettings(shell);
    void shell.savePrefs(false);
  }

export function setToolExtensions(shell: ShellState, kind: ToolKind, toolId: string, text: string) {
    const parts = text
      .split(/[,;\s]+/)
      .map(normalizeToolExtension)
      .filter((item) => item.length > 0 && item !== "*");
    const extensions = [...new Set(parts)];
    const tools = toolsOf(shell, kind).map((tool) => (tool.id === toolId ? { ...tool, extensions } : tool));
    setTools(shell, kind, tools);
    applyToolSettings(shell);
    void shell.savePrefs(false);
  }

export function setToolField(shell: ShellState, kind: ToolKind, toolId: string, field: "name" | "path" | "args", value: string) {
    const tools = toolsOf(shell, kind).map((tool) => (tool.id === toolId ? { ...tool, [field]: value } : tool));
    setTools(shell, kind, tools);
    applyToolSettings(shell);
    void shell.savePrefs(false);
  }

export function toolTypeRows(shell: ShellState, kind: ToolKind) {
    const tools = toolsOf(shell, kind);
    const suffixes = [...new Set(
      tools.flatMap((tool) => tool.extensions.map(normalizeToolExtension).filter((item) => item.length > 0 && item !== "*")),
    )].sort();
    const rows = suffixes.map((suffix) => ({
      key: suffix,
      label: `.${suffix}`,
      tool: tools.find((tool) => tool.extensions.some((item) => normalizeToolExtension(item) === suffix)) ?? null,
    }));
    rows.push({ key: "*", label: "*", tool: defaultTool(shell, kind) });
    return rows;
  }

export function setFallbackTool(shell: ShellState, kind: ToolKind, toolId: string) {
    if (!toolId) return;
    const tools = toolsOf(shell, kind).map((tool) => ({ ...tool, fallback: tool.id === toolId }));
    setTools(shell, kind, tools);
    applyToolSettings(shell);
    void shell.savePrefs(false);
  }

function cleanedExtensions(extensions: string[]) {
    return [...new Set(
      extensions
        .map(normalizeToolExtension)
        .filter((item) => item.length > 0 && item !== "*"),
    )];
  }

/** Move `suffix` to `toolId`; every other tool loses it. */
export function assignToolType(shell: ShellState, kind: ToolKind, suffix: string, toolId: string) {
    const extension = normalizeToolExtension(suffix);
    if (!extension || extension === "*" || !toolId) return;
    const tools = toolsOf(shell, kind).map((tool) => tool.id === toolId
      ? { ...tool, extensions: cleanedExtensions([...tool.extensions, extension]) }
      : { ...tool, extensions: tool.extensions.filter((item) => normalizeToolExtension(item) !== extension) });
    setTools(shell, kind, tools);
    applyToolSettings(shell);
    void shell.savePrefs(false);
  }

export function removeToolType(shell: ShellState, kind: ToolKind, suffix: string) {
    const extension = normalizeToolExtension(suffix);
    if (!extension || extension === "*") return;
    const tools = toolsOf(shell, kind).map((tool) => ({
      ...tool,
      extensions: tool.extensions.filter((item) => normalizeToolExtension(item) !== extension),
    }));
    setTools(shell, kind, tools);
    applyToolSettings(shell);
    void shell.savePrefs(false);
  }

/** Rename a suffix in place, keeping the tool that owns it. */
export function renameToolType(shell: ShellState, kind: ToolKind, oldSuffix: string, newSuffix: string) {
    const from = normalizeToolExtension(oldSuffix);
    const to = normalizeToolExtension(newSuffix);
    if (!from || from === "*" || from === to) return;
    if (!to || to === "*") {
      removeToolType(shell, kind, from);
      return;
    }
    const owner = toolsOf(shell, kind).find((tool) => tool.extensions.some((item) => normalizeToolExtension(item) === from));
    if (!owner) return;
    const tools = toolsOf(shell, kind).map((tool) => {
      const extensions = tool.extensions.filter((item) => {
        const value = normalizeToolExtension(item);
        return value !== from && value !== to;
      });
      if (tool.id === owner.id) extensions.push(to);
      return { ...tool, extensions: cleanedExtensions(extensions) };
    });
    setTools(shell, kind, tools);
    applyToolSettings(shell);
    void shell.savePrefs(false);
  }

export async function browseToolPath(shell: ShellState, kind: ToolKind, toolId: string) {
    if (!shell.inApp()) return;
    const tool = toolsOf(shell, kind).find((item) => item.id === toolId);
    try {
      const file = await invoke<string | null>("pick_file", { initial: tool?.path.trim() || null, kind: "program" });
      if (!file) return;
      const tools = toolsOf(shell, kind).map((item) => {
        if (item.id !== toolId) return item;
        const name = item.name.trim() || toolLabel({ name: "", path: file });
        return { ...item, name, path: file };
      });
      setTools(shell, kind, tools);
      applyToolSettings(shell);
      void shell.savePrefs(false);
    } catch (error) {
      shell.actionError = shell.message(error);
    }
  }

export async function importTools(shell: ShellState, kind: ToolKind, source: ToolSource) {
    let incoming: { name: string; path: string; args: string }[] = [];
    try {
      if (source === "common") {
        const rows = await invoke<CommonTool[]>("common_tools");
        incoming = rows
          .map((row) => ({ name: row.name, path: row.path, args: kind === "merge" ? row.mergeArgs : row.diffArgs }))
          .filter((row) => row.args.trim().length > 0);
      } else {
        const catalog = kind === "merge" ? shell.mergeTools : shell.diffTools;
        incoming = catalog.map((item) => ({ name: item.name, path: item.path, args: item.args }));
      }
    } catch (error) {
      shell.actionError = shell.message(error);
      return 0;
    }
    const tools = [...toolsOf(shell, kind)];
    let added = 0;
    for (const item of incoming) {
      if (!item.path.trim() || tools.some((tool) => shell.pathsMatch(tool.path, item.path))) continue;
      tools.push({ id: newToolId(), name: item.name, path: item.path, args: item.args, extensions: [], fallback: false });
      added += 1;
    }
    if (added > 0) {
      setTools(shell, kind, tools);
      applyToolSettings(shell);
      void shell.savePrefs(false);
    }
    return added;
  }

/** Open the editor picker and scan install paths only now, when the user asked for it. */
export function openEditorImport(shell: ShellState) {
    shell.dialog = "editorImport";
    shell.editorCandidates = [];
    shell.editorPicked = [];
    shell.editorDetecting = false;
    void detectEditors(shell);
  }

export async function detectEditors(shell: ShellState) {
    shell.editorDetecting = true;
    try {
      const rows = await invoke<EditorTool[]>("editor_tools");
      if (shell.dialog !== "editorImport") return;
      shell.editorCandidates = rows;
      shell.editorPicked = shell.editorPicked.filter((path) => rows.some((row) => shell.pathsMatch(row.path, path)));
    } catch (error) {
      if (shell.dialog === "editorImport") shell.actionError = shell.message(error);
    } finally {
      shell.editorDetecting = false;
    }
  }

export function toggleEditorCandidate(shell: ShellState, path: string) {
    const on = shell.editorPicked.some((item) => shell.pathsMatch(item, path));
    shell.editorPicked = on
      ? shell.editorPicked.filter((item) => !shell.pathsMatch(item, path))
      : [...shell.editorPicked, path];
  }

export function importEditorCandidates(shell: ShellState) {
    const picked = shell.editorCandidates.filter((row) => shell.editorPicked.some((path) => shell.pathsMatch(path, row.path)));
    const tools = [...toolsOf(shell, "editor")];
    const fresh: ToolRecord[] = [];
    for (const row of picked) {
      if (!row.path.trim() || tools.some((tool) => shell.pathsMatch(tool.path, row.path))) continue;
      const tool: ToolRecord = { id: newToolId(), name: row.name, path: row.path, args: row.args, extensions: [], fallback: false };
      tools.push(tool);
      fresh.push(tool);
    }
    if (fresh.length > 0) {
      setTools(shell, "editor", tools);
      shell.toolPick = fresh[0].id;
      void shell.savePrefs(false);
    }
    closeEditorImport(shell);
  }

export function closeEditorImport(shell: ShellState) {
    shell.editorCandidates = [];
    shell.editorPicked = [];
    shell.editorDetecting = false;
    shell.dialog = "prefs";
    shell.prefTab = "integration";
    shell.toolKind = "editor";
  }

export async function loadExternalTools(shell: ShellState) {
    if (shell.labPage) return;
    if (!shell.backend()) {
      shell.mergeTools = [];
      shell.diffTools = [];
      return;
    }
    let catalog: { merge: ExternalTool[]; diff: ExternalTool[] };
    try {
      catalog = await invoke("external_tools");
    } catch {
      return;
    }
    shell.mergeTools = catalog.merge ?? [];
    shell.diffTools = catalog.diff ?? [];
    // Editors are detected only from the "Detect" button, never at startup.
    let changed = repairToolList(shell, "editor").changed;
    for (const kind of ["diff", "merge"] as const) {
      changed = ensureToolList(shell, kind, kind === "merge" ? shell.mergeTools : shell.diffTools) || changed;
    }
    if (changed) {
      applyToolSettings(shell);
      await shell.savePrefs(false);
    }
  }

function repairToolList(shell: ShellState, kind: ToolKind) {
    const list = toolsOf(shell, kind);
    let changed = false;
    const repaired = list.map((tool) => {
      if (tool.id && tool.extensions && typeof tool.fallback === "boolean") return tool;
      changed = true;
      return { ...tool, id: tool.id || newToolId(), extensions: tool.extensions ?? [], fallback: tool.fallback ?? false };
    });
    if (changed) setTools(shell, kind, repaired);
    return { list: repaired, changed };
  }

function ensureToolList(shell: ShellState, kind: ToolKind, catalog: { name: string; path: string; args: string; primary?: boolean }[]) {
    const { list: repaired, changed } = repairToolList(shell, kind);
    if (repaired.length > 0) return changed;
    const path = legacyToolPath(shell, kind).trim();
    if (path) {
      const record: ToolRecord = {
        id: newToolId(),
        name: legacyToolName(shell, kind).trim() || toolLabel({ name: "", path }),
        path,
        args: legacyToolArgs(shell, kind),
        extensions: [],
        fallback: true,
      };
      setTools(shell, kind, [record]);
      return true;
    }
    const preset = catalog.find((item) => item.primary) ?? catalog[0];
    if (!preset) return false;
    setTools(shell, kind, [{ id: newToolId(), name: preset.name, path: preset.path, args: preset.args, extensions: [], fallback: true }]);
    return true;
  }

export function fillCommand(shell: ShellState, template: string, answers: Record<string, string>) {
    let text = template;
    const entries = Object.entries(answers);
    entries.forEach(([label, value], index) => {
      text = text.replaceAll(`\${${label}}`, value).replaceAll(`\${${index + 1}}`, value);
      if (index === 0) text = text.replaceAll("${input}", value);
    });
    return text;
  }

export function sendCommand(shell: ShellState, command: string) {
    const text = shell.fillCommand(command, {});
    if (/^https?:\/\//i.test(text.trim())) {
      void openUrl(text.trim());
      shell.dialog = null;
      return;
    }
    void shell.mutate({
      action: "custom",
      command: text,
      repo: shell.snapshot?.path ?? "",
      sha: shell.selectedCommit ?? "",
      branch: shell.snapshot?.branch ?? "",
      file: shell.selectedPath ?? shell.historyFile ?? "",
    });
  }

export function runCommand(shell: ShellState, command: Pick<CommandRecord, "command" | "prompt">) {
    const fields = command.prompt.split("\n").map((item) => item.trim()).filter(Boolean);
    if (fields.length > 0) {
      shell.pendingCommand = { id: "", name: "", target: "", command: command.command, prompt: command.prompt, shared: true, repo: "" };
      shell.draft = "";
      shell.fieldValues = Object.fromEntries(fields.map((label) => [label, ""]));
      shell.dialog = "askInput";
      return;
    }
    shell.sendCommand(command.command);
  }

export function saveCommand(shell: ShellState) {
    const name = shell.draft.trim();
    const command = shell.draftUser.trim();
    if (!name || !command) return;
    const record: CommandRecord = { id: `cmd-${Date.now()}`, name, target: shell.draftExtra || "repository", command, prompt: shell.commandPrompt.trim(), shared: shell.commandShared, repo: shell.snapshot?.path ?? "" };
    shell.settings = { ...shell.settings, commands: [...shell.settings.commands, record] };
    shell.draft = "";
    shell.draftUser = "";
    void shell.savePrefs(false);
  }

export function removeCommand(shell: ShellState, id: string) {
    shell.settings = { ...shell.settings, commands: shell.settings.commands.filter((command) => command.id !== id) };
    void shell.savePrefs(false);
  }

export function saveAccount(shell: ShellState) {
    const account = { id: shell.editingAccount || `acct-${Date.now()}`, forge: shell.hostForge, label: shell.draft || shell.hostForge, token: shell.draftUser, host: shell.draftExtra, clientId: shell.settings.oauthClientId };
    const accounts = shell.editingAccount ? shell.settings.accounts.map((item) => (item.id === shell.editingAccount ? account : item)) : [...shell.settings.accounts, account];
    shell.settings = { ...shell.settings, accounts };
    shell.editingAccount = account.id;
    void shell.savePrefs(false);
  }

export function openRebase(shell: ShellState) {
    if (!shell.selectedCommit) return;
    const source = shell.mode === "live" ? shell.commitsLive : commits.map((commit) => ({ id: commit.id, summary: commit.summary }));
    const index = source.findIndex((commit) => commit.id === shell.selectedCommit);
    const newer = index > 0 ? source.slice(0, index).slice().reverse() : [];
    if (newer.length === 0) {
      shell.actionError = shell.tr("dialog.rebaseNothing");
      return;
    }
    shell.updateRefs = shell.settings.rebaseUpdateRefs;
    shell.rebaseSteps = newer.map((commit) => ({ verb: "pick", rev: commit.id, summary: commit.summary, message: commit.summary }));
    shell.rebasePicked = newer.map((commit) => commit.id);
    shell.rebaseConflictFiles = [];
    shell.rebaseConflictKnown = false;
    shell.dialog = "rebase";
    if (shell.mode === "live") {
      const path = shell.repoPath();
      const onto = shell.selectedCommit;
      const stillEditing = () => shell.dialog === "rebase" && shell.mode === "live" && shell.selectedCommit === onto && shell.viewing(path);
      void invoke<string[]>("rebase_conflicts", { path, onto })
        .then((files) => { if (stillEditing()) { shell.rebaseConflictFiles = files; shell.rebaseConflictKnown = true; } })
        .catch(() => { if (stillEditing()) shell.rebaseConflictKnown = false; });
    }
  }

export function moveStep(shell: ShellState, index: number, delta: number) {
    const next = index + delta;
    if (next < 0 || next >= shell.rebaseSteps.length) return;
    const copy = [...shell.rebaseSteps];
    const [item] = copy.splice(index, 1);
    copy.splice(next, 0, item);
    shell.rebaseSteps = copy;
  }

/** Backend marker: the branch has commits that exist nowhere else. */
const NOT_MERGED = "branch is not fully merged";

/** Offer the force delete only after Git reported commits that would be lost. */
export async function deleteBranchNow(shell: ShellState, name: string, force = false) {
    const outcome = await shell.mutate({ action: "deleteBranch", name, force });
    if (outcome !== "failed" || force) return;
    if (!(shell.actionError ?? "").includes(NOT_MERGED)) return;
    shell.actionError = null;
    shell.ask(shell.tr("dialog.forceDeleteBranch"), shell.tr("dialog.forceDeleteBranchBody"), () => void deleteBranchNow(shell, name, true), [], {
      subject: name,
      danger: true,
      confirmLabel: shell.tr("menu.forceDelete"),
    });
  }

export function removeBranch(shell: ShellState, name: string) {
    shell.ask(shell.tr("dialog.deleteBranch"), shell.tr("dialog.deleteBranchBody"), () => void deleteBranchNow(shell, name), [], {
      subject: name,
      danger: true,
    });
  }

/** Cancel a running command and offer to clear an index.lock the kill left behind. */
export async function cancelOperation(shell: ShellState, path: string) {
    try {
      const lock = await invoke<string | null>("cancel_operation", { path });
      if (!lock) return;
      shell.ask(
        shell.tr("dialog.staleIndexLock"),
        shell.tr("dialog.staleIndexLockBody"),
        () => void invoke<boolean>("remove_index_lock", { path }).catch((error) => (shell.actionError = shell.message(error))),
        [],
        { subject: lock, danger: true, confirmLabel: shell.tr("menu.delete") },
      );
    } catch (error) {
      shell.actionError = shell.message(error);
    }
  }

export function hardReset(shell: ShellState) {
    shell.ask(shell.tr("dialog.hardReset"), shell.tr("dialog.hardResetBody"), () => void shell.mutate({ action: "reset", rev: shell.draft || "HEAD", mode: "hard" }), [], { danger: true });
  }

export async function selectWorkspace(shell: ShellState) {
    await shell.savePrefs(false);
    const workspace = shell.settings.workspaces.find((item) => item.id === shell.settings.currentWorkspace);
    if (!workspace) return;
    const tabs = workspace.openTabs.length > 0 ? [...workspace.openTabs] : [...workspace.repositories];
    shell.repos = tabs;
    const path = tabs[Math.min(workspace.selectedTab, Math.max(0, tabs.length - 1))] ?? tabs[0];
    if (path) {
      await shell.openRepo(path);
      return;
    }
    shell.snapshot = null;
    shell.refs = null;
    shell.commitsLive = [];
    shell.selectedCommit = null;
    shell.mode = "error";
    shell.loadError = "";
  }

export async function switchWorkspace(shell: ShellState, id: string) {
    if (id === shell.settings.currentWorkspace || shell.busy) return;
    if (id === recentWorkspaceId) shell.syncRecentWorkspace();
    shell.settings = { ...shell.settings, currentWorkspace: id };
    await shell.selectWorkspace();
  }

export async function createWorkspace(shell: ShellState, name: string) {
    const label = name.trim();
    if (!label) return;
    let list = shell.settings.workspaces;
    const userWorkspaces = list.filter((item) => item.id !== recentWorkspaceId);
    if (userWorkspaces.length === 0 && shell.repos.length > 0) {
      list = [
        ...list.filter((item) => item.id === recentWorkspaceId),
        {
          id: `ws-default-${Date.now()}`,
          name: shell.tr("chrome.workspaceDefault"),
          repositories: [...shell.repos],
          openTabs: [...shell.repos],
          selectedTab: Math.max(0, shell.repos.indexOf(shell.snapshot?.path ?? "")),
          tabWidth: workspaceTabWidth(shell.repos.length),
        },
      ];
    }
    const id = `ws-${Date.now()}`;
    shell.settings = { ...shell.settings, workspaces: [...list, { id, name: label, repositories: [], openTabs: [], selectedTab: 0, tabWidth: workspaceTabWidth(0) }], currentWorkspace: id };
    await shell.selectWorkspace();
  }

export function renameWorkspace(shell: ShellState, id: string, name: string) {
    if (id === recentWorkspaceId) return;
    const label = name.trim();
    if (!label) return;
    shell.settings = { ...shell.settings, workspaces: shell.settings.workspaces.map((item) => (item.id === id ? { ...item, name: label } : item)) };
    void shell.savePrefs(false);
  }

export async function deleteWorkspace(shell: ShellState, id: string) {
    if (id === recentWorkspaceId) return;
    const rest = shell.settings.workspaces.filter((item) => item.id !== id);
    const wasCurrent = shell.settings.currentWorkspace === id;
    shell.settings = { ...shell.settings, workspaces: rest, currentWorkspace: wasCurrent ? (rest[0]?.id ?? "") : shell.settings.currentWorkspace };
    if (wasCurrent && rest[0]) await shell.selectWorkspace();
    else await shell.savePrefs(false);
  }

export function openRepoSettings(shell: ShellState) {
    shell.draft = shell.settings.authorName;
    shell.draftExtra = shell.settings.authorEmail;
    shell.draftUser = shell.refs?.signOffFormat || shell.settings.signOffFormat;
    shell.repoSignOff = shell.refs?.signOffSet ? !!shell.refs.signOff : shell.settings.signOff;
    shell.dialog = "repo";
  }

export async function keepPassphrase(shell: ShellState) {
    await invoke("set_passphrase", { user: shell.passUser, secret: shell.passphrase });
    shell.passphrase = "";
  }

export async function loadMergePreview(shell: ShellState) {
    const source = shell.draft.trim();
    const into = shell.snapshot?.branch || "HEAD";
    shell.mergeInto = into;
    if (shell.mode !== "live" || !source) {
      shell.mergeFiles = [];
      fillMergeText(shell, into, source);
      return;
    }
    const preview = await invoke<{ into: string; files: { status: string; path: string }[] }>("merge_preview", {
      path: shell.repoPath(),
      name: source,
    }).catch(() => null);
    if (shell.dialog !== "merge" || shell.draft.trim() !== source) return;
    shell.mergeInto = preview?.into || into;
    shell.mergeFiles = preview?.files ?? [];
    fillMergeText(shell, shell.mergeInto, source);
  }

export function refreshMergeSummary(shell: ShellState) {
    fillMergeText(shell, shell.mergeInto || shell.snapshot?.branch || "HEAD", shell.draft.trim());
  }

function fillMergeText(shell: ShellState, into: string, source: string) {
    const summary = shell.tr(shell.draftExtra === "squash" ? "dialog.mergeSquashTitle" : "dialog.mergeTitleLine", { source, into });
    if (shell.mergeSummary === shell.mergeAutoSummary) shell.mergeSummary = summary;
    shell.mergeAutoSummary = summary;
  }

export async function saveWindow(shell: ShellState) {
    if (!shell.inApp()) return;
    const win = getCurrentWindow();
    const factor = await win.scaleFactor();
    const size = await win.innerSize();
    const position = await win.outerPosition();
    shell.settings.windowWidth = Math.round(size.width / factor);
    shell.settings.windowHeight = Math.round(size.height / factor);
    shell.settings.windowX = Math.round(position.x / factor);
    shell.settings.windowY = Math.round(position.y / factor);
    shell.settings = await invoke<Settings>("save_settings", { values: shell.settings });
  }
