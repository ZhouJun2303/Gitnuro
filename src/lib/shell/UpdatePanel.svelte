<script lang="ts">
  import { getShell } from "./context";
  import "./UpdatePanel.css";
  import { openUrl } from "@tauri-apps/plugin-opener";

  const shell = getShell();
  let copied = $state(false);
  let copiedTimer = 0;
  const notice = $derived(shell.updateNotice);
  const phase = $derived(shell.updatePhase);
  const busy = $derived(shell.updateBusy);
  const releaseUrl = $derived(notice?.releaseUrl || shell.updateReleaseUrl);
  const working = $derived(phase === "downloading" || phase === "verifying");
  const ringLength = 2 * Math.PI * 16;
  const ringOffset = $derived(ringLength * (1 - Math.min(100, Math.max(0, shell.updatePercent ?? 0)) / 100));
  const checked = $derived(
    shell.updateCheckedAt
      ? new Date(shell.updateCheckedAt).toLocaleTimeString(shell.activeLocale === "zh" ? "zh-CN" : "en-GB", { hour: "2-digit", minute: "2-digit", hour12: false })
      : "",
  );

  function formatBytes(value: number) {
    if (!Number.isFinite(value) || value <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB"];
    let size = value;
    let index = 0;
    while (size >= 1024 && index < units.length - 1) {
      size /= 1024;
      index += 1;
    }
    return `${size >= 100 || index === 0 ? Math.round(size) : size.toFixed(1)} ${units[index]}`;
  }

  function openRelease(url: string | undefined) {
    if (url) void openUrl(url);
  }

  function copyDownload(url: string) {
    shell.copyText(url);
    copied = true;
    window.clearTimeout(copiedTimer);
    copiedTimer = window.setTimeout(() => {
      copied = false;
    }, 1500);
  }
</script>

<h2>{shell.tr("chrome.updateTitle")}</h2>
<div class="update-panel">
  <div class="update-identity">
    <span class="update-icon" class:working={working} aria-hidden="true">
      {#if working}
        <svg class="update-ring" viewBox="0 0 36 36">
          <circle class="update-ring-track" cx="18" cy="18" r="16"></circle>
          <circle class="update-ring-value" cx="18" cy="18" r="16" style:stroke-dasharray={ringLength} style:stroke-dashoffset={ringOffset}></circle>
        </svg>
      {/if}
      <svg viewBox="0 0 24 24" class="line-icon"><path d="M12 3v11M8 10l4 4 4-4"/><path d="M4 16v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3"/></svg>
    </span>
    <div class="update-identity-copy">
      <strong>AweGit {shell.appVersion}</strong>
      <span class="meta">{checked ? shell.tr("chrome.updateCheckedAt", { time: checked }) : shell.tr("chrome.updateHint")}</span>
    </div>
    <button class="text-button update-check" type="button" disabled={busy || phase === "checking"} onclick={() => void shell.checkForUpdate()}>
      {phase === "checking" ? shell.tr("chrome.updateChecking") : notice || phase === "current" ? shell.tr("chrome.updateCheckAgain") : shell.tr("chrome.checkUpdate")}
    </button>
  </div>

  {#if phase === "checking" && !notice}
    <div class="update-waiting" role="status">
      <span class="update-spinner" aria-hidden="true"></span>
      <span>{shell.tr("chrome.updateChecking")}</span>
    </div>
  {:else if phase === "current" && !notice}
    <div class="update-result ok" role="status">
      <span class="update-dot" aria-hidden="true"></span>
      <span>{shell.tr("chrome.updateCurrent", { version: shell.appVersion })}</span>
    </div>
  {:else if phase === "error" && !notice}
    <p class="dialog-error" role="alert">{shell.updateMessage}</p>
    <div class="composer-row">
      <button class="commit" type="button" onclick={() => void shell.checkForUpdate()}>{shell.tr("chrome.updateRetry")}</button>
    </div>
  {:else if notice}
    <section class="update-release">
      <header class="update-release-head">
        <span class="update-version-chip">v{notice.appVersion}</span>
        <strong>{shell.tr("chrome.updateAvailable", { name: `AweGit ${notice.appVersion}` })}</strong>
      </header>
      {#if notice.notes}
        <div class="update-notes-label">{shell.tr("chrome.updateReleaseNotes")}</div>
        <div class="update-notes">{notice.notes}</div>
      {/if}
      {#if notice.fileName}<p class="meta update-file">{shell.tr("chrome.updateFileName", { name: notice.fileName })}</p>{/if}
    </section>

    {#if phase === "downloading"}
      <div class="update-progress">
        <div class="update-progress-head">
          <span>{shell.tr("chrome.updateDownloading")}</span>
          {#if shell.updatePercent != null}<span class="update-percent">{shell.updatePercent}<small>%</small></span>{/if}
        </div>
        <div
          class="update-track"
          class:indeterminate={shell.updatePercent == null}
          role="progressbar"
          aria-valuemin="0"
          aria-valuemax="100"
          aria-valuenow={shell.updatePercent ?? undefined}
          aria-valuetext={shell.updatePercent == null ? shell.tr("chrome.updateDownloading") : `${shell.updatePercent}%`}
        >
          <span style:width={shell.updatePercent == null ? "36%" : `${shell.updatePercent}%`}></span>
        </div>
        <div class="update-progress-meta">
          {#if shell.updateTotal > 0}
            <span>{formatBytes(shell.updateReceived)} / {formatBytes(shell.updateTotal)}</span>
          {:else if shell.updateReceived > 0}
            <span>{formatBytes(shell.updateReceived)}</span>
          {/if}
          {#if shell.updateSpeed != null && shell.updateSpeed > 0}<span>{formatBytes(shell.updateSpeed)}/s</span>{/if}
        </div>
      </div>
    {:else if phase === "verifying"}
      <div class="update-waiting" role="status">
        <span class="update-spinner" aria-hidden="true"></span>
        <span>{shell.tr("chrome.updateVerifying")}</span>
      </div>
    {:else if phase === "ready"}
      <div class="update-result ok" role="status">
        <span class="update-dot" aria-hidden="true"></span>
        <span>{shell.tr("chrome.updateStarted")}</span>
      </div>
    {:else if phase === "cancelled"}
      <p class="empty update-note">{shell.tr("chrome.updateCancelled")}</p>
    {:else if phase === "error"}
      <p class="dialog-error" role="alert">{shell.updateMessage}</p>
    {/if}

    {#if !notice.installable}
      <p class="empty update-note">{shell.tr("chrome.updateNoInstaller")}</p>
    {/if}

    <div class="composer-row update-actions">
      {#if phase === "downloading"}
        <button class="text-button" type="button" disabled={shell.updateCancelling} onclick={() => shell.cancelUpdate()}>
          {shell.updateCancelling ? shell.tr("chrome.updateCancelling") : shell.tr("chrome.updateCancel")}
        </button>
      {:else if notice.installable && (phase === "available" || phase === "cancelled" || phase === "error")}
        <button class="commit" type="button" disabled={busy} onclick={() => void shell.installUpdate()}>
          {phase === "available" ? shell.tr("chrome.updateInstall") : shell.tr("chrome.updateRetry")}
        </button>
      {:else if !notice.installable}
        <button class="commit" type="button" onclick={() => openRelease(notice.downloadUrl || releaseUrl)}>{shell.tr("chrome.updateOpenRelease")}</button>
      {/if}
      {#if notice.installable && releaseUrl}
        <button class="text-button" type="button" onclick={() => openRelease(releaseUrl)}>{shell.tr("chrome.updateOpenRelease")}</button>
      {/if}
      {#if notice.downloadUrl}
        <button class="text-button" type="button" onclick={() => copyDownload(notice.downloadUrl)}>{copied ? shell.tr("chrome.copied") : shell.tr("chrome.updateCopyDownload")}</button>
      {/if}
      <span class="grow"></span>
      {#if phase === "ready"}
        <button class="text-button" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("chrome.close")}</button>
      {:else if phase !== "downloading" && phase !== "verifying"}
        <button class="text-button" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("chrome.updateLater")}</button>
        <button class="text-button" type="button" disabled={busy} onclick={() => shell.dismissUpdate()}>{shell.tr("chrome.updateSkip")}</button>
      {/if}
    </div>
  {:else}
    <p class="empty update-note">{shell.tr("chrome.updatePanelIdle")}</p>
    <div class="composer-row">
      <button class="commit" type="button" onclick={() => void shell.checkForUpdate()}>{shell.tr("chrome.checkUpdate")}</button>
    </div>
  {/if}

  {#if !notice && releaseUrl}
    <div class="update-reference">
      <button class="text-button" type="button" onclick={() => openRelease(releaseUrl)}>{shell.tr("chrome.updateOpenRelease")}</button>
      <span class="meta update-url" title={releaseUrl}>{releaseUrl}</span>
    </div>
  {/if}
</div>
