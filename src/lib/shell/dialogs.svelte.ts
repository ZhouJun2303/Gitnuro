import { onMount, tick } from "svelte";
import { invoke } from "../perf";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { commits, diffs, sampleConflictText, sampleConflictViews, sampleImages, sampleNotices, samplePulls, sampleReflog, staged as sampleStaged, unstaged as sampleUnstaged, type FileChange } from "../sample";
import { clipboardRemote, cloneTarget, cloneUrl, remoteFromText, repoNameFromUrl, safeFolderName } from "../clonePath";
import {
    badge,
    type BlameLine,
    type CommandRecord,
    type CommitDetail,
    type CommitRow,
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
    type UpdateNotice,
    workspaceTabWidth,
  } from "../status";
import type { Mode, Side, Row, Dialog, MergePiece, RepoFacts, MenuItem, ConfirmAsk, TreeEntry, RepoEntry, CommandEntry, CommandFilter, RepoJob, PrefTab, LaunchItem, Place, SearchKey, StatusNoticeId } from "./types";
import { STATUS_NOTICE_ENTRY } from "./types";
import type { ShellState } from "./state.svelte";
import { alignCommitTemplate, isCommentary, isCommitType, splitSubject, withoutRepeatedSubject } from "./commitMessage";

export async function selectPrefTab(shell: ShellState, tab: PrefTab) {
    shell.prefTab = tab;
    if (tab === "integration" && !shell.labPage) void shell.loadExternalTools();
    await tick();
    shell.prefPage?.scrollTo(0, 0);
  }

export async function openLaunch(shell: ShellState) {
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
    shell.menu = null;
    shell.submenu = null;
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
        pieces.push({ kind: "hunk", ours: ours.join("\n"), theirs: theirs.join("\n"), pick: "ours" });
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
      .map((piece) => {
        if (piece.kind === "text") return piece.text;
        if (piece.pick === "theirs") return piece.theirs;
        if (piece.pick === "both") return [piece.ours, piece.theirs].filter((part) => part.length > 0).join("\n");
        return piece.ours;
      })
      .join("\n");
  }

export function pickHunk(shell: ShellState, index: number, pick: "ours" | "theirs" | "both") {
    shell.mergePieces = shell.mergePieces.map((piece, at) => (piece.kind === "hunk" && at === index ? { ...piece, pick } : piece));
    shell.mergeResult = shell.joinedPieces(shell.mergePieces);
  }

export function takeMergeSide(shell: ShellState, pick: "ours" | "theirs") {
    shell.mergePieces = shell.mergePieces.map((piece) => (piece.kind === "hunk" ? { ...piece, pick } : piece));
    shell.mergeResult = shell.joinedPieces(shell.mergePieces);
  }

function piecesFromSides(shell: ShellState, sides: { ours: string; theirs: string; working: string }) {
    const pieces = shell.splitConflicts(sides.working);
    if (pieces.some((piece) => piece.kind === "hunk")) return pieces;
    return [{ kind: "hunk" as const, ours: sides.ours, theirs: sides.theirs, pick: "ours" as const }];
  }

/** Past this, drawing every line would stall the window. Show the file card instead. */
const MERGE_TEXT_LIMIT = 200_000;

function textTooBig(text: string) {
    if (text.length > MERGE_TEXT_LIMIT) return true;
    let lines = 1;
    for (let index = 0; index < text.length; index += 1) {
      if (text.charCodeAt(index) === 10 && ++lines > 8000) return true;
    }
    return false;
  }

function showFileMerge(shell: ShellState, file: string) {
    shell.mergeFile = file;
    shell.mergeKind = "file";
    shell.mergePieces = [];
    shell.mergeResult = "";
    shell.conflict = null;
    shell.textMerge = true;
  }

export async function openTextMerge(shell: ShellState, file: string) {
    shell.section = "changes";
    const already = shell.selectedPath === file;
    shell.selectedPath = file;
    let view = shell.mode === "sample" ? sampleConflictViews[file] : already ? shell.activeConflict : null;
    if (shell.mode === "live" && !view) {
      try {
        view = await invoke<ConflictView>("conflict_view", { path: shell.repoPath(), file });
        if (shell.selectedPath === file) shell.conflictView = view;
      } catch (error) {
        shell.actionError = shell.message(error);
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
      const sides = shell.mode === "sample" ? sampleConflictText[file] : await invoke<ConflictSides>("conflict_sides", { path: shell.repoPath(), file });
      if (!sides || textTooBig(sides.working) || textTooBig(sides.ours) || textTooBig(sides.theirs)) {
        showFileMerge(shell, file);
        return;
      }
      shell.conflict = sides;
      shell.mergeFile = file;
      shell.mergeKind = "text";
      shell.mergePieces = piecesFromSides(shell, sides);
      shell.mergeResult = shell.joinedPieces(shell.mergePieces);
      shell.textMerge = true;
    } catch (error) {
      shell.actionError = shell.message(error);
    }
  }

export function closeTextMerge(shell: ShellState) {
    shell.textMerge = false;
  }

export async function applyTextMerge(shell: ShellState) {
    const file = shell.mergeFile;
    if (!file) return;
    if (shell.mode !== "live") {
      shell.textMerge = false;
      return;
    }
    if (shell.busy) return;
    await shell.mutate({ action: "resolve", file, side: "text", text: shell.mergeResult });
    shell.textMerge = false;
  }

export async function applyFileMerge(shell: ShellState, side: "ours" | "theirs") {
    const file = shell.mergeFile;
    if (!file) return;
    if (shell.mode !== "live") {
      shell.textMerge = false;
      return;
    }
    if (shell.busy) return;
    await shell.mutate({ action: "resolve", file, side, text: "" });
    shell.textMerge = false;
  }

export async function openResolve(shell: ShellState, file: string) {
    await openTextMerge(shell, file);
  }

export async function fillResolveDialog(shell: ShellState, file: string) {
    shell.draft = file;
    shell.draftExtra = "text";
    try {
      const sides = await invoke<ConflictSides>("conflict_sides", { path: shell.repoPath(), file });
      shell.conflict = sides;
      if (!sides || textTooBig(sides.working) || textTooBig(sides.ours) || textTooBig(sides.theirs)) {
        shell.mergePieces = [];
        shell.mergeResult = sides?.working ?? "";
        return;
      }
      shell.mergePieces = piecesFromSides(shell, sides);
      shell.mergeResult = shell.joinedPieces(shell.mergePieces);
    } catch {
      shell.conflict = null;
      shell.mergePieces = [];
      shell.mergeResult = "";
    }
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
    if (shell.settings.checkoutStash) {
      void checkoutWithStash(shell, name);
      return;
    }
    shell.pendingCheckout = name;
    shell.draftExtra = "bring";
    shell.dialog = "checkoutDirty";
  }

async function checkoutWithStash(shell: ShellState, name: string, remote?: { remote: string; branch: string }) {
    await shell.mutate({ action: "stash", message: shell.tr("dialog.checkoutStashMessage", { name }), includeUntracked: shell.settings.stashIncludeUntracked });
    if (shell.actionError) return;
    if (remote) await shell.mutate({ action: "checkoutRemote", remote: remote.remote, branch: remote.branch });
    else await shell.mutate({ action: "checkout", name });
    if (shell.actionError) return;
    await shell.mutate({ action: "stashPop", name: "stash@{0}" });
    if (shell.actionError) {
      shell.pendingCheckout = name;
      shell.dialog = "checkoutNotice";
    }
  }

export async function quickCheckout(shell: ShellState, name: string, remote?: { remote: string; branch: string }) {
    if (shell.mode !== "live") return;
    const dirty = (shell.snapshot?.staged.length ?? 0) + (shell.snapshot?.unstaged.length ?? 0) > 0;
    if (dirty && shell.settings.checkoutStash) {
      await checkoutWithStash(shell, name, remote);
      return;
    }
    if (dirty) {
      await shell.mutate({ action: "stash", message: shell.tr("dialog.checkoutStashMessage", { name }), includeUntracked: true });
      if (shell.actionError) return;
    }
    if (remote) await shell.mutate({ action: "checkoutRemote", remote: remote.remote, branch: remote.branch });
    else await shell.mutate({ action: "checkout", name });
    if (shell.actionError) return;
    if (dirty) {
      shell.pendingCheckout = name;
      shell.dialog = "checkoutNotice";
    }
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
    shell.syncStash = shell.settings.pullStash;
    shell.syncForce = false;
    shell.pushBranch = current?.name ?? shell.snapshot?.branch ?? "";
    shell.pushTo = "";
    shell.draft = upstreamRemote || firstRemote;
    shell.draftExtra = kind === "pull" ? (upstreamRemote ? upstream.slice(upstreamRemote.length + 1) : shell.snapshot?.branch ?? "") : "";
    shell.dialog = kind;
  }

export function ask(shell: ShellState, title: string, body: string, run: () => void) {
    shell.confirmAsk = { title, body, run };
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
    if (kind === "notes") shell.notices = await invoke<({ id: string; title: string; url: string; unread: boolean }[])>("forge_notifications").catch(() => []);
    else shell.pulls = await invoke<({ title: string; author: string; branch: string; url: string }[])>("forge_pulls", { path: shell.repoPath() }).catch(() => []);
  }

export function setRebaseVerb(shell: ShellState, rev: string, verb: string) {
    const targets = shell.rebasePicked.includes(rev) && shell.rebasePicked.length > 1 ? new Set(shell.rebasePicked) : new Set([rev]);
    shell.rebaseSteps = shell.rebaseSteps.map((step) => (targets.has(step.rev) ? { ...step, verb } : step));
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
    if (shell.mode !== "live" || !shell.canCommit) return;
    const messageText = shell.fullSummary.trim();
    if (!messageText) return;
    const description = isCommentary(shell.description) ? "" : shell.description;
    const value = await shell.runChange("commit_changes", { summary: messageText, description, amend: shell.amend, signOff: shell.signOff, skipHooks: shell.skipHooks });
    if (value === undefined) return;
    const failed = value ? "" : (shell.actionError ?? "");
    shell.recordCommand("commit", { title: shell.tr("chrome.commandCommit", { name: messageText || "HEAD" }), command: shell.gitLine(["commit", "-m", messageText]) }, (value?.log ?? failed).trim(), !!value, "user");
    if (!shell.actionError) {
      shell.summary = "";
      shell.description = "";
      shell.amend = false;
      shell.skipHooks = false;
      if (shell.snapshot?.path) shell.drafts.delete(shell.snapshot.path);
      await shell.loadContext();
      void shell.fillTemplate();
      if (pushAfter || shell.settings.pushOnCommit) await shell.doPush();
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

export async function suggestMessage(shell: ShellState) {
    const path = shell.repoPath();
    if (shell.busy) return;
    shell.actionError = null;
    if (shell.mode !== "live" || !path) {
      shell.actionError = shell.tr("chrome.suggestNeedRepo");
      return;
    }
    if (!shell.settings.aiEnabled) {
      shell.actionError = shell.tr("chrome.suggestOff");
      return;
    }
    if (shell.staged.length === 0) {
      shell.actionError = shell.tr("chrome.suggestNeedStaged");
      return;
    }
    const style = shell.settings.aiPromptStyle === "long" ? "long" : "short";
    const styleName = shell.tr(style === "long" ? "chrome.promptLong" : "chrome.promptShort");
    const language = shell.settings.aiLanguage.trim() || (shell.activeLocale === "zh" ? "中文" : "English");
    shell.startWork(shell.tr("chrome.suggestGenerate", { style: styleName }), path);
    try {
      const value = await invoke<Suggestion>("suggest_commit_message", { path, style, language });
      if (shell.viewing(path)) {
        shell.applySubject(value.summary);
        shell.description = withoutRepeatedSubject(value.description, shell.commitType, shell.summary);
      }
    } catch (error) {
      if (shell.viewing(path)) shell.actionError = explainSuggest(shell, error);
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
    shell.settings.diffTool = shell.composeTool(shell.settings.diffToolPath, shell.settings.diffToolArgs);
    shell.settings.mergeTool = shell.composeTool(shell.settings.mergeToolPath, shell.settings.mergeToolArgs);
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
    try {
      const saved = await invoke<Settings>("save_settings", { values });
      if (ticket === shell.prefsSave) {
        const commitType = isCommitType(saved.commitType) ? saved.commitType : values.commitType;
        shell.settings = isCommitType(commitType) ? { ...saved, commitType } : saved;
      }
    } catch (error) {
      if (ticket === shell.prefsSave) shell.actionError = shell.message(error);
    }
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
    void invoke("cancel_operation", { path: shell.cloneJob?.destination ?? "" });
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
    try {
      await openPath(path);
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
    const mergeChanged = await adoptTool(shell, "merge");
    const diffChanged = await adoptTool(shell, "diff");
    if (mergeChanged || diffChanged) {
      shell.applyToolSettings();
      await shell.savePrefs(false);
    }
  }

async function adoptTool(shell: ShellState, kind: "merge" | "diff") {
    const path = (kind === "merge" ? shell.settings.mergeToolPath : shell.settings.diffToolPath).trim();
    if (path) {
      try {
        if (await invoke<boolean>("program_exists", { path })) return false;
      } catch {
        return false;
      }
    }
    const tools = kind === "merge" ? shell.mergeTools : shell.diffTools;
    return writeTool(shell, kind, tools.find((item) => item.primary) ?? tools[0] ?? null);
  }

function writeTool(shell: ShellState, kind: "merge" | "diff", tool: ExternalTool | null) {
    const name = tool?.name ?? "";
    const path = tool?.path ?? "";
    const args = tool?.args ?? "";
    if (kind === "merge") {
      if (shell.settings.mergeToolName === name && shell.settings.mergeToolPath === path && shell.settings.mergeToolArgs === args) return false;
      shell.settings = { ...shell.settings, mergeToolName: name, mergeToolPath: path, mergeToolArgs: args };
      return true;
    }
    if (shell.settings.diffToolName === name && shell.settings.diffToolPath === path && shell.settings.diffToolArgs === args) return false;
    shell.settings = { ...shell.settings, diffToolName: name, diffToolPath: path, diffToolArgs: args };
    return true;
  }

export function useMergePreset(shell: ShellState, path: string) {
    const preset = shell.mergeTools.find((item) => shell.pathsMatch(item.path, path));
    if (!preset) return;
    writeTool(shell, "merge", preset);
  }

export function useDiffPreset(shell: ShellState, path: string) {
    const preset = shell.diffTools.find((item) => shell.pathsMatch(item.path, path));
    if (!preset) return;
    writeTool(shell, "diff", preset);
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
    const newer = index > 0 ? source.slice(0, index).slice().reverse() : source.slice(0, 3);
    shell.updateRefs = shell.settings.rebaseUpdateRefs;
    shell.rebaseSteps = newer.map((commit) => ({ verb: "pick", rev: commit.id, summary: commit.summary, message: commit.summary }));
    shell.rebasePicked = [];
    shell.rebaseConflictFiles = [];
    shell.dialog = "rebase";
    if (shell.mode === "live") {
      void invoke<string[]>("rebase_conflicts", { path: shell.repoPath(), onto: shell.selectedCommit }).then((files) => (shell.rebaseConflictFiles = files)).catch(() => (shell.rebaseConflictFiles = []));
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

export function removeBranch(shell: ShellState, name: string) {
    shell.ask(shell.tr("dialog.deleteBranch"), shell.tr("dialog.deleteBranchBody", { name }), () => void shell.mutate({ action: "deleteBranch", name }));
  }

export function hardReset(shell: ShellState) {
    shell.ask(shell.tr("dialog.hardReset"), shell.tr("dialog.hardResetBody"), () => void shell.mutate({ action: "reset", rev: shell.draft || "HEAD", mode: "hard" }));
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
    shell.settings = { ...shell.settings, currentWorkspace: id };
    await shell.selectWorkspace();
  }

export async function createWorkspace(shell: ShellState, name: string) {
    const label = name.trim();
    if (!label) return;
    let list = shell.settings.workspaces;
    if (list.length === 0 && shell.repos.length > 0) {
      list = [{
        id: `ws-default-${Date.now()}`,
        name: shell.tr("chrome.workspaceDefault"),
        repositories: [...shell.repos],
        openTabs: [...shell.repos],
        selectedTab: Math.max(0, shell.repos.indexOf(shell.snapshot?.path ?? "")),
        tabWidth: workspaceTabWidth(shell.repos.length),
      }];
    }
    const id = `ws-${Date.now()}`;
    shell.settings = { ...shell.settings, workspaces: [...list, { id, name: label, repositories: [], openTabs: [], selectedTab: 0, tabWidth: workspaceTabWidth(0) }], currentWorkspace: id };
    await shell.selectWorkspace();
  }

export function renameWorkspace(shell: ShellState, id: string, name: string) {
    const label = name.trim();
    if (!label) return;
    shell.settings = { ...shell.settings, workspaces: shell.settings.workspaces.map((item) => (item.id === id ? { ...item, name: label } : item)) };
    void shell.savePrefs(false);
  }

export async function deleteWorkspace(shell: ShellState, id: string) {
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
    const body = shell.mergeFiles.map((file) => `${file.status}\t${file.path}`).join("\n");
    if (shell.mergeSummary === shell.mergeAutoSummary) shell.mergeSummary = summary;
    shell.mergeAutoSummary = summary;
    if (shell.mergeBody === shell.mergeAutoBody) shell.mergeBody = body;
    shell.mergeAutoBody = body;
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
