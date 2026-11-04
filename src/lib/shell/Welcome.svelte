<script lang="ts">
  import { getShell } from "./context";
  import "./Welcome.css";
  import type { SearchKey } from "./types";

  const shell = getShell();
  const focusOnMount = (node: HTMLInputElement) => shell.focusOnMount(node);
</script>

<div class="welcome-view">
      <div class="welcome-card">
        <div class="welcome-brand">
          <svg class="welcome-logo" viewBox="0 0 32 32" fill="none">
            <circle cx="16" cy="16" r="14" stroke="var(--accent)" stroke-width="2.5" />
            <circle cx="11" cy="11" r="3" fill="var(--accent)" />
            <circle cx="21" cy="14" r="3" fill="var(--accent)" />
            <circle cx="11" cy="21" r="3" fill="var(--accent)" />
            <path d="M11 11v10M11 16c4 0 10-2 10-2" stroke="var(--accent)" stroke-width="2" stroke-linecap="round" />
          </svg>
          <div class="welcome-titles">
            <h1>AweGit</h1>
            <p>{shell.tr("chrome.welcomeTagline")}</p>
          </div>
        </div>

        {#if shell.loadError}
          <div class="welcome-error" role="alert">
            <svg class="welcome-error-icon" viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M8 1.4 14.8 13.4H1.2L8 1.4z"/><path d="M8 6.1v3.3" stroke="var(--on-deleted)" stroke-width="1.4" stroke-linecap="round"/><circle cx="8" cy="11.4" r=".75" fill="var(--on-deleted)"/></svg>
            <span>{shell.loadError}</span>
          </div>
        {/if}

        <div class="welcome-actions">
          <button class="welcome-btn" type="button" onclick={() => shell.beginOpen()}>
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M2 4a1 1 0 0 1 1-1h3.5l1.5 2H13a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V4z"/></svg>
            <div class="btn-meta">
              <strong>{shell.tr("chrome.open")}</strong>
              <span>{shell.tr("chrome.openRepo")}</span>
            </div>
          </button>
          <button class="welcome-btn" type="button" onclick={() => void shell.beginClone("clone")}>
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M3 11V4a1 1 0 0 1 1-1h7a1 1 0 0 1 1 1v7M6 14h7M10 11l3 3-3 3"/></svg>
            <div class="btn-meta">
              <strong>{shell.tr("chrome.clone")}</strong>
              <span>{shell.tr("chrome.cloneHint")}</span>
            </div>
          </button>
          <button class="welcome-btn" type="button" onclick={() => void shell.beginClone("init")}>
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M8 3v10M3 8h10"/></svg>
            <div class="btn-meta">
              <strong>{shell.tr("chrome.newRepo")}</strong>
              <span>{shell.tr("chrome.new")}</span>
            </div>
          </button>
        </div>

        <div class="welcome-recent">
          <div class="recent-bar">
            <h3>{shell.tr("chrome.recent")}</h3>
            {#if shell.settings.recent.length > 2}
              <div class="search-wrap">
                {#if shell.searchOpen.recent}
                  <input class="search recent-input" placeholder={shell.tr("chrome.searchRecent")} aria-label={shell.tr("chrome.searchRecent")} bind:value={shell.recentQuery} use:focusOnMount onkeydown={(event) => shell.searchKeys("recent", event)} onblur={() => shell.searchBlur("recent")} />
                {/if}
                {@render searchToggle("recent", shell.tr("chrome.searchRecent"))}
              </div>
            {/if}
          </div>
          <div class="recent-list">
            {#each shell.settings.recent.filter((path) => shell.matchesQuery(path, shell.recentQuery)) as path (path)}
              <button class="recent-card" type="button" onclick={() => shell.openRepo(path)}>
                <div class="recent-main">
                  <span class="recent-title">{shell.folderName(path)}</span>
                  <span class="recent-path" title={path}>{path}</span>
                </div>
                {#if shell.recentStats[path]}
                  <span class="recent-stats">{shell.recentStats[path].branch} · ↑{shell.recentStats[path].ahead} ↓{shell.recentStats[path].behind} · {shell.recentStats[path].lastSummary}</span>
                {/if}
              </button>
            {:else}
              <p class="empty">{shell.tr("chrome.noRecentRepos")}</p>
            {/each}
          </div>
        </div>
      </div>
    </div>
{#snippet searchToggle(key: SearchKey, label: string)}
    <button class="icon-btn search-toggle" class:on={shell.searchOpen[key]} type="button" aria-label={label} aria-pressed={shell.searchOpen[key]} title={label} onmousedown={(event) => event.preventDefault()} onclick={() => shell.toggleSearch(key)}>
      {#if shell.searchOpen[key]}
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/></svg>
      {:else}
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="7" cy="7" r="4.25"/><path d="M10.25 10.25L13.5 13.5"/></svg>
      {/if}
    </button>
  {/snippet}
