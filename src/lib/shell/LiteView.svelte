<script lang="ts">
  import { getShell } from "./context";
  import { shortcutLabel } from "../keys";
  import { terminalCliNames } from "../status";
  import "./LiteView.css";

  const shell = getShell();
  const liteShortcut = $derived.by(() => { shell.settings.shortcuts; return shortcutLabel("toggleLite"); });
  const repoName = $derived(shell.snapshot ? shell.folderName(shell.snapshot.path) : shell.tr("chrome.welcomeTitle"));
  const branch = $derived(shell.branchLabel);
  const current = $derived(shell.currentBranch);
  const selectedCli = $derived(shell.settings.liteAgent || shell.liteAgentOptions[0] || "");
  const status = $derived.by(() => {
    const task = shell.liteTask;
    if (shell.liteWorking) return task?.detail || shell.tr("chrome.working");
    if (task?.state === "failed") return task.detail || shell.tr("dialog.liteAgentFailed");
    if (task?.state === "completed") return task.detail || shell.tr("dialog.liteDone");
    return shell.tr("dialog.liteReady");
  });

  function chooseCli(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value;
    shell.settings = { ...shell.settings, liteAgent: value };
    void shell.savePrefs(false);
  }

  function branchMenu(event: MouseEvent) {
    shell.openBar(event, shell.branchItems());
  }

  $effect(() => {
    void shell.fitLiteWindow();
    return () => { void shell.restoreLiteWindow(); };
  });
</script>

<main class="lite-shell" data-layer="lite" aria-label={shell.tr("dialog.liteTitle")}>
  <header class="lite-header">
    <button class="lite-menu" type="button" aria-label={shell.tr("chrome.view")} title={shell.tr("chrome.view")} onclick={(event) => shell.openBar(event, shell.viewItems())}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 7h14M5 12h14M5 17h14" /></svg>
    </button>
    <div class="lite-identity">
      <strong title={shell.snapshot?.path ?? ""}>{repoName}</strong>
      <span>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 3v12a3 3 0 1 0 3 3" /><circle cx="6" cy="18" r="2.2" /><circle cx="18" cy="6" r="2.2" /><path d="M18 8.2A6 6 0 0 1 9 12" /></svg>
        <em title={branch}>{branch}</em>
      </span>
    </div>
    <div class="lite-header-actions">
      <span class="lite-badge">Lite</span>
      <button class="lite-branch" type="button" aria-haspopup="menu" onclick={branchMenu}>
        <span>{shell.tr("dialog.branch")}</span>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="m7 10 5 5 5-5" /></svg>
      </button>
    </div>
  </header>

  <section class="lite-status" aria-live="polite">
    <div class="lite-status-row">
      <span class="lite-state-dot" class:working={shell.liteWorking} class:failed={shell.liteTask?.state === "failed"}></span>
      <span>{status}</span>
      <span class="lite-spacer"></span>
      <span class="lite-count">{shell.changeCount > 0 ? shell.tr("dialog.liteChanges", { count: String(shell.changeCount) }) : shell.tr("dialog.liteNoChanges")}</span>
      {#if current}<span class="lite-count">{shell.tr("dialog.liteAheadBehind", { ahead: String(current.ahead), behind: String(current.behind) })}</span>{/if}
    </div>
    {#if shell.liteTask}
      <div class="lite-progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={shell.liteTask.percent} aria-valuetext={`${shell.liteTask.detail} · ${shell.liteTask.percent}%`}>
        <span style:width={`${shell.liteTask.percent}%`}></span>
      </div>
      {#if shell.liteAgentOutput}<pre class="lite-output">{shell.liteAgentOutput}</pre>{/if}
    {/if}
  </section>

  <section class="lite-actions" aria-label={shell.tr("dialog.liteTitle")}>
    <button class="lite-action primary" type="button" disabled={shell.liteWorking} onclick={() => void shell.runLiteSync()}>
      <span class="lite-action-icon" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M4 12a8 8 0 0 1 13.4-5.9L20 8.7M20 4v4.7h-4.7M20 12a8 8 0 0 1-13.4 5.9L4 15.3M4 20v-4.7h4.7" /></svg></span>
      <span><strong>{shell.tr("dialog.liteSync")}</strong><small>{shell.tr("dialog.liteStage")} · {shell.tr("dialog.litePull")} · {shell.tr("dialog.litePush")}</small></span>
    </button>
    <button class="lite-action" type="button" disabled={shell.liteWorking} onclick={() => void shell.runLiteAgent()}>
      <span class="lite-action-icon" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M12 3 13.7 9.3 20 11l-6.3 1.7L12 19l-1.7-6.3L4 11l6.3-1.7z" /><path d="m18.5 16 .6 2.1 2.1.6-2.1.6-.6 2.1-.6-2.1-2.1-.6 2.1-.6z" /></svg></span>
      <span><strong>{shell.tr("dialog.liteAgent")}</strong><small>{selectedCli || shell.tr("dialog.liteNoCli")}</small></span>
    </button>
  </section>

  <footer class="lite-footer">
    <label class="lite-cli">
      <span>{shell.tr("dialog.liteSelectCli")}</span>
      <select value={selectedCli} onchange={chooseCli} disabled={shell.liteWorking}>
        {#each terminalCliNames as cli (cli)}
          <option value={cli}>{cli}{shell.liteCliReady(cli) ? " · ✓" : ""}</option>
        {/each}
      </select>
    </label>
    <span class="lite-spacer"></span>
    <button class="lite-normal" type="button" onclick={() => shell.setLiteMode(false)} title={`${shell.tr("dialog.liteSwitchNormal")} (${liteShortcut})`}>{shell.tr("dialog.liteSwitchNormal")}</button>
  </footer>
</main>
