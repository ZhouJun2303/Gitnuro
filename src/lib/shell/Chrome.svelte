<script lang="ts">
  import { getShell } from "./context";
  import { fillWorkspaceTabs } from "../status";
  import "./Chrome.css";
  import { shortcutLabel } from "../keys";
  import { invoke } from "../perf";

  const shell = getShell();

  /** Write tab width once per frame, and only after the count or the strip size is marked dirty. */
  function fillTabs(node: HTMLElement, count: number) {
    let current = Math.max(1, count);
    let dirty = false;
    let frame = 0;
    let painted = "";
    let misses = 0;

    const flush = () => {
      frame = 0;
      if (!dirty) return;
      if (!node.isConnected || node.clientWidth <= 0) {
        dirty = true;
        if (misses < 8) {
          misses += 1;
          frame = requestAnimationFrame(flush);
        }
        return;
      }
      dirty = false;
      misses = 0;
      if (node.closest(".vertical-tabs")) {
        if (!painted) return;
        node.style.removeProperty("--tab-width");
        node.style.removeProperty("--tab-extra");
        painted = "";
        return;
      }
      const { width, extra } = fillWorkspaceTabs(node.clientWidth, current);
      const next = `${width}:${extra}`;
      if (next === painted) return;
      painted = next;
      node.style.setProperty("--tab-width", `${width}px`);
      node.style.setProperty("--tab-extra", `${extra}px`);
    };

    const mark = () => {
      dirty = true;
      if (frame) return;
      if (node.isConnected && node.clientWidth > 0) {
        flush();
        return;
      }
      frame = requestAnimationFrame(flush);
    };

    const observer = new ResizeObserver(mark);
    observer.observe(node);
    mark();
    return {
      update(next: number) {
        const countNext = Math.max(1, next);
        if (countNext === current) return;
        current = countNext;
        mark();
      },
      destroy() {
        if (frame) cancelAnimationFrame(frame);
        observer.disconnect();
      },
    };
  }

</script>

<nav class="menubar">
    <button type="button" onclick={(event) => shell.openBar(event, shell.viewItems())}>{shell.tr("chrome.view")}</button>
    <button type="button" onclick={(event) => shell.openBar(event, shell.repositoryItems())}>{shell.tr("chrome.repository")}</button>
    <button type="button" onclick={(event) => shell.openBar(event, shell.windowItems())}>{shell.tr("chrome.window")}</button>
    <button type="button" onclick={(event) => shell.openBar(event, shell.helpItems())}>{shell.tr("chrome.help")}</button>
  </nav>
  <header class="chrome">
    <div class="chrome-tools">
      <div class="chrome-left">
        <button class="tool-btn stack" type="button" title="{shell.tr("chrome.quickLaunch")} ({shortcutLabel("launch")})" onclick={() => void shell.openLaunch()}>
          <svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M13 2 4 14h7l-1 8 9-12h-7z"/></svg>
          <span>{shell.tr("chrome.launch")}</span>
        </button>
        <button class="tool-btn stack" type="button" disabled={shell.busy} onclick={() => shell.confirmSync("fetch")} oncontextmenu={(event) => shell.openMenu(event, [
          { label: shell.tr("chrome.fetch"), shortcut: shortcutLabel("fetch"), disabled: shell.mode !== "live", run: () => void shell.doFetch() },
          { label: shell.tr("chrome.fetchAll"), disabled: shell.mode !== "live", run: () => void shell.mutate({ action: "fetch", remote: null, prune: shell.settings.fetchPrune, tags: shell.settings.fetchTags }) },
        ])}><svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/></svg><span>{shell.tr("chrome.fetch")}</span></button>
        <button class="tool-btn stack" type="button" disabled={shell.busy} onclick={() => shell.confirmSync("pull")} oncontextmenu={(event) => shell.openMenu(event, [
          { label: shell.tr("chrome.pull"), shortcut: shortcutLabel("pull"), disabled: shell.mode !== "live", run: () => void shell.doPull() },
          { label: shell.tr("dialog.fastForward"), disabled: shell.mode !== "live", run: () => { shell.settings.pullRebase = false; void shell.doPull(); } },
        ])}><svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3v14M6 11l6 6 6-6M5 21h14"/></svg><span>{shell.tr("chrome.pull")}</span>{#if shell.currentBranch && shell.currentBranch.behind > 0}<em class="sync-count">{shell.currentBranch.behind}</em>{/if}</button>
        <button class="tool-btn stack" type="button" disabled={shell.busy} onclick={() => shell.confirmSync("push")} oncontextmenu={(event) => shell.openMenu(event, [
          { label: shell.tr("chrome.push"), shortcut: shortcutLabel("push"), disabled: shell.mode !== "live", run: () => void shell.doPush() },
          { label: shell.tr("chrome.pushTags"), disabled: shell.mode !== "live", run: () => void shell.mutate({ action: "push", remote: null, setUpstream: true, tags: true, forceWithLease: shell.settings.forceWithLease }) },
        ])}><svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 21V7M6 13l6-6 6 6M5 3h14"/></svg><span>{shell.tr("chrome.push")}</span>{#if shell.currentBranch && shell.currentBranch.ahead > 0}<em class="sync-count">{shell.currentBranch.ahead}</em>{/if}</button>
        <button class="tool-btn stack" type="button" disabled={shell.busy || shell.mode !== "live"} onclick={() => shell.openStash([])} oncontextmenu={(event) => shell.openMenu(event, shell.stashItems())}>
          <svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M21 8v13H3V8M1 3h22v5H1zM10 12h4"/></svg>
          <span>{shell.tr("chrome.stash")}</span>
        </button>
        <button class="tool-btn caret-only" type="button" disabled={shell.busy || shell.mode !== "live"} aria-label={shell.tr("chrome.stash")} aria-haspopup="menu" onclick={(event) => shell.openBar(event, shell.stashItems())}>
          <svg class="line-icon caret" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>
        </button>
      </div>
      <div class="chrome-identity">
        <div class="identity-floor" class:working={shell.workDepth > 0}>
          <button class="icon-btn history-btn" class:on={shell.commandLogOpen} class:alert={shell.unseenErrors > 0} type="button" bind:this={shell.commandButton} title={shell.unseenErrors > 0 ? `${shell.tr("chrome.commandHistory")} (${shell.unseenErrors})` : shell.tr("chrome.commandHistory")} aria-label={shell.unseenErrors > 0 ? `${shell.tr("chrome.commandHistory")} (${shell.unseenErrors})` : shell.tr("chrome.commandHistory")} aria-expanded={shell.commandLogOpen} aria-haspopup="dialog" onclick={() => (shell.commandLogOpen ? (shell.commandLogOpen = false) : shell.openCommandLog())}>
            <svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h12"/></svg>
            {#if shell.unseenErrors > 0}<em class="error-count">{shell.unseenErrors > 99 ? "99+" : shell.unseenErrors}</em>{/if}
          </button>
          <div class="identity-copy">
            <strong>{shell.focusPath && !shell.viewing(shell.focusPath) ? shell.folderName(shell.focusPath) : shell.snapshot ? shell.folderName(shell.snapshot.path) : shell.mode === "sample" ? "AweGit" : shell.tr("chrome.welcomeTitle")}</strong>
            <span>
              {#if shell.showingClone && shell.cloneJob}
                <em class="identity-branch">{shell.tr("chrome.cloning", { name: shell.cloneJob.name })}</em>
              {:else if shell.workDepth > 0}
                <em class="identity-branch">{shell.busyLabel ? `${shell.tr("chrome.working")} ${shell.busyLabel}` : shell.tr("chrome.working")}</em>
              {:else if shell.snapshot || shell.mode === "sample"}
                <svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M6 3v12a3 3 0 1 0 3 3"/><circle cx="6" cy="18" r="2.2"/><circle cx="18" cy="6" r="2.2"/><path d="M18 8.2A6 6 0 0 1 9 12"/></svg>
                <em class="identity-branch">{shell.branchLabel}</em>
              {:else}
                {shell.tr("chrome.openRepo")}
              {/if}
              {#if shell.workPercent != null}<em class="identity-pct">{shell.workPercent}%</em>{/if}
            </span>
          </div>
          {#if shell.workDepth > 0}
            <button class="text-button identity-cancel" type="button" onclick={() => (shell.showingClone ? shell.cancelClone() : invoke("cancel_operation", { path: shell.meterPath || shell.repoPath() }))}>{shell.tr("chrome.cancel")}</button>
          {/if}
          <button class="tool-btn stack branch-chip" type="button" aria-haspopup="menu" onclick={(event) => shell.openBar(event, shell.branchItems())}>
            <svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M6 3v12a3 3 0 1 0 3 3"/><circle cx="6" cy="18" r="2.2"/><circle cx="18" cy="6" r="2.2"/><path d="M18 8.2A6 6 0 0 1 9 12"/></svg>
            <span>{shell.tr("dialog.branch")}<svg class="line-icon caret" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg></span>
          </button>
          {#if shell.workDepth > 0}
            {#if shell.workPercent != null}
              <div class="identity-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={shell.workPercent} aria-valuetext="{shell.workPercent}%">
                <span style:width="{shell.workPercent}%"></span>
              </div>
            {/if}
            <div class="identity-sweep" aria-hidden="true"></div>
          {/if}
        </div>
      </div>
      <div class="chrome-right">
        <button class="tool-btn stack" type="button" disabled={!shell.snapshot} onclick={() => shell.snapshot && void shell.openLocal(shell.snapshot.path)}>
          <svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/></svg>
          <span>{shell.tr("chrome.explorer")}</span>
        </button>
        <button class="tool-btn stack" type="button" disabled={shell.mode !== "live"} onclick={() => invoke("open_terminal", { path: shell.repoPath() })}>
          <svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="3"/><path d="m7 9 3 3-3 3M12 15h5"/></svg>
          <span>{shell.tr("chrome.terminal")}</span>
        </button>
        <button class="tool-btn stack" type="button" aria-haspopup="menu" onclick={(event) => shell.openBar(event, shell.appearanceItems())}>
          <svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18M9 9v11"/></svg>
          <span>{shell.tr("dialog.appearance")}</span>
        </button>
        <button class="tool-btn stack workspace-chip" type="button" title={shell.currentWorkspace?.name ?? shell.tr("chrome.workspace")} aria-label={shell.tr("chrome.workspace")} aria-haspopup="menu" onclick={(event) => shell.openBar(event, shell.workspaceItems())}>
          <svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><rect x="3" y="3" width="13" height="13" rx="2"/><path d="M8 20h11a2 2 0 0 0 2-2V8"/></svg>
          <span><span class="workspace-name">{shell.currentWorkspace?.name ?? shell.tr("chrome.workspace")}</span><svg class="line-icon caret" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg></span>
        </button>
      </div>
    </div>
    <div class="chrome-tabs">
      <div class="chrome-side tabs" use:fillTabs={shell.repos.length}>
      {#each shell.repos as repo (repo)}
        {@const job = shell.jobs[shell.jobKey(repo)]}
        <div class="tab" role="group" class:active={shell.jobKey(repo) === shell.jobKey(shell.meterPath)} class:working={(job?.depth ?? 0) > 0} class:dirty={shell.settings.tabIndicator && !shell.switching && shell.jobKey(shell.snapshot?.path) === shell.jobKey(repo) && shell.changeCount > 0} style:box-shadow={shell.tabColor(repo) ? `inset 0 -2px 0 ${shell.tabColor(repo)}` : ""} oncontextmenu={(event) => shell.openMenu(event, [
          { label: shell.tr("chrome.terminal"), disabled: shell.mode !== "live", run: () => void invoke("open_terminal", { path: repo }) },
          { label: shell.tr("chrome.explorer"), run: () => void shell.openLocal(repo) },
          { label: shell.tr("menu.renameTab"), run: () => { shell.draft = shell.tabText(repo); shell.draftExtra = repo; shell.dialog = "renameTab"; } },
          { label: shell.tr("menu.tabColor"), run: () => { shell.draft = shell.tabColor(repo) || "#5b8def"; shell.draftExtra = repo; shell.dialog = "colorTab"; } },
          { label: shell.tr("chrome.close"), run: () => shell.closeTab(repo) },
        ])}>
          <button class="file-select" type="button" title={job && job.depth > 0 ? (job.percent != null ? `${job.label} ${job.percent}%` : job.label) : shell.tabText(repo)} onclick={() => shell.openRepo(repo)}>{#if job && job.depth > 0}<i class="tab-job" aria-hidden="true"></i>{/if}{shell.tabText(repo)}</button>
          <button class="text-button row-action tab-close" type="button" onclick={() => shell.closeTab(repo)} aria-label={shell.tr("chrome.close")}>×</button>
        </div>
      {:else}
        <button class="tab active" type="button">{shell.tabLabel}</button>
      {/each}
      </div>
      <button class="tab-add" type="button" bind:this={shell.repoPickerButton} title={shell.tr("chrome.manager")} aria-label={shell.tr("chrome.manager")} aria-expanded={shell.repoPickerOpen} aria-haspopup="dialog" onclick={() => void shell.toggleRepoPicker()}>
        <svg class="line-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
      </button>
    </div>
  </header>

  <div class="notice-stack">
  {#if shell.cloneDone}
    <div class="banner">
      <span>{shell.tr("chrome.cloneReady", { name: shell.folderName(shell.cloneDone) })}</span>
      <button class="text-button" type="button" onclick={() => { const path = shell.cloneDone; shell.cloneDone = null; if (path) void shell.openRepo(path); }}>{shell.tr("chrome.open")}</button>
    </div>
  {/if}
  {#if shell.updateNotice}
    <div class="banner">
      <span>{shell.tr("chrome.updateAvailable", { name: `AweGit ${shell.updateNotice.appVersion}` })}</span>
      {#if shell.updateNotice.notes}<span class="meta">{shell.updateNotice.notes}</span>{/if}
      {#if shell.updatePhase === "downloading" && shell.updatePercent != null}<span class="update-track" aria-hidden="true"><span style:width={`${shell.updatePercent}%`}></span></span>{/if}
      {#if shell.updatePhase === "checking" || shell.updatePhase === "downloading" || shell.updatePhase === "ready" || shell.updatePhase === "error"}<span>{shell.updateMessage}</span>{/if}
      <button class="text-button" type="button" disabled={shell.updateBusy} onclick={() => void shell.installUpdate()}>{shell.updateBusy ? shell.tr("chrome.updateDownloading") : shell.tr("chrome.updateInstall")}</button>
      <button class="text-button" type="button" disabled={shell.updateBusy} onclick={() => shell.dismissUpdate()}>{shell.tr("chrome.updateLater")}</button>
    </div>
  {:else if (shell.updatePhase === "checking" || shell.updatePhase === "current" || shell.updatePhase === "error") && shell.updateMessage}
    <div class="banner">
      <span>{shell.updateMessage}</span>
    </div>
  {/if}
  {#if shell.mode === "live" && shell.facts?.bisectActive}
    <div class="banner">
      <span>{shell.tr("chrome.bisect")}</span>
      <button class="text-button" type="button" disabled={shell.busy} onclick={() => void shell.mutate({ action: "bisect", verb: "good", rev: "" }).then(() => shell.loadFacts(true))}>{shell.tr("chrome.good")}</button>
      <button class="text-button" type="button" disabled={shell.busy} onclick={() => void shell.mutate({ action: "bisect", verb: "bad", rev: "" }).then(() => shell.loadFacts(true))}>{shell.tr("chrome.bad")}</button>
      <button class="text-button" type="button" disabled={shell.busy} onclick={() => void shell.mutate({ action: "bisect", verb: "skip", rev: "" }).then(() => shell.loadFacts(true))}>{shell.tr("chrome.skip")}</button>
      <button class="text-button" type="button" disabled={shell.busy} onclick={() => void shell.mutate({ action: "bisect", verb: "reset", rev: "" }).then(() => shell.loadFacts(true))}>{shell.tr("chrome.abort")}</button>
    </div>
  {/if}
  {#if shell.mode === "live" && shell.conflictCount > 0}
    <div class="banner unmerged">
      <span>{shell.conflictCount === 1 ? shell.tr("chrome.unmergedOne") : shell.tr("chrome.unmergedMany", { count: String(shell.conflictCount) })}</span>
      <span class="grow"></span>
      {#if shell.progress && shell.progress !== "merge"}
        <button class="text-button" type="button" disabled={shell.busy || shell.conflicted} onclick={() => { if (shell.progress === "rebase") { shell.draft = ""; shell.dialog = "continue"; } else void shell.mutate({ action: "continue" }); }}>{shell.tr("chrome.continueAction")}</button>
        <button class="text-button" type="button" disabled={shell.busy} onclick={() => shell.mutate({ action: "skip" })}>{shell.tr("chrome.skip")}</button>
      {/if}
      <button class="text-button" type="button" disabled={shell.busy} onclick={() => shell.mutate({ action: "abort" })}>{shell.tr("chrome.abort")}</button>
    </div>
  {:else if shell.mode === "live" && shell.progress}
    <div class="banner">
      <span>{shell.tr("chrome.inProgress", { name: shell.progressLabel(shell.progress) })}</span>
      <span class="grow"></span>
      <button class="text-button" type="button" disabled={shell.busy} onclick={() => shell.mutate({ action: "abort" })}>{shell.tr("chrome.abort")}</button>
      <button class="text-button" type="button" disabled={shell.busy || shell.conflicted} onclick={() => { if (shell.progress === "rebase") { shell.draft = ""; shell.dialog = "continue"; } else void shell.mutate({ action: "continue" }); }}>{shell.tr("chrome.continueAction")}</button>
      <button class="text-button" type="button" disabled={shell.busy || shell.progress === "merge"} onclick={() => shell.mutate({ action: "skip" })}>{shell.tr("chrome.skip")}</button>
    </div>
  {/if}
  {#if shell.actionError}
    <div class="banner error">
      <span>{shell.actionError}</span>
      <button class="text-button" type="button" onclick={() => shell.closeError()}>{shell.tr("chrome.close")}</button>
    </div>
  {/if}
  </div>
