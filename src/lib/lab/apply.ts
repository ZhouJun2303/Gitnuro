import { invoke } from "../perf";
import type { CommandRecord } from "../status";
import type { ShellState } from "../shell/state.svelte";
import type { LabDialog, LabScene } from "./scenes";

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

async function fillSurface(shell: ShellState, id: string) {
  if (id === "changes") {
    shell.section = "changes";
    return;
  }
  if (id === "diff-text") return showChange(shell, "README.md", "unstaged");
  if (id === "diff-image") return showChange(shell, "art/mark.png", "unstaged");
  if (id === "diff-binary") return showChange(shell, "data/sheet.xlsx", "unstaged");
  if (id === "diff-rename") return showChange(shell, "src/renamed.txt", "staged");
  if (id === "diff-long") return showChange(shell, "notes/long.txt", "unstaged");
  if (id === "history" || id === "commit") return showHistory(shell);
  if (id === "tree") {
    await showHistory(shell);
    await shell.openHistoryTab("tree");
    return;
  }
  if (id === "search") {
    shell.commitSearch = "merge";
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
  if (id === "picker") {
    await shell.toggleRepoPicker();
    return;
  }
  if (id === "commands") {
    shell.commandLog = [
      {
        id: 1,
        title: "git fetch origin",
        summary: "Already up to date",
        command: "git fetch --prune origin",
        output: "Already up to date",
        ok: true,
        at: Date.now(),
        source: "user",
      },
    ];
    shell.commandPick = 1;
    shell.openCommandLog();
    return;
  }
  if (id === "notes") {
    shell.notices = [{ id: "lab-1", title: "实验室通知", url: "https://example.com/lab/notice", unread: true }];
    shell.forgeOpen = "notes";
    return;
  }
  if (id === "pulls") {
    shell.pulls = [{ title: "实验室拉取请求", author: "lab", branch: "topic", url: "https://example.com/lab/pull/1" }];
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
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "p", ctrlKey: true, altKey: true, bubbles: true }));
    return;
  }
  if (id === "conflict") return showChange(shell, "src/both-sides.ts", "unstaged");
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
    shell.sideQuery = "vendor";
    return;
  }
  if (id === "worktree") {
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
    return;
  }
  if (id === "update") {
    shell.updateNotice = {
      appVersion: "9.9.9",
      appCode: 999,
      downloadUrl: "https://example.com/awegit",
      notes: "实验室更新说明",
      installable: false,
    };
    return;
  }
  if (id === "action-error") {
    shell.actionError = "实验室操作错误：推送被拒绝";
    return;
  }
  if (id === "text-merge") {
    await showChange(shell, "src/both-sides.ts", "unstaged");
    await shell.openTextMerge("src/both-sides.ts");
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
  if (id === "menu") {
    shell.menu = {
      x: 72,
      y: 96,
      items: [
        { label: "提交", run: () => {} },
        { label: "更多", children: [{ label: "变基", run: () => {} }] },
      ],
    };
    shell.submenu = { x: 300, y: 124, items: [{ label: "变基", run: () => {} }] };
  }
}

async function fillDialog(shell: ShellState, dialog: LabDialog) {
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
      break;
    case "open":
      shell.draft = shell.repoPath() ?? "";
      break;
    case "reset":
      shell.draft = head.slice(0, 7) || "HEAD";
      break;
    case "command":
      shell.settings = {
        ...shell.settings,
        commands: [
          { id: "lab", name: "状态", target: "repository", command: "git status", prompt: "", shared: true, repo: "" } satisfies CommandRecord,
        ],
      };
      shell.draft = "状态";
      shell.draftExtra = "repository";
      shell.draftUser = "git status";
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
    case "pull":
    case "push":
      shell.draft = "origin";
      break;
    case "stash":
      shell.draft = "WIP before the palette";
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
      shell.draft = "src/both-sides.ts";
      shell.draftExtra = "text";
      shell.mergeResult = "export const side = \"lab\";\n";
      break;
    case "rebase": {
      const onto = shell.commitsLive[shell.commitsLive.length - 1];
      if (onto) shell.selectedCommit = onto.id;
      shell.openRebase();
      return;
    }
    case "patch":
      shell.draft = "diff --git a/README.md b/README.md\n";
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
      shell.commandOutput = "实验室审查：这个分支可以合并。";
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
      shell.draft = "secret.tmp\n";
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
    case "checkoutDirty":
      shell.pendingCheckout = "topic";
      shell.draftExtra = "stash";
      break;
    case "checkoutNotice":
      shell.pendingCheckout = "topic";
      break;
    case "activity":
      shell.sessionLog = ["实验室打开了仓库"];
      shell.activityLines = await invoke<string[]>("activity_log").catch(() => []);
      break;
    case "mailmap":
      shell.draft = "AweGit Lab <lab@example.com> <lab@example.com>\n";
      break;
    case "accounts":
      shell.settings = {
        ...shell.settings,
        accounts: [{ id: "lab", forge: "github", label: "lab", token: "lab-token", host: "github.com", clientId: "lab-client" }],
      };
      shell.hostForge = "github";
      shell.draft = "lab";
      shell.draftExtra = "github.com";
      shell.draftUser = "lab-token";
      break;
    case "ssh":
      shell.draft = "Host lab\n  User git\n";
      break;
    case "stale":
      shell.staleNames = await invoke<string[]>("merged_branches", { path: shell.repoPath() }).catch(() => []);
      if (shell.staleNames.length === 0) shell.staleNames = ["old"];
      shell.stalePicked = [shell.staleNames[0] ?? "old"];
      break;
    case "pushAs":
      shell.draft = "topic";
      shell.draftExtra = "origin";
      shell.draftUser = "topic";
      break;
    case "askInput":
      shell.pendingCommand = { id: "lab", name: "状态", target: "repository", command: "git status", prompt: "说明", shared: false, repo: "" };
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

export async function applyLabScene(shell: ShellState, item: LabScene) {
  if (item.dialog) await fillDialog(shell, item.dialog);
  else await fillSurface(shell, item.id);
}
