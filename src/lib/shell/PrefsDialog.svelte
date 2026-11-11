<script lang="ts">
  import { getShell } from "./context";
  import "./PrefsDialog.css";
  import { invoke } from "../perf";
  import type { MenuItem, PrefTab } from "./types";

  const shell = getShell();

  type ToolKind = "diff" | "merge" | "editor";

  let typeSuffix = $state("");
  let typeToolId = $state("");
  let commitTypeName = $state("");
  let commitTypeColor = $state("#1a7f37");
  let commitTypeLabel = $state("");
  const kindTools = $derived(shell.toolsOf(shell.toolKind));
  const addToolPick = $derived(kindTools.find((tool) => tool.id === typeToolId) ?? kindTools[0] ?? null);

  const navGroups = $derived([
    {
      label: shell.tr("dialog.prefGroupGeneral"),
      tabs: [
        { id: "general" as PrefTab, label: shell.tr("dialog.general"), hint: shell.tr("dialog.prefHintGeneral") },
        { id: "workspace" as PrefTab, label: shell.tr("dialog.workspacePage"), hint: shell.tr("dialog.prefHintWorkspace") },
        { id: "diff" as PrefTab, label: shell.tr("dialog.diff"), hint: shell.tr("dialog.prefHintDiff") },
      ],
    },
    {
      label: shell.tr("dialog.prefGroupWorkflow"),
      tabs: [
        { id: "commit" as PrefTab, label: shell.tr("dialog.commitPage"), hint: shell.tr("dialog.prefHintCommit") },
        { id: "ai" as PrefTab, label: shell.tr("dialog.ai"), hint: shell.tr("dialog.prefHintAi") },
        { id: "git" as PrefTab, label: shell.tr("dialog.git"), hint: shell.tr("dialog.prefHintGit") },
        { id: "integration" as PrefTab, label: shell.tr("dialog.integration"), hint: shell.tr("dialog.prefHintIntegration") },
        { id: "commands" as PrefTab, label: shell.tr("dialog.customCommands"), hint: shell.tr("dialog.prefHintCommands") },
      ],
    },
    {
      label: shell.tr("dialog.prefGroupSystem"),
      tabs: [
        { id: "network" as PrefTab, label: shell.tr("dialog.network"), hint: shell.tr("dialog.prefHintNetwork") },
        { id: "accounts" as PrefTab, label: shell.tr("dialog.accountsPage"), hint: shell.tr("dialog.prefHintAccounts") },
        { id: "advanced" as PrefTab, label: shell.tr("dialog.advanced"), hint: shell.tr("dialog.prefHintAdvanced") },
      ],
    },
  ]);
  const activeTab = $derived(navGroups.flatMap((group) => group.tabs).find((tab) => tab.id === shell.prefTab) ?? navGroups[0].tabs[0]);

  async function importSource(kind: ToolKind, source: "detected" | "common") {
    const count = await shell.importTools(kind, source);
    shell.forkImportNote = count > 0
      ? shell.tr("dialog.toolImported", { count: String(count) })
      : shell.tr("dialog.toolNoImport");
  }

  function toolImportItems(kind: ToolKind): MenuItem[] {
    return [
      { label: shell.tr("dialog.toolImportDetected"), run: () => void importSource(kind, "detected") },
      { label: shell.tr("dialog.toolImportCommon"), run: () => void importSource(kind, "common") },
      { sep: true, label: "" },
      { label: shell.tr("dialog.toolImportFork"), run: () => void shell.importForkFile() },
    ];
  }
</script>

{#snippet navIcon(id: PrefTab)}
  <svg viewBox="0 0 16 16" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
    {#if id === "general"}
      <path d="M2.4 4.4h11.2M2.4 8h11.2M2.4 11.6h11.2" />
      <circle cx="5.4" cy="4.4" r="1.3" fill="var(--elevated)" />
      <circle cx="10.2" cy="8" r="1.3" fill="var(--elevated)" />
      <circle cx="6.6" cy="11.6" r="1.3" fill="var(--elevated)" />
    {:else if id === "workspace"}
      <path d="M1.9 4.3c0-.7.6-1.3 1.3-1.3h3l1.2 1.3h5.4c.7 0 1.3.6 1.3 1.3v6.2c0 .7-.6 1.3-1.3 1.3H3.2c-.7 0-1.3-.6-1.3-1.3z" />
    {:else if id === "diff"}
      <rect x="1.8" y="2.4" width="5.3" height="11.2" rx="1.1" />
      <rect x="8.9" y="2.4" width="5.3" height="11.2" rx="1.1" />
      <path d="M3.3 5.4h2.3M3.3 7.8h2.3M10.4 5.4h2.3M10.4 7.8h2.3M10.4 10.2h2.3" />
    {:else if id === "commit"}
      <circle cx="8" cy="8" r="2.4" />
      <path d="M1.9 8h3.7M10.4 8h3.7" />
    {:else if id === "ai"}
      <path d="M8 1.9 9.15 6 13.2 7.15 9.15 8.3 8 12.4 6.85 8.3 2.8 7.15 6.85 6z" />
      <path d="M12.4 11.2l.4 1.2 1.2.4-1.2.4-.4 1.2-.4-1.2-1.2-.4 1.2-.4z" />
    {:else if id === "git"}
      <circle cx="4.4" cy="3.6" r="1.5" />
      <circle cx="4.4" cy="12.4" r="1.5" />
      <circle cx="11.6" cy="5.8" r="1.5" />
      <path d="M4.4 5.1v5.8M11.6 7.3c0 2.5-2.2 3.6-4.6 3.9" />
    {:else if id === "integration"}
      <rect x="2" y="2" width="5" height="5" rx="1.1" />
      <rect x="9" y="2" width="5" height="5" rx="1.1" />
      <rect x="2" y="9" width="5" height="5" rx="1.1" />
      <path d="M11.5 9.4v4.2M9.4 11.5h4.2" />
    {:else if id === "commands"}
      <rect x="1.8" y="2.8" width="12.4" height="10.4" rx="1.5" />
      <path d="M4.4 6.2 6.6 8l-2.2 1.8M8.6 10h3" />
    {:else if id === "network"}
      <circle cx="8" cy="8" r="6" />
      <path d="M2 8h12M8 2c1.8 1.9 2.7 3.9 2.7 6S9.8 12.1 8 14c-1.8-1.9-2.7-3.9-2.7-6S6.2 3.9 8 2z" />
    {:else if id === "accounts"}
      <circle cx="8" cy="5.4" r="2.6" />
      <path d="M2.9 13.4c.6-2.6 2.7-4 5.1-4s4.5 1.4 5.1 4" />
    {:else}
      <path d="M9.7 2.5a3.6 3.6 0 0 0-4.6 4.6L2.1 10a1.7 1.7 0 0 0 0 2.4l1.5 1.5a1.7 1.7 0 0 0 2.4 0l2.9-2.9a3.6 3.6 0 0 0 4.6-4.6l-2.3 2.3-2.1-2.1z" />
    {/if}
  </svg>
{/snippet}

<div class="pref-shell">
  <nav class="pref-nav" aria-label={shell.tr("dialog.preferences")}>
    <h2 class="pref-title">{shell.tr("dialog.preferences")}</h2>
    {#each navGroups as group (group.label)}
      <div class="pref-nav-group">
        <span class="pref-nav-group-title">{group.label}</span>
        {#each group.tabs as tab (tab.id)}
          <button
            type="button"
            class="pref-nav-item"
            class:on={shell.prefTab === tab.id}
            aria-current={shell.prefTab === tab.id ? "page" : undefined}
            onclick={() => shell.selectPrefTab(tab.id)}
          >
            {@render navIcon(tab.id)}
            <span>{tab.label}</span>
          </button>
        {/each}
      </div>
    {/each}
  </nav>

  <div class="pref-main">
    <header class="pref-head">
      <h3 class="pref-head-title">{activeTab.label}</h3>
      <p class="pref-head-hint">{activeTab.hint}</p>
    </header>

    <div class="pref-page" bind:this={shell.prefPage}>
      {#if shell.prefTab === "general"}
        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecLanguage")}</h3>
          <div class="pref-card-body">
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.language")}</span>
              <span class="pref-control">
                <select bind:value={shell.settings.locale}>
                  <option value="system">{shell.tr("dialog.followSystem")}</option>
                  <option value="zh">{shell.tr("dialog.chinese")}</option>
                  <option value="en">{shell.tr("dialog.english")}</option>
                </select>
              </span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.theme")}</span>
              <span class="pref-control">
                <select bind:value={shell.settings.theme}>
                  <option value="system">{shell.tr("dialog.system")}</option>
                  <option value="light">{shell.tr("dialog.light")}</option>
                  <option value="dark">{shell.tr("dialog.dark")}</option>
                </select>
              </span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecDate")}</h3>
          <div class="pref-card-body">
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.dateRelative} />
              <span class="pref-label">{shell.tr("dialog.dateRelative")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.date24h} />
              <span class="pref-label">{shell.tr("dialog.date24")}</span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.dateFormat")}</span>
              <span class="pref-control"><input bind:value={shell.settings.dateFormat} placeholder="dd MMM yyyy" /></span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecInterface")}</h3>
          <div class="pref-card-body">
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.workspaceShowTitle} />
              <span class="pref-label">{shell.tr("dialog.workspaceTitle")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.tabIndicator} />
              <span class="pref-label">{shell.tr("dialog.tabIndicator")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.verticalTabs} />
              <span class="pref-label">{shell.tr("dialog.verticalTabs")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.compactBranchLabels} />
              <span class="pref-label">{shell.tr("dialog.compactLabels")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.treeFiles} />
              <span class="pref-label">{shell.tr("chrome.treeView")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.swapPanes} />
              <span class="pref-label">{shell.tr("dialog.swapPanes")}</span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecHistory")}</h3>
          <div class="pref-card-body">
            <div class="pref-row">
              <span class="pref-label">{shell.tr("dialog.sortCommits")}</span>
              <span class="pref-control">
                <span class="pref-segment">
                  <label class="check">
                    <input type="radio" name="commit-sort" value="topo" checked={shell.settings.commitSort === "topo"} onchange={() => { shell.settings.commitSort = "topo"; if (shell.mode === "live") void shell.loadContext(); }} />
                    {shell.tr("dialog.topologically")}
                  </label>
                  <label class="check">
                    <input type="radio" name="commit-sort" value="date" checked={shell.settings.commitSort !== "topo"} onchange={() => { shell.settings.commitSort = "date"; if (shell.mode === "live") void shell.loadContext(); }} />
                    {shell.tr("dialog.byDate")}
                  </label>
                </span>
              </span>
            </div>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.maxCommits")}</span>
              <span class="pref-control">
                <input type="number" min="50" max="100000" bind:value={shell.settings.maxCommits} onchange={() => { shell.settings.maxCommits = Math.min(100000, Math.max(50, Math.round(shell.settings.maxCommits || 500))); shell.logLimit = shell.settings.maxCommits; if (shell.mode === "live") void shell.loadContext(); }} />
              </span>
            </label>
          </div>
        </section>
      {:else if shell.prefTab === "workspace"}
        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.sourceFolder")}</h3>
          <div class="pref-card-body">
            <div class="pref-list">
              {#each shell.sourceRootList() as root (root.path)}
                <div class="pref-source-row">
                  <input class="alias" aria-label={shell.tr("dialog.alias")} placeholder={shell.tr("dialog.alias")} value={root.alias} onchange={(event) => shell.setSourceAlias(root.path, event.currentTarget.value)} />
                  <span class="meta source-path" title={root.path}>{root.path}</span>
                  <button class="text-button" type="button" onclick={() => shell.removeSourceRoot(root.path)}>{shell.tr("menu.remove")}</button>
                </div>
              {:else}
                <p class="empty">{shell.tr("dialog.noSourceFolders")}</p>
              {/each}
            </div>
            <div class="pref-card-actions">
              <button class="text-button" type="button" onclick={() => shell.browseSourceRoot()}>{shell.tr("dialog.browse")}</button>
            </div>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecScan")}</h3>
          <div class="pref-card-body">
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.scanDepth")}</span>
              <span class="pref-control"><input type="number" min="1" max="20" bind:value={shell.settings.scanDepth} /></span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.showWorktrees} />
              <span class="pref-label">{shell.tr("dialog.showWorktrees")}</span>
            </label>
          </div>
        </section>
      {:else if shell.prefTab === "diff"}
        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecPreview")}</h3>
          <div class="pref-card-body">
            <div class="diff-sample" class:plain={shell.settings.disableSyntaxHighlight}>
              <div><span class="tok-word">public func</span> elementsEqual&lt;OtherSequence&gt;(</div>
              <div class="del">{shell.settings.showDiffMarks ? "- " : ""}isEquivalent isEquivalent: ({"{GElement}"}, {"{GElement}"})</div>
              <div class="add">{shell.settings.showDiffMarks ? "+ " : ""}isEquivalent: ({"{GElement}"}, {"{GElement}"}) thr</div>
            </div>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecDiffDisplay")}</h3>
          <div class="pref-card-body">
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.showDiffMarks} />
              <span class="pref-label">{shell.tr("dialog.showMarks")}</span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.fontSize")}</span>
              <span class="pref-control"><input type="number" min="8" max="32" bind:value={shell.settings.diffFontSize} /></span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.disableSyntaxHighlight} />
              <span class="pref-label">{shell.tr("dialog.disableHighlight")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.diffWordWrap} />
              <span class="pref-label">{shell.tr("dialog.wordWrap")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.ignoreSpace} />
              <span class="pref-label">{shell.tr("dialog.ignoreSpace")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.showWhitespace} />
              <span class="pref-label">{shell.tr("dialog.showSpace")}</span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.diffContext")}</span>
              <span class="pref-control"><input type="number" min="0" max="100" bind:value={shell.settings.diffContext} /></span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecDiffLayout")}</h3>
          <div class="pref-card-body">
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.diffStyle")}</span>
              <span class="pref-control">
                <select data-pref="diff-style" bind:value={shell.settings.diffStyle}>
                  <option value="split">{shell.tr("dialog.diffSplit")}</option>
                  <option value="unified">{shell.tr("dialog.diffUnified")}</option>
                </select>
              </span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.linesHeight")}</span>
              <span class="pref-control">
                <select bind:value={shell.settings.linesHeight}>
                  <option value="compact">{shell.tr("dialog.lineCompact")}</option>
                  <option value="spaced">{shell.tr("dialog.lineSpaced")}</option>
                </select>
              </span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.showEntireFile} />
              <span class="pref-label">{shell.tr("dialog.wholeFile")}</span>
            </label>
          </div>
        </section>
      {:else if shell.prefTab === "commit"}
        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.commitTypes")}</h3>
          <div class="pref-card-body">
            <div class="commit-type-list">
              {#each shell.commitTypeList() as type (type.name)}
                <div class="commit-type-row">
                  <input class="commit-type-color" type="color" value={type.color} aria-label={shell.tr("dialog.commitTypeColor")} onchange={(event) => shell.setCommitTypeField(type.name, "color", event.currentTarget.value)} />
                  <input class="commit-type-name" value={type.name} aria-label={shell.tr("dialog.commitTypeName")} onchange={(event) => { if (!shell.renameCommitType(type.name, event.currentTarget.value)) event.currentTarget.value = type.name; }} />
                  <input class="commit-type-label" value={type.label} placeholder={shell.commitTypeLabel(type.name)} aria-label={shell.tr("dialog.commitTypeLabel")} onchange={(event) => shell.setCommitTypeField(type.name, "label", event.currentTarget.value)} />
                  <button class="text-button" type="button" title={shell.tr("dialog.toolRemove")} aria-label={shell.tr("dialog.toolRemove")} disabled={shell.commitTypeList().length <= 1} onclick={() => shell.removeCommitType(type.name)}>×</button>
                </div>
              {/each}
              <div class="commit-type-row commit-type-new">
                <input class="commit-type-color" type="color" bind:value={commitTypeColor} aria-label={shell.tr("dialog.commitTypeColor")} />
                <input class="commit-type-name" placeholder="ui" aria-label={shell.tr("dialog.commitTypeName")} bind:value={commitTypeName} />
                <input class="commit-type-label" placeholder={shell.tr("dialog.commitTypeLabel")} aria-label={shell.tr("dialog.commitTypeLabel")} bind:value={commitTypeLabel} />
                <button class="text-button" type="button" disabled={!commitTypeName.trim()} onclick={() => { if (shell.addCommitType(commitTypeName, commitTypeColor, commitTypeLabel)) { commitTypeName = ""; commitTypeLabel = ""; } }}>{shell.tr("dialog.toolAdd")}</button>
              </div>
              <button class="text-button" type="button" onclick={() => shell.resetCommitTypes()}>{shell.tr("dialog.commitTypeReset")}</button>
            </div>
            <p class="empty">{shell.tr("dialog.commitTypesHint")}</p>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecCommitMessage")}</h3>
          <div class="pref-card-body">
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.lengthIndicator")}</span>
              <span class="pref-control">
                <label class="pref-mini">{shell.tr("dialog.lowLimit")} <input type="number" min="1" bind:value={shell.settings.messageLow} /></label>
                <label class="pref-mini">{shell.tr("dialog.highLimit")} <input type="number" min="1" bind:value={shell.settings.messageHigh} /></label>
              </span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.spellChecking")}</span>
              <span class="pref-control">
                <select bind:value={shell.settings.spellChecking}>
                  <option value="disable">{shell.tr("dialog.spellDisable")}</option>
                  <option value="enable">{shell.tr("dialog.spellEnable")}</option>
                </select>
              </span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.pageGuide")}</span>
              <span class="pref-control"><input type="number" min="20" max="200" bind:value={shell.settings.pageGuide} /></span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.highlightIssues} />
              <span class="pref-label">{shell.tr("dialog.issueLinks")}</span>
            </label>
          </div>
        </section>
      {:else if shell.prefTab === "ai"}
        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecAiGenerate")}</h3>
          <div class="pref-card-body">
            <label class="pref-row pref-toggle">
              <input data-ai-field="enabled" type="checkbox" bind:checked={shell.settings.aiEnabled} />
              <span class="pref-label">{shell.tr("dialog.aiEnabled")}</span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.aiLanguage")}</span>
              <span class="pref-control">
                <select bind:value={shell.settings.aiLanguage}>
                  <option value="zh">{shell.tr("dialog.chinese")}</option>
                  <option value="en">{shell.tr("dialog.english")}</option>
                </select>
              </span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.promptStyle")}</span>
              <span class="pref-control">
                <select bind:value={shell.settings.aiPromptStyle}>
                  <option value="short">{shell.tr("dialog.promptShort")}</option>
                  <option value="long">{shell.tr("dialog.promptLong")}</option>
                </select>
              </span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.aiMaxChars")}</span>
              <span class="pref-control"><input type="number" bind:value={shell.settings.aiMaxChars} /></span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.aiTemperature")}</span>
              <span class="pref-control"><input type="number" step="0.1" bind:value={shell.settings.aiTemperature} /></span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.promptExtra")}</span>
              <span class="pref-control"><input bind:value={shell.settings.aiPrompt} placeholder={shell.tr("dialog.promptExtra")} /></span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecAiEndpoint")}</h3>
          <div class="pref-card-body">
            <label class="pref-row wide">
              <span class="pref-label">Base URL</span>
              <span class="pref-control"><input data-ai-field="baseUrl" bind:value={shell.settings.aiBaseUrl} oninput={() => { shell.aiModels = []; shell.aiEndpointNote = ""; }} /></span>
            </label>
            <div class="pref-card-actions">
              <button class="text-button" type="button" disabled={shell.aiEndpointBusy !== ""} onclick={() => void shell.testAiEndpoint()}>{shell.aiEndpointBusy === "test" ? shell.tr("chrome.working") : shell.tr("dialog.testEndpoint")}</button>
              <button class="text-button" type="button" disabled={shell.aiEndpointBusy !== ""} onclick={() => void shell.loadAiModels()}>{shell.aiEndpointBusy === "models" ? shell.tr("chrome.working") : shell.tr("dialog.fetchModels")}</button>
            </div>
            {#if shell.aiEndpointNote}<p class="empty">{shell.aiEndpointNote}</p>{/if}
            <label class="pref-row wide">
              <span class="pref-label">Model</span>
              <span class="pref-control"><input bind:value={shell.settings.aiModel} /></span>
            </label>
            {#if shell.aiModels.length}
              <label class="pref-row wide">
                <span class="pref-label">{shell.tr("dialog.fetchedModels")}</span>
                <span class="pref-control">
                  <select bind:value={shell.settings.aiModel}>
                    {#if !shell.settings.aiModel || !shell.aiModels.includes(shell.settings.aiModel)}
                      <option value={shell.settings.aiModel}>{shell.settings.aiModel || shell.tr("dialog.pickModel")}</option>
                    {/if}
                    {#each shell.aiModels as id (id)}
                      <option value={id}>{id}</option>
                    {/each}
                  </select>
                </span>
              </label>
            {/if}
            <label class="pref-row wide">
              <span class="pref-label">API key</span>
              <span class="pref-control"><input data-ai-field="apiKey" type="password" bind:value={shell.settings.aiApiKey} /></span>
            </label>
          </div>
        </section>
      {:else if shell.prefTab === "git"}
        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecGitFetch")}</h3>
          <div class="pref-card-body">
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.fetchAutomatically} />
              <span class="pref-label">{shell.tr("dialog.fetchAuto")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.fetchTags} />
              <span class="pref-label">{shell.tr("dialog.fetchTagsAlways")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.fetchAllRemotes} />
              <span class="pref-label">{shell.tr("dialog.fetchAllRemotes")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.fetchPrune} />
              <span class="pref-label">{shell.tr("dialog.pruneOnFetch")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.pullRebase} />
              <span class="pref-label">{shell.tr("dialog.pullWithRebase")}</span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecGitPush")}</h3>
          <div class="pref-card-body">
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.pushOnCommit} />
              <span class="pref-label">{shell.tr("dialog.pushOnCommit")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.pushAllTags} />
              <span class="pref-label">{shell.tr("dialog.pushAllTags")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.pushTagOnCreate} />
              <span class="pref-label">{shell.tr("dialog.pushTagOnCreate")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.forceWithLease} />
              <span class="pref-label">{shell.tr("dialog.forceWithLease")}</span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecGitBranch")}</h3>
          <div class="pref-card-body">
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.branchSpace")}</span>
              <span class="pref-control">
                <select bind:value={shell.settings.branchSpace}>
                  <option value="-">-</option>
                  <option value="_">_</option>
                  <option value=".">.</option>
                  <option value="">{shell.tr("dialog.keepSpace")}</option>
                </select>
              </span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.checkoutAfterCreate} />
              <span class="pref-label">{shell.tr("dialog.checkoutAfter")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.updateSubmodulesOnCheckout} />
              <span class="pref-label">{shell.tr("dialog.submoduleCheckout")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.rebaseUpdateRefs} />
              <span class="pref-label">{shell.tr("dialog.rebaseUpdateRefs")}</span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecGitMerge")}</h3>
          <div class="pref-card-body">
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.mergeNoFf} />
              <span class="pref-label">{shell.tr("dialog.mergeNoFfPref")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.signCommits} />
              <span class="pref-label">{shell.tr("dialog.signCommits")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.signOff} onchange={() => (shell.signOff = shell.settings.signOff)} />
              <span class="pref-label">{shell.tr("dialog.signOffDefault")}</span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.signOffFormat")}</span>
              <span class="pref-control"><input bind:value={shell.settings.signOffFormat} /></span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecGitStash")}</h3>
          <div class="pref-card-body">
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.stashIncludeUntracked} />
              <span class="pref-label">{shell.tr("dialog.includeUntracked")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.applyStashDelete} />
              <span class="pref-label">{shell.tr("dialog.applyStashDelete")}</span>
            </label>
          </div>
        </section>
      {:else if shell.prefTab === "integration"}
        {@const kind = shell.toolKind}
        {@const tools = shell.toolsOf(kind)}
        {@const picked = shell.selectedTool(kind)}
        {@const pickedAt = picked ? tools.findIndex((tool) => tool.id === picked.id) : -1}
        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecLinks")}</h3>
          <div class="pref-card-body">
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.highlightIssues} />
              <span class="pref-label">{shell.tr("dialog.issueLinks")}</span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecTools")}</h3>
          <div class="pref-card-body">
            <div class="pref-tabs tool-tabs">
              <button type="button" class:on={kind === "diff"} onclick={() => (shell.toolKind = "diff")}>{shell.tr("dialog.externalDiff")}</button>
              <button type="button" class:on={kind === "merge"} onclick={() => (shell.toolKind = "merge")}>{shell.tr("dialog.externalMerge")}</button>
              <button type="button" class:on={kind === "editor"} onclick={() => (shell.toolKind = "editor")}>{shell.tr("dialog.textEditor")}</button>
            </div>
            <div class="tool-grid">
              <div class="tool-list">
                {#each tools as tool, index (tool.id)}
                  <button type="button" class:on={picked?.id === tool.id} title={tool.path} onclick={() => shell.selectTool(tool.id)}>
                    <span class="tool-title"><em class="tool-rank">{index + 1}</em>{shell.toolLabel(tool) || shell.tr("dialog.toolUntitled")}{#if tool.fallback}<em class="tool-default">{shell.tr("dialog.toolDefault")}</em>{/if}</span>
                    <span class="tool-path">{tool.path || shell.tr("dialog.toolNoPath")}</span>
                    <span class="tool-path">{tool.extensions.length ? shell.toolExtensionLabel(tool) : shell.tr("dialog.toolAllFiles")}</span>
                  </button>
                {:else}
                  <p class="empty tool-empty">{shell.tr("dialog.toolEmpty")}</p>
                {/each}
              </div>
              <div class="tool-fields">
                <div class="tool-actions">
                  <button class="text-button" type="button" onclick={() => shell.addTool(kind)}>{shell.tr("dialog.toolAdd")}</button>
                  <button class="text-button" type="button" disabled={!picked} onclick={() => picked && shell.removeTool(kind, picked.id)}>{shell.tr("dialog.toolRemove")}</button>
                  {#if kind === "editor"}
                    <button class="text-button tool-import" type="button" onclick={() => shell.openEditorImport()}>{shell.tr("dialog.detect")}</button>
                  {:else}
                    <button class="text-button tool-import" type="button" onclick={(event) => shell.openMenu(event, toolImportItems(kind))}>{shell.tr("dialog.toolImport")}</button>
                  {/if}
                  <span class="grow"></span>
                  <button class="text-button" type="button" disabled={pickedAt <= 0} title={shell.tr("dialog.toolMoveUp")} aria-label={shell.tr("dialog.toolMoveUp")} onclick={() => picked && shell.moveTool(kind, picked.id, -1)}>↑</button>
                  <button class="text-button" type="button" disabled={pickedAt < 0 || pickedAt >= tools.length - 1} title={shell.tr("dialog.toolMoveDown")} aria-label={shell.tr("dialog.toolMoveDown")} onclick={() => picked && shell.moveTool(kind, picked.id, 1)}>↓</button>
                </div>
                {#if picked}
                  <label>{shell.tr("dialog.name")}
                    <input value={picked.name} placeholder={shell.toolLabel(picked)} onchange={(event) => shell.setToolField(kind, picked.id, "name", event.currentTarget.value)} />
                  </label>
                  <label>{shell.tr("dialog.toolPath")}
                    <span class="tool-path-row">
                      <input value={picked.path} placeholder={shell.tr("dialog.toolHint")} onchange={(event) => shell.setToolField(kind, picked.id, "path", event.currentTarget.value)} />
                      <button class="text-button" type="button" onclick={() => void shell.browseToolPath(kind, picked.id)}>{shell.tr("dialog.browse")}</button>
                    </span>
                  </label>
                  <label>{shell.tr("dialog.toolArguments")}
                    <input value={picked.args} placeholder={kind === "merge" ? "{ours} {base} {theirs} {working}" : kind === "editor" ? "-g \"{file}:{line}\"" : "{left} {right}"} onchange={(event) => shell.setToolField(kind, picked.id, "args", event.currentTarget.value)} />
                  </label>
                  {#if kind === "editor"}<p class="empty">{shell.tr("dialog.toolEditorHint")}</p>{/if}
                  <label>{shell.tr("dialog.toolExtensions")}
                    <input value={picked.extensions.join(", ")} placeholder={shell.tr("dialog.toolAllFiles")} onchange={(event) => shell.setToolExtensions(kind, picked.id, event.currentTarget.value)} />
                  </label>
                  <p class="empty">{shell.tr("dialog.toolExtensionsHint")}</p>
                {:else}
                  <p class="empty">{shell.tr("dialog.toolNone")}</p>
                {/if}
                {#if shell.forkImportNote}<p class="empty">{shell.forkImportNote}</p>{/if}
              </div>
            </div>
            {#if tools.length > 0}
              <div class="tool-types">
                <div class="k">{shell.tr("dialog.toolFileTypes")}</div>
                {#each shell.toolTypeRows(kind) as row (row.key)}
                  {#if row.key === "*"}
                    <div class="tool-type-row" data-suffix="*">
                      <span class="tool-type-ext">*</span>
                      <select value={row.tool?.id ?? ""} aria-label={shell.tr("dialog.toolTypePick")} onchange={(event) => shell.setFallbackTool(kind, event.currentTarget.value)}>
                        {#if !row.tool}<option value="">{shell.tr("dialog.toolNone")}</option>{/if}
                        {#each tools as tool (tool.id)}
                          <option value={tool.id}>{shell.toolLabel(tool)}</option>
                        {/each}
                      </select>
                      <span></span>
                    </div>
                  {:else}
                    <div class="tool-type-row" data-suffix={row.key}>
                      <input class="tool-type-ext" value={row.key} aria-label={shell.tr("dialog.toolTypeSuffix")} onchange={(event) => shell.renameToolType(kind, row.key, event.currentTarget.value)} />
                      <select value={row.tool?.id ?? ""} aria-label={shell.tr("dialog.toolTypePick")} onchange={(event) => shell.assignToolType(kind, row.key, event.currentTarget.value)}>
                        {#if !row.tool}<option value="">{shell.tr("dialog.toolNone")}</option>{/if}
                        {#each tools as tool (tool.id)}
                          <option value={tool.id}>{shell.toolLabel(tool)}</option>
                        {/each}
                      </select>
                      <button class="text-button" type="button" title={shell.tr("dialog.toolRemove")} aria-label={shell.tr("dialog.toolRemove")} onclick={() => shell.removeToolType(kind, row.key)}>×</button>
                    </div>
                  {/if}
                {/each}
                <div class="tool-type-add">
                  <input placeholder="xlsx" aria-label={shell.tr("dialog.toolTypeSuffix")} bind:value={typeSuffix} />
                  <select aria-label={shell.tr("dialog.toolTypePick")} value={addToolPick?.id ?? ""} onchange={(event) => (typeToolId = event.currentTarget.value)}>
                    {#each tools as tool (tool.id)}
                      <option value={tool.id}>{shell.toolLabel(tool)}</option>
                    {/each}
                  </select>
                  <button class="text-button" type="button" disabled={!typeSuffix.trim() || !addToolPick} onclick={() => { if (!addToolPick) return; shell.assignToolType(kind, typeSuffix, addToolPick.id); typeSuffix = ""; }}>{shell.tr("dialog.toolAdd")}</button>
                </div>
                <p class="empty">{shell.tr("dialog.toolTypeHint")}</p>
              </div>
            {/if}
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecShell")}</h3>
          <div class="pref-card-body">
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.shell")}</span>
              <span class="pref-control">
                <select bind:value={shell.settings.shellKind}>
                  <option value="default">{shell.tr("dialog.shellDefault")}</option>
                  <option value="custom">{shell.tr("dialog.shellPath")}</option>
                </select>
              </span>
            </label>
            <div class="pref-card-actions">
              <button class="text-button" type="button" onclick={() => { shell.settings.shellKind = "custom"; shell.settings.shellPath = "wt.exe"; shell.settings.shellArgs = "-d ."; }}>Windows Terminal</button>
              <button class="text-button" type="button" onclick={() => { shell.settings.shellKind = "custom"; shell.settings.shellPath = "powershell.exe"; shell.settings.shellArgs = "-NoExit"; }}>PowerShell</button>
              <button class="text-button" type="button" onclick={() => { shell.settings.shellKind = "custom"; shell.settings.shellPath = "C:\\Program Files\\Git\\bin\\bash.exe"; shell.settings.shellArgs = "--login -i"; }}>Git Bash</button>
              <button class="text-button" type="button" onclick={() => { shell.settings.shellKind = "custom"; shell.settings.shellPath = "warp.exe"; shell.settings.shellArgs = ""; }}>Warp</button>
              <button class="text-button" type="button" onclick={() => { shell.settings.shellKind = "custom"; shell.settings.shellPath = "ghostty"; shell.settings.shellArgs = ""; }}>Ghostty</button>
            </div>
            {#if shell.settings.shellKind === "custom"}
              <label class="pref-row wide">
                <span class="pref-label">{shell.tr("dialog.shellPath")}</span>
                <span class="pref-control"><input bind:value={shell.settings.shellPath} /></span>
              </label>
              <label class="pref-row wide">
                <span class="pref-label">{shell.tr("dialog.shellArgs")}</span>
                <span class="pref-control"><input bind:value={shell.settings.shellArgs} /></span>
              </label>
            {/if}
            <p class="empty">{shell.tr("dialog.toolHint")}</p>
          </div>
        </section>
      {:else if shell.prefTab === "commands"}
        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecSavedCommands")}</h3>
          <div class="pref-card-body">
            {#each shell.settings.commands as command (command.id)}
              <div class="pref-command-row">
                <span class="pref-command-name">{command.name}</span>
                <span class="meta">{command.target}</span>
                <span class="grow"></span>
                <button class="text-button" type="button" onclick={() => shell.runCommand(command)}>{shell.tr("dialog.run")}</button>
                <button class="text-button" type="button" onclick={() => shell.removeCommand(command.id)}>{shell.tr("menu.delete")}</button>
              </div>
            {:else}
              <p class="empty">{shell.tr("dialog.noCommands")}</p>
            {/each}
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecNewCommand")}</h3>
          <div class="pref-card-body">
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.name")}</span>
              <span class="pref-control"><input placeholder={shell.tr("dialog.name")} bind:value={shell.draft} /></span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.commandTarget")}</span>
              <span class="pref-control">
                <select bind:value={shell.draftExtra}>
                  <option value="repository">{shell.tr("chrome.repository")}</option>
                  <option value="commit">{shell.tr("chrome.commit")}</option>
                  <option value="branch">{shell.tr("dialog.branch")}</option>
                  <option value="file">{shell.tr("dialog.file")}</option>
                </select>
              </span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.commandTemplate")}</span>
              <span class="pref-control"><input placeholder={"${repo} ${sha} ${branch} ${file} ${input} https://"} bind:value={shell.draftUser} /></span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.commandPrompt")}</span>
              <span class="pref-control"><textarea rows="3" placeholder={shell.tr("dialog.commandPrompt")} bind:value={shell.commandPrompt}></textarea></span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.commandShared} />
              <span class="pref-label">{shell.tr("chrome.shared")}</span>
            </label>
            <div class="pref-card-actions">
              <button class="text-button" type="button" onclick={shell.saveCommand.bind(shell)}>{shell.tr("dialog.save")}</button>
            </div>
          </div>
        </section>
      {:else if shell.prefTab === "network"}
        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecProxy")}</h3>
          <div class="pref-card-body">
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.proxyEnabled} />
              <span class="pref-label">{shell.tr("dialog.proxyEnabled")}</span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.proxyType")}</span>
              <span class="pref-control">
                <select bind:value={shell.settings.proxyType}>
                  <option value="http">HTTP</option>
                  <option value="socks">SOCKS</option>
                </select>
              </span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.proxyHost")}</span>
              <span class="pref-control"><input bind:value={shell.settings.proxyHost} /></span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.proxyPort")}</span>
              <span class="pref-control"><input type="number" bind:value={shell.settings.proxyPort} /></span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.proxyUser")}</span>
              <span class="pref-control"><input bind:value={shell.settings.proxyUser} /></span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.proxyPassword")}</span>
              <span class="pref-control"><input type="password" bind:value={shell.settings.proxyPassword} /></span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.proxyUrl")}</span>
              <span class="pref-control"><input bind:value={shell.settings.proxy} /></span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecCertificate")}</h3>
          <div class="pref-card-body">
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.sslVerify} />
              <span class="pref-label">{shell.tr("dialog.sslVerify")}</span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.sslCaFile")}</span>
              <span class="pref-control"><input bind:value={shell.settings.sslCaFile} /></span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecTokens")}</h3>
          <div class="pref-card-body">
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.githubToken")}</span>
              <span class="pref-control"><input type="password" bind:value={shell.settings.githubToken} /></span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.gitlabToken")}</span>
              <span class="pref-control"><input type="password" bind:value={shell.settings.gitlabToken} /></span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.gitlabHost")}</span>
              <span class="pref-control"><input bind:value={shell.settings.gitlabHost} placeholder="gitlab.com" /></span>
            </label>
          </div>
        </section>
      {:else if shell.prefTab === "accounts"}
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
              <button class="text-button" type="button" onclick={() => (shell.dialog = "accounts")}>{shell.tr("chrome.accounts")}</button>
              <button class="text-button" type="button" onclick={() => void invoke<string>("read_ssh_config").then((text) => { shell.draft = text; shell.dialog = "ssh"; }).catch((error) => (shell.actionError = shell.message(error)))}>{shell.tr("chrome.sshConfig")}</button>
              <button class="text-button" type="button" onclick={() => void invoke<string>("generate_ssh_key").then((key) => { shell.commandOutput = key; shell.dialog = "output"; }).catch((error) => (shell.actionError = shell.message(error)))}>{shell.tr("dialog.sshKey")}</button>
            </div>
          </div>
        </section>
      {:else if shell.prefTab === "advanced"}
        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecFlow")}</h3>
          <div class="pref-card-body">
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.flowAutoOpen} />
              <span class="pref-label">{shell.tr("flow.autoOpen")}</span>
            </label>
            <label class="pref-row pref-toggle">
              <input type="checkbox" bind:checked={shell.settings.flowAutoClose} />
              <span class="pref-label">{shell.tr("flow.autoClose")}</span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("flow.historyDays")}</span>
              <span class="pref-control">
                <input type="number" min="1" max="90" bind:value={shell.settings.flowHistoryDays} onchange={() => { shell.settings.flowHistoryDays = Math.min(90, Math.max(1, Math.round(shell.settings.flowHistoryDays || 3))); }} />
              </span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecLogs")}</h3>
          <div class="pref-card-body">
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.statusInterval")}</span>
              <span class="pref-control"><input type="number" min="0" max="3600" bind:value={shell.settings.statusInterval} /></span>
            </label>
            <label class="pref-row">
              <span class="pref-label">{shell.tr("dialog.errorLogDays")}</span>
              <span class="pref-control">
                <input type="number" min="1" max="90" bind:value={shell.settings.errorLogDays} onchange={() => { shell.settings.errorLogDays = shell.errorDays(); shell.pruneErrorLog(); }} />
              </span>
            </label>
            <label class="pref-row wide">
              <span class="pref-label">{shell.tr("dialog.logDirectory")}</span>
              <span class="pref-control"><input bind:value={shell.settings.logDirectory} /></span>
            </label>
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecConfigFile")}</h3>
          <div class="pref-card-body">
            <p class="empty">{shell.tr("dialog.settingsFile")}</p>
            {#if shell.settingsPath}<p class="empty source-path" title={shell.settingsPath}>{shell.settingsPath}</p>{/if}
            <div class="pref-card-actions">
              <button class="text-button" type="button" onclick={() => void shell.importFork()}>{shell.tr("dialog.importFork")}</button>
              <button class="text-button" type="button" onclick={() => void shell.importForkFile()}>{shell.tr("dialog.importForkFile")}</button>
            </div>
            {#if shell.forkImportNote}<p class="empty">{shell.forkImportNote}</p>{/if}
          </div>
        </section>

        <section class="pref-card">
          <h3 class="pref-card-title">{shell.tr("dialog.prefSecMaintenance")}</h3>
          <div class="pref-card-body">
            <div class="pref-card-actions">
              <button class="text-button" type="button" onclick={() => shell.mutate({ action: "lfsPull" })}>{shell.tr("dialog.lfsPull")}</button>
              <button class="text-button" type="button" onclick={() => shell.mutate({ action: "lfsPush" })}>{shell.tr("dialog.lfsPush")}</button>
              <button class="text-button" type="button" onclick={() => (shell.dialog = "patch")}>{shell.tr("dialog.applyPatch")}</button>
            </div>
          </div>
        </section>
      {/if}
    </div>
  </div>
</div>

<div class="composer-row actions prefs-foot">
  <span class="meta">{shell.tr("dialog.settingsSaveHint")}</span>
  <button class="commit" type="button" onclick={() => shell.savePrefs(true)}>{shell.tr("dialog.done")}</button>
</div>
