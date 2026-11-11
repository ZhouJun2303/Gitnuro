<script lang="ts">
  import { getShell } from "./context";
  import "./PrefsDialog.css";
  import { invoke } from "../perf";
  import type { MenuItem } from "./types";

  const shell = getShell();

  type ToolKind = "diff" | "merge" | "editor";

  let typeSuffix = $state("");
  let typeToolId = $state("");
  let commitTypeName = $state("");
  let commitTypeColor = $state("#1a7f37");
  let commitTypeLabel = $state("");
  const kindTools = $derived(shell.toolsOf(shell.toolKind));
  const addToolPick = $derived(kindTools.find((tool) => tool.id === typeToolId) ?? kindTools[0] ?? null);

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

<div class="pref-tabs">
            <button type="button" class:on={shell.prefTab === "general"} onclick={() => shell.selectPrefTab("general")}>{shell.tr("dialog.general")}</button>
            <button type="button" class:on={shell.prefTab === "commit"} onclick={() => shell.selectPrefTab("commit")}>{shell.tr("dialog.commitPage")}</button>
            <button type="button" class:on={shell.prefTab === "git"} onclick={() => shell.selectPrefTab("git")}>{shell.tr("dialog.git")}</button>
            <button type="button" class:on={shell.prefTab === "integration"} onclick={() => shell.selectPrefTab("integration")}>{shell.tr("dialog.integration")}</button>
            <button type="button" class:on={shell.prefTab === "commands"} onclick={() => shell.selectPrefTab("commands")}>{shell.tr("dialog.customCommands")}</button>
          </div>
          <div class="pref-page" bind:this={shell.prefPage}>
          {#if shell.prefTab === "general"}
            <div class="pref-section">
              <label>{shell.tr("dialog.language")}
                <select bind:value={shell.settings.locale}>
                  <option value="system">{shell.tr("dialog.followSystem")}</option>
                  <option value="zh">{shell.tr("dialog.chinese")}</option>
                  <option value="en">{shell.tr("dialog.english")}</option>
                </select>
              </label>
              <label>{shell.tr("dialog.theme")}
                <select bind:value={shell.settings.theme}>
                  <option value="system">{shell.tr("dialog.system")}</option>
                  <option value="light">{shell.tr("dialog.light")}</option>
                  <option value="dark">{shell.tr("dialog.dark")}</option>
                </select>
              </label>
            </div>
            <div class="source-folders">
              <div class="k">{shell.tr("dialog.sourceFolder")}</div>
              {#each shell.sourceRootList() as root (root.path)}
                <div class="composer-row source-root">
                  <input class="alias" aria-label={shell.tr("dialog.alias")} placeholder={shell.tr("dialog.alias")} value={root.alias} onchange={(event) => shell.setSourceAlias(root.path, event.currentTarget.value)} />
                  <span class="meta source-path" title={root.path}>{root.path}</span>
                  <button class="text-button" type="button" onclick={() => shell.removeSourceRoot(root.path)}>{shell.tr("menu.remove")}</button>
                </div>
              {:else}
                <p class="empty">{shell.tr("dialog.noSourceFolders")}</p>
              {/each}
              <button class="text-button" type="button" onclick={() => shell.browseSourceRoot()}>{shell.tr("dialog.browse")}</button>
            </div>
            <div class="diff-sample" class:plain={shell.settings.disableSyntaxHighlight}>
              <div><span class="tok-word">public func</span> elementsEqual&lt;OtherSequence&gt;(</div>
              <div class="del">{shell.settings.showDiffMarks ? "- " : ""}isEquivalent isEquivalent: ({"{GElement}"}, {"{GElement}"})</div>
              <div class="add">{shell.settings.showDiffMarks ? "+ " : ""}isEquivalent: ({"{GElement}"}, {"{GElement}"}) thr</div>
            </div>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.showDiffMarks} /> {shell.tr("dialog.showMarks")}</label>
            <label class="pref-inline">{shell.tr("dialog.fontSize")} <input type="number" min="8" max="32" bind:value={shell.settings.diffFontSize} /></label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.disableSyntaxHighlight} /> {shell.tr("dialog.disableHighlight")}</label>
            <div class="pref-inline">{shell.tr("dialog.sortCommits")}
              <label class="check"><input type="radio" name="commit-sort" value="topo" checked={shell.settings.commitSort === "topo"} onchange={() => { shell.settings.commitSort = "topo"; if (shell.mode === "live") void shell.loadContext(); }} /> {shell.tr("dialog.topologically")}</label>
              <label class="check"><input type="radio" name="commit-sort" value="date" checked={shell.settings.commitSort !== "topo"} onchange={() => { shell.settings.commitSort = "date"; if (shell.mode === "live") void shell.loadContext(); }} /> {shell.tr("dialog.byDate")}</label>
            </div>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.fetchAutomatically} /> {shell.tr("dialog.fetchAuto")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.fetchTags} /> {shell.tr("dialog.fetchTagsAlways")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.tabIndicator} /> {shell.tr("dialog.tabIndicator")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.updateSubmodulesOnCheckout} /> {shell.tr("dialog.submoduleCheckout")}</label>
            <label class="pref-inline">{shell.tr("dialog.branchSpace")}
              <select bind:value={shell.settings.branchSpace}>
                <option value="-">-</option>
                <option value="_">_</option>
                <option value=".">.</option>
                <option value="">{shell.tr("dialog.keepSpace")}</option>
              </select>
            </label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.pushOnCommit} /> {shell.tr("dialog.pushOnCommit")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.compactBranchLabels} /> {shell.tr("dialog.compactLabels")}</label>
            <label class="pref-inline">{shell.tr("dialog.errorLogDays")} <input type="number" min="1" max="90" bind:value={shell.settings.errorLogDays} onchange={() => { shell.settings.errorLogDays = shell.errorDays(); shell.pruneErrorLog(); }} /></label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.flowAutoOpen} /> {shell.tr("flow.autoOpen")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.flowAutoClose} /> {shell.tr("flow.autoClose")}</label>
            <label class="pref-inline">{shell.tr("flow.historyDays")} <input type="number" min="1" max="90" bind:value={shell.settings.flowHistoryDays} onchange={() => { shell.settings.flowHistoryDays = Math.min(90, Math.max(1, Math.round(shell.settings.flowHistoryDays || 3))); }} /></label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.fetchAllRemotes} /> {shell.tr("dialog.fetchAllRemotes")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.pushAllTags} /> {shell.tr("dialog.pushAllTags")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.checkoutAfterCreate} /> {shell.tr("dialog.checkoutAfter")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.stashIncludeUntracked} /> {shell.tr("dialog.includeUntracked")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.diffWordWrap} /> {shell.tr("dialog.wordWrap")}</label>
            <label class="pref-inline">{shell.tr("dialog.diffContext")} <input type="number" min="0" max="100" bind:value={shell.settings.diffContext} /></label>
            <label class="pref-inline">{shell.tr("dialog.statusInterval")} <input type="number" min="0" max="3600" bind:value={shell.settings.statusInterval} /></label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.showWorktrees} /> {shell.tr("dialog.showWorktrees")}</label>
            <label class="pref-inline">{shell.tr("dialog.maxCommits")} <input type="number" min="50" max="100000" bind:value={shell.settings.maxCommits} onchange={() => { shell.settings.maxCommits = Math.min(100000, Math.max(50, Math.round(shell.settings.maxCommits || 500))); shell.logLimit = shell.settings.maxCommits; if (shell.mode === "live") void shell.loadContext(); }} /></label>
            <label class="pref-inline">{shell.tr("dialog.scanDepth")} <input type="number" min="1" max="20" bind:value={shell.settings.scanDepth} /></label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.workspaceShowTitle} /> {shell.tr("dialog.workspaceTitle")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.rebaseUpdateRefs} /> {shell.tr("dialog.rebaseUpdateRefs")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.applyStashDelete} /> {shell.tr("dialog.applyStashDelete")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.pushTagOnCreate} /> {shell.tr("dialog.pushTagOnCreate")}</label>
            <p class="empty">{shell.tr("dialog.settingsFile")}</p>
            {#if shell.settingsPath}<p class="empty source-path" title={shell.settingsPath}>{shell.settingsPath}</p>{/if}
            <div class="composer-row">
              <button class="text-button" type="button" onclick={() => void shell.importFork()}>{shell.tr("dialog.importFork")}</button>
              <button class="text-button" type="button" onclick={() => void shell.importForkFile()}>{shell.tr("dialog.importForkFile")}</button>
            </div>
            {#if shell.forkImportNote}<p class="empty">{shell.forkImportNote}</p>{/if}
          {:else if shell.prefTab === "commit"}
            <div class="pref-section">
              <div class="k">{shell.tr("dialog.commitTypes")}</div>
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
            <div class="pref-inline">{shell.tr("dialog.lengthIndicator")}
              <label>{shell.tr("dialog.lowLimit")} <input type="number" min="1" bind:value={shell.settings.messageLow} /></label>
              <label>{shell.tr("dialog.highLimit")} <input type="number" min="1" bind:value={shell.settings.messageHigh} /></label>
            </div>
            <label class="pref-inline">{shell.tr("dialog.spellChecking")}
              <select bind:value={shell.settings.spellChecking}>
                <option value="disable">{shell.tr("dialog.spellDisable")}</option>
                <option value="enable">{shell.tr("dialog.spellEnable")}</option>
              </select>
            </label>
            <label class="pref-inline">{shell.tr("dialog.pageGuide")} <input type="number" min="20" max="200" bind:value={shell.settings.pageGuide} /></label>
            <div class="k">{shell.tr("dialog.generateMessage")}</div>
            <label class="pref-inline">{shell.tr("dialog.promptStyle")}
              <select bind:value={shell.settings.aiPromptStyle}>
                <option value="short">{shell.tr("dialog.promptShort")}</option>
                <option value="long">{shell.tr("dialog.promptLong")}</option>
              </select>
            </label>
            <label>Base URL <input bind:value={shell.settings.aiBaseUrl} oninput={() => { shell.aiModels = []; shell.aiEndpointNote = ""; }} /></label>
            <div class="ai-actions">
              <button class="text-button" type="button" disabled={shell.aiEndpointBusy !== ""} onclick={() => void shell.testAiEndpoint()}>{shell.aiEndpointBusy === "test" ? shell.tr("chrome.working") : shell.tr("dialog.testEndpoint")}</button>
              <button class="text-button" type="button" disabled={shell.aiEndpointBusy !== ""} onclick={() => void shell.loadAiModels()}>{shell.aiEndpointBusy === "models" ? shell.tr("chrome.working") : shell.tr("dialog.fetchModels")}</button>
            </div>
            {#if shell.aiEndpointNote}<p class="empty">{shell.aiEndpointNote}</p>{/if}
            <label>Model <input bind:value={shell.settings.aiModel} /></label>
            {#if shell.aiModels.length}
              <label>{shell.tr("dialog.fetchedModels")}
                <select bind:value={shell.settings.aiModel}>
                  {#if !shell.settings.aiModel || !shell.aiModels.includes(shell.settings.aiModel)}
                    <option value={shell.settings.aiModel}>{shell.settings.aiModel || shell.tr("dialog.pickModel")}</option>
                  {/if}
                  {#each shell.aiModels as id (id)}
                    <option value={id}>{id}</option>
                  {/each}
                </select>
              </label>
            {/if}
            <label>API key <input type="password" bind:value={shell.settings.aiApiKey} /></label>
          {:else if shell.prefTab === "integration"}
            {@const kind = shell.toolKind}
            {@const tools = shell.toolsOf(kind)}
            {@const picked = shell.selectedTool(kind)}
            {@const pickedAt = picked ? tools.findIndex((tool) => tool.id === picked.id) : -1}
            <label class="check"><input type="checkbox" bind:checked={shell.settings.highlightIssues} /> {shell.tr("dialog.issueLinks")}</label>
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
            <div class="k">{shell.tr("dialog.shell")}</div>
            <label class="pref-inline">{shell.tr("dialog.shell")}
              <select bind:value={shell.settings.shellKind}>
                <option value="default">{shell.tr("dialog.shellDefault")}</option>
                <option value="custom">{shell.tr("dialog.shellPath")}</option>
              </select>
            </label>
            <div class="composer-row">
              <button class="text-button" type="button" onclick={() => { shell.settings.shellKind = "custom"; shell.settings.shellPath = "wt.exe"; shell.settings.shellArgs = "-d ."; }}>Windows Terminal</button>
              <button class="text-button" type="button" onclick={() => { shell.settings.shellKind = "custom"; shell.settings.shellPath = "powershell.exe"; shell.settings.shellArgs = "-NoExit"; }}>PowerShell</button>
              <button class="text-button" type="button" onclick={() => { shell.settings.shellKind = "custom"; shell.settings.shellPath = "C:\\Program Files\\Git\\bin\\bash.exe"; shell.settings.shellArgs = "--login -i"; }}>Git Bash</button>
              <button class="text-button" type="button" onclick={() => { shell.settings.shellKind = "custom"; shell.settings.shellPath = "warp.exe"; shell.settings.shellArgs = ""; }}>Warp</button>
              <button class="text-button" type="button" onclick={() => { shell.settings.shellKind = "custom"; shell.settings.shellPath = "ghostty"; shell.settings.shellArgs = ""; }}>Ghostty</button>
            </div>
            {#if shell.settings.shellKind === "custom"}
              <label>{shell.tr("dialog.shellPath")} <input bind:value={shell.settings.shellPath} /></label>
              <label>{shell.tr("dialog.shellArgs")} <input bind:value={shell.settings.shellArgs} /></label>
            {/if}
            <p class="empty">{shell.tr("dialog.toolHint")}</p>
          {:else if shell.prefTab === "commands"}
            {#each shell.settings.commands as command (command.id)}
              <div class="composer-row">
                <span>{command.name}</span>
                <span class="meta">{command.target}</span>
                <span class="grow"></span>
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
            <input placeholder={"${repo} ${sha} ${branch} ${file} ${input} https://"} bind:value={shell.draftUser} />
            <textarea rows="3" placeholder={shell.tr("dialog.commandPrompt")} bind:value={shell.commandPrompt}></textarea>
            <label class="check"><input type="checkbox" bind:checked={shell.commandShared} /> {shell.tr("chrome.shared")}</label>
            <button class="text-button" type="button" onclick={shell.saveCommand.bind(shell)}>{shell.tr("dialog.save")}</button>
          {:else if shell.prefTab === "git"}
            <div class="pref-section">
              <div class="k">{shell.tr("dialog.gitBehavior")}</div>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.pullRebase} /> {shell.tr("dialog.pullWithRebase")}</label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.fetchPrune} /> {shell.tr("dialog.pruneOnFetch")}</label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.swapPanes} /> {shell.tr("dialog.swapPanes")}</label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.treeFiles} /> {shell.tr("chrome.treeView")}</label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.verticalTabs} /> {shell.tr("dialog.verticalTabs")}</label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.ignoreSpace} /> {shell.tr("dialog.ignoreSpace")}</label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.showWhitespace} /> {shell.tr("dialog.showSpace")}</label>
              <div class="composer-row">
                <button class="text-button" type="button" onclick={() => (shell.dialog = "accounts")}>{shell.tr("chrome.accounts")}</button>
                <button class="text-button" type="button" onclick={() => void invoke<string>("read_ssh_config").then((text) => { shell.draft = text; shell.dialog = "ssh"; }).catch((error) => (shell.actionError = shell.message(error)))}>{shell.tr("chrome.sshConfig")}</button>
              </div>
            </div>
            <div class="pref-section">
              <div class="k">{shell.tr("dialog.dates")}</div>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.dateRelative} /> {shell.tr("dialog.dateRelative")}</label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.date24h} /> {shell.tr("dialog.date24")}</label>
              <label>{shell.tr("dialog.dateFormat")} <input bind:value={shell.settings.dateFormat} placeholder="dd MMM yyyy" /></label>
            </div>
            <div class="pref-section">
              <div class="k">{shell.tr("dialog.diff")}</div>
              <label>{shell.tr("dialog.diffStyle")}
                <select bind:value={shell.settings.diffStyle}>
                  <option value="split">{shell.tr("dialog.diffSplit")}</option>
                  <option value="unified">{shell.tr("dialog.diffUnified")}</option>
                </select>
              </label>
              <label>{shell.tr("dialog.linesHeight")}
                <select bind:value={shell.settings.linesHeight}>
                  <option value="compact">{shell.tr("dialog.lineCompact")}</option>
                  <option value="spaced">{shell.tr("dialog.lineSpaced")}</option>
                </select>
              </label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.showEntireFile} /> {shell.tr("dialog.wholeFile")}</label>
            </div>
            <div class="pref-section">
              <div class="k">{shell.tr("dialog.merge")}</div>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.mergeNoFf} /> {shell.tr("dialog.mergeNoFfPref")}</label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.signCommits} /> {shell.tr("dialog.signCommits")}</label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.signOff} onchange={() => (shell.signOff = shell.settings.signOff)} /> {shell.tr("dialog.signOffDefault")}</label>
              <label>{shell.tr("dialog.signOffFormat")} <input bind:value={shell.settings.signOffFormat} /></label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.forceWithLease} /> {shell.tr("dialog.forceWithLease")}</label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.gravatar} /> {shell.tr("dialog.gravatar")}</label>
            </div>
            <div class="pref-section">
              <div class="k">{shell.tr("dialog.ai")}</div>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.aiEnabled} /> {shell.tr("dialog.aiEnabled")}</label>
              <label>{shell.tr("dialog.aiLanguage")}
                <select bind:value={shell.settings.aiLanguage}>
                  <option value="zh">{shell.tr("dialog.chinese")}</option>
                  <option value="en">{shell.tr("dialog.english")}</option>
                </select>
              </label>
              <label>{shell.tr("dialog.aiMaxChars")} <input type="number" bind:value={shell.settings.aiMaxChars} /></label>
              <label>{shell.tr("dialog.promptExtra")} <input bind:value={shell.settings.aiPrompt} placeholder={shell.tr("dialog.promptExtra")} /></label>
              <label>{shell.tr("dialog.aiTemperature")} <input type="number" step="0.1" bind:value={shell.settings.aiTemperature} /></label>
              <label>{shell.tr("dialog.authorName")} <input bind:value={shell.settings.authorName} placeholder={shell.tr("dialog.authorPlaceholder")} /></label>
              <label>{shell.tr("dialog.authorEmail")} <input bind:value={shell.settings.authorEmail} /></label>
              <label>{shell.tr("dialog.logDirectory")} <input bind:value={shell.settings.logDirectory} /></label>
            </div>
            <div class="pref-section">
              <div class="k">{shell.tr("dialog.network")}</div>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.sslVerify} /> {shell.tr("dialog.sslVerify")}</label>
              <label class="check"><input type="checkbox" bind:checked={shell.settings.proxyEnabled} /> {shell.tr("dialog.proxyEnabled")}</label>
              <label>{shell.tr("dialog.proxyType")}
                <select bind:value={shell.settings.proxyType}>
                  <option value="http">HTTP</option>
                  <option value="socks">SOCKS</option>
                </select>
              </label>
              <label>{shell.tr("dialog.proxyHost")} <input bind:value={shell.settings.proxyHost} /></label>
              <label>{shell.tr("dialog.proxyPort")} <input type="number" bind:value={shell.settings.proxyPort} /></label>
              <label>{shell.tr("dialog.proxyUser")} <input bind:value={shell.settings.proxyUser} /></label>
              <label>{shell.tr("dialog.proxyPassword")} <input type="password" bind:value={shell.settings.proxyPassword} /></label>
              <label>{shell.tr("dialog.proxyUrl")} <input bind:value={shell.settings.proxy} /></label>
              <label>{shell.tr("dialog.sslCaFile")} <input bind:value={shell.settings.sslCaFile} /></label>
              <label>{shell.tr("dialog.githubToken")} <input type="password" bind:value={shell.settings.githubToken} /></label>
              <label>{shell.tr("dialog.gitlabToken")} <input type="password" bind:value={shell.settings.gitlabToken} /></label>
              <label>{shell.tr("dialog.gitlabHost")} <input bind:value={shell.settings.gitlabHost} placeholder="gitlab.com" /></label>
            </div>
            <div class="pref-section">
              <div class="k">{shell.tr("dialog.advanced")}</div>
              <label>{shell.tr("dialog.signingPassphrase")} <input type="password" bind:value={shell.passphrase} /></label>
              <label>{shell.tr("dialog.askpassUser")} <input bind:value={shell.passUser} /></label>
              <div class="composer-row">
                <button class="text-button" type="button" onclick={() => shell.keepPassphrase()}>{shell.tr("dialog.usePassphrase")}</button>
                <button class="text-button" type="button" onclick={() => { shell.draft = "https"; shell.draftExtra = ""; shell.draftUser = ""; shell.draftSecret = ""; shell.dialog = "credential"; }}>{shell.tr("dialog.httpsLogin")}</button>
                <button class="text-button" type="button" onclick={() => shell.mutate({ action: "lfsPull" })}>{shell.tr("dialog.lfsPull")}</button>
                <button class="text-button" type="button" onclick={() => shell.mutate({ action: "lfsPush" })}>{shell.tr("dialog.lfsPush")}</button>
                <button class="text-button" type="button" onclick={() => (shell.dialog = "patch")}>{shell.tr("dialog.applyPatch")}</button>
                <button class="text-button" type="button" onclick={() => invoke("set_repo_author", { path: shell.repoPath(), name: shell.settings.authorName, email: shell.settings.authorEmail }).catch((error) => (shell.actionError = shell.message(error)))}>{shell.tr("dialog.saveAuthorRepo")}</button>
                <button class="text-button" type="button" onclick={() => void invoke<string>("generate_ssh_key").then((key) => { shell.commandOutput = key; shell.dialog = "output"; }).catch((error) => (shell.actionError = shell.message(error)))}>{shell.tr("dialog.sshKey")}</button>
              </div>
            </div>
          {:else}
            <p class="empty">{shell.tr("dialog.settingsSaveHint")}</p>
          {/if}
          </div>
          <div class="composer-row actions prefs-foot">
            <span class="meta">{shell.tr("dialog.settingsSaveHint")}</span>
            <button class="commit" type="button" onclick={() => shell.savePrefs(true)}>{shell.tr("dialog.done")}</button>
          </div>
