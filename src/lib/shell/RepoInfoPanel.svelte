<script lang="ts">
  import { onMount } from "svelte";
  import { getShell } from "./context";
  import { invoke } from "../perf";
  import { resolveRemoteAccount } from "./remoteAccount";
  import AccountAvatar from "./AccountAvatar.svelte";
  import "./RepoInfoPanel.css";

  let { path }: { path: string } = $props();
  const shell = getShell();
  type Inspection = {
    kinds: string[]; files: number; directories: number; worktreeBytes: number;
    gitBytes: number; skipped: number; remotes: [string, string][];
  };
  type RemoteMatch = { remote: string; url: string; accountId: string; reason: string };
  let info = $state<Inspection | null>(null);
  let matches = $state<RemoteMatch[]>([]);
  let error = $state("");

  onMount(() => {
    let alive = true;
    void invoke<Inspection>("repository_inspection", { path }).then((value) => {
      if (alive) info = value;
    }).catch((reason) => { if (alive) error = shell.message(reason); });
    void invoke<RemoteMatch[]>("remote_matches", { path }).then((value) => {
      if (alive) matches = value;
    }).catch(() => {});
    return () => { alive = false; };
  });

  function size(bytes: number) {
    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
    return `${new Intl.NumberFormat(shell.activeLocale, { maximumFractionDigits: unit ? 1 : 0 }).format(value)} ${units[unit]}`;
  }

  function accountReason(reason: string) {
    switch (reason) {
      case "override": return shell.tr("dialog.remoteAccountOverride");
      case "login": return shell.tr("dialog.remoteAccountLogin");
      case "default": return shell.tr("dialog.remoteAccountDefault");
      case "only": return shell.tr("dialog.remoteAccountOnly");
      default: return "";
    }
  }

  async function editRemote(name: string, url: string) {
    if (shell.repoPath() !== path) await shell.openRepo(path);
    if (shell.repoPath() !== path) return;
    shell.remoteEdit = true;
    shell.draft = name;
    shell.draftExtra = url;
    shell.dialog = "remote";
  }
</script>

<h2>{shell.tr("repoInfo.title")}</h2>
<div class="repo-info" data-repo-info={path}>
  <div class="repo-info-fact repo-info-path"><span>{shell.tr("repoInfo.path")}</span><strong title={path}>{path}</strong></div>
  {#if info}
    <div class="repo-info-fact"><span>{shell.tr("repoInfo.kind")}</span><strong>{info.kinds.length ? info.kinds.map((kind) => shell.tr(`repoInfo.${kind}`)).join(" · ") : shell.tr("repoInfo.generic")}</strong></div>
    <div class="repo-info-stats">
      <div><span>{shell.tr("repoInfo.files")}</span><strong>{info.files.toLocaleString()}</strong></div>
      <div><span>{shell.tr("repoInfo.directories")}</span><strong>{info.directories.toLocaleString()}</strong></div>
      <div><span>{shell.tr("repoInfo.worktreeSize")}</span><strong>{size(info.worktreeBytes)}</strong></div>
      <div><span>{shell.tr("repoInfo.gitSize")}</span><strong>{size(info.gitBytes)}</strong></div>
      <div><span>{shell.tr("repoInfo.totalSize")}</span><strong>{size(info.worktreeBytes + info.gitBytes)}</strong></div>
    </div>
    <p class="repo-info-note">{shell.tr("repoInfo.scope")}{info.skipped ? ` · ${shell.tr("repoInfo.skipped", { count: String(info.skipped) })}` : ""}</p>
    <h3>{shell.tr("repoInfo.remotes")} <small>{info.remotes.length}</small></h3>
    {#if info.remotes.length}
      <div class="repo-info-remotes">
        {#each info.remotes as [name, url] (name)}
          {@const match = matches.find((item) => item.remote === name && item.url === url)}
          {@const preview = resolveRemoteAccount(shell.settings.accounts, url, match?.reason === "override" ? match.accountId : "")}
          <div class="repo-info-remote">
            <div class="repo-info-remote-head"><strong>{name}</strong><button class="text-button" type="button" title={shell.tr("repoInfo.edit")} onclick={() => void editRemote(name, url)}>{shell.tr("repoInfo.edit")}</button></div>
            <div class="repo-info-url" title={url}>{url}</div>
            <div class="repo-info-account">
              <span>{[preview.protocol, preview.host, preview.path].filter(Boolean).join(" · ")}</span>
              {#if preview.account}
                {@const account = preview.account}
                <div class="repo-info-identity">
                  <AccountAvatar url={account.avatarUrl} initial={(account.name || account.login || account.label || "?").slice(0, 1).toUpperCase()} />
                  <div class="repo-info-account-copy">
                    <strong>{account.name || account.label || account.login || account.forge}</strong>
                    {#if account.defaultForHost}<em>{shell.tr("dialog.accountsDefault")}</em>{/if}
                    <span>{[account.login ? `@${account.login}` : "", account.email].filter(Boolean).join(" · ")}</span>
                  </div>
                </div>
                <span>{accountReason(preview.reason)} · {shell.tr(preview.ssh ? "dialog.accountsSshNote" : "dialog.accountsHttpsNote")}</span>
              {:else}<span>{shell.tr("repoInfo.noAccount")}</span>{/if}
            </div>
          </div>
        {/each}
      </div>
    {:else}<p class="empty">{shell.tr("repoInfo.noRemotes")}</p>{/if}
  {:else if error}
    <p class="dialog-error" role="alert">{shell.tr("repoInfo.error", { message: error })}</p>
  {:else}
    <p class="repo-info-loading" role="status"><span class="repo-info-spinner" aria-hidden="true"></span>{shell.tr("repoInfo.loading")}</p>
  {/if}
</div>
<div class="composer-row actions"><button class="text-button" type="button" onclick={() => (shell.dialog = null)}>{shell.tr("repoInfo.close")}</button></div>
