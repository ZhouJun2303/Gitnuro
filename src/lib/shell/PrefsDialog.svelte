<script lang="ts">
  import { getShell } from "./context";
  import "./PrefsDialog.css";
  import { invoke } from "../perf";
  import type { UpdateNotice } from "../status";

  const shell = getShell();

</script>

<div class="pref-tabs">
            <button type="button" class:on={shell.prefTab === "general"} onclick={() => shell.selectPrefTab("general")}>{shell.tr("dialog.general")}</button>
            <button type="button" class:on={shell.prefTab === "commit"} onclick={() => shell.selectPrefTab("commit")}>{shell.tr("dialog.commitPage")}</button>
            <button type="button" class:on={shell.prefTab === "git"} onclick={() => shell.selectPrefTab("git")}>{shell.tr("dialog.git")}</button>
            <button type="button" class:on={shell.prefTab === "integration"} onclick={() => shell.selectPrefTab("integration")}>{shell.tr("dialog.integration")}</button>
            <button type="button" class:on={shell.prefTab === "commands"} onclick={() => shell.selectPrefTab("commands")}>{shell.tr("dialog.customCommands")}</button>
            <button type="button" class:on={shell.prefTab === "updates"} onclick={() => shell.selectPrefTab("updates")}>{shell.tr("dialog.updates")}</button>
          </div>
          <div class="pref-page" bind:this={shell.prefPage}>
          {#if shell.prefTab === "general"}
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
          {:else if shell.prefTab === "commit"}
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
            {#if shell.settings.aiApiKey && !shell.aiFieldsOpen}
              <button class="text-button" type="button" disabled={shell.mode !== "live" || shell.busy} onclick={() => shell.suggestMessage()}>{shell.settings.aiModel || shell.tr("chrome.suggest")}</button>
            {:else}
              <button class="text-button" type="button" onclick={() => (shell.aiFieldsOpen = true)}>{shell.tr("chrome.suggest")}</button>
            {/if}
            {#if shell.aiFieldsOpen || !shell.settings.aiApiKey}
              <label>Base URL <input bind:value={shell.settings.aiBaseUrl} /></label>
              <label>Model <input bind:value={shell.settings.aiModel} /></label>
              <label>API key <input type="password" bind:value={shell.settings.aiApiKey} /></label>
            {/if}
          {:else if shell.prefTab === "integration"}
            <label class="check"><input type="checkbox" bind:checked={shell.settings.highlightIssues} /> {shell.tr("dialog.issueLinks")}</label>
            <div class="k">{shell.tr("dialog.externalMerge")}</div>
            <div class="tool-grid">
              <div class="tool-list">
                {#each shell.toolPresets as preset (preset.name)}
                  <button type="button" class:on={shell.settings.mergeToolName === preset.name} onclick={() => shell.useMergePreset(preset.name)}>{preset.name}</button>
                {/each}
              </div>
              <div class="tool-fields">
                <label>{shell.tr("dialog.name")} <input bind:value={shell.settings.mergeToolName} /></label>
                <label>{shell.tr("dialog.toolPath")} <input bind:value={shell.settings.mergeToolPath} placeholder={shell.tr("dialog.toolHint")} /></label>
                <label>{shell.tr("dialog.toolArguments")} <input bind:value={shell.settings.mergeToolArgs} /></label>
              </div>
            </div>
            <div class="k">{shell.tr("dialog.externalDiff")}</div>
            <div class="tool-grid">
              <div class="tool-list">
                {#each shell.toolPresets as preset (`diff-${preset.name}`)}
                  <button type="button" class:on={shell.settings.diffToolName === preset.name} onclick={() => shell.useDiffPreset(preset.name)}>{preset.name}</button>
                {/each}
              </div>
              <div class="tool-fields">
                <label>{shell.tr("dialog.name")} <input bind:value={shell.settings.diffToolName} /></label>
                <label>{shell.tr("dialog.toolPath")} <input bind:value={shell.settings.diffToolPath} /></label>
                <label>{shell.tr("dialog.toolArguments")} <input bind:value={shell.settings.diffToolArgs} /></label>
              </div>
            </div>
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
                <button class="text-button" type="button" onclick={() => shell.runCommand(command)}>Run</button>
                <button class="text-button" type="button" onclick={() => shell.removeCommand(command.id)}>Delete</button>
              </div>
            {/each}
            <input placeholder="Name" bind:value={shell.draft} />
            <select bind:value={shell.draftExtra}>
              <option value="repository">Repository</option>
              <option value="commit">Commit</option>
              <option value="branch">Branch</option>
              <option value="file">File</option>
            </select>
            <input placeholder={"${repo} ${sha} ${branch} ${file} ${input} https://"} bind:value={shell.draftUser} />
            <textarea rows="3" placeholder={shell.tr("dialog.commandPrompt")} bind:value={shell.commandPrompt}></textarea>
            <label class="check"><input type="checkbox" bind:checked={shell.commandShared} /> {shell.tr("chrome.shared")}</label>
            <button class="text-button" type="button" onclick={shell.saveCommand.bind(shell)}>Save</button>
          {:else if shell.prefTab === "updates"}
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
            <button class="text-button" type="button" onclick={() => invoke("check_for_update").then((value) => (shell.updateNotice = value as UpdateNotice | null))}>{shell.tr("chrome.download")}</button>
          {:else}
            <label class="check"><input type="checkbox" bind:checked={shell.settings.pullRebase} /> Pull with rebase</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.fetchPrune} /> Prune on fetch</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.swapPanes} /> Show staged above unstaged</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.treeFiles} /> {shell.tr("chrome.treeView")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.verticalTabs} /> {shell.tr("dialog.verticalTabs")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.ignoreSpace} /> {shell.tr("dialog.ignoreSpace")}</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.showWhitespace} /> {shell.tr("dialog.showSpace")}</label>
            <button class="text-button" type="button" onclick={() => { shell.dialog = "accounts"; }}>{shell.tr("chrome.accounts")}</button>
            <button class="text-button" type="button" onclick={() => void invoke<string>("read_ssh_config").then((text) => { shell.draft = text; shell.dialog = "ssh"; }).catch((error) => (shell.actionError = shell.message(error)))}>{shell.tr("chrome.sshConfig")}</button>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.mergeNoFf} /> Merge with --no-ff</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.mergeAutostash} /> Autostash before merge</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.signCommits} /> Sign commits with git</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.signOff} onchange={() => (shell.signOff = shell.settings.signOff)} /> Sign-off by default</label>
            <label>Sign-off format <input bind:value={shell.settings.signOffFormat} /></label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.forceWithLease} /> Push with --force-with-lease</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.gravatar} /> Gravatar avatars</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.dateRelative} /> Relative dates</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.date24h} /> 24-hour clock</label>
            <label>Date pattern <input bind:value={shell.settings.dateFormat} placeholder="dd MMM yyyy" /></label>
            <label>Diff
              <select bind:value={shell.settings.diffStyle}>
                <option value="split">Split</option>
                <option value="unified">Unified</option>
              </select>
            </label>
            <label>Line height
              <select bind:value={shell.settings.linesHeight}>
                <option value="compact">Compact</option>
                <option value="spaced">Spaced</option>
              </select>
            </label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.showEntireFile} /> Show the whole file in diffs</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.aiEnabled} /> AI commit messages</label>
            <label>AI language <input bind:value={shell.settings.aiLanguage} placeholder="English" /></label>
            <label>AI max characters <input type="number" bind:value={shell.settings.aiMaxChars} /></label>
            <label>AI prompt <input bind:value={shell.settings.aiPrompt} /></label>
            <label>AI temperature <input type="number" step="0.1" bind:value={shell.settings.aiTemperature} /></label>
            <label>Author name <input bind:value={shell.settings.authorName} placeholder="uses git config when empty" /></label>
            <label>Author email <input bind:value={shell.settings.authorEmail} /></label>
            <label>Log directory <input bind:value={shell.settings.logDirectory} /></label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.sslVerify} /> Verify SSL</label>
            <label class="check"><input type="checkbox" bind:checked={shell.settings.proxyEnabled} /> Use proxy host</label>
            <label>Proxy type
              <select bind:value={shell.settings.proxyType}>
                <option value="http">HTTP</option>
                <option value="socks">SOCKS</option>
              </select>
            </label>
            <label>Proxy host <input bind:value={shell.settings.proxyHost} /></label>
            <label>Proxy port <input type="number" bind:value={shell.settings.proxyPort} /></label>
            <label>Proxy user <input bind:value={shell.settings.proxyUser} /></label>
            <label>Proxy password <input type="password" bind:value={shell.settings.proxyPassword} /></label>
            <label>Proxy URL <input bind:value={shell.settings.proxy} /></label>
            <label>CA file <input bind:value={shell.settings.sslCaFile} /></label>
            <label>{shell.tr("dialog.githubToken")} <input type="password" bind:value={shell.settings.githubToken} /></label>
            <label>{shell.tr("dialog.gitlabToken")} <input type="password" bind:value={shell.settings.gitlabToken} /></label>
            <label>{shell.tr("dialog.gitlabHost")} <input bind:value={shell.settings.gitlabHost} placeholder="gitlab.com" /></label>
            <label>Signing passphrase <input type="password" bind:value={shell.passphrase} /></label>
            <label>Askpass user <input bind:value={shell.passUser} /></label>
            <div class="composer-row">
              <button class="text-button" type="button" onclick={() => shell.keepPassphrase()}>Use passphrase</button>
              <button class="text-button" type="button" onclick={() => { shell.draft = "https"; shell.draftExtra = ""; shell.draftUser = ""; shell.draftSecret = ""; shell.dialog = "credential"; }}>Save HTTPS login</button>
              <button class="text-button" type="button" onclick={() => shell.mutate({ action: "lfsPull" })}>LFS pull</button>
              <button class="text-button" type="button" onclick={() => shell.mutate({ action: "lfsPush" })}>LFS push</button>
              <button class="text-button" type="button" onclick={() => (shell.dialog = "patch")}>Apply patch</button>
              <button class="text-button" type="button" onclick={() => invoke("set_repo_author", { path: shell.repoPath(), name: shell.settings.authorName, email: shell.settings.authorEmail })}>Save author in this repo</button>
              <button class="text-button" type="button" onclick={() => void invoke<string>("generate_ssh_key").then((key) => { shell.commandOutput = key; shell.dialog = "output"; }).catch((error) => (shell.actionError = shell.message(error)))}>{shell.tr("dialog.sshKey")}</button>
            </div>
          {/if}
          </div>
          <div class="composer-row">
            <button class="commit" type="button" onclick={() => shell.savePrefs(true)}>{shell.tr("chrome.close")}</button>
          </div>
