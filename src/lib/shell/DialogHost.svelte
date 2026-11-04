<script lang="ts">
  import { getShell } from "./context";
  import "./DialogHost.css";
  import PrefsDialog from "./PrefsDialog.svelte";
  import { flushClientLog, invoke } from "../perf";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { cloneTarget, cloneUrl } from "../clonePath";

  const shell = getShell();
  let diagnosticsCopied = $state(false);

  async function copyDiagnostics() {
    try {
      await flushClientLog();
      const text = await invoke<string>("diagnostics");
      shell.copyText(text);
      diagnosticsCopied = true;
      window.setTimeout(() => {
        diagnosticsCopied = false;
      }, 1500);
    } catch (error) {
      shell.actionError = shell.message(error);
    }
  }
  let primedDialog = "";
  const largeDialogs = new Set(["prefs", "resolve", "stats", "rebase", "accounts"]);
  const smallDialogs = new Set(["reset", "branch", "tag", "rename", "upstream", "reword", "remote", "submodule", "worktree", "continue", "about", "checkoutNotice", "stashRename", "stashBranch", "renameTab", "colorTab", "groupRename", "askInput", "checkoutDirty", "workspace"]);
  function dialogSize(kind: string) {
    if (largeDialogs.has(kind)) return "lg";
    if (smallDialogs.has(kind)) return "sm";
    return "md";
  }
  $effect(() => {
    const dialog = shell.dialog ?? "";
    if (dialog === primedDialog) return;
    primedDialog = dialog;
    if (dialog === "branch") shell.checkoutNewBranch = shell.settings.checkoutAfterCreate;
    if (dialog === "rebase") shell.updateRefs = shell.settings.rebaseUpdateRefs;
    if (dialog === "merge") {
      shell.mergeSummary = "";
      shell.mergeBody = "";
      shell.mergeAutoSummary = "";
      shell.mergeAutoBody = "";
      shell.mergeFiles = [];
      void shell.loadMergePreview();
    }
  });
  const focusOnMount = (node: HTMLInputElement) => shell.focusOnMount(node);
  const focusWhen = (node: HTMLInputElement, on: boolean) => shell.focusWhen(node, on);
</script>

{#if shell.dialog}
    <div class="scrim" role="presentation" onclick={() => { if (shell.dialog === "prefs") void shell.savePrefs(true); else shell.dialog = null; }}>
      <div class="dialog {dialogSize(shell.dialog ?? "")}" class:prefs={shell.dialog === "prefs"} class:path-dialog={shell.dialog === "clone"} class:workspace={shell.dialog === "workspace"} data-dialog={shell.dialog} role="dialog" tabindex="-1" onclick={(event) => event.stopPropagation()}>
      <form
        onchange={() => { if (shell.dialog === "prefs") void shell.savePrefs(false); }}
        onsubmit={async (event) => {
          event.preventDefault();
          if (shell.dialog === "branch") {
            const name = shell.branchName(shell.draft);
            await shell.mutate({ action: "createBranch", name, start: shell.draftExtra || null });
            if (shell.checkoutNewBranch && !shell.actionError) await shell.mutate({ action: "checkout", name });
          }
          else if (shell.dialog === "clone") {
            if (shell.cloneIntent === "init") void shell.startInit();
            else void shell.startClone();
          }
          else if (shell.dialog === "command") void shell.runCommand({ command: shell.draftUser || shell.draft, prompt: shell.commandPrompt });
          else if (shell.dialog === "prefs") void shell.savePrefs();
          else if (shell.dialog === "tag") {
            await shell.mutate({ action: "tag", name: shell.draft, rev: shell.draftExtra || "HEAD", message: shell.draftUser });
            if (!shell.actionError && shell.settings.pushTagOnCreate) await shell.mutate({ action: "push", remote: null, setUpstream: true, tags: true, forceWithLease: shell.settings.forceWithLease });
          }
          else if (shell.dialog === "remote") void shell.mutate({ action: "addRemote", name: shell.draft, url: shell.draftExtra });
          else if (shell.dialog === "fetch") {
            const remote = shell.syncAll ? null : shell.draft || null;
            const prune = shell.settings.fetchPrune;
            const tags = shell.syncTags;
            if (shell.busy) return;
            shell.dialog = null;
            void shell.mutate({ action: "fetch", remote, prune, tags });
          }
          else if (shell.dialog === "pull") {
            const request = shell.draft && shell.draftExtra
              ? { action: "pullRef", remote: shell.draft, branch: shell.draftExtra, rebase: shell.syncRebase, autostash: shell.syncStash }
              : { action: "pull", rebase: shell.syncRebase, autostash: shell.syncStash };
            if (shell.busy) return;
            shell.dialog = null;
            void shell.mutate(request);
          }
          else if (shell.dialog === "push") {
            const target = shell.pushTarget(shell.pushBranch, shell.pushTo);
            const request = target && shell.pushBranch
              ? { action: "pushRef", remote: target.remote, branch: shell.pushBranch, remoteBranch: target.branch, setUpstream: target.setUpstream, tags: shell.syncTags, forceWithLease: shell.syncForce, force: false }
              : { action: "push", remote: shell.draft || null, setUpstream: !shell.refs?.branches.some((branch) => branch.current && branch.upstream), tags: shell.syncTags, forceWithLease: shell.syncForce };
            if (shell.busy) return;
            shell.dialog = null;
            void shell.mutate(request);
          }
          else if (shell.dialog === "stash") {
            const files = shell.stashFiles;
            shell.stashFiles = [];
            if (files.length > 0) {
              await shell.mutate({ action: "stashPaths", files, message: shell.draft, includeUntracked: shell.includeUntracked });
              if (!shell.actionError) shell.picked = shell.picked.filter((path) => !files.includes(path));
            } else await shell.mutate({ action: "stash", message: shell.draft, includeUntracked: shell.includeUntracked });
          }
          else if (shell.dialog === "rename") void shell.mutate({ action: "renameBranch", name: shell.draft, to: shell.draftExtra });
          else if (shell.dialog === "upstream") void shell.mutate({ action: "setUpstream", branch: shell.draft, upstream: shell.draftExtra });
          else if (shell.dialog === "reword") void shell.mutate({ action: "reword", rev: shell.draftExtra || "HEAD", summary: shell.draft });
          else if (shell.dialog === "rebase" && shell.selectedCommit) void shell.mutate({ action: "rebaseInteractive", onto: shell.selectedCommit, drop: [], steps: shell.rebaseSteps.map((step) => ({ verb: step.verb, rev: step.rev, message: step.verb === "reword" || step.verb === "squash" || step.verb === "exec" ? step.message : "" })), autostash: shell.settings.rebaseAutostash, updateRefs: shell.updateRefs });
          else if (shell.dialog === "merge") void shell.mutate({ action: "merge", name: shell.draft, squash: shell.draftExtra === "squash", noFf: shell.draftExtra === "no-ff", autostash: shell.settings.mergeAutostash, summary: shell.mergeSummary, description: shell.mergeBody });
          else if (shell.dialog === "continue") void shell.mutate({ action: "continue", message: shell.draft });
          else if (shell.dialog === "repo") {
            try {
              await invoke("set_repo_author", { path: shell.repoPath(), name: shell.draft, email: shell.draftExtra });
              await shell.mutate({ action: "setSignOff", enabled: shell.repoSignOff, format: shell.draftUser || shell.settings.signOffFormat });
            } catch (error) {
              shell.actionError = shell.message(error);
            }
          }
          else if (shell.dialog === "patch") void shell.mutate({ action: "applyPatch", patch: shell.draft });
          else if (shell.dialog === "submodule") void shell.mutate({ action: "submoduleAdd", url: shell.draft, path: shell.draftExtra });
          else if (shell.dialog === "worktree") void shell.mutate({ action: "addWorktree", path: shell.draft, branch: shell.draftExtra });
          else if (shell.dialog === "workspace") {
            if (shell.draft.trim()) {
              shell.dialog = null;
              await shell.createWorkspace(shell.draft);
            }
          }
          else if (shell.dialog === "resolve") void shell.mutate({ action: "resolve", file: shell.draft, side: shell.draftExtra || "text", text: shell.mergeResult });
          else if (shell.dialog === "gitignore") void shell.mutate({ action: "writeGitignore", text: shell.draft });
          else if (shell.dialog === "stashRename") void shell.mutate({ action: "stashRename", name: shell.draftExtra, message: shell.draft });
          else if (shell.dialog === "stashBranch") void shell.mutate({ action: "stashBranch", name: shell.draftExtra, branch: shell.draft });
          else if (shell.dialog === "github") {
            try {
              const url = await invoke<string>("create_hosted_repo", { forgeName: shell.hostForge, name: shell.draft, privateRepo: shell.privateRepo, host: shell.draftExtra, org: shell.draftUser || shell.settings.azureOrg });
              shell.dialog = null;
              if (url) void openUrl(url);
            } catch (error) {
              shell.actionError = shell.message(error);
            }
          }
          else if (shell.dialog === "mailmap") void shell.mutate({ action: "writeMailmap", text: shell.draft });
          else if (shell.dialog === "ssh") {
            try {
              await invoke("write_ssh_config", { text: shell.draft });
              shell.dialog = null;
            } catch (error) {
              shell.actionError = shell.message(error);
            }
          }
          else if (shell.dialog === "pushAs") void shell.mutate({ action: "pushRef", remote: shell.draftExtra || "origin", branch: shell.draft, remoteBranch: shell.draftUser, forceWithLease: false, force: false });
          else if (shell.dialog === "askInput" && shell.pendingCommand) {
            const answers = Object.keys(shell.fieldValues).length > 0 ? { ...shell.fieldValues } : { input: shell.draft };
            const text = shell.fillCommand(shell.pendingCommand.command, answers);
            shell.pendingCommand = null;
            if (/^https?:\/\//i.test(text.trim())) {
              shell.dialog = null;
              void openUrl(text.trim());
            } else shell.sendCommand(text);
          }
          else if (shell.dialog === "renameTab") {
            shell.settings = { ...shell.settings, tabLabels: shell.pairSetting(shell.settings.tabLabels, shell.draftExtra, shell.draft) };
            void shell.savePrefs(false);
            shell.dialog = null;
          }
          else if (shell.dialog === "colorTab") {
            shell.settings = { ...shell.settings, tabColors: shell.pairSetting(shell.settings.tabColors, shell.draftExtra, shell.draft) };
            void shell.savePrefs(false);
            shell.dialog = null;
          }
          else if (shell.dialog === "groupRename" && shell.draft.trim()) {
            const folder = shell.draftExtra;
            shell.settings = { ...shell.settings, groupNames: [...shell.settings.groupNames.filter((item) => !item.startsWith(`${folder}\t`)), `${folder}\t${shell.draft.trim()}`] };
            void shell.savePrefs(false);
            shell.dialog = null;
          }
          else if (shell.dialog === "checkoutDirty") {
            const name = shell.pendingCheckout;
            shell.dialog = null;
            if (shell.draftExtra === "stash") {
              await shell.mutate({ action: "stash", message: "", includeUntracked: false });
              if (!shell.actionError) await shell.mutate({ action: "checkout", name });
            } else if (shell.draftExtra === "discard") {
              await shell.mutate({ action: "reset", rev: "HEAD", mode: "hard" });
              if (!shell.actionError) await shell.mutate({ action: "checkout", name });
            } else await shell.mutate({ action: "checkout", name });
          }
          else if (shell.dialog === "squash" && shell.selectedCommit) {
            const chosen = shell.shownCommits.filter((commit) => shell.drops.includes(commit.id));
            const from = chosen.length >= 2 ? chosen[chosen.length - 1].id : shell.selectedCommit;
            const to = chosen.length >= 2 ? chosen[0].id : "HEAD";
            void shell.mutate({ action: "squash", from, to, summary: shell.draft });
          }
          else if (shell.dialog === "credential") {
            try {
              await invoke("approve_credential", {
                path: shell.repoPath(),
                protocol: shell.draft,
                host: shell.draftExtra,
                username: shell.draftUser,
                password: shell.draftSecret,
              });
              shell.draftSecret = "";
              shell.dialog = null;
            } catch (error) {
              shell.actionError = shell.message(error);
            }
          }
        }}
      >
        {#if shell.dialog === "prefs"}
          <PrefsDialog />
        {:else if shell.dialog === "reset"}
          <h2>{shell.tr("dialog.resetTo", { name: shell.draft.slice(0, 7) })}</h2>
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => shell.mutate({ action: "reset", rev: shell.draft, mode: "soft" })}>{shell.tr("menu.soft")}</button>
            <button class="text-button" type="button" onclick={() => shell.mutate({ action: "reset", rev: shell.draft, mode: "mixed" })}>{shell.tr("menu.mixed")}</button>
            <button class="text-button" type="button" onclick={shell.hardReset.bind(shell)}>{shell.tr("menu.hard")}</button>
          </div>
        {:else if shell.dialog === "flow"}
          <h2>{shell.tr("chrome.flow")}</h2>
          <input placeholder={shell.tr("dialog.flowName")} bind:value={shell.draft} />
          <input placeholder={shell.tr("dialog.flowMaster")} bind:value={shell.settings.flowMaster} />
          <input placeholder={shell.tr("dialog.flowDevelop")} bind:value={shell.settings.flowDevelop} />
          <input placeholder={shell.tr("dialog.flowFeaturePrefix")} bind:value={shell.settings.flowFeature} />
          <input placeholder={shell.tr("dialog.flowReleasePrefix")} bind:value={shell.settings.flowRelease} />
          <input placeholder={shell.tr("dialog.flowHotfixPrefix")} bind:value={shell.settings.flowHotfix} />
          <input placeholder={shell.tr("dialog.flowSupportPrefix")} bind:value={shell.settings.flowSupport} />
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => shell.mutate({ action: "gitFlowInit", master: shell.settings.flowMaster, develop: shell.settings.flowDevelop, feature: shell.settings.flowFeature, release: shell.settings.flowRelease, hotfix: shell.settings.flowHotfix, support: shell.settings.flowSupport })}>{shell.tr("dialog.flowInit")}</button>
            <label>{shell.tr("dialog.flowKind")}
              <select bind:value={shell.flowKind}>
                <option value="feature">feature</option>
                <option value="release">release</option>
                <option value="hotfix">hotfix</option>
                <option value="support">support</option>
              </select>
            </label>
            <button class="text-button" type="button" onclick={() => shell.mutate({ action: "gitFlowStart", name: shell.draft, kind: shell.flowKind })}>{shell.tr("dialog.flowStart")}</button>
            <button class="text-button" type="button" onclick={() => shell.mutate({ action: "gitFlowFinish", name: shell.draft, kind: shell.flowKind })}>{shell.tr("dialog.flowFinish")}</button>
          </div>
        {:else if shell.dialog === "branch"}
          <h2>{shell.tr("dialog.newBranch")}</h2>
          <input placeholder={shell.tr("dialog.name")} bind:value={shell.draft} />
          <input placeholder={shell.tr("dialog.startPoint")} bind:value={shell.draftExtra} />
          <label class="check"><input type="checkbox" bind:checked={shell.checkoutNewBranch} /> {shell.tr("dialog.checkoutAfter")}</label>
          <button class="commit" type="submit">{shell.tr("dialog.create")}</button>
        {:else if shell.dialog === "clone"}
          <h2>{shell.cloneIntent === "init" ? shell.tr("chrome.newRepo") : shell.tr("chrome.clone")}</h2>
          {#if shell.cloneIntent === "clone"}
            <label>{shell.tr("dialog.url")}
              <input placeholder="https://github.com/org/repo.git" value={shell.draft} use:focusOnMount oninput={(event) => shell.onCloneUrl(event.currentTarget.value)} />
            </label>
          {/if}
          <label>{shell.tr("dialog.folderName")}
            <input placeholder={shell.tr("dialog.folderName")} value={shell.cloneFolder} use:focusWhen={shell.cloneIntent === "init"} oninput={(event) => shell.onCloneFolder(event.currentTarget.value)} />
          </label>
          <div class="k">{shell.tr("dialog.sourceFolder")}</div>
          {#if shell.sourceRootList().length === 0}
            <p class="empty">{shell.tr("dialog.noSourceFolders")}</p>
          {:else}
            <div class="open-col">
              {#each shell.sourceRootList() as root (root.path)}
                <button class="text-button pick clone-root" type="button" aria-pressed={shell.cloneRoot === root.path} onclick={() => (shell.cloneRoot = root.path)}>
                  <strong>{root.alias}</strong>
                  <span>{cloneTarget(root.path, shell.cloneFolder, shell.draft) || root.path}</span>
                </button>
              {/each}
            </div>
          {/if}
          <button class="text-button left" type="button" onclick={() => void shell.browseCloneRoot()}>{shell.tr("dialog.browse")}</button>
          {#if shell.cloneTargetPath}
            <p class="empty">{shell.tr("dialog.cloneInto", { path: shell.cloneTargetPath })}</p>
          {/if}
          {#if shell.actionError}<p class="empty">{shell.actionError}</p>{/if}
          <div class="composer-row">
            {#if shell.cloneIntent === "init"}
              <button class="commit" type="submit" disabled={!shell.cloneTargetPath || !!shell.cloneJob}>{shell.tr("dialog.create")}</button>
            {:else}
              <button class="commit" type="submit" disabled={!cloneUrl(shell.draft) || !shell.cloneTargetPath || !!shell.cloneJob}>{shell.tr("chrome.clone")}</button>
              <button class="text-button" type="button" disabled={!shell.cloneTargetPath || !!shell.cloneJob} onclick={() => void shell.startInit()}>{shell.tr("chrome.newRepo")}</button>
            {/if}
          </div>
        {:else if shell.dialog === "open"}
          <h2>{shell.tr("chrome.openRepo")}</h2>
          {#if shell.sourceRootList().length === 0}
            <p class="empty">{shell.tr("dialog.noSourceFolders")}</p>
          {:else}
            <div class="open-col">
              {#each shell.sourceRootList() as root (root.path)}
                <button class="text-button pick" type="button" onclick={() => shell.openSourceRoot(root.path)}>{root.alias}</button>
              {/each}
            </div>
          {/if}
          <div class="composer-row">
            <button class="text-button" type="button" onclick={async () => { const folder = await shell.browseFolder(); if (folder) void shell.openRepo(folder); }}>{shell.tr("dialog.browse")}</button>
            <button class="text-button" type="button" onclick={() => void shell.beginClone("clone")}>{shell.tr("chrome.clone")}</button>
          </div>
          {#if shell.actionError}<p class="empty">{shell.actionError}</p>{/if}
        {:else if shell.dialog === "tag"}
          <h2>{shell.tr("dialog.newTag")}</h2>
          <input placeholder={shell.tr("dialog.name")} bind:value={shell.draft} />
          <input placeholder={shell.tr("dialog.tagRevision")} bind:value={shell.draftExtra} />
          <input placeholder={shell.tr("dialog.tagMessage")} bind:value={shell.draftUser} />
          <button class="commit" type="submit">{shell.tr("dialog.create")}</button>
        {:else if shell.dialog === "fetch"}
          <h2>{shell.tr("chrome.fetch")}</h2>
          <p>{shell.tr("dialog.fetchBody")}</p>
          <label class="sync-row">
            <span>{shell.tr("dialog.remoteName")}</span>
            {#if (shell.refs?.remotes.length ?? 0) > 0}
              <select bind:value={shell.draft} disabled={shell.syncAll}>
                {#each shell.refs?.remotes ?? [] as remote (remote.name)}
                  <option value={remote.name}>{remote.name}</option>
                {/each}
              </select>
            {:else}
              <input placeholder={shell.tr("dialog.remoteName")} bind:value={shell.draft} disabled={shell.syncAll} />
            {/if}
          </label>
          <label class="check sync-indent"><input type="checkbox" bind:checked={shell.syncAll} /> {shell.tr("dialog.fetchAllRemotes")}</label>
          <label class="check sync-indent"><input type="checkbox" bind:checked={shell.settings.fetchPrune} /> {shell.tr("dialog.pruneOnFetch")}</label>
          <label class="check sync-indent"><input type="checkbox" bind:checked={shell.syncTags} /> {shell.tr("dialog.includeTags")}</label>
          <div class="composer-row">
            <button class="commit" type="submit">{shell.tr("chrome.fetch")}</button>
            <button class="text-button" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("dialog.cancel")}</button>
          </div>
        {:else if shell.dialog === "pull"}
          <h2>{shell.tr("chrome.pull")}</h2>
          <p>{shell.tr("dialog.pullBody")}</p>
          <label class="sync-row">
            <span>{shell.tr("dialog.remoteName")}</span>
            {#if (shell.refs?.remotes.length ?? 0) > 0}
              <select value={shell.draft} onchange={(event) => { shell.draft = event.currentTarget.value; shell.draftExtra = shell.pullBranchFor(shell.draft); }}>
                {#each shell.refs?.remotes ?? [] as remote (remote.name)}
                  <option value={remote.name}>{remote.name}</option>
                {/each}
              </select>
            {:else}
              <input placeholder={shell.tr("dialog.remoteName")} bind:value={shell.draft} />
            {/if}
          </label>
          <label class="sync-row">
            <span>{shell.tr("dialog.branch")}</span>
            {#if (shell.refs?.remotes.find((remote) => remote.name === shell.draft)?.branches.length ?? 0) > 0}
              <select bind:value={shell.draftExtra}>
                {#if shell.draftExtra && !(shell.refs?.remotes.find((remote) => remote.name === shell.draft)?.branches.includes(shell.draftExtra))}
                  <option value={shell.draftExtra}>{shell.draft}/{shell.draftExtra}</option>
                {/if}
                {#each shell.refs?.remotes.find((remote) => remote.name === shell.draft)?.branches ?? [] as branch (branch)}
                  <option value={branch}>{shell.draft}/{branch}</option>
                {/each}
              </select>
            {:else}
              <input placeholder={shell.tr("dialog.branch")} bind:value={shell.draftExtra} />
            {/if}
          </label>
          <div class="sync-row">
            <span>{shell.tr("dialog.pullInto")}</span>
            <strong class="sync-value">{shell.snapshot?.branch ?? ""}</strong>
          </div>
          <label class="check sync-indent"><input type="checkbox" bind:checked={shell.syncRebase} /> {shell.tr("dialog.pullWithRebase")}</label>
          <label class="check sync-indent"><input type="checkbox" bind:checked={shell.syncStash} /> {shell.tr("dialog.pullStash")}</label>
          <div class="composer-row">
            <button class="commit" type="submit">{shell.tr("chrome.pull")}</button>
            <button class="text-button" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("dialog.cancel")}</button>
          </div>
        {:else if shell.dialog === "push"}
          <h2>{shell.tr("chrome.push")}</h2>
          <p>{shell.tr("dialog.pushBody")}</p>
          <label class="sync-row">
            <span>{shell.tr("dialog.branch")}</span>
            <select value={shell.pushBranch} onchange={(event) => { shell.pushBranch = event.currentTarget.value; shell.pushTo = ""; }}>
              {#if shell.pushBranch && !(shell.refs?.branches.some((branch) => branch.name === shell.pushBranch))}
                <option value={shell.pushBranch}>{shell.pushBranch}</option>
              {/if}
              {#each shell.refs?.branches ?? [] as branch (branch.name)}
                <option value={branch.name}>{branch.name}</option>
              {/each}
            </select>
          </label>
          <label class="sync-row">
            <span>{shell.tr("dialog.pushTo")}</span>
            <select bind:value={shell.pushTo}>
              <option value="">{shell.tr("dialog.pushDefault", { target: shell.pushTargetLabel(shell.pushBranch) })}</option>
              {#each shell.refs?.remotes ?? [] as remote (remote.name)}
                {#each remote.branches as branch (branch)}
                  <option value={`${remote.name}/${branch}`}>{remote.name}/{branch}</option>
                {/each}
              {/each}
            </select>
          </label>
          <label class="check sync-indent"><input type="checkbox" bind:checked={shell.syncTags} /> {shell.tr("dialog.pushAllTags")}</label>
          <label class="check sync-indent"><input type="checkbox" bind:checked={shell.syncForce} /> {shell.tr("dialog.forcePush")}</label>
          <div class="composer-row">
            <button class="commit" type="submit">{shell.tr("chrome.push")}</button>
            <button class="text-button" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("dialog.cancel")}</button>
          </div>
        {:else if shell.dialog === "stash"}
          <h2>{shell.stashFiles.length > 1 ? shell.tr("dialog.stashFiles", { count: String(shell.stashFiles.length) }) : shell.stashFiles.length === 1 ? shell.tr("dialog.stashFile", { name: shell.stashFiles[0] }) : shell.tr("chrome.stash")}</h2>
          {#if shell.stashFiles.length > 0}<pre class="diff-body stash-paths">{shell.stashFiles.join("\n")}</pre>{/if}
          <input placeholder={shell.tr("dialog.message")} bind:value={shell.draft} />
          <label class="check"><input type="checkbox" bind:checked={shell.includeUntracked} /> {shell.tr("dialog.includeUntracked")}</label>
          <button class="commit" type="submit">{shell.tr("chrome.stash")}</button>
        {:else if shell.dialog === "rename"}
          <h2>{shell.tr("dialog.renameBranch")}</h2>
          <input placeholder={shell.tr("dialog.currentName")} bind:value={shell.draft} />
          <input placeholder={shell.tr("dialog.to")} bind:value={shell.draftExtra} />
          <button class="commit" type="submit">{shell.tr("dialog.renameBranch")}</button>
        {:else if shell.dialog === "upstream"}
          <h2>{shell.tr("dialog.upstreamFor", { name: shell.draft })}</h2>
          <input placeholder={shell.tr("dialog.upstreamEmpty")} bind:value={shell.draftExtra} />
          <button class="commit" type="submit">{shell.tr("dialog.save")}</button>
        {:else if shell.dialog === "reword"}
          <h2>{shell.tr("dialog.rewordTitle", { name: shell.draftExtra.slice(0, 7) })}</h2>
          <input placeholder={shell.tr("dialog.message")} bind:value={shell.draft} />
          <button class="commit" type="submit">{shell.tr("dialog.rewordAction")}</button>
        {:else if shell.dialog === "rebase"}
          <h2>{shell.tr("dialog.rebase")}</h2>
          <p class="empty">{shell.rebaseConflictFiles.length === 0 ? shell.tr("dialog.conflictNone") : shell.tr("dialog.conflictSome", { name: shell.rebaseConflictFiles.join(", ") })}</p>
          <label class="check"><input type="checkbox" bind:checked={shell.updateRefs} /> {shell.tr("dialog.updateRefs")}</label>
          <div class="dialog-scroll">
          {#each shell.rebaseSteps as step, index (step.rev)}
            <div class="composer-row" role="listitem" draggable="true" ondragstart={() => (shell.dragStep = index)} ondragover={(event) => event.preventDefault()} ondrop={() => { if (shell.dragStep == null || shell.dragStep === index) return; const copy = [...shell.rebaseSteps]; const [item] = copy.splice(shell.dragStep, 1); copy.splice(index, 0, item); shell.rebaseSteps = copy; shell.dragStep = null; }}>
              <input type="checkbox" checked={shell.rebasePicked.includes(step.rev)} aria-label={step.summary} onchange={() => { shell.rebasePicked = shell.rebasePicked.includes(step.rev) ? shell.rebasePicked.filter((rev) => rev !== step.rev) : [...shell.rebasePicked, step.rev]; }} />
              <select value={step.verb} onchange={(event) => shell.setRebaseVerb(step.rev, event.currentTarget.value)}>
                <option value="pick">pick</option>
                <option value="reword">reword</option>
                <option value="squash">squash</option>
                <option value="fixup">fixup</option>
                <option value="drop">drop</option>
                <option value="edit">edit</option>
              </select>
              <span>{step.summary}</span>
            </div>
            {#if step.verb === "reword" || step.verb === "squash" || step.verb === "exec"}
              <input placeholder={shell.tr("dialog.message")} bind:value={step.message} />
            {/if}
          {/each}
          </div>
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => { shell.rebaseSteps = [...shell.rebaseSteps, { verb: "break", rev: `break-${Date.now()}`, summary: "break", message: "" }]; }}>break</button>
            <button class="text-button" type="button" onclick={() => { shell.rebaseSteps = [...shell.rebaseSteps, { verb: "exec", rev: `exec-${Date.now()}`, summary: "exec", message: "echo awegit" }]; }}>exec</button>
          </div>
          <button class="commit" type="submit" disabled={shell.mode !== "live"}>{shell.tr("dialog.startRebase")}</button>
        {:else if shell.dialog === "patch"}
          <h2>{shell.tr("dialog.applyPatch")}</h2>
          <textarea rows="8" placeholder={shell.tr("dialog.patchBody")} bind:value={shell.draft}></textarea>
          <button class="commit" type="submit">{shell.tr("dialog.applyNow")}</button>
        {:else if shell.dialog === "submodule"}
          <h2>{shell.tr("dialog.addSubmodule")}</h2>
          <input placeholder={shell.tr("dialog.url")} bind:value={shell.draft} />
          <input placeholder={shell.tr("dialog.path")} bind:value={shell.draftExtra} />
          <button class="commit" type="submit">{shell.tr("dialog.create")}</button>
        {:else if shell.dialog === "worktree"}
          <h2>{shell.tr("dialog.addWorktree")}</h2>
          <input placeholder={shell.tr("dialog.path")} bind:value={shell.draft} />
          <input placeholder={shell.tr("dialog.branch")} bind:value={shell.draftExtra} />
          <button class="commit" type="submit">{shell.tr("dialog.create")}</button>
        {:else if shell.dialog === "workspace"}
          <h2>{shell.tr("chrome.workspaceConfigure")}</h2>
          <div class="workspace-list">
            {#each shell.settings.workspaces as item (item.id)}
              <div class="workspace-row" class:current={item.id === shell.settings.currentWorkspace}>
                <span class="workspace-mark" aria-hidden="true"></span>
                <input value={item.name} aria-label={shell.tr("chrome.workspaceName")} onchange={(event) => shell.renameWorkspace(item.id, event.currentTarget.value)} />
                <span class="workspace-count" title={shell.tr("chrome.manager")}>{item.repositories.length}</span>
                <button class="text-button workspace-delete" type="button" onclick={() => shell.ask(shell.tr("dialog.deleteWorkspace"), shell.tr("dialog.deleteWorkspaceBody", { name: item.name }), () => void shell.deleteWorkspace(item.id))}>{shell.tr("menu.delete")}</button>
              </div>
            {/each}
          </div>
          <div class="composer-row workspace-create">
            <input placeholder={shell.tr("chrome.workspaceName")} bind:value={shell.draft} />
            <button class="commit" type="submit">{shell.tr("chrome.workspaceCreate")}</button>
          </div>
        {:else if shell.dialog === "resolve"}
          <h2>{shell.tr("dialog.mergeResolve")} {shell.draft}</h2>
          <div class="merge-grid">
            <label>{shell.tr("dialog.useOurs")}<textarea rows="8" readonly value={shell.conflict?.ours ?? ""}></textarea></label>
            <label>{shell.tr("dialog.useTheirs")}<textarea rows="8" readonly value={shell.conflict?.theirs ?? ""}></textarea></label>
            <label>{shell.tr("dialog.result")}<textarea rows="8" bind:value={shell.mergeResult}></textarea></label>
          </div>
          <div class="hunk-marks">
            {#each shell.mergePieces as piece, index (index)}
              {#if piece.kind === "hunk"}<button type="button" aria-label={shell.tr("dialog.result")} style:left="{(index + 1) / Math.max(shell.mergePieces.length, 1) * 100}%" onclick={() => { const area = document.querySelector<HTMLTextAreaElement>(".merge-grid textarea:last-of-type"); if (area) area.scrollTop = (index / Math.max(shell.mergePieces.length, 1)) * area.scrollHeight; }}></button>{/if}
            {/each}
          </div>
          {#each shell.mergePieces as piece, index (index)}
            {#if piece.kind === "hunk"}
              <div class="composer-row">
                <span class="meta">{shell.tr("dialog.result")} {index + 1}</span>
                <button class="text-button" type="button" class:on={piece.pick === "ours"} onclick={() => shell.pickHunk(index, "ours")}>{shell.tr("dialog.useOurs")}</button>
                <button class="text-button" type="button" class:on={piece.pick === "theirs"} onclick={() => shell.pickHunk(index, "theirs")}>{shell.tr("dialog.useTheirs")}</button>
                <button class="text-button" type="button" class:on={piece.pick === "both"} onclick={() => shell.pickHunk(index, "both")}>{shell.tr("dialog.useBoth")}</button>
              </div>
            {/if}
          {/each}
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => (shell.draftExtra = "ours")}>{shell.tr("dialog.useOurs")}</button>
            <button class="text-button" type="button" onclick={() => (shell.draftExtra = "theirs")}>{shell.tr("dialog.useTheirs")}</button>
            <button class="text-button" type="button" onclick={() => (shell.draftExtra = "both")}>{shell.tr("dialog.useBoth")}</button>
            <button class="text-button" type="button" onclick={() => (shell.draftExtra = "text")}>{shell.tr("dialog.result")}</button>
          </div>
          <button class="commit" type="submit">{shell.tr("dialog.applyNow")}</button>
        {:else if shell.dialog === "merge"}
          {@const into = shell.mergeInto || shell.snapshot?.branch || "HEAD"}
          <h2>{shell.tr(shell.draftExtra === "squash" ? "dialog.mergeSquashTitle" : "dialog.mergeTitleLine", { source: shell.draft, into })}</h2>
          <div class="merge-route">
            <span>{shell.draft}</span>
            <span class="meta" aria-hidden="true">→</span>
            <span>{into}</span>
          </div>
          <label>{shell.tr("dialog.merge")}
            <select value={shell.draftExtra} onchange={(event) => { const target = event.currentTarget; if (!(target instanceof HTMLSelectElement)) return; shell.draftExtra = target.value; shell.refreshMergeSummary(); }}>
              <option value="ff">{shell.tr("dialog.fastForward")}</option>
              <option value="no-ff">{shell.tr("dialog.noFastForward")}</option>
              <option value="squash">{shell.tr("dialog.squash")}</option>
            </select>
          </label>
          <label>{shell.tr("dialog.message")}
            <input bind:value={shell.mergeSummary} />
          </label>
          <label>{shell.tr("dialog.mergeDetail")}
            <textarea class="merge-body" rows="6" bind:value={shell.mergeBody}></textarea>
          </label>
          <p class="empty">{shell.tr("dialog.mergeFileCount", { count: String(shell.mergeFiles.length) })}</p>
          {#if shell.draftExtra === "ff"}<p class="empty">{shell.tr("dialog.mergeFastForwardNote")}</p>{/if}
          <p class="empty">{shell.settings.mergeAutostash ? shell.tr("dialog.mergeAutostash") : shell.tr("dialog.mergeClean")}</p>
          <button class="commit" type="submit">{shell.tr("dialog.merge")}</button>
        {:else if shell.dialog === "repo"}
          <h2>{shell.tr("chrome.repoSettings")}</h2>
          <label>{shell.tr("dialog.authorName")} <input bind:value={shell.draft} placeholder={shell.tr("dialog.authorEmpty")} /></label>
          <label>{shell.tr("dialog.authorEmail")} <input bind:value={shell.draftExtra} /></label>
          <label class="check"><input type="checkbox" bind:checked={shell.repoSignOff} /> {shell.tr("dialog.signOffRepo")}</label>
          <label>{shell.tr("dialog.signOffFormat")} <input bind:value={shell.draftUser} /></label>
          <p class="empty">{shell.tr("dialog.repoLocalHint")}</p>
          <button class="commit" type="submit">{shell.tr("dialog.save")}</button>
        {:else if shell.dialog === "continue"}
          <h2>{shell.tr("dialog.continueRebase")}</h2>
          <p class="empty">{shell.tr("dialog.continueHint")}</p>
          <input placeholder={shell.tr("dialog.continueMessage")} bind:value={shell.draft} />
          <button class="commit" type="submit">{shell.tr("dialog.continueAction")}</button>
        {:else if shell.dialog === "about"}
          <h2>{shell.tr("dialog.about")} {shell.appVersion}</h2>
          <p class="empty">{shell.tr("dialog.aboutBody")}</p>
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => void copyDiagnostics()}>{diagnosticsCopied ? shell.tr("chrome.copied") : shell.tr("dialog.copyDiagnostics")}</button>
            <button class="commit" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("chrome.close")}</button>
          </div>
        {:else if shell.dialog === "output"}
          <h2>{shell.tr("dialog.commandOutput")}</h2>
          <pre class="diff-body">{shell.commandOutput}</pre>
          <button class="commit" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("chrome.close")}</button>
        {:else if shell.dialog === "mergeConflict"}
          <h2>{shell.tr("dialog.mergeConflictTitle")}</h2>
          <p class="empty">{shell.tr("dialog.mergeConflictBody")}</p>
          <pre class="diff-body merge-conflict-log" data-layer="merge-conflict">{shell.commandOutput}</pre>
          <div class="composer-row">
            <span class="grow"></span>
            <button class="commit" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("chrome.close")}</button>
          </div>
        {:else if shell.dialog === "remote"}
          <h2>{shell.tr("chrome.groupRemote")}</h2>
          <input placeholder={shell.tr("dialog.name")} bind:value={shell.draft} />
          <input placeholder={shell.tr("dialog.url")} bind:value={shell.draftExtra} />
          <div class="composer-row">
            <button class="commit" type="submit">{shell.tr("dialog.create")}</button>
            <button class="text-button" type="button" onclick={() => shell.mutate({ action: "setRemoteUrl", name: shell.draft, url: shell.draftExtra })}>{shell.tr("dialog.setRemoteUrl")}</button>
          </div>
        {:else if shell.dialog === "squash"}
          <h2>{shell.tr("dialog.squash")}</h2>
          <p class="empty">{shell.tr("dialog.squashHint")}</p>
          <input placeholder={shell.tr("dialog.message")} bind:value={shell.draft} />
          <button class="commit" type="submit">{shell.tr("dialog.squash")}</button>
        {:else if shell.dialog === "checkoutDirty"}
          <h2>{shell.tr("menu.checkout")} {shell.pendingCheckout}</h2>
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => { shell.draftExtra = "bring"; }}> {shell.tr("dialog.bringChanges")}</button>
            <button class="text-button" type="button" onclick={() => { shell.draftExtra = "stash"; }}>{shell.tr("dialog.stashFirst")}</button>
            <button class="text-button" type="button" onclick={() => { shell.draftExtra = "discard"; }}>{shell.tr("dialog.discardChanges")}</button>
          </div>
          <button class="commit" type="submit">{shell.draftExtra || shell.tr("dialog.bringChanges")}</button>
        {:else if shell.dialog === "checkoutNotice"}
          <h2>{shell.tr("menu.checkout")}</h2>
          <p class="empty">{shell.tr("dialog.checkoutStashed", { name: shell.pendingCheckout })}</p>
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("chrome.close")}</button>
            <button class="commit" type="button" onclick={() => { shell.dialog = null; void shell.mutate({ action: "stashPop", name: "stash@{0}" }); }}>{shell.tr("dialog.applyStash")}</button>
          </div>
        {:else if shell.dialog === "activity"}
          <h2>{shell.tr("chrome.activity")}</h2>
          <pre class="diff-body">{[...shell.sessionLog, ...shell.activityLines].join("\n") || shell.tr("chrome.noChanges")}</pre>
          <button class="commit" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("chrome.close")}</button>
        {:else if shell.dialog === "mailmap"}
          <h2>{shell.tr("chrome.mailmap")}</h2>
          <textarea rows="8" placeholder="Name <email> <alias>" bind:value={shell.draft}></textarea>
          <button class="commit" type="submit">{shell.tr("dialog.save")}</button>
        {:else if shell.dialog === "accounts"}
          <h2>{shell.tr("chrome.accounts")}</h2>
          {#each shell.settings.accounts as account (account.id)}
            <div class="composer-row">
              <button class="text-button" type="button" onclick={() => { shell.editingAccount = account.id; shell.hostForge = account.forge; shell.draft = account.label; shell.draftExtra = account.host; shell.draftUser = account.token; shell.settings.oauthClientId = account.clientId; }}>{account.forge} {account.label}</button>
              <button class="text-button" type="button" onclick={() => { shell.settings = { ...shell.settings, accounts: shell.settings.accounts.filter((item) => item.id !== account.id) }; if (shell.editingAccount === account.id) shell.editingAccount = ""; void shell.savePrefs(false); }}>{shell.tr("menu.delete")}</button>
            </div>
          {/each}
          <select bind:value={shell.hostForge}>
            <option value="github">GitHub</option>
            <option value="gitlab">GitLab</option>
            <option value="bitbucket">Bitbucket</option>
            <option value="azure">Azure</option>
          </select>
          <input placeholder={shell.tr("dialog.name")} bind:value={shell.draft} />
          <input placeholder="host" bind:value={shell.draftExtra} />
          <input placeholder="token" bind:value={shell.draftUser} />
          <input placeholder={shell.tr("dialog.clientId")} bind:value={shell.settings.oauthClientId} />
          <div class="composer-row">
            <button class="text-button" type="button" onclick={shell.saveAccount.bind(shell)}>{shell.editingAccount ? shell.tr("menu.update") : shell.tr("dialog.save")}</button>
            <button class="text-button" type="button" onclick={() => void invoke<{ verificationUri: string; userCode: string }>("device_start", { forgeName: shell.hostForge, clientId: shell.settings.oauthClientId, host: shell.draftExtra }).then((value) => { shell.commandOutput = `${value.userCode}\n${value.verificationUri}`; shell.dialog = "output"; if (value.verificationUri) void openUrl(value.verificationUri); }).catch((error) => (shell.actionError = shell.message(error)))}>{shell.tr("dialog.deviceLogin")}</button>
            <button class="text-button" type="button" onclick={() => void invoke<string>("device_poll").then((token) => { if (token === "pending") { shell.actionError = shell.tr("dialog.pending"); return; } shell.draftUser = token; })}>{shell.tr("dialog.pending")}</button>
          </div>
        {:else if shell.dialog === "ssh"}
          <h2>{shell.tr("chrome.sshConfig")}</h2>
          <textarea rows="10" bind:value={shell.draft}></textarea>
          <button class="commit" type="submit">{shell.tr("dialog.save")}</button>
        {:else if shell.dialog === "stale"}
          <h2>{shell.tr("chrome.stale")}</h2>
          {#each shell.staleNames as name (name)}
            <label class="check"><input type="checkbox" checked={shell.stalePicked.includes(name)} onchange={() => { shell.stalePicked = shell.stalePicked.includes(name) ? shell.stalePicked.filter((item) => item !== name) : [...shell.stalePicked, name]; }} /> {name}</label>
          {/each}
          <button class="commit" type="button" onclick={async () => { const names = [...shell.stalePicked]; shell.dialog = null; for (const name of names) await shell.mutate({ action: "deleteBranch", name, force: false }); }}>{shell.tr("menu.delete")}</button>
        {:else if shell.dialog === "pushAs"}
          <h2>{shell.tr("menu.pushAs")}</h2>
          <input placeholder={shell.tr("dialog.branch")} bind:value={shell.draft} />
          <input placeholder="remote" bind:value={shell.draftExtra} />
          <input placeholder={shell.tr("dialog.remoteBranch")} bind:value={shell.draftUser} />
          <button class="commit" type="submit">{shell.tr("chrome.push")}</button>
        {:else if shell.dialog === "askInput"}
          <h2>{shell.pendingCommand?.prompt.split("\n")[0]}</h2>
          {#each Object.keys(shell.fieldValues) as label (label)}
            <label>{label}<input value={shell.fieldValues[label] ?? ""} oninput={(event) => { shell.fieldValues = { ...shell.fieldValues, [label]: (event.currentTarget as HTMLInputElement).value }; }} /></label>
          {:else}
            <input bind:value={shell.draft} />
          {/each}
          <button class="commit" type="submit">{shell.tr("chrome.commit")}</button>
        {:else if shell.dialog === "renameTab"}
          <h2>{shell.tr("menu.renameTab")}</h2>
          <input bind:value={shell.draft} />
          <button class="commit" type="submit">{shell.tr("dialog.save")}</button>
        {:else if shell.dialog === "colorTab"}
          <h2>{shell.tr("menu.tabColor")}</h2>
          <div class="composer-row">
            {#each ["#5b8def", "#e07a3d", "#3aa76d", "#c45b8a", "#7a6ad8", "#c9a227", "#d16464"] as color (color)}
              <button class="swatch" type="button" style:background={color} aria-label={color} onclick={() => (shell.draft = color)}></button>
            {/each}
            <input type="color" bind:value={shell.draft} aria-label={shell.tr("menu.tabColor")} />
          </div>
          <button class="commit" type="submit">{shell.tr("dialog.save")}</button>
        {:else if shell.dialog === "groupRename"}
          <h2>{shell.tr("chrome.manager")}</h2>
          <input bind:value={shell.draft} />
          <button class="commit" type="submit">{shell.tr("dialog.save")}</button>
        {:else if shell.dialog === "gitignore"}
          <h2>{shell.tr("dialog.gitignore")}</h2>
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => (shell.draft = "node_modules/\ndist/\n")}>Node</button>
            <button class="text-button" type="button" onclick={() => (shell.draft = "target/\n")}>Rust</button>
            <button class="text-button" type="button" onclick={() => (shell.draft = ".DS_Store\nThumbs.db\n")}>OS</button>
            <button class="text-button" type="button" onclick={() => (shell.draft = "__pycache__/\n.venv/\n")}>Python</button>
          </div>
          <textarea rows="8" bind:value={shell.draft}></textarea>
          <button class="commit" type="submit">{shell.tr("dialog.save")}</button>
        {:else if shell.dialog === "stats"}
          <h2>{shell.tr("chrome.stats")}</h2>
          <div class="stats-row">{#each shell.tr("dialog.weekdays").split(" ") as day}<span>{day}</span>{/each}</div>
          <div class="stats-row">{#each shell.facts?.weekday ?? [] as count, index (index)}<span title={String(count)}>{count}</span>{/each}</div>
          <p class="empty">{shell.tr("dialog.byHour")}</p>
          <div class="stats-row">{#each shell.facts?.hour ?? [] as count, index (index)}<span title={`${index}:00 ${count}`}>{count}</span>{/each}</div>
          <div class="treemap">
            {#each shell.facts?.hot ?? [] as item (item.path)}
              <span style:flex-grow={item.count} title={`${item.path} ${item.count}`}>{item.path.split("/").pop()}</span>
            {/each}
          </div>
          <button class="commit" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("chrome.close")}</button>
        {:else if shell.dialog === "lfs"}
          <h2>{shell.tr("chrome.lfsFiles")}</h2>
          <pre class="diff-body">{shell.facts?.lfs || shell.tr("chrome.noChanges")}</pre>
          <div class="composer-row">
            <button class="text-button" type="button" onclick={() => void shell.mutate({ action: "lfsPull" })}>{shell.tr("chrome.pull")}</button>
            <button class="text-button" type="button" onclick={() => void shell.mutate({ action: "lfsPush" })}>{shell.tr("chrome.push")}</button>
          </div>
          <button class="commit" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("chrome.close")}</button>
        {:else if shell.dialog === "github"}
          <h2>{shell.tr("chrome.githubRepo")}</h2>
          <select bind:value={shell.hostForge}>
            <option value="github">GitHub</option>
            <option value="gitlab">GitLab</option>
            <option value="bitbucket">Bitbucket</option>
            <option value="azure">Azure</option>
          </select>
          <input placeholder={shell.tr("dialog.name")} bind:value={shell.draft} />
          <input placeholder={shell.hostForge === "azure" ? "project" : "host"} bind:value={shell.draftExtra} />
          <input placeholder={shell.hostForge === "bitbucket" ? "workspace" : shell.tr("dialog.remoteBranch")} bind:value={shell.draftUser} />
          <label class="check"><input type="checkbox" bind:checked={shell.privateRepo} /> {shell.tr("dialog.privateRepo")}</label>
          <button class="commit" type="submit">{shell.tr("dialog.create")}</button>
        {:else if shell.dialog === "stashRename"}
          <h2>{shell.tr("menu.renameStash")}</h2>
          <input placeholder={shell.tr("dialog.message")} bind:value={shell.draft} />
          <button class="commit" type="submit">{shell.tr("dialog.save")}</button>
        {:else if shell.dialog === "stashBranch"}
          <h2>{shell.tr("menu.stashBranch")}</h2>
          <input placeholder={shell.tr("dialog.branch")} bind:value={shell.draft} />
          <button class="commit" type="submit">{shell.tr("dialog.create")}</button>
        {:else if shell.dialog === "credential"}
          <h2>{shell.tr("dialog.httpsLogin")}</h2>
          <p class="empty">{shell.tr("dialog.httpsHint")}</p>
          <input placeholder={shell.tr("dialog.protocol")} bind:value={shell.draft} />
          <input placeholder={shell.tr("dialog.host")} bind:value={shell.draftExtra} />
          <input placeholder={shell.tr("dialog.username")} bind:value={shell.draftUser} />
          <input placeholder={shell.tr("dialog.password")} type="password" bind:value={shell.draftSecret} />
          <button class="commit" type="submit">{shell.tr("dialog.saveInGit")}</button>
        {:else}
          <h2>{shell.tr("dialog.customCommands")}</h2>
          {#each shell.settings.commands as command (command.id)}
            <div class="composer-row">
              <span>{command.name}</span>
              <span class="meta">{command.target}</span>
              <button class="text-button" type="button" onclick={() => shell.runCommand(command)}>{shell.tr("dialog.run")}</button>
              <button class="text-button" type="button" onclick={() => shell.removeCommand(command.id)}>{shell.tr("menu.delete")}</button>
            </div>
          {/each}
          <input placeholder={shell.tr("dialog.name")} bind:value={shell.draft} />
          <select bind:value={shell.draftExtra}>
            <option value="repository">{shell.tr("chrome.repository")}</option>
            <option value="commit">{shell.tr("chrome.commit")}</option>
            <option value="branch">{shell.tr("dialog.branch")}</option>
            <option value="file">{shell.tr("dialog.file")}</option>
          </select>
          <input placeholder={"${repo} ${sha} ${branch} ${file} ${input}"} bind:value={shell.draftUser} />
          <textarea rows="3" placeholder={shell.tr("dialog.commandPrompt")} bind:value={shell.commandPrompt}></textarea>
          <label class="check"><input type="checkbox" bind:checked={shell.commandShared} /> {shell.tr("chrome.shared")}</label>
          <div class="composer-row">
            <button class="text-button" type="button" onclick={shell.saveCommand.bind(shell)}>{shell.tr("dialog.save")}</button>
            <button class="commit" type="submit">{shell.tr("dialog.run")}</button>
          </div>
        {/if}
      </form>
      </div>
    </div>
  {/if}
