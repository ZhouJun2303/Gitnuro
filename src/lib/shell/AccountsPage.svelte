<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { invoke } from "../perf";
  import type { Account, Settings } from "../status";
  import { getShell } from "./context";
  import "./AccountsPage.css";

  const shell = getShell();

  type ServiceInfo = { id: string; label: string; host: string; tokenUrl: string; browser: boolean; probe: string };
  type AccountProbe = { code: string; login: string; name: string; email: string; avatarUrl: string; message: string };
  type LoginResult = { ok: boolean; needsToken: boolean; tokenUrl: string; account: Account | null; message: string };
  type RemoteMatch = { remote: string; url: string; protocol: string; host: string; accountId: string; label: string; ssh: boolean; reason: string };

  const gitForges = new Set(["atomgit", "gitcode", "coding", "codeup", "cnb", "sourcehut", "generic"]);

  let services = $state<ServiceInfo[]>([]);
  let remotes = $state<RemoteMatch[]>([]);
  let noRepo = $state(false);
  let syncing = $state(false);
  let logging = $state(false);
  let notice = $state("");
  let savedNote = $state("");
  let statusOf = $state<Record<string, string>>({});
  let probeUrls = $state<Record<string, string>>({});
  let deviceOpen = $state(false);
  let deviceCode = $state("");
  let deviceUrl = $state("");
  let deviceTries = 0;
  let deviceTimer: ReturnType<typeof setInterval> | undefined;
  let draftService = $state("github");
  let draftHost = $state("github.com");
  let draftFamily = $state("github");
  let draftToken = $state("");
  let draftLogin = $state("");
  let draftProbe = $state("");
  let draftClient = $state("");
  let draftStatus = $state("");

  const selected = $derived(services.find((item) => item.id === draftService));
  const draftForge = $derived(draftService === "generic" ? draftFamily || "generic" : draftService);
  const accounts = $derived(shell.settings.accounts);

  function hostKey(forge: string, host: string) {
    const fallback = defaultHost(forge);
    let value = (host.trim() || fallback).trim().toLowerCase();
    value = value.split("/")[0]?.split("?")[0] ?? value;
    value = value.replace(/:\d+$/, "");
    if (value === "www.github.com" || value === "ssh.github.com" || value === "gist.github.com") return "github.com";
    if (value === "www.gitlab.com") return "gitlab.com";
    return value;
  }

  function defaultHost(forge: string) {
    switch (forge) {
      case "gitlab": return "gitlab.com";
      case "bitbucket": return "bitbucket.org";
      case "azure": return "dev.azure.com";
      case "gitee": return "gitee.com";
      case "atomgit": return "atomgit.com";
      case "gitcode": return "gitcode.com";
      case "codeup": return "codeup.aliyun.com";
      case "cnb": return "cnb.cool";
      case "gitea": return "codeberg.org";
      case "sourcehut": return "git.sr.ht";
      case "github": return "github.com";
      default: return "";
    }
  }

  function sameHost(left: Account, forge: string, host: string) {
    return hostKey(left.forge, left.host) === hostKey(forge, host);
  }

  function blankAccount(partial: Partial<Account> & Pick<Account, "forge" | "host" | "token">): Account {
    return {
      id: partial.id || `acct-${Date.now()}`,
      forge: partial.forge,
      label: partial.label || partial.name || partial.login || partial.forge,
      token: partial.token,
      host: partial.host,
      clientId: partial.clientId || "",
      login: partial.login || "",
      name: partial.name || "",
      email: partial.email || "",
      avatarUrl: partial.avatarUrl || "",
      defaultForHost: partial.defaultForHost ?? false,
    };
  }

  function upsert(account: Account) {
    const key = hostKey(account.forge, account.host);
    let next = shell.settings.accounts.filter((item) => item.id !== account.id);
    const hasDefault = next.some((item) => hostKey(item.forge, item.host) === key && item.defaultForHost);
    const row = { ...account, defaultForHost: account.defaultForHost || !hasDefault };
    if (row.defaultForHost) {
      next = next.map((item) => (hostKey(item.forge, item.host) === key ? { ...item, defaultForHost: false } : item));
    }
    shell.settings = { ...shell.settings, accounts: [...next, row] };
  }

  function statusText(code: string) {
    switch (code) {
      case "ok": return shell.tr("dialog.accountsConnected");
      case "unauthorized": return shell.tr("dialog.accountsUnauthorized");
      case "network": return shell.tr("dialog.accountsNetwork");
      case "host": return shell.tr("dialog.accountsHostMismatch");
      case "missing": return shell.tr("dialog.accountsMissing");
      case "need-repo": return shell.tr("dialog.accountsNeedRepo");
      case "testing": return shell.tr("dialog.accountsTesting");
      case "git": return shell.tr("dialog.accountsFailed");
      default: return shell.tr("dialog.accountsStatusUnknown");
    }
  }

  function initial(account: Account) {
    const source = (account.name || account.login || account.label || account.forge).trim();
    return source.slice(0, 1).toUpperCase() || "?";
  }

  function applyProbe(account: Account, probe: AccountProbe) {
    if (probe.code !== "ok") return account;
    const name = probe.name || account.name;
    return {
      ...account,
      login: probe.login || account.login,
      name,
      email: probe.email || account.email,
      avatarUrl: probe.avatarUrl || account.avatarUrl,
      label: account.label || name || probe.login || account.forge,
    };
  }

  async function loadServices() {
    services = await invoke<ServiceInfo[]>("account_services").catch(() => []);
    if (!services.some((item) => item.id === draftService) && services[0]) {
      draftService = services[0].id;
      draftHost = services[0].host;
    }
  }

  async function loadRemotes() {
    const path = shell.repoPath();
    if (!path) {
      noRepo = true;
      remotes = [];
      return;
    }
    noRepo = false;
    remotes = await invoke<RemoteMatch[]>("remote_matches", { path }).catch(() => []);
  }

  async function syncOnce() {
    if (syncing) return;
    syncing = true;
    notice = "";
    try {
      const saved = await invoke<Settings>("sync_accounts");
      shell.settings = {
        ...shell.settings,
        accounts: saved.accounts,
        githubToken: saved.githubToken,
        gitlabToken: saved.gitlabToken,
        bitbucketToken: saved.bitbucketToken,
        azureToken: saved.azureToken,
      };
    } catch (error) {
      shell.actionError = shell.message(error);
    } finally {
      syncing = false;
    }
  }

  function fields(account: Account, probeUrl = "") {
    return {
      id: account.id,
      forgeName: account.forge,
      label: account.label,
      token: account.token,
      host: account.host,
      clientId: account.clientId,
      login: account.login,
      name: account.name,
      email: account.email,
      avatarUrl: account.avatarUrl,
      defaultForHost: account.defaultForHost,
      probeUrl,
    };
  }

  async function runProbe(account: Account, probeUrl = "") {
    statusOf = { ...statusOf, [account.id]: "testing" };
    try {
      const probe = await invoke<AccountProbe>("test_account", fields(account, probeUrl));
      statusOf = { ...statusOf, [account.id]: probe.code || "git" };
      return probe;
    } catch (error) {
      statusOf = { ...statusOf, [account.id]: "network" };
      shell.actionError = shell.message(error);
      return { code: "network", login: "", name: "", email: "", avatarUrl: "", message: "" } satisfies AccountProbe;
    }
  }

  async function testCard(account: Account) {
    const probe = await runProbe(account, probeUrls[account.id] ?? "");
    if (probe.code !== "ok") return;
    const next = applyProbe(account, probe);
    shell.settings = {
      ...shell.settings,
      accounts: shell.settings.accounts.map((item) => (item.id === account.id ? next : item)),
    };
    await shell.savePrefs(false);
  }

  async function startLogin(forge: string, host: string) {
    if (logging) return;
    logging = true;
    notice = shell.tr("dialog.accountsBrowserWait");
    try {
      const result = await invoke<LoginResult>("browser_login", { forgeName: forge, host });
      if (result.message === "cancelled") {
        notice = "";
        return;
      }
      if (result.ok && result.account) {
        upsert(blankAccount(result.account));
        statusOf = { ...statusOf, [result.account.id]: "ok" };
        await shell.savePrefs(false);
        notice = shell.tr("dialog.accountsSaved");
        return;
      }
      draftService = forge || draftService;
      if (host) draftHost = host;
      notice = shell.tr("dialog.accountsBrowserMissing");
      if (result.tokenUrl) {
        const match = services.find((item) => item.id === forge);
        if (match) match.tokenUrl = result.tokenUrl;
      }
    } catch (error) {
      notice = shell.tr("dialog.accountsBrowserMissing");
      shell.actionError = shell.message(error);
    } finally {
      logging = false;
    }
  }

  async function cancelLogin() {
    await invoke("cancel_browser_login").catch(() => {});
    logging = false;
    notice = "";
  }

  function chooseService(service: ServiceInfo) {
    draftService = service.id;
    draftHost = service.host;
    draftFamily = service.id === "generic" ? "github" : service.id;
    draftStatus = "";
    if (service.id === "codeup") {
      void openAccount("https://account.aliyun.com/");
      return;
    }
    if (service.browser && service.host) void startLogin(service.id, service.host);
  }

  function onService(id: string) {
    draftService = id;
    const service = services.find((item) => item.id === id);
    draftHost = service?.host ?? "";
    draftFamily = id === "generic" ? "github" : id;
    draftStatus = "";
  }

  async function openAccount(url: string) {
    if (draftService === "codeup" || draftForge === "codeup") notice = shell.tr("dialog.accountsAliyunPaste");
    if (url) await openUrl(url).catch((error) => (shell.actionError = shell.message(error)));
  }

  async function openTokenPage() {
    const service = services.find((item) => item.id === draftService);
    const url = draftForge === "codeup" ? "https://account.aliyun.com/" : service?.tokenUrl || "";
    await openAccount(url);
  }

  function draftAccount() {
    return blankAccount({
      forge: draftForge,
      host: draftHost.trim(),
      token: draftToken,
      clientId: draftClient.trim(),
      login: draftLogin.trim(),
      label: draftLogin.trim() || draftForge,
    });
  }

  async function testDraft() {
    const account = draftAccount();
    const probe = await runProbe(account, draftProbe.trim());
    draftStatus = probe.code;
    if (probe.code === "ok") {
      const next = applyProbe(account, probe);
      draftToken = next.token;
      return next;
    }
    return null;
  }

  async function saveDraft() {
    const tested = await testDraft();
    if (!tested) return;
    upsert(tested);
    draftToken = "";
    draftStatus = "ok";
    savedNote = shell.tr("dialog.accountsSaved");
    await shell.savePrefs(false);
  }

  function makeDefault(account: Account) {
    const key = hostKey(account.forge, account.host);
    shell.settings = {
      ...shell.settings,
      accounts: shell.settings.accounts.map((item) => ({ ...item, defaultForHost: hostKey(item.forge, item.host) === key && item.id === account.id })),
    };
    void shell.savePrefs(false);
  }

  function useAuthor(account: Account) {
    const name = account.name || account.login;
    if (!name && !account.email) return;
    shell.settings = { ...shell.settings, authorName: name || shell.settings.authorName, authorEmail: account.email || shell.settings.authorEmail };
    void shell.savePrefs(false);
  }

  function signOut(account: Account) {
    const subject = `${account.name || account.login || account.label} · ${account.host || account.forge}`;
    shell.ask(shell.tr("menu.delete"), shell.tr("dialog.deleteAccountBody"), () => {
      shell.settings = { ...shell.settings, accounts: shell.settings.accounts.filter((item) => item.id !== account.id) };
      void shell.savePrefs(false);
    }, [], { subject, danger: true });
  }

  async function pickRemote(row: RemoteMatch, accountId: string) {
    const path = shell.repoPath();
    if (!path) return;
    await invoke("save_remote_account", { path, remote: row.remote, accountId }).catch((error) => (shell.actionError = shell.message(error)));
    await loadRemotes();
  }

  function accountsFor(host: string) {
    return accounts.filter((item) => hostKey(item.forge, item.host) === hostKey("", host) || hostKey(item.forge, item.host) === host);
  }

  function stopDevice() {
    if (deviceTimer) clearInterval(deviceTimer);
    deviceTimer = undefined;
  }

  async function pollDevice() {
    deviceTries += 1;
    if (deviceTries > 20) {
      stopDevice();
      return;
    }
    const token = await invoke<string>("device_poll").catch(() => "pending");
    if (token === "pending" || !token) return;
    stopDevice();
    draftToken = token;
    await saveDraft();
  }

  async function startDevice() {
    deviceCode = "";
    deviceUrl = "";
    try {
      const value = await invoke<{ verificationUri: string; userCode: string }>("device_start", {
        forgeName: draftForge,
        clientId: draftClient.trim() || shell.settings.oauthClientId,
        host: draftHost.trim(),
      });
      deviceCode = value.userCode;
      deviceUrl = value.verificationUri;
      if (value.verificationUri) void openUrl(value.verificationUri);
      deviceTries = 0;
      stopDevice();
      deviceTimer = setInterval(() => void pollDevice(), 3000);
    } catch (error) {
      shell.actionError = shell.message(error);
    }
  }

  onMount(() => {
    void loadServices();
    void loadRemotes();
    return () => stopDevice();
  });
</script>

<section class="pref-card">
  <h3 class="pref-card-title">{shell.tr("dialog.accountsPage")}</h3>
  <div class="pref-card-body">
    <p class="account-intro">{shell.tr("dialog.accountsIntro")}</p>
    <div class="account-sync-line">
      {#if syncing}<span>{shell.tr("dialog.accountsSyncing")}</span>{:else if notice}<span>{notice}</span>{/if}
      <span class="grow"></span>
      <button class="text-button" type="button" disabled={syncing} onclick={() => void syncOnce()}>{shell.tr("dialog.accountsSync")}</button>
      {#if logging}<button class="text-button" type="button" onclick={() => void cancelLogin()}>{shell.tr("dialog.accountsCancelLogin")}</button>{/if}
    </div>
    {#if accounts.length === 0}
      <p class="empty">{shell.tr("dialog.accountsEmpty")}</p>
      <div class="service-grid">
        {#each services as service (service.id)}
          <button class="service-tile" type="button" onclick={() => chooseService(service)}>
            <span class="service-name">{service.label}</span>
            {#if service.host}<span class="service-host">{service.host}</span>{/if}
            <span class="service-action">{service.id === "codeup" ? shell.tr("dialog.accountsAliyunOpen") : service.browser ? shell.tr("dialog.accountsBrowser") : shell.tr("dialog.accountsPaste")}</span>
          </button>
        {/each}
      </div>
    {:else}
      <div class="account-list">
        {#each accounts as account (account.id)}
          {@const code = statusOf[account.id] || "unknown"}
          <article class="account-card" data-account-card={account.id} data-status={code}>
            <div class="account-identity">
              {#if account.avatarUrl}
                <img class="account-avatar" alt="" src={account.avatarUrl} />
              {:else}
                <span class="account-avatar" aria-hidden="true">{initial(account)}</span>
              {/if}
              <div class="account-copy">
                <div class="account-name">
                  <span>{account.name || account.label || account.login || account.forge}</span>
                  {#if account.defaultForHost}<em class="account-badge">{shell.tr("dialog.accountsDefault")}</em>{/if}
                </div>
                <div class="account-meta">{account.login ? `@${account.login}` : shell.tr("dialog.accountsLogin")} · {account.host || defaultHost(account.forge) || account.forge}</div>
              </div>
              <span class="account-status" data-status={code}>{statusText(code)}</span>
            </div>
            {#if gitForges.has(account.forge)}
              <label class="pref-row wide">
                <span class="pref-label">{shell.tr("dialog.accountsProbeUrl")}</span>
                <span class="pref-control"><input value={probeUrls[account.id] ?? ""} oninput={(event) => (probeUrls = { ...probeUrls, [account.id]: (event.currentTarget as HTMLInputElement).value })} /></span>
              </label>
            {/if}
            <div class="account-actions">
              <button class="text-button" type="button" data-account-test={account.id} onclick={() => void testCard(account)}>{code === "testing" ? shell.tr("dialog.accountsTesting") : shell.tr("dialog.accountsTest")}</button>
              <button class="text-button" type="button" onclick={() => void testCard(account)}>{shell.tr("dialog.accountsResync")}</button>
              {#if !account.defaultForHost}<button class="text-button" type="button" onclick={() => makeDefault(account)}>{shell.tr("dialog.accountsMakeDefault")}</button>{/if}
              <button class="text-button" type="button" onclick={() => useAuthor(account)}>{shell.tr("dialog.accountsUseAuthor")}</button>
              <button class="text-button danger" type="button" onclick={() => signOut(account)}>{shell.tr("dialog.accountsSignOut")}</button>
            </div>
          </article>
        {/each}
      </div>
    {/if}
  </div>
</section>

<section class="pref-card">
  <h3 class="pref-card-title">{shell.tr("dialog.accountsAdd")}</h3>
  <div class="pref-card-body">
    <label class="pref-row">
      <span class="pref-label">{shell.tr("dialog.accountsService")}</span>
      <span class="pref-control">
        <select aria-label={shell.tr("dialog.accountsService")} value={draftService} onchange={(event) => onService((event.currentTarget as HTMLSelectElement).value)}>
          {#each services as service (service.id)}
            <option value={service.id}>{service.label}</option>
          {/each}
        </select>
      </span>
    </label>
    <label class="pref-row wide">
      <span class="pref-label">{shell.tr("dialog.accountsHost")}</span>
      <span class="pref-control"><input value={draftHost} oninput={(event) => (draftHost = (event.currentTarget as HTMLInputElement).value)} /></span>
    </label>
    {#if draftService === "generic"}
      <label class="pref-row">
        <span class="pref-label">{shell.tr("dialog.accountsFamily")}</span>
        <span class="pref-control">
          <select aria-label={shell.tr("dialog.accountsFamily")} bind:value={draftFamily}>
            <option value="github">GitHub</option>
            <option value="gitlab">GitLab</option>
            <option value="gitea">Gitea</option>
            <option value="generic">{shell.tr("dialog.accountsFamilyGeneric")}</option>
          </select>
        </span>
      </label>
    {/if}
    {#if gitForges.has(draftForge)}
      <label class="pref-row wide">
        <span class="pref-label">{shell.tr("dialog.accountsProbeUrl")}</span>
        <span class="pref-control"><input placeholder="https://host/org/repo.git" bind:value={draftProbe} /></span>
      </label>
    {/if}
    {#if draftForge === "codeup"}
      <label class="pref-row wide">
        <span class="pref-label">{shell.tr("dialog.accountsCloneUser")}</span>
        <span class="pref-control"><input autocomplete="off" bind:value={draftLogin} /></span>
      </label>
    {/if}
    <label class="pref-row wide">
      <span class="pref-label">{shell.tr("dialog.accountsToken")}</span>
      <span class="pref-control"><input type="password" autocomplete="off" placeholder={shell.tr("dialog.accountsPaste")} bind:value={draftToken} /></span>
    </label>
    {#if draftStatus}<p class="account-note">{statusText(draftStatus)}</p>{/if}
    {#if savedNote}<p class="account-note">{savedNote}</p>{/if}
    <div class="pref-card-actions">
      <button class="text-button" type="button" disabled={logging || !(selected?.browser)} onclick={() => void startLogin(draftForge, draftHost.trim())}>{logging ? shell.tr("dialog.accountsBrowserWait") : shell.tr("dialog.accountsBrowser")}</button>
      <button class="text-button" type="button" onclick={() => void openTokenPage()}>{draftForge === "codeup" ? shell.tr("dialog.accountsAliyunOpen") : shell.tr("dialog.accountsCreateToken")}</button>
      <button class="text-button" type="button" onclick={() => void testDraft()}>{shell.tr("dialog.accountsTest")}</button>
      <button class="text-button" type="button" disabled={!draftToken.trim()} onclick={() => void saveDraft()}>{shell.tr("dialog.accountsSave")}</button>
    </div>
    {#if draftForge === "github" || draftForge === "gitlab"}
      <div class="account-device">
        <button class="text-button" type="button" onclick={() => (deviceOpen = !deviceOpen)}>{shell.tr("dialog.accountsDevice")}</button>
        {#if deviceOpen}
          <label class="pref-row wide">
            <span class="pref-label">{shell.tr("dialog.clientId")}</span>
            <span class="pref-control"><input bind:value={draftClient} /></span>
          </label>
          <div class="pref-card-actions">
            <button class="text-button" type="button" disabled={!draftClient.trim() && !shell.settings.oauthClientId.trim()} onclick={() => void startDevice()}>{shell.tr("dialog.deviceLogin")}</button>
            <button class="text-button" type="button" onclick={() => void pollDevice()}>{shell.tr("dialog.accountsLoginWait")}</button>
          </div>
          {#if deviceCode}<p class="account-code">{deviceCode}</p>{/if}
          {#if deviceUrl}<p class="account-note">{deviceUrl}</p>{/if}
        {/if}
      </div>
    {/if}
  </div>
</section>

<section class="pref-card">
  <h3 class="pref-card-title">{shell.tr("dialog.accountsRemotes")}</h3>
  <div class="pref-card-body">
    {#if noRepo}
      <p class="empty">{shell.tr("dialog.accountsNoRepo")}</p>
    {:else if remotes.length === 0}
      <p class="empty">{shell.tr("dialog.accountsNoRemote")}</p>
    {:else}
      <div class="remote-list">
        {#each remotes as row (row.remote)}
          <div class="remote-row" data-remote-match={row.remote}>
            <span class="remote-name">{row.remote}</span>
            <span class="remote-url" title={row.url}>{row.protocol} · {row.host || row.url}</span>
            <select aria-label={shell.tr("dialog.accountsPick")} value={row.accountId} onchange={(event) => void pickRemote(row, (event.currentTarget as HTMLSelectElement).value)}>
              <option value="">{shell.tr("dialog.accountsMatchNone")}</option>
              {#each accountsFor(row.host) as account (account.id)}
                <option value={account.id}>{account.name || account.login || account.label}</option>
              {/each}
            </select>
          </div>
          <p class="account-note">{row.ssh ? shell.tr("dialog.accountsSshNote") : shell.tr("dialog.accountsHttpsNote")}</p>
        {/each}
      </div>
    {/if}
  </div>
</section>

<section class="pref-card">
  <h3 class="pref-card-title">{shell.tr("dialog.prefSecAuthor")}</h3>
  <div class="pref-card-body">
    <label class="pref-row wide">
      <span class="pref-label">{shell.tr("dialog.authorName")}</span>
      <span class="pref-control"><input bind:value={shell.settings.authorName} placeholder={shell.tr("dialog.authorPlaceholder")} /></span>
    </label>
    <label class="pref-row wide">
      <span class="pref-label">{shell.tr("dialog.authorEmail")}</span>
      <span class="pref-control"><input bind:value={shell.settings.authorEmail} placeholder={shell.tr("dialog.authorPlaceholder")} /></span>
    </label>
    <label class="pref-row pref-toggle">
      <input type="checkbox" bind:checked={shell.settings.gravatar} />
      <span class="pref-label">{shell.tr("dialog.gravatar")}</span>
    </label>
    <div class="pref-card-actions">
      <button class="text-button" type="button" onclick={() => invoke("set_repo_author", { path: shell.repoPath(), name: shell.settings.authorName, email: shell.settings.authorEmail }).catch((error) => (shell.actionError = shell.message(error)))}>{shell.tr("dialog.saveAuthorRepo")}</button>
    </div>
    <p class="empty">{shell.tr("dialog.repoLocalHint")}</p>
  </div>
</section>

<section class="pref-card">
  <h3 class="pref-card-title">{shell.tr("dialog.prefSecCredentials")}</h3>
  <div class="pref-card-body">
    <label class="pref-row wide">
      <span class="pref-label">{shell.tr("dialog.signingPassphrase")}</span>
      <span class="pref-control"><input type="password" bind:value={shell.passphrase} /></span>
    </label>
    <div class="pref-card-actions">
      <button class="text-button" type="button" onclick={() => shell.keepPassphrase()}>{shell.tr("dialog.usePassphrase")}</button>
    </div>
    <label class="pref-row wide">
      <span class="pref-label">{shell.tr("dialog.askpassUser")}</span>
      <span class="pref-control"><input bind:value={shell.passUser} /></span>
    </label>
    <div class="pref-card-actions">
      <button class="text-button" type="button" onclick={() => { shell.draft = "https"; shell.draftExtra = ""; shell.draftUser = ""; shell.draftSecret = ""; shell.dialog = "credential"; }}>{shell.tr("dialog.httpsLogin")}</button>
    </div>
  </div>
</section>

<section class="pref-card">
  <h3 class="pref-card-title">{shell.tr("dialog.prefSecKeys")}</h3>
  <div class="pref-card-body">
    <div class="pref-card-actions">
      <button class="text-button" type="button" onclick={() => void invoke<string>("read_ssh_config").then((text) => { shell.draft = text; shell.dialog = "ssh"; }).catch((error) => (shell.actionError = shell.message(error)))}>{shell.tr("chrome.sshConfig")}</button>
      <button class="text-button" type="button" onclick={() => void invoke<string>("generate_ssh_key").then((key) => { shell.commandOutput = key; shell.dialog = "output"; }).catch((error) => (shell.actionError = shell.message(error)))}>{shell.tr("dialog.sshKey")}</button>
    </div>
  </div>
</section>
