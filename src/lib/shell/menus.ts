import { invoke } from "../perf";
import { openUrl } from "@tauri-apps/plugin-opener";
import { shortcut, shortcutLabel, typing } from "../keys";
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
  } from "../status";
import type { Mode, Side, Row, Dialog, MergePiece, RepoFacts, MenuItem, ConfirmAsk, TreeEntry, RepoEntry, CommandEntry, RepoJob, PrefTab, LaunchItem, Place, SearchKey } from "./types";
import type { ShellState } from "./state.svelte";
import { launchExternal } from "./diff.svelte";
import { openInEditor } from "./dialogs.svelte";

export function branchItems(shell: ShellState) {
    const rows = shell.refs?.branches ?? [];
    if (shell.mode !== "live" || rows.length === 0) return [{ label: shell.branchLabel, disabled: true }];
    return rows.slice(0, 30).map((branch) => ({
      label: branch.name,
      checked: branch.current,
      disabled: branch.current || shell.busy,
      run: () => void shell.mutate({ action: "checkout", name: branch.name }),
    }));
  }

export function folderFoldItems(shell: ShellState): MenuItem[] {
    const locked = shell.fileLayout !== "tree";
    return [
      { label: shell.tr("chrome.expandAll"), disabled: locked, run: () => shell.setFolderFold("open-all") },
      { label: shell.tr("chrome.collapseAll"), disabled: locked, run: () => shell.setFolderFold("close-all") },
      { label: shell.tr("chrome.expandFirstLevel"), disabled: locked, run: () => shell.setFolderFold("open-first") },
      { label: shell.tr("chrome.collapseFirstLevel"), disabled: locked, run: () => shell.setFolderFold("close-first") },
    ];
  }

export function fileViewMenu(shell: ShellState) {
    const picked = shell.pickedIn(shell.shownUnstaged);
    const selection: MenuItem[] = picked.length > 1
      ? [
          { label: shell.tr("menu.discardSelected", { count: String(picked.length) }), disabled: shell.mode !== "live" || shell.busy, shortcut: shortcutLabel("discard"), run: () => shell.discardChosen(picked) },
          { label: shell.tr("menu.deleteSelected", { count: String(picked.length) }), disabled: shell.mode !== "live" || shell.busy, run: () => shell.deleteChosen(picked) },
          { label: shell.tr("menu.stashFiles", { count: String(picked.length) }), disabled: shell.mode !== "live" || shell.busy, run: () => shell.openStash(picked) },
          { label: shell.tr("menu.addGitignoreSelected", { count: String(picked.length) }), disabled: shell.mode !== "live" || shell.busy, run: () => void shell.addIgnore(picked, false, false) },
          { label: shell.tr("menu.addFakeignoreSelected", { count: String(picked.length) }), disabled: shell.mode !== "live" || shell.busy, run: () => void shell.addIgnore(picked, true, false) },
          { sep: true, label: "" },
        ]
      : [];
    return [
      ...selection,
      { label: shell.tr("chrome.viewAsTree"), checked: shell.fileLayout === "tree", run: () => shell.setLayout("tree") },
      { label: shell.tr("chrome.viewAsList"), checked: shell.fileLayout === "list", run: () => shell.setLayout("list") },
      { label: shell.tr("chrome.viewAsCombined"), checked: shell.fileLayout === "combined", run: () => shell.setLayout("combined") },
      { sep: true, label: "" },
      ...folderFoldItems(shell),
      { sep: true, label: "" },
      { label: shell.tr("chrome.conflictsOnly"), checked: shell.conflictsOnly, run: () => shell.setConflictsOnly(!shell.conflictsOnly) },
      { label: shell.tr("chrome.showIgnoredFiles"), checked: shell.showIgnored, run: () => { shell.showIgnored = !shell.showIgnored; if (shell.showIgnored) void shell.loadFacts(true, true); } },
      { label: shell.tr("chrome.fakeIgnore"), checked: shell.settings.fakeIgnore !== false, run: () => void shell.toggleFakeIgnore() },
    ];
  }

export function fileMenu(shell: ShellState, file: Row, side: Side) {
    const locked = shell.mode !== "live";
    const targets = shell.ignoreTargets(file.path, side);
    const many = targets.length > 1;
    const count = String(targets.length);
    const conflicts = shell.pickedConflicts(targets);
    const sideName = (party: { label: string; commit?: { short: string } | null } | undefined | null, fallback: string) =>
      party ? (party.commit?.short ? `${party.label} · ${party.commit.short}` : party.label) : fallback;
    const oursName = sideName(shell.activeConflict?.ours, shell.mergeState?.into || shell.tr("dialog.useOurs"));
    const theirsName = sideName(shell.activeConflict?.theirs, shell.mergeState?.source || shell.tr("dialog.useTheirs"));
    return [
      { label: many ? `${shell.tr(side === "staged" ? "chrome.unstageSelected" : "chrome.stageSelected")} (${count})` : shell.tr(side === "staged" ? "menu.unstage" : "menu.stage"), disabled: locked, shortcut: shortcutLabel("stageToggle"), run: () => shell.stageChosen(side, targets) },
      { label: many ? shell.tr("menu.discardSelected", { count }) : shell.tr("menu.discard"), disabled: locked || (side === "staged" && many), shortcut: shortcutLabel("discard"), run: () => shell.discardChosen(targets) },
      { label: many ? shell.tr("menu.deleteSelected", { count }) : shell.tr("menu.deleteFile"), disabled: locked, run: () => shell.deleteChosen(targets) },
      { sep: true, label: "" },
      { label: many ? shell.tr("menu.addGitignoreSelected", { count }) : shell.tr("menu.addGitignore"), disabled: locked, run: () => void shell.addIgnore(targets, false, false) },
      { label: many ? shell.tr("menu.addFakeignoreSelected", { count }) : shell.tr("menu.addFakeignore"), disabled: locked, run: () => void shell.addIgnore(targets, true, false) },
      { label: many ? shell.tr("menu.stashFiles", { count }) : shell.tr("menu.stashFile"), disabled: locked, run: () => shell.openStash(targets) },
      { sep: true, label: "" },
      { label: shell.tr("menu.blame"), disabled: locked, run: () => shell.showBlame(file.path) },
      { label: shell.tr("menu.history"), disabled: locked, run: () => void invoke("open_window", { view: "history", repo: shell.repoPath(), file: file.path }) },
      { label: shell.tr("menu.externalDiff"), disabled: locked, children: externalItems(shell, "diff", file.path) },
      { label: shell.tr("menu.externalMerge"), disabled: locked || file.letter !== "C", children: externalItems(shell, "merge", file.path) },
      { label: shell.tr("menu.openInEditor"), disabled: locked || file.letter === "D", children: editorItems(shell, file.path, 0) },
      { label: shell.tr("menu.lfsLock"), disabled: locked || !shell.lfsAvailable, run: () => void shell.mutate({ action: "lfsLock", path: file.path }) },
      { label: shell.tr("menu.lfsUnlock"), disabled: locked || !shell.lfsAvailable, run: () => void shell.mutate({ action: "lfsUnlock", path: file.path }) },
      { sep: true, label: "" },
      { label: shell.tr("menu.reveal"), disabled: locked, run: () => void shell.openLocal(shell.fullPath(file.path)) },
      { label: shell.tr("menu.openFile"), disabled: locked, run: () => void shell.openLocal(shell.fullPath(file.path)) },
      { label: shell.tr("menu.copyPath"), run: () => shell.copyText(shell.fullPath(file.path)) },
      { label: shell.tr("menu.copyRelative"), run: () => shell.copyText(file.path) },
      ...(file.letter === "C" ? [
        { label: shell.tr("menu.keepSideCount", { name: oursName, count: String(conflicts.length) }), disabled: locked, run: () => void shell.resolvePicked("ours", conflicts) },
        { label: shell.tr("menu.keepSideCount", { name: theirsName, count: String(conflicts.length) }), disabled: locked, run: () => void shell.resolvePicked("theirs", conflicts) },
        { label: shell.tr("menu.resolve"), disabled: locked, run: () => shell.openResolve(file.path) },
      ] : []),
      ...shell.commandItems("file"),
    ];
  }

export function commitMenu(shell: ShellState, commit: { id: string; summary: string }) {
    const locked = shell.mode !== "live";
    return [
      { label: shell.tr("menu.checkout"), disabled: locked || !!shell.progress, run: () => shell.requestCheckout(commit.id) },
      { label: shell.tr("menu.cherryPick"), disabled: locked || !!shell.progress, run: () => void shell.previewCherry(commit.id, false, commit.summary) },
      { label: shell.tr("menu.cherryPickRecord"), disabled: locked || !!shell.progress, run: () => void shell.previewCherry(commit.id, true, commit.summary) },
      { label: shell.tr("menu.revert"), disabled: locked || !!shell.progress, run: () => shell.confirmRevert(commit.id, commit.summary) },
      {
        label: shell.tr("menu.reset"),
        children: [
          { label: shell.tr("menu.soft"), disabled: locked || !!shell.progress, run: () => shell.confirmReset(commit.id, "soft", commit.summary) },
          { label: shell.tr("menu.mixed"), disabled: locked || !!shell.progress, run: () => shell.confirmReset(commit.id, "mixed", commit.summary) },
          { label: shell.tr("menu.hard"), disabled: locked || !!shell.progress, run: () => shell.ask(shell.tr("dialog.hardReset"), shell.tr("dialog.hardResetBody"), () => void shell.mutate({ action: "reset", rev: commit.id, mode: "hard" })) },
        ],
      },
      { sep: true, label: "" },
      { label: shell.tr("menu.createBranch"), run: () => { shell.draft = ""; shell.draftExtra = commit.id; shell.dialog = "branch"; } },
      { label: shell.tr("menu.createTag"), run: () => { shell.draft = ""; shell.draftExtra = commit.id; shell.dialog = "tag"; } },
      { label: shell.tr("menu.editMessage"), disabled: locked || !!shell.progress || !shell.refs?.head || commit.id !== shell.refs.head, run: () => { shell.draft = commit.summary; shell.draftExtra = commit.id; shell.dialog = "reword"; } },
      { label: shell.tr("menu.interactiveRebase"), disabled: locked || !!shell.progress, run: () => { shell.selectedCommit = commit.id; shell.openRebase(); } },
      { label: shell.tr("menu.copySha"), run: () => shell.copyText(commit.id) },
      { label: shell.mergeOpen(commit.id) ? shell.tr("menu.fold") : shell.tr("menu.unfold"), disabled: !shell.mergeOwner(commit.id), run: () => shell.toggleFold(commit.id) },
      { label: shell.tr("menu.openWeb"), disabled: !shell.commitWeb(commit.id), run: () => void openUrl(shell.commitWeb(commit.id)) },
      ...(shell.historyFilter ? [{ label: shell.tr("menu.restoreFile"), disabled: locked, run: () => shell.mutate({ action: "restoreFile", rev: commit.id, file: shell.historyFilter }) }] : []),
      ...shell.commandItems("commit"),
    ];
  }

export function reflogMenu(shell: ShellState, row: { id: string; selector: string }) {
    return [
      { label: shell.tr("menu.checkout"), disabled: shell.mode !== "live" || !!shell.progress, run: () => shell.mutate({ action: "checkout", name: row.selector || row.id }) },
      { label: shell.tr("menu.createBranch"), run: () => { shell.draft = ""; shell.draftExtra = row.id; shell.dialog = "branch"; } },
    ];
  }

export function viewItems(shell: ShellState) {
    return [
      { label: shell.tr("chrome.localChanges"), run: () => (shell.section = "changes") },
      { label: shell.tr("chrome.allCommits"), run: () => (shell.section = "history") },
      { label: shell.tr("chrome.fileTree"), disabled: !shell.selectedCommit, run: () => { shell.section = "history"; void shell.openHistoryTab("tree"); } },
      { label: shell.tr("chrome.reflog"), run: () => { shell.section = "history"; void shell.showReflog(); } },
      { label: shell.tr("chrome.loadMore"), disabled: shell.mode !== "live", run: () => { shell.section = "history"; shell.logLimit += 200; void shell.loadContext(); } },
      { label: shell.tr("menu.drop"), disabled: shell.mode !== "live" || shell.drops.length === 0 || shell.busy || !!shell.progress, run: () => shell.dropCommits() },
      { label: shell.tr("dialog.squash"), disabled: shell.mode !== "live" || !shell.selectedCommit || shell.busy || !!shell.progress, run: () => { shell.draft = ""; shell.dialog = "squash"; } },
      { label: shell.tr("chrome.compare"), disabled: shell.mode !== "live" || shell.drops.length !== 2, run: () => shell.compareDrops() },
      { label: shell.tr("chrome.refresh"), shortcut: shortcutLabel("refresh"), run: () => void shell.refresh() },
      { label: shell.tr("chrome.preferences"), shortcut: shortcutLabel("settings"), run: () => (shell.dialog = "prefs") },
    ];
  }

export function stashItems(shell: ShellState) {
    const locked = shell.mode !== "live" || shell.busy;
    return [
      { label: shell.tr("chrome.stash"), disabled: locked, run: () => shell.openStash([]) },
      { label: shell.tr("chrome.pop"), disabled: locked || (shell.refs?.stashes ?? []).length === 0, run: () => shell.confirmPopStash() },
      ...(shell.refs?.stashes ?? []).map((stash) => ({
        label: stash.summary || stash.name,
        disabled: locked,
        run: () => shell.confirmPopStash(stash.name),
      })),
    ];
  }

export function appearanceItems(shell: ShellState) {
    const mark = (value: string) => ((shell.settings.theme || "system") === value ? "✓ " : "");
    const pick = (value: string) => () => {
      shell.settings.theme = value;
      void shell.savePrefs(false);
    };
    return [
      { label: `${mark("system")}${shell.tr("dialog.system")}`, run: pick("system") },
      { label: `${mark("light")}${shell.tr("dialog.light")}`, run: pick("light") },
      { label: `${mark("dark")}${shell.tr("dialog.dark")}`, run: pick("dark") },
      { sep: true, label: "" },
      { label: shell.tr("chrome.preferences"), shortcut: shortcutLabel("settings"), run: () => (shell.dialog = "prefs") },
    ];
  }

export function repositoryItems(shell: ShellState) {
    const locked = shell.mode !== "live";
    return [
      { label: shell.tr("chrome.fetch"), disabled: shell.syncDisabled("fetch"), run: () => shell.confirmSync("fetch") },
      { label: shell.tr("chrome.pull"), disabled: shell.syncDisabled("pull"), run: () => shell.confirmSync("pull") },
      { label: shell.tr("chrome.push"), disabled: shell.syncDisabled("push"), run: () => shell.confirmSync("push") },
      { label: shell.tr("chrome.stash"), disabled: locked || shell.busy, run: () => shell.openStash([]) },
      { label: shell.tr("chrome.bisect"), disabled: locked || shell.busy, run: () => void shell.mutate({ action: "bisect", verb: shell.facts?.bisectActive ? "reset" : "start", rev: "" }).then(() => shell.loadFacts(true)) },
      { label: shell.tr("dialog.gitignore"), disabled: locked, run: () => { shell.draft = "node_modules/\ntarget/\n"; shell.dialog = "gitignore"; } },
      { label: shell.tr("chrome.stats"), disabled: locked, run: () => { void shell.loadFacts(true, true); shell.dialog = "stats"; } },
      { label: shell.tr("chrome.lfsFiles"), disabled: locked, run: () => { void shell.loadFacts(true, true); shell.dialog = "lfs"; } },
      { label: shell.tr("chrome.githubRepo"), disabled: locked, run: () => { shell.draft = ""; shell.draftExtra = ""; shell.draftUser = ""; shell.privateRepo = false; shell.hostForge = "github"; shell.dialog = "github"; } },
      { label: shell.tr("chrome.activity"), run: () => shell.openCommandLog() },
      { label: shell.tr("flow.title"), disabled: locked, run: () => void invoke("open_window", { view: "flow", repo: shell.repoPath(), file: "history" }) },
      { label: shell.tr("chrome.contentSearch"), disabled: locked, run: () => { shell.section = "history"; shell.searchOpen.commit = true; shell.commitQuery = "S:"; } },
      { label: shell.tr("chrome.stale"), disabled: locked, run: () => void invoke<string[]>("merged_branches", { path: shell.repoPath() }).then((names) => { shell.staleNames = names; shell.stalePicked = []; shell.dialog = "stale"; }) },
      { label: shell.tr("chrome.mailmap"), disabled: locked, run: () => void invoke<{ text?: string }>("blob_view", { path: shell.repoPath(), rev: "HEAD", file: ".mailmap" }).then((view) => { shell.draft = view.text ?? ""; shell.dialog = "mailmap"; }).catch(() => { shell.draft = ""; shell.dialog = "mailmap"; }) },
      { label: shell.tr("chrome.review"), disabled: locked, run: () => void invoke<string>("review_branch", { path: shell.repoPath(), base: shell.snapshot?.branch ?? "HEAD~1" }).then((text) => { shell.commandOutput = text; shell.dialog = "output"; }).catch((error) => (shell.actionError = shell.message(error))) },
      { label: shell.tr("menu.bareForce"), disabled: shell.syncDisabled("push"), run: () => shell.ask(shell.tr("menu.bareForce"), shell.tr("dialog.forcePushBody"), () => void shell.mutate({ action: "push", remote: null, setUpstream: false, tags: false, forceWithLease: false, force: true })) },
      { sep: true, label: "" },
      { label: shell.tr("menu.createBranch"), run: () => { shell.draft = ""; shell.draftExtra = ""; shell.dialog = "branch"; } },
      { label: shell.tr("menu.createTag"), run: () => { shell.draft = ""; shell.draftExtra = "HEAD"; shell.draftUser = ""; shell.dialog = "tag"; } },
      { label: shell.tr("chrome.repoSettings"), disabled: locked, run: () => shell.openRepoSettings() },
      { sep: true, label: "" },
      { label: shell.tr("chrome.clone"), run: () => void shell.beginClone("clone") },
      { label: shell.tr("chrome.open"), run: () => shell.beginOpen() },
      { label: shell.tr("chrome.newRepo"), run: () => void shell.beginClone("init") },
      { sep: true, label: "" },
      { label: shell.tr("chrome.terminal"), disabled: locked, run: () => void invoke("open_terminal", { path: shell.repoPath() }) },
      { label: shell.tr("chrome.explorer"), disabled: !shell.snapshot, run: () => shell.snapshot && void shell.openLocal(shell.snapshot.path) },
    ];
  }

export function windowItems(shell: ShellState) {
    return [
      { label: shell.tr("chrome.close"), disabled: !shell.snapshot, run: () => shell.snapshot && shell.closeTab(shell.snapshot.path) },
      { label: shell.tr("chrome.reopenTab"), shortcut: shortcutLabel("reopen"), disabled: shell.closedTabs.length === 0, run: () => shell.reopenClosed() },
      { label: shell.tr("chrome.newWindow"), run: () => void invoke("open_window", { view: null, repo: shell.repoPath() ?? null, file: null }) },
      { label: shell.tr("chrome.notifications"), run: () => void shell.openForge("notes") },
      { label: shell.tr("chrome.pulls"), run: () => void shell.openForge("pulls") },
    ];
  }

export function helpItems(shell: ShellState) {
    return [
      { label: shell.tr("chrome.about"), run: () => (shell.dialog = "about") },
      { label: shell.tr("chrome.checkUpdate"), run: () => shell.openUpdatePanel(true) },
      { label: shell.tr("chrome.performance"), shortcut: "Ctrl+Alt+P", run: () => window.dispatchEvent(new CustomEvent("awegit-perf")) },
    ];
  }

export function commitExtras(shell: ShellState) {
    return [
      { label: `${shell.signOff ? "✓ " : ""}${shell.tr("chrome.signOff")}`, run: () => { shell.signOff = !shell.signOff; if (shell.mode === "live") void shell.mutate({ action: "setSignOff", enabled: shell.signOff, format: shell.refs?.signOffFormat || shell.settings.signOffFormat }); } },
      { label: `${shell.skipHooks ? "✓ " : ""}${shell.tr("chrome.skipHooks")}`, run: () => (shell.skipHooks = !shell.skipHooks) },
      { label: shell.tr("chrome.suggestGenerate", { style: shell.tr(shell.settings.aiPromptStyle === "long" ? "chrome.promptLong" : "chrome.promptShort") }), disabled: shell.busy || shell.staged.length === 0, run: () => void shell.suggestMessage() },
      { label: shell.tr("chrome.commitAndPush"), disabled: !shell.canCommit, run: () => void shell.submitCommit(true) },
    ];
  }

export function dirMenu(shell: ShellState, dir: string, side: Side) {
    const unstage = side === "staged";
    const locked = shell.mode !== "live";
    const files = shell.filesInDir(dir, side);
    const count = String(files.length);
    return [
      {
        label: shell.tr(unstage ? "menu.unstageFolder" : "menu.stageFolder"),
        disabled: locked,
        run: () => shell.stageFolder(dir, unstage),
      },
      { label: files.length > 1 ? shell.tr("menu.discardSelected", { count }) : shell.tr("menu.discard"), disabled: locked || unstage || files.length === 0, shortcut: shortcutLabel("discard"), run: () => shell.discardChosen(files) },
      { label: files.length > 1 ? shell.tr("menu.deleteSelected", { count }) : shell.tr("menu.deleteFile"), disabled: locked || files.length === 0, run: () => shell.deleteChosen(files) },
      { sep: true, label: "" },
      { label: shell.tr("menu.addGitignore"), disabled: locked, run: () => void shell.addIgnore([dir], false, true) },
      { label: shell.tr("menu.addFakeignore"), disabled: locked, run: () => void shell.addIgnore([dir], true, true) },
      { label: files.length > 1 ? shell.tr("menu.stashFiles", { count }) : shell.tr("menu.stashFile"), disabled: locked || files.length === 0, run: () => shell.openStash(files) },
      { sep: true, label: "" },
      ...folderFoldItems(shell),
    ];
  }

export function commandItems(shell: ShellState, target: string) {
    return shell.settings.commands
      .filter((command) => command.target === target && (command.shared !== false || command.repo === shell.snapshot?.path))
      .map((command) => ({ label: command.name, run: () => shell.runCommand(command) }));
  }

export function diffLineMenu(shell: ShellState, clicked: number) {
    const indexes = shell.chosenDiffLines(clicked);
    const entries = indexes
      .map((index) => ({ line: shell.diffLines[index], index }))
      .filter((entry): entry is { line: DiffLine; index: number } => !!entry.line);
    const stageable = entries.filter((entry) => entry.line.stageAt != null && (entry.line.kind === "add" || entry.line.kind === "delete"));
    const discardable = shell.discardableEntries(indexes);
    const hunks = new Set(indexes.map(shell.hunkOf.bind(shell)).filter((hunk): hunk is number => hunk != null));
    const oneHunk = hunks.size === 1 ? [...hunks][0] : null;
    const locked = shell.busy || shell.mode !== "live";
    const file = shell.selectedPath;
    const items: MenuItem[] = [];
    if (stageable.length > 0) {
      const many = stageable.length > 1;
      items.push({
        label: shell.selectedSide === "staged" ? shell.tr(many ? "menu.unstageLines" : "menu.unstageLine") : shell.tr(many ? "menu.stageLines" : "menu.stageLine"),
        disabled: locked,
        run: () => void shell.applyLineOps("stageLine", stageable),
      });
    }
    if (discardable.length > 0) {
      const many = discardable.length > 1;
      items.push({
        label: shell.tr(many ? "menu.discardLines" : "menu.discardLine"),
        disabled: locked,
        run: () => {
          if (!many) {
            void shell.applyLineOps("discardLine", discardable);
            return;
          }
          shell.ask(shell.tr("menu.discardLines"), shell.tr("dialog.discardLinesBody", { count: String(discardable.length) }), () => void shell.applyLineOps("discardLine", discardable));
        },
      });
    }
    if (oneHunk != null) {
      const hunkIndex = oneHunk;
      items.push({
        label: shell.selectedSide === "staged" ? shell.tr("menu.unstageHunk") : shell.tr("menu.stageHunk"),
        disabled: locked,
        run: () => {
          shell.diffSelected = [];
          void shell.mutate({ action: "stageHunk", file, index: hunkIndex, unstage: shell.selectedSide === "staged", ignoreSpace: shell.settings.ignoreSpace });
        },
      });
      if (shell.selectedSide === "unstaged") {
        items.push({
          label: shell.tr("menu.discardHunk"),
          disabled: locked,
          run: () => {
            shell.diffSelected = [];
            void shell.mutate({ action: "discardHunk", file, index: hunkIndex, ignoreSpace: shell.settings.ignoreSpace });
          },
        });
      }
    }
    items.push(
      {
        label: shell.selectedSide === "staged" ? shell.tr("menu.unstage") : shell.tr("menu.stage"),
        disabled: locked || !file,
        run: () => void shell.runChange(shell.selectedSide === "staged" ? "unstage_path" : "stage_path", { file }),
      },
      {
        label: shell.tr("menu.discard"),
        disabled: locked || !file,
        run: () => shell.ask(shell.tr("menu.discard"), shell.tr("dialog.discardBody", { name: file ?? "" }), () => void shell.mutate({ action: "discard", file })),
      },
    );
    items.push({ sep: true, label: "" });
    const historyLine = shell.diffLines[clicked];
    const historyOk = !!historyLine && (historyLine.kind === "add" || historyLine.kind === "delete" || historyLine.kind === "context") && shell.lineText(historyLine).trim().length > 0;
    const hunkCount = shell.diffLines.reduce((count, line) => count + (line.kind === "hunk" ? 1 : 0), 0);
    items.push(
      { label: shell.tr("menu.copyPatch"), disabled: indexes.length === 0, run: () => shell.copyPatch(indexes) },
      { label: shell.tr("menu.openInEditorAtLine", { line: String(shell.editorLineNo(clicked)) }), disabled: !shell.selectedPath, children: shell.selectedPath ? editorItems(shell, shell.selectedPath, shell.editorLineNo(clicked)) : [] },
      { label: shell.tr("menu.lineHistory"), disabled: !historyOk || !shell.selectedPath, run: () => void shell.showLineHistory(historyLine) },
      { label: shell.tr("menu.popBlame"), disabled: !shell.selectedPath, run: () => void invoke("open_window", { view: "blame", repo: shell.repoPath(), file: shell.selectedPath }) },
      { sep: true, label: "" },
      { label: shell.tr("menu.prevHunk"), disabled: hunkCount === 0, run: () => shell.moveHunk(-1) },
      { label: shell.tr("menu.nextHunk"), disabled: hunkCount === 0, run: () => shell.moveHunk(1) },
      { sep: true, label: "" },
      {
        label: shell.tr("dialog.ignoreSpace"),
        checked: shell.settings.ignoreSpace,
        run: () => {
          shell.settings.ignoreSpace = !shell.settings.ignoreSpace;
          shell.diffSelected = [];
          void shell.savePrefs(false);
          void shell.loadDiff();
        },
      },
      {
        label: shell.tr("dialog.showSpace"),
        checked: shell.settings.showWhitespace,
        run: () => {
          shell.settings.showWhitespace = !shell.settings.showWhitespace;
          void shell.savePrefs(false);
        },
      },
    );
    return items;
  }

/** Tools for one file: exact suffix matches first, then all-files tools by priority. */
export function externalItems(shell: ShellState, kind: "diff" | "merge", file: string): MenuItem[] {
    const tools = shell.applicableTools(kind, file);
    const items: MenuItem[] = tools.map((tool, index) => ({
      label: `${index === 0 ? "✓ " : ""}${shell.toolLabel(tool)}`,
      shortcut: shell.toolExtensionLabel(tool),
      disabled: !tool.path.trim(),
      run: () => void launchExternal(shell, kind, file, shell.composeTool(tool.path, tool.args)),
    }));
    if (items.length === 0) items.push({ label: shell.tr("menu.noExternalTools"), disabled: true });
    items.push({ sep: true, label: "" });
    items.push({ label: shell.tr("menu.configureTools"), run: () => shell.openToolSettings(kind) });
    return items;
  }

/** Editors for one file: exact suffix matches first, then the fallback editor. */
export function editorItems(shell: ShellState, file: string, line = 0): MenuItem[] {
    const tools = shell.applicableTools("editor", file);
    const items: MenuItem[] = tools.map((tool, index) => ({
      label: `${index === 0 ? "✓ " : ""}${shell.toolLabel(tool)}`,
      shortcut: shell.toolExtensionLabel(tool),
      disabled: !tool.path.trim(),
      run: () => void openInEditor(shell, file, line, shell.composeTool(tool.path, tool.args)),
    }));
    if (items.length === 0) items.push({ label: shell.tr("menu.noEditorTools"), disabled: true });
    if (!tools.some((tool) => tool.path.trim())) {
      items.push({ sep: true, label: "" });
      items.push({
        label: shell.tr("menu.autoEditor"),
        run: () => {
          const tool = shell.resolvedTool("editor", file);
          void openInEditor(shell, file, line, tool?.path.trim() ? shell.composeTool(tool.path, tool.args) : undefined);
        },
      });
    }
    items.push({ label: shell.tr("menu.configureEditors"), run: () => shell.openToolSettings("editor") });
    return items;
  }

function revisionExternalItems(shell: ShellState, file: string, parent: string, rev: string): MenuItem[] {
    const tools = shell.applicableTools("diff", file);
    const items: MenuItem[] = tools.map((tool, index) => ({
      label: `${index === 0 ? "✓ " : ""}${shell.toolLabel(tool)}`,
      shortcut: shell.toolExtensionLabel(tool),
      disabled: !tool.path.trim(),
      run: () => void invoke("launch_tool", { path: shell.repoPath(), kind: "diff", file, tool: shell.composeTool(tool.path, tool.args), left: parent, right: rev }).catch((error) => (shell.actionError = shell.message(error))),
    }));
    if (items.length === 0) items.push({ label: shell.tr("menu.noExternalTools"), disabled: true });
    items.push({ sep: true, label: "" });
    items.push({ label: shell.tr("menu.configureTools"), run: () => shell.openToolSettings("diff") });
    return items;
  }

export function revisionFileMenu(shell: ShellState, file: string): MenuItem[] {
    const rev = shell.selectedCommit ?? "";
    const parent = shell.commitDetail?.parents[0] ?? "";
    const added = shell.commitFiles.some((item) => item.path === file && item.kind === "added");
    const locked = shell.mode !== "live" || !rev;
    const external: MenuItem = !parent || added
      ? { label: shell.tr("menu.externalDiff"), disabled: true }
      : {
          label: shell.tr("menu.externalDiff"),
          children: revisionExternalItems(shell, file, parent, rev),
        };
    return [
      { label: shell.tr("menu.open"), disabled: locked, run: () => void shell.openLocal(shell.fullPath(file)) },
      external,
      { label: shell.tr("menu.openInEditor"), disabled: locked, children: editorItems(shell, file, 0) },
      { label: shell.tr("menu.showInExplorer"), disabled: locked, run: () => void shell.openLocal(shell.fullPath(file)) },
      {
        label: shell.tr("menu.resetFileTo"),
        disabled: locked,
        children: [
          { label: shell.tr("menu.stateAtCommit"), disabled: locked, run: () => shell.ask(shell.tr("menu.stateAtCommit"), shell.tr("dialog.restoreFileBody", { name: file }), () => void shell.mutate({ action: "restoreFile", rev, file })) },
          { label: shell.tr("menu.stateBeforeCommit"), disabled: locked || !parent || added, run: () => shell.ask(shell.tr("menu.stateBeforeCommit"), shell.tr("dialog.restoreFileBody", { name: file }), () => void shell.mutate({ action: "restoreFile", rev: parent, file })) },
        ],
      },
      { label: shell.tr("menu.blameTimeline"), disabled: locked, run: () => { shell.section = "changes"; shell.selectedPath = file; void shell.showBlame(file); } },
      { label: shell.tr("menu.history"), disabled: locked, run: () => void invoke("open_window", { view: "history", repo: shell.repoPath(), file }) },
      { label: shell.tr("menu.showInTree"), disabled: !rev, run: () => { shell.historyFile = file; shell.section = "history"; void shell.openHistoryTab("tree"); } },
      { label: shell.tr("menu.saveAs"), disabled: locked, run: () => void invoke("save_revision", { path: shell.repoPath(), rev, file }).catch((error) => (shell.actionError = shell.message(error))) },
      { label: shell.tr("menu.copyRepoPath"), run: () => shell.copyText(file) },
      { label: shell.tr("menu.copyFullPath"), run: () => shell.copyText(shell.fullPath(file)) },
    ];
  }

export function historyFileMenu(shell: ShellState, commit: CommitRow): MenuItem[] {
    const file = shell.historyFilter;
    const locked = shell.mode !== "live" || !file;
    const parent = commit.parents[0] ?? "";
    const info = [commit.summary, commit.author, commit.at ? shell.forkWhen(commit.at) : commit.when, commit.id].filter((part) => part).join("\n");
    return [
      { label: shell.tr("menu.openSeparate"), disabled: locked, run: () => void invoke("open_window", { view: "blob", repo: shell.repoPath(), file, rev: commit.id }) },
      { label: shell.tr("menu.revealMain"), disabled: locked, run: () => void invoke("reveal_commit", { repo: shell.repoPath(), id: commit.id }).catch((error) => (shell.actionError = shell.message(error))) },
      { label: shell.tr("menu.open"), disabled: locked, run: () => void shell.openLocal(shell.fullPath(file)) },
      { label: shell.tr("menu.showInExplorer"), disabled: locked, run: () => void shell.openLocal(shell.fullPath(file)) },
      { label: shell.tr("menu.externalDiff"), disabled: locked || !parent, children: revisionExternalItems(shell, file, parent, commit.id) },
      { label: shell.tr("menu.resetAtCommit"), disabled: locked, run: () => shell.ask(shell.tr("menu.resetAtCommit"), shell.tr("dialog.restoreFileBody", { name: file }), () => void shell.mutate({ action: "restoreFile", rev: commit.id, file })) },
      { label: shell.tr("menu.saveAs"), disabled: locked, run: () => void invoke("save_revision", { path: shell.repoPath(), rev: commit.id, file }).catch((error) => (shell.actionError = shell.message(error))) },
      { label: shell.tr("menu.copySha"), run: () => shell.copyText(commit.id) },
      { label: shell.tr("menu.copyCommitInfo"), run: () => shell.copyText(info) },
    ];
  }

export function workspaceItems(shell: ShellState) {
    return [
      ...shell.settings.workspaces.map((item) => ({
        label: shell.workspaceName(item),
        checked: item.id === shell.settings.currentWorkspace,
        disabled: shell.busy,
        run: () => void shell.switchWorkspace(item.id),
      })),
      ...(shell.settings.workspaces.length > 0 ? [{ label: "", sep: true }] : []),
      { label: shell.tr("chrome.workspaceConfigure"), run: () => { shell.draft = ""; shell.dialog = "workspace"; } },
    ];
  }
