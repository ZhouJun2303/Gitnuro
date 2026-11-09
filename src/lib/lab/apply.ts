import { invoke } from "../perf";
import type { CommandRecord } from "../status";
import type { ShellState } from "../shell/state.svelte";
import { fillResolveDialog } from "../shell/dialogs.svelte";
import type { LabDialog, LabScene } from "./scenes";

const FILL = 24;

function pad(n: number) {
  return String(n).padStart(2, "0");
}

function lines(count: number, prefix: string) {
  return Array.from({ length: count }, (_, index) => `${prefix} ${index + 1}`).join("\n");
}

function fillCommandLog(shell: ShellState) {
  const now = Date.now();
  shell.commandLog = Array.from({ length: FILL }, (_, index) => {
    const n = pad(index + 1);
    return {
      id: index + 1,
      title: `git status #${n}`,
      summary: index % 5 === 0 ? "failed in lab" : "ok in lab",
      command: `git status --short # ${n}`,
      output: lines(FILL, `output ${n}`),
      ok: index % 5 !== 0,
      at: now - index * 60_000,
      source: index % 3 === 0 ? "background" as const : "user" as const,
    };
  });
}

function manyCommands(): CommandRecord[] {
  return Array.from({ length: FILL }, (_, index) => {
    const n = pad(index + 1);
    return {
      id: `lab-${n}`,
      name: `实验室命令 ${n}`,
      target: "repository",
      command: `git status --short # ${n}`,
      prompt: "",
      shared: true,
      repo: "",
    };
  });
}

function fillSourceRoots(shell: ShellState) {
  const recent = shell.settings.recent;
  const roots = recent.length >= FILL
    ? recent.slice(0, FILL)
    : Array.from({ length: FILL }, (_, index) => {
      const base = shell.settings.cloneDirectory || recent[0] || "D:\\lab";
      return `${base}\\source-${pad(index + 1)}`;
    });
  shell.settings = {
    ...shell.settings,
    sourceDirectories: roots,
    groupNames: roots.map((path, index) => `${path}\t源目录 ${pad(index + 1)}`),
  };
}

async function showChange(shell: ShellState, path: string, side: "staged" | "unstaged") {
  shell.section = "changes";
  shell.selectedSide = side;
  shell.selectedPath = path;
  await shell.loadDiff();
}

async function showHistory(shell: ShellState) {
  shell.section = "history";
  const first = shell.commitsLive[0];
  if (first) await shell.selectCommit(first.id);
}

async function showCommitBySummary(shell: ShellState, summary: string) {
  shell.section = "history";
  const target = shell.commitsLive.find((row) => row.summary === summary);
  if (target) await shell.selectCommit(target.id);
}

async function fillSurface(shell: ShellState, id: string) {
  if (id === "changes") {
    shell.section = "changes";
    return;
  }
  if (id === "changes-combined") {
    shell.section = "changes";
    shell.setLayout("combined");
    return;
  }
  if (id === "diff-text") return showChange(shell, "README.md", "unstaged");
  if (id === "diff-image") return showChange(shell, "art/mark.png", "unstaged");
  if (id === "diff-binary") return showChange(shell, "data/sheet.xlsx", "unstaged");
  if (id === "diff-binary-large") return showChange(shell, "data/large.pak", "unstaged");
  if (id === "diff-rename") return showChange(shell, "src/renamed.txt", "staged");
  if (id === "diff-long") return showChange(shell, "notes/long.txt", "unstaged");
  if (id === "diff-wrap") {
    shell.settings = { ...shell.settings, diffWordWrap: true };
    return showChange(shell, "notes/wrap.txt", "unstaged");
  }
  if (id === "diff-wrap-unified") {
    shell.settings = { ...shell.settings, diffWordWrap: true, diffStyle: "unified" };
    return showChange(shell, "notes/wrap.txt", "unstaged");
  }
  if (id === "history" || id === "commit" || id === "history-remote") return showHistory(shell);
  if (id === "commit-binary") {
    await showCommitBySummary(shell, "add the large pak");
    await shell.openHistoryTab("changes");
    return;
  }
  if (id === "tree") {
    await showHistory(shell);
    await shell.openHistoryTab("tree");
    return;
  }
  if (id === "search") {
    shell.commitSearch = "topic";
    shell.showSideSearch();
    shell.queueCommitSearch();
    return;
  }
  if (id === "file-history") {
    await shell.showFileHistory("README.md");
    return;
  }
  if (id === "blob") {
    shell.blobFile = "README.md";
    shell.blobRev = "HEAD";
    await shell.loadBlobPane();
    return;
  }
  if (id === "sidebar") {
    shell.collapsedSide = [];
    shell.sideQuery = "";
    return;
  }
  if (id === "vertical-tabs") {
    shell.settings = { ...shell.settings, verticalTabs: true };
    return;
  }
  if (id === "picker") {
    await shell.toggleRepoPicker();
    return;
  }
  if (id === "picker-truncated") {
    await shell.toggleRepoPicker();
    shell.repoScanTruncated = true;
    return;
  }
  if (id === "commands") {
    fillCommandLog(shell);
    shell.commandPick = 1;
    shell.openCommandLog();
    return;
  }
  if (id === "commands-errors") {
    fillCommandLog(shell);
    // Saved errors keep their own id counter, so a stale command id can happen to match one.
    shell.errorLog = Array.from({ length: 80 }, (_, index) => ({
      id: 80 - index,
      at: Date.now() - index * 60_000,
      title: `lab error ${index + 1}`,
      text: `lab error text ${index + 1}`,
      command: "",
    }));
    shell.errorSeq = 80;
    shell.unseenErrors = 1;
    shell.commandPick = 1;
    shell.openCommandLog();
    return;
  }
  if (id === "notes") {
    shell.notices = Array.from({ length: FILL }, (_, index) => {
      const n = pad(index + 1);
      return {
        id: `lab-${n}`,
        title: `实验室通知 ${n}`,
        url: `https://example.com/lab/notice/${n}`,
        unread: index % 2 === 0,
      };
    });
    shell.forgeOpen = "notes";
    return;
  }
  if (id === "pulls") {
    shell.pulls = Array.from({ length: FILL }, (_, index) => {
      const n = pad(index + 1);
      return {
        title: `实验室拉取请求 ${n}`,
        author: `lab-${n}`,
        branch: `feat/${n}`,
        url: `https://example.com/lab/pull/${n}`,
      };
    });
    shell.forgeOpen = "pulls";
    return;
  }
  if (id === "launch") {
    shell.launchMode = "commands";
    shell.launch = "";
    shell.launchOpen = true;
    return;
  }
  if (id === "launch-files") {
    shell.launchMode = "files";
    shell.launch = "";
    await shell.loadLaunchFiles();
    shell.launchOpen = true;
    return;
  }
  if (id === "perf") {
    window.dispatchEvent(new CustomEvent("awegit-perf", { detail: { open: true } }));
    return;
  }
  if (id === "conflict") return showChange(shell, "src/both-sides.ts", "unstaged");
  if (id === "conflict-batch") {
    shell.section = "changes";
    shell.picked = [];
    return;
  }
  if (id === "bisect") {
    await shell.loadFacts(true, false);
    return;
  }
  if (id === "ignored") {
    shell.section = "changes";
    shell.showIgnored = true;
    await shell.loadFacts(true, true);
    return;
  }
  if (id === "submodule") {
    shell.collapsedSide = [];
    shell.sideQuery = "vendor";
    return;
  }
  if (id === "worktree") {
    shell.collapsedSide = [];
    shell.sideQuery = "extra";
    return;
  }
  if (id === "blame") {
    await showChange(shell, "README.md", "unstaged");
    await shell.showBlame("README.md");
    return;
  }
  if (id === "reflog") {
    shell.section = "history";
    if (!shell.reflogOn) await shell.showReflog();
    return;
  }
  if (id === "clone-progress") {
    const repo = shell.repoPath() ?? "";
    const destination = `${repo.replace(/[\\/][^\\/]+$/, "")}/lab-clone`;
    shell.cloneJob = { name: "lab-clone", destination, workspace: shell.settings.currentWorkspace || "" };
    shell.jobs = { ...shell.jobs, [shell.jobKey(destination)]: { label: "clone", percent: 42, depth: 1 } };
    return;
  }
  if (id === "clone-done") {
    shell.cloneDone = shell.repoPath() ?? null;
    shell.openCommandLog("clone-done");
    return;
  }
  if (id === "update") {
    shell.updateNotice = {
      appVersion: "9.9.9",
      appCode: 999,
      downloadUrl: "https://example.com/awegit",
      notes: lines(FILL, "实验室更新说明"),
      installable: false,
      sha256: "",
    };
    shell.openCommandLog("update");
    return;
  }
  if (id === "action-error") {
    shell.actionError = "实验室操作错误：推送被拒绝";
    return;
  }
  if (id === "action-error-coded") {
    shell.actionError = shell.message("awegit:update:sha256 mismatch");
    return;
  }
  if (id === "action-error-retry") {
    shell.actionError = "error: Your local changes to the following files would be overwritten by merge:";
    shell.stashRetry = { request: { action: "stashApply", name: "stash@{0}" }, source: "user" };
    return;
  }
  if (id === "settings-recovered") {
    shell.adoptSettings({ ...shell.settings, recoveryNotice: "recovered-backup" });
    return;
  }
  if (id === "text-merge-many") {
    await showChange(shell, "src/many-hunks.ts", "unstaged");
    await shell.openTextMerge("src/many-hunks.ts");
    return;
  }
  if (id === "text-merge" || id === "text-merge-window") {
    await showChange(shell, "src/both-sides.ts", "unstaged");
    await shell.openTextMerge("src/both-sides.ts");
    return;
  }
  if (id === "diff-large-asset") {
    await showChange(shell, "assets/Config/Generated.asset", "unstaged");
    return;
  }
  if (id === "conflict-large-asset") {
    await showChange(shell, "assets/Config/Generated.asset", "unstaged");
    await shell.openTextMerge("assets/Config/Generated.asset");
    return;
  }
  if (id === "file-merge") {
    await showChange(shell, "art/removed-here.png", "unstaged");
    await shell.openTextMerge("art/removed-here.png");
    return;
  }
  if (id === "confirm") {
    shell.draft = "feature/lab";
    shell.draftExtra = "main";
    shell.dialog = "branch";
    shell.confirmAsk = { title: "删除分支", body: "确认删除 topic？", run: () => {} };
    return;
  }
  if (id === "stash-apply-confirm") {
    const stash = shell.refs?.stashes[0];
    shell.confirmApplyStash(stash?.name ?? "stash@{0}", stash?.summary ?? "WIP before the palette");
    return;
  }
  if (id === "revert-confirm") {
    const commit = shell.commitsLive[0];
    shell.confirmRevert(commit?.id ?? "f2c87af", commit?.summary ?? "feat: 英雄词条和技能翻译修复");
    return;
  }
  if (id === "stash-pop-confirm") {
    shell.confirmPopStash();
    return;
  }
  if (id === "reset-confirm") {
    const commit = shell.commitsLive[0];
    shell.confirmReset(commit?.id ?? "HEAD", "mixed", commit?.summary ?? "");
    return;
  }
  if (id === "cherry-confirm") {
    const commit = shell.commitsLive[0];
    if (commit) await shell.previewCherry(commit.id, true, commit.summary);
    return;
  }
  if (id === "menu") {
    shell.menu = {
      x: 72,
      y: 96,
      items: Array.from({ length: FILL }, (_, index) => {
        const n = pad(index + 1);
        if (index === 1) {
          return {
            label: `更多 ${n}`,
            children: Array.from({ length: FILL }, (_, child) => ({
              label: `子项 ${pad(child + 1)}`,
              run: () => {},
            })),
          };
        }
        return { label: `菜单项 ${n}`, run: () => {} };
      }),
    };
    shell.submenu = {
      x: 300,
      y: 124,
      items: Array.from({ length: FILL }, (_, index) => ({
        label: `子项 ${pad(index + 1)}`,
        run: () => {},
      })),
    };
  }
}

async function fillDialog(shell: ShellState, dialog: LabDialog, sceneId = "") {
  const head = shell.commitsLive[0]?.id ?? "HEAD";
  const parent = (shell.repoPath() ?? "").replace(/[\\/][^\\/]+$/, "");
  switch (dialog) {
    case "branch":
      shell.draft = "feature/lab";
      shell.draftExtra = "main";
      break;
    case "flow":
      shell.draft = "palette";
      shell.flowKind = "feature";
      break;
    case "clone":
      shell.cloneIntent = "clone";
      shell.draft = "https://example.com/lab/repo.git";
      fillSourceRoots(shell);
      break;
    case "open":
      shell.draft = shell.repoPath() ?? "";
      fillSourceRoots(shell);
      break;
    case "prefs":
      shell.prefTab = sceneId === "prefs-git" ? "git" : sceneId === "prefs-commit" ? "commit" : sceneId === "prefs-integration" ? "integration" : sceneId === "prefs-updates" ? "updates" : "general";
      fillSourceRoots(shell);
      fillLabTools(shell);
      break;
    case "reset":
      shell.draft = head.slice(0, 7) || "HEAD";
      break;
    case "command":
      shell.settings = {
        ...shell.settings,
        commands: manyCommands(),
      };
      shell.draft = "实验室命令 01";
      shell.draftExtra = "repository";
      shell.draftUser = "git status --short # 01";
      break;
    case "tag":
      shell.draft = "v2.0.1";
      shell.draftExtra = "HEAD";
      shell.draftUser = "实验室标签";
      break;
    case "remote":
      shell.draft = "origin";
      shell.draftExtra = "https://example.com/lab/repo.git";
      break;
    case "squash":
      if (shell.commitsLive[0]) shell.selectedCommit = shell.commitsLive[0].id;
      shell.draft = "squash the lab range";
      break;
    case "credential":
      shell.draft = "https";
      shell.draftExtra = "github.com";
      shell.draftUser = "lab";
      shell.draftSecret = "lab-secret";
      break;
    case "fetch":
      shell.draft = "origin";
      break;
    case "pull":
      shell.draft = "origin";
      shell.draftExtra = shell.pullBranchFor("origin") || "main";
      break;
    case "push":
      shell.pushBranch = shell.refs?.branches.find((branch) => branch.current)?.name ?? shell.snapshot?.branch ?? "main";
      shell.pushTo = "";
      shell.draft = "origin";
      break;
    case "stash":
      shell.draft = "WIP before the palette";
      shell.stashFiles = Array.from({ length: FILL }, (_, index) => `pending/file-${pad(index + 1)}.txt`);
      break;
    case "rename":
      shell.draft = "main";
      shell.draftExtra = "trunk";
      break;
    case "upstream":
      shell.draft = "main";
      shell.draftExtra = "origin/main";
      break;
    case "resolve":
      await fillResolveDialog(shell, "src/both-sides.ts");
      break;
    case "rebase": {
      const onto = shell.commitsLive[shell.commitsLive.length - 1];
      if (onto) shell.selectedCommit = onto.id;
      shell.openRebase();
      return;
    }
    case "patch":
      shell.draft = [
        "diff --git a/README.md b/README.md",
        "--- a/README.md",
        "+++ b/README.md",
        ...Array.from({ length: FILL }, (_, index) => `+patch line ${index + 1}`),
      ].join("\n");
      break;
    case "submodule":
      shell.draft = "https://example.com/lab/lib.git";
      shell.draftExtra = "vendor";
      break;
    case "worktree":
      shell.draft = parent ? `${parent}/extra-wt` : "";
      shell.draftExtra = "extra";
      break;
    case "output":
      shell.commandOutput = lines(FILL, "实验室审查输出");
      break;
    case "mergeConflict":
      shell.commandOutput = [
        "Auto-merging README.md",
        "CONFLICT (content): Merge conflict in README.md",
        "Automatic merge failed; fix conflicts and then commit the result.",
        "On branch main",
        "You have unmerged paths.",
        "  (fix conflicts and run \"git commit\")",
        "Unmerged paths:",
        "  both modified:   README.md",
        ...Array.from({ length: Math.min(12, FILL) }, (_, index) => `  both modified:   conflict-${pad(index + 1)}.txt`),
      ].join("\n");
      break;
    case "workspace":
      shell.draft = "实验室工作区";
      break;
    case "reword":
      shell.draft = "实验室改写提交说明";
      shell.draftExtra = head;
      break;
    case "merge":
      shell.draft = "topic";
      shell.draftExtra = "ff";
      shell.dialog = "merge";
      await Promise.resolve();
      await shell.loadMergePreview();
      return;
    case "repo":
      shell.draft = shell.settings.authorName || "AweGit Lab";
      shell.draftExtra = shell.settings.authorEmail || "lab@example.com";
      shell.draftUser = "AweGit Lab <lab@example.com>";
      break;
    case "continue":
      shell.draft = "继续变基并保留说明";
      break;
    case "gitignore":
      shell.draft = ["secret.tmp", "ignored/", ...Array.from({ length: FILL }, (_, index) => `pattern-${pad(index + 1)}.tmp`)].join("\n") + "\n";
      break;
    case "stats":
    case "lfs":
      await shell.loadFacts(true, true);
      break;
    case "github":
      shell.hostForge = "github";
      shell.draft = "awegit-lab";
      shell.draftExtra = "github.com";
      shell.draftUser = "lab";
      shell.privateRepo = true;
      break;
    case "stashRename":
      shell.draft = "整理过的贮藏";
      shell.draftExtra = "stash@{0}";
      break;
    case "stashBranch":
      shell.draft = "from-stash";
      shell.draftExtra = "stash@{0}";
      break;
    case "checkoutNotice":
      shell.pendingCheckout = "topic";
      break;
    case "activity":
      shell.sessionLog = Array.from({ length: FILL }, (_, index) => `实验室会话 ${pad(index + 1)}`);
      shell.activityLines = await invoke<string[]>("activity_log").catch(() => []);
      if (shell.activityLines.length < FILL) {
        shell.activityLines = [
          ...shell.activityLines,
          ...Array.from({ length: FILL - shell.activityLines.length }, (_, index) => `实验室活动 ${pad(index + 1)}`),
        ];
      }
      break;
    case "mailmap":
      shell.draft = Array.from({ length: FILL }, (_, index) => {
        const n = pad(index + 1);
        return `Lab Author ${n} <lab${n}@example.com> <alias${n}@example.com>`;
      }).join("\n") + "\n";
      break;
    case "accounts":
      shell.settings = {
        ...shell.settings,
        accounts: Array.from({ length: FILL }, (_, index) => {
          const n = pad(index + 1);
          return {
            id: `lab-acct-${n}`,
            forge: index % 2 === 0 ? "github" : "gitlab",
            label: `lab-${n}`,
            token: `lab-token-${n}`,
            host: index % 2 === 0 ? "github.com" : "gitlab.com",
            clientId: `lab-client-${n}`,
          };
        }),
      };
      shell.hostForge = "github";
      shell.draft = "lab-01";
      shell.draftExtra = "github.com";
      shell.draftUser = "lab-token-01";
      break;
    case "ssh":
      shell.draft = Array.from({ length: FILL }, (_, index) => {
        const n = pad(index + 1);
        return `Host lab-${n}\n  HostName git.example.com\n  User git\n  IdentityFile ~/.ssh/lab_${n}`;
      }).join("\n\n") + "\n";
      break;
    case "stale":
      shell.staleNames = await invoke<string[]>("merged_branches", { path: shell.repoPath() }).catch(() => []);
      if (shell.staleNames.length < FILL) {
        shell.staleNames = [
          ...shell.staleNames,
          ...Array.from({ length: FILL - shell.staleNames.length }, (_, index) => `merged/${pad(index + 1)}`),
        ];
      }
      shell.stalePicked = shell.staleNames.slice(0, 3);
      break;
    case "pushAs":
      shell.draft = "topic";
      shell.draftExtra = "origin";
      shell.draftUser = "topic";
      break;
    case "askInput":
      shell.pendingCommand = {
        id: "lab",
        name: "状态",
        target: "repository",
        command: "git status",
        prompt: "说明",
        shared: false,
        repo: "",
      };
      shell.commandPrompt = "说明";
      shell.draft = "实验室";
      shell.fieldValues = {};
      break;
    case "renameTab":
      shell.draft = "实验室";
      break;
    case "colorTab":
      shell.draft = "#5b8def";
      break;
    case "groupRename":
      shell.draft = "工作";
      break;
    default:
      break;
  }
  shell.dialog = dialog;
}

function fillLabTools(shell: ShellState) {
  shell.mergeTools = Array.from({ length: FILL }, (_, index) => {
    const n = pad(index + 1);
    return { name: `Merge Tool ${n}`, path: `C:\\Tools\\merge-${n}.exe`, args: "$LOCAL $REMOTE $MERGED", primary: index === 0 };
  });
  shell.diffTools = Array.from({ length: FILL }, (_, index) => {
    const n = pad(index + 1);
    return { name: `Diff Tool ${n}`, path: `C:\\Tools\\diff-${n}.exe`, args: "$LOCAL $REMOTE", primary: index === 0 };
  });
  shell.aiModels = Array.from({ length: FILL }, (_, index) => `lab-model-${pad(index + 1)}`);
}

export async function applyLabScene(shell: ShellState, item: LabScene) {
  if (item.dialog) await fillDialog(shell, item.dialog, item.id);
  else await fillSurface(shell, item.id);
}
