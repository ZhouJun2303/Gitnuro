<script lang="ts">
  import { getShell } from "./context";
  import "./Sidebar.css";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { branchGroup, orderRefNames } from "../view";
  import type { Side } from "./types";

  const shell = getShell();

</script>

<svelte:window onclick={() => { if (shell.commitSearchOpen) shell.commitSearchOpen = false; }} />

<aside class="sidebar" style:width="{shell.sidebarWidth}px">
      {#snippet glyph(kind: string)}
        <svg class="line-icon side-glyph" class:check={kind === "check"} viewBox="0 0 24 24" aria-hidden="true">
          {#if kind === "changes"}
            <path d="M6 3.5h8l4 4V20.5H6z"/><path d="M14 3.5v4h4"/><path d="M9 13h6M9 17h4"/>
          {:else if kind === "commits"}
            <path d="M9 6h11M9 12h11M9 18h11M4 6h.01M4 12h.01M4 18h.01"/>
          {:else if kind === "branch"}
            <path d="M6 3v12"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/>
          {:else if kind === "search"}
            <circle cx="11" cy="11" r="6"/><path d="M15.5 15.5 20 20"/>
          {:else if kind === "folder"}
            <path d="M3 7.5V17.5A1.5 1.5 0 0 0 4.5 19h15a1.5 1.5 0 0 0 1.5-1.5v-8A1.5 1.5 0 0 0 19.5 8H12l-2-2H4.5A1.5 1.5 0 0 0 3 7.5z"/>
          {:else if kind === "check"}
            <path d="M5 12.5 9.2 17 19 7"/>
          {:else if kind === "remote"}
            <path d="M7 17.5h9.2a3 3 0 0 0 .4-6 4.4 4.4 0 0 0-8.4-1.6A3.1 3.1 0 0 0 7 17.5z"/>
          {:else if kind === "tag"}
            <path d="M4 12.2V5h7.2L20 13.8 13.8 20z"/><circle cx="8" cy="8.2" r="1.1"/>
          {:else if kind === "stash"}
            <path d="M4 9h16v10H4z"/><path d="M12 4v7M9 8.5 12 11.5 15 8.5"/>
          {:else if kind === "submodule"}
            <path d="M4 9h16v9H4z"/><path d="M7 9V6.5h10V9M8 13h8"/>
          {:else if kind === "worktree"}
            <rect x="3.5" y="4" width="7" height="6" rx="1"/><rect x="13.5" y="4" width="7" height="6" rx="1"/><rect x="3.5" y="14" width="7" height="6" rx="1"/><rect x="13.5" y="14" width="7" height="6" rx="1"/>
          {:else if kind === "add"}
            <path d="M12 5v14M5 12h14"/>
          {:else if kind === "gear"}
            <circle cx="12" cy="12" r="3"/><path d="M12 3.5v2.2M12 18.3V20.5M3.5 12h2.2M18.3 12h2.2M5.8 5.8l1.6 1.6M16.6 16.6l1.6 1.6M18.2 5.8l-1.6 1.6M7.4 16.6l-1.6 1.6"/>
          {/if}
        </svg>
      {/snippet}
      {#snippet chevron(open: boolean)}
        <svg class="line-icon side-chevron" viewBox="0 0 24 24" aria-hidden="true">
          {#if open}<path d="m6 9 6 6 6-6"/>{:else}<path d="m9 6 6 6-6 6"/>{/if}
        </svg>
      {/snippet}

      <div class="side-pin">
      <div class="side-brand">
        <span>AweGit</span>
        <button class="icon-btn" type="button" title={shell.tr("chrome.preferences")} aria-label={shell.tr("chrome.preferences")} onclick={() => (shell.dialog = "prefs")}>{@render glyph("gear")}</button>
      </div>
      <button class="side nav" type="button" class:selected={shell.section === "changes"} onclick={() => (shell.section = "changes")}>
        {@render glyph("changes")}
        <span class="name">{shell.tr("chrome.localChanges")}</span>
        {#if shell.changeCount > 0}<span class="count">{shell.changeCount}</span>{/if}
      </button>
      <button class="side nav" type="button" class:selected={shell.section === "history"} onclick={() => (shell.section = "history")}>
        {@render glyph("commits")}
        <span class="name">{shell.tr("chrome.allCommits")}</span>
      </button>
      <div class="side-tools">
        <button class="icon-btn side-tool" class:on={shell.sideView === "refs"} type="button" title={shell.tr("chrome.localBranches")} aria-pressed={shell.sideView === "refs"} onclick={() => shell.showSideRefs()}>{@render glyph("branch")}</button>
        <button class="icon-btn side-tool" class:on={shell.sideView === "search"} type="button" title={shell.tr("chrome.searchCommits")} aria-label={shell.tr("chrome.searchCommits")} aria-pressed={shell.sideView === "search"} onclick={() => shell.showSideSearch()}>{@render glyph("search")}</button>
      </div>
      {#if shell.sideView === "refs"}
      <label class="side-filter">
        {@render glyph("search")}
        <input placeholder={shell.tr("chrome.filter")} aria-label={shell.tr("chrome.filterSidebar")} bind:this={shell.sideFilter} bind:value={shell.sideQuery} onkeydown={(event) => { if (event.key === "Escape") { event.stopPropagation(); shell.sideQuery = ""; } }} />
        {#if shell.sideQuery}<button class="side-filter-clear" type="button" aria-label={shell.tr("chrome.clearFilter")} onclick={() => (shell.sideQuery = "")}>×</button>{/if}
      </label>
      {/if}
      </div>

      {#snippet sideHead(key: string, label: string, count: number | null = null, actions: { label: string; run: () => void; icon?: "add" }[] = [], empty = count === 0)}
        <div class="side section" class:selected={shell.sideSelected === key} role="group">
          <button class="file-select" type="button" aria-expanded={shell.sideOpen(key)} onclick={() => shell.pickSide(key)}>
            <span class="twist">{@render chevron(shell.sideOpen(key))}</span>
            <span class="name">{label}</span>
            {#if count !== null && count > 0}<span class="count">{count}</span>{/if}
          </button>
          {#if actions.length > 0}
            <div class="side-actions">
              {#each actions as action (action.label)}
                {#if action.icon === "add"}
                  <button class="icon-btn side-add" type="button" title={action.label} aria-label={action.label} onclick={action.run}>{@render glyph("add")}</button>
                {:else}
                  <button class="text-button" type="button" onclick={action.run}>{action.label}</button>
                {/if}
              {/each}
            </div>
          {/if}
        </div>
      {/snippet}

      {#if shell.sideView === "refs"}
      <div class="side-scroll">
      {#if shell.mode === "live" && shell.refs}
        {@render sideHead("branches", shell.tr("chrome.localBranches"), null, [
          { label: shell.tr("chrome.new"), icon: "add", run: () => { shell.draft = ""; shell.draftExtra = ""; shell.dialog = "branch"; } },
          { label: shell.tr("chrome.showAll"), run: shell.showAllRefs.bind(shell) },
        ], shell.visibleBranches.length === 0)}
        {#if shell.sideOpen("branches")}
        {#each shell.visibleBranches as branch, index (branch.name)}
          {@const group = branchGroup(branch.name)}
          {#if group && group !== branchGroup(shell.visibleBranches[index - 1]?.name ?? "")}
            <button class="side nested" type="button" aria-expanded={shell.groupOpen(branch.name)} onclick={() => shell.toggleGroup(group)}>
              <span class="twist">{@render chevron(shell.groupOpen(branch.name))}</span>
              {@render glyph("folder")}
              <span class="name">{group}</span>
            </button>
          {/if}
          {#if shell.groupOpen(branch.name)}
          <div
            class="side nested"
            class:deep={!!group}
            class:current={branch.current}
            class:selected={shell.sideSelected === `local:${branch.name}`}
            role="group"
            oncontextmenu={(event) => shell.openMenu(event, [
              ...(!branch.current ? [
                { label: shell.tr("menu.checkout"), run: () => shell.requestCheckout(branch.name) },
                { label: shell.tr("menu.compareBranches"), disabled: !shell.compareWith || shell.compareWith === branch.name, run: () => { shell.drops = [shell.compareWith, branch.name]; void shell.compareDrops(); } },
                { label: shell.tr("menu.pushAs"), run: () => { shell.draft = branch.name; shell.draftExtra = shell.refs?.remotes[0]?.name ?? "origin"; shell.draftUser = branch.name; shell.dialog = "pushAs"; } },
                { label: shell.tr("menu.merge"), run: () => { shell.draft = branch.name; shell.draftExtra = shell.settings.mergeNoFf ? "no-ff" : "ff"; shell.dialog = "merge"; } },
                { label: shell.tr("menu.rebaseOnto"), run: () => shell.mutate({ action: "rebase", onto: branch.name, autostash: shell.settings.mergeAutostash }) },
                { label: shell.tr("menu.delete"), run: () => shell.removeBranch(branch.name) },
                { label: shell.tr("menu.forceDelete"), run: () => shell.ask(shell.tr("menu.forceDelete"), branch.name, () => void shell.mutate({ action: "deleteBranch", name: branch.name, force: true })) },
              ] : []),
              { label: shell.pinnedHere.has(branch.name) ? shell.tr("menu.unpin") : shell.tr("menu.pin"), run: () => shell.togglePin(branch.name) },
              { label: shell.tr("menu.createPr"), disabled: !shell.pullRequestUrl(branch.name), run: () => void openUrl(shell.pullRequestUrl(branch.name)) },
              { label: shell.tr("menu.rename"), run: () => { shell.draft = branch.name; shell.draftExtra = branch.name; shell.dialog = "rename"; } },
              { label: shell.tr("menu.setUpstream"), run: () => { shell.draft = branch.name; shell.draftExtra = branch.upstream ?? ""; shell.dialog = "upstream"; } },
              { label: shell.tr("menu.copyName"), run: () => shell.copyText(branch.name) },
              { label: shell.tr("menu.hide"), run: () => shell.hideRef(branch.name) },
              { label: shell.tr("menu.showOnly"), run: () => shell.showOnly(branch.name) },
              ...shell.commandItems("branch"),
            ])}
          >
            <button class="file-select" type="button" title={branch.name} onclick={(event) => { shell.sideSelected = `local:${branch.name}`; if (event.ctrlKey || event.metaKey) shell.compareWith = branch.name; else void shell.locateRef(branch.name); }} ondblclick={() => !branch.current && void shell.quickCheckout(branch.name)}>
              <span class="twist"></span>
              {@render glyph(branch.current ? "check" : "branch")}
              <span class="name">{shell.pinnedHere.has(branch.name) ? "• " : ""}{group ? branch.name.slice(group.length + 1) : branch.name}</span>
              {#if branch.ahead > 0}<span class="ahead">{branch.ahead}↑</span>{/if}
              {#if branch.behind > 0}<span class="ahead">{branch.behind}↓</span>{/if}
            </button>
          </div>
          {/if}
        {/each}
        {/if}
        {@render sideHead("remotes", shell.tr("chrome.remotes"), null, [
          { label: shell.tr("chrome.add"), icon: "add", run: () => { shell.draft = ""; shell.draftExtra = ""; shell.dialog = "remote"; } },
        ], shell.refs.remotes.length === 0)}
        {#if shell.sideOpen("remotes")}
        {#each shell.refs.remotes as remote (remote.name)}
          {#if shell.matchesQuery(remote.name, shell.sideQuery) || remote.branches.some((branch) => shell.matchesQuery(branch, shell.sideQuery))}
          {@const remoteKey = `remote:${remote.name}`}
          {@const remoteEmpty = remote.branches.length === 0 && !remote.head}
          <div class="side nested" class:selected={shell.sideSelected === remoteKey} role="group" oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("chrome.fetch"), run: () => shell.mutate({ action: "fetch", remote: remote.name, prune: shell.settings.fetchPrune, tags: shell.settings.fetchTags }) },
            { label: shell.tr("dialog.url"), run: () => { shell.draft = remote.name; shell.draftExtra = remote.url ?? ""; shell.dialog = "remote"; } },
            { label: shell.tr("menu.remove"), run: () => shell.mutate({ action: "removeRemote", name: remote.name }) },
          ])}>
            <button class="file-select" type="button" aria-expanded={remoteEmpty ? undefined : shell.sideOpen(remoteKey)} title={remote.url ?? ""} onclick={() => (remoteEmpty ? (shell.sideSelected = remoteKey) : shell.pickSide(remoteKey))}>
              <span class="twist">{#if !remoteEmpty}{@render chevron(shell.sideOpen(remoteKey))}{/if}</span>
              {@render glyph("remote")}
              <span class="name">{remote.name}</span>
            </button>
          </div>
          {#if shell.sideOpen(remoteKey)}
          {@const remoteBranches = orderRefNames(remote.branches)}
          {#if remote.head && !shell.sideQuery.trim()}<div class="side nested deep quiet"><span class="twist"></span>{@render glyph("branch")}<span class="name">HEAD → {remote.head}</span></div>{/if}
          {#each remoteBranches as branch, index (remote.name + branch)}
            {@const folder = branchGroup(branch)}
            {@const folderKey = `${remoteKey}/${folder}`}
            {#if folder && folder !== branchGroup(remoteBranches[index - 1] ?? "") && remoteBranches.some((item) => branchGroup(item) === folder && shell.matchesQuery(item, shell.sideQuery))}
              <button class="side nested deep" type="button" aria-expanded={shell.folderOpen(folderKey)} onclick={() => shell.toggleFolder(folderKey)}>
                <span class="twist">{@render chevron(shell.folderOpen(folderKey))}</span>
                {@render glyph("folder")}
                <span class="name">{folder}</span>
              </button>
            {/if}
            {#if shell.matchesQuery(branch, shell.sideQuery) && (!folder || shell.folderOpen(folderKey))}
            <div
              class="side nested deep"
              class:deeper={!!folder}
              class:selected={shell.sideSelected === `${remoteKey}:${branch}`}
              role="group"
              oncontextmenu={(event) => shell.openMenu(event, [
                { label: shell.tr("menu.checkout"), run: () => shell.mutate({ action: "checkoutRemote", remote: remote.name, branch }) },
                { label: shell.tr("menu.pullInto"), run: () => shell.mutate({ action: "pullRef", remote: remote.name, branch, rebase: shell.settings.pullRebase, autostash: shell.settings.mergeAutostash }) },
                { label: shell.tr("menu.pushHere"), run: () => shell.mutate({ action: "pushRef", remote: remote.name, branch, forceWithLease: shell.settings.forceWithLease }) },
                { label: shell.tr("menu.deleteRemote"), run: () => shell.ask(shell.tr("menu.deleteRemote"), shell.tr("dialog.deleteRemoteBody", { name: `${remote.name}/${branch}` }), () => void shell.mutate({ action: "deleteRemoteBranch", remote: remote.name, branch })) },
                { label: shell.tr("menu.copyName"), run: () => shell.copyText(`${remote.name}/${branch}`) },
              ])}
            >
              <button class="file-select" type="button" title={`${remote.name}/${branch}`} onclick={() => { shell.sideSelected = `${remoteKey}:${branch}`; void shell.locateRef(`${remote.name}/${branch}`); }} ondblclick={() => void shell.quickCheckout(`${remote.name}/${branch}`, { remote: remote.name, branch })}>
                <span class="twist"></span>
                {@render glyph("branch")}
                <span class="name">{folder ? branch.slice(folder.length + 1) : branch}</span>
              </button>
            </div>
            {/if}
          {/each}
          {/if}
          {/if}
        {/each}
        {/if}
      {:else}
        {@render sideHead("branches", shell.tr("chrome.localBranches"))}
        {#if shell.sideOpen("branches") && shell.matchesQuery(shell.branchLabel, shell.sideQuery)}
        <div
          class="side nested current"
          class:selected={shell.sideSelected === `local:${shell.branchLabel}`}
          role="group"
          oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.checkout"), disabled: true },
            { label: shell.tr("menu.merge"), run: () => { shell.draft = shell.branchLabel; shell.draftExtra = "ff"; shell.dialog = "merge"; } },
            { label: shell.tr("menu.rebaseOnto"), disabled: true },
            { label: shell.tr("menu.rename"), run: () => { shell.draft = shell.branchLabel; shell.draftExtra = shell.branchLabel; shell.dialog = "rename"; } },
            { label: shell.tr("menu.delete"), run: () => shell.ask(shell.tr("dialog.deleteBranch"), shell.tr("dialog.deleteBranchBody", { name: shell.branchLabel }), () => {}) },
            { label: shell.tr("menu.setUpstream"), run: () => { shell.draft = shell.branchLabel; shell.draftExtra = "origin/main"; shell.dialog = "upstream"; } },
            { label: shell.tr("menu.copyName"), run: () => shell.copyText(shell.branchLabel) },
            { label: shell.tr("menu.hide"), disabled: true },
            { label: shell.tr("menu.showOnly"), disabled: true },
          ])}
        >
          <button class="file-select" type="button" onclick={() => (shell.sideSelected = `local:${shell.branchLabel}`)}>
            <span class="twist"></span>
            {@render glyph("check")}
            <span class="name">{shell.branchLabel}</span>
            {#if shell.mode === "sample"}<span class="ahead">5↑</span>{/if}
          </button>
        </div>
        {/if}
        {@render sideHead("remotes", shell.tr("chrome.remotes"))}
        {#if shell.sideOpen("remotes") && (shell.matchesQuery("origin", shell.sideQuery) || shell.matchesQuery("main", shell.sideQuery))}
        <div class="side nested" class:selected={shell.sideSelected === "remote:origin"}>
          <button class="file-select" type="button" aria-expanded={shell.sideOpen("remote:origin")} onclick={() => shell.pickSide("remote:origin")}>
            <span class="twist">{@render chevron(shell.sideOpen("remote:origin"))}</span>
            {@render glyph("remote")}
            <span class="name">origin</span>
          </button>
        </div>
        {#if shell.sideOpen("remote:origin")}
        <div class="side nested deep quiet"><span class="twist"></span>{@render glyph("branch")}<span class="name">HEAD → main</span></div>
        <div
          class="side nested deep"
          class:selected={shell.sideSelected === "remote:origin:main"}
          role="group"
          oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.checkout"), disabled: true },
            { label: shell.tr("menu.pullInto"), disabled: true },
            { label: shell.tr("menu.pushHere"), disabled: true },
            { label: shell.tr("menu.deleteRemote"), run: () => shell.ask(shell.tr("menu.deleteRemote"), shell.tr("dialog.deleteRemoteBody", { name: "origin/main" }), () => {}) },
            { label: shell.tr("menu.copyName"), run: () => shell.copyText("origin/main") },
          ])}
        ><button class="file-select" type="button" onclick={() => (shell.sideSelected = "remote:origin:main")}><span class="twist"></span>{@render glyph("branch")}<span class="name">main</span></button></div>
        {/if}
        {/if}
      {/if}
      {@render sideHead("tags", shell.tr("chrome.tags"), shell.visibleTags.length)}
      {#if shell.sideOpen("tags")}
        {#each shell.visibleTags as tag, index (tag.name)}
          {@const group = branchGroup(tag.name)}
          {@const tagFolder = group ? `tag:${group}` : ""}
          {#if group && group !== branchGroup(shell.visibleTags[index - 1]?.name ?? "")}
            <button class="side nested" type="button" aria-expanded={shell.folderOpen(tagFolder)} onclick={() => shell.toggleFolder(tagFolder)}>
              <span class="twist">{@render chevron(shell.folderOpen(tagFolder))}</span>
              {@render glyph("folder")}
              <span class="name">{group}</span>
            </button>
          {/if}
          {#if shell.folderOpen(tagFolder)}
          <div class="side nested" class:deep={!!group} class:selected={shell.sideSelected === `tag:${tag.name}`} role="group" oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.checkout"), disabled: shell.mode !== "live", run: () => shell.mutate({ action: "checkout", name: tag.name }) },
            { label: shell.tr("menu.delete"), disabled: shell.mode !== "live", run: () => shell.ask(shell.tr("menu.delete"), tag.name, () => void shell.mutate({ action: "deleteTag", name: tag.name })) },
            { label: shell.tr("menu.deleteRemoteTag"), disabled: shell.mode !== "live" || !shell.refs?.remotes[0], run: () => { const remote = shell.refs?.remotes[0]?.name ?? ""; shell.ask(shell.tr("menu.deleteRemoteTag"), `${remote} ${tag.name}`, () => void shell.mutate({ action: "deleteRemoteTag", remote, name: tag.name })); } },
            { label: shell.tr("menu.copyName"), run: () => shell.copyText(tag.name) },
          ])}>
            <button class="file-select" type="button" title={tag.name} onclick={() => { shell.sideSelected = `tag:${tag.name}`; void shell.locateRef(tag.name); }} ondblclick={() => void shell.quickCheckout(tag.name)}><span class="twist"></span>{@render glyph("tag")}<span class="name">{group ? tag.name.slice(group.length + 1) : tag.name}</span></button>
          </div>
          {/if}
        {/each}
      {/if}
      {@render sideHead("stashes", shell.tr("chrome.stashes"), shell.refs?.stashes.length ?? (shell.mode === "sample" ? 1 : 0))}
      {#if shell.sideOpen("stashes")}
        {#each (shell.refs?.stashes ?? (shell.mode === "sample" ? [{ name: "stash@{0}", summary: "WIP before the palette" }] : [])) as stash (stash.name)}
          {#if shell.matchesQuery(stash.summary, shell.sideQuery) || shell.matchesQuery(stash.name, shell.sideQuery)}
          <div class="side nested" class:selected={shell.sideSelected === `stash:${stash.name}`} role="group" oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.apply"), disabled: shell.mode !== "live", run: () => void shell.applyStash(stash.name) },
            { label: shell.tr("menu.pop"), disabled: shell.mode !== "live", run: () => shell.mutate({ action: "stashPop", name: stash.name }) },
            { label: shell.tr("menu.drop"), disabled: shell.mode !== "live", run: () => shell.ask(shell.tr("menu.drop"), stash.summary, () => void shell.mutate({ action: "stashDrop", name: stash.name })) },
            { label: shell.tr("menu.renameStash"), disabled: shell.mode !== "live", run: () => { shell.draft = stash.summary; shell.draftExtra = stash.name; shell.dialog = "stashRename"; } },
            { label: shell.tr("menu.stashPatch"), disabled: shell.mode !== "live", run: () => void shell.mutate({ action: "stashPatch", name: stash.name }) },
            { label: shell.tr("menu.stashBranch"), disabled: shell.mode !== "live", run: () => { shell.draft = ""; shell.draftExtra = stash.name; shell.dialog = "stashBranch"; } },
          ])}>
            <button class="file-select" type="button" title={stash.name} onclick={() => { shell.sideSelected = `stash:${stash.name}`; void shell.locateStash(stash); }}><span class="twist"></span>{@render glyph("stash")}<span class="name">{stash.summary}</span></button>
          </div>
          {/if}
        {/each}
      {/if}
      {@render sideHead("submodules", shell.tr("chrome.submodules"), shell.refs?.submodules.length ?? 0, [
        { label: shell.tr("chrome.add"), icon: "add", run: () => { shell.draft = ""; shell.draftExtra = ""; shell.dialog = "submodule"; } },
      ])}
      {#if shell.sideOpen("submodules")}
        {#each shell.refs?.submodules ?? [] as row (row.path)}
          {#if shell.matchesQuery(row.path, shell.sideQuery)}
          <div class="side nested" class:selected={shell.sideSelected === `submodule:${row.path}`} role="group" oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.initialize"), disabled: shell.mode !== "live" || row.ready, run: () => shell.mutate({ action: "submoduleInit", path: row.path }) },
            { label: shell.tr("menu.openFile"), disabled: !row.ready, run: () => shell.openRepo(shell.fullPath(row.path)) },
            { label: shell.tr("menu.sync"), disabled: shell.mode !== "live" || !row.ready, run: () => shell.mutate({ action: "submoduleSync", path: row.path }) },
            { label: shell.tr("menu.update"), disabled: shell.mode !== "live" || !row.ready, run: () => shell.mutate({ action: "submoduleUpdate" }) },
            { label: shell.tr("menu.delete"), disabled: shell.mode !== "live", run: () => shell.ask(shell.tr("menu.delete"), row.path, () => void shell.mutate({ action: "submoduleRemove", path: row.path })) },
          ])}>
            <button class="file-select" type="button" title={row.path} onclick={() => (shell.sideSelected = `submodule:${row.path}`)} ondblclick={() => row.ready && shell.openRepo(shell.fullPath(row.path))}><span class="twist"></span>{@render glyph("submodule")}<span class="name">{row.path}</span></button>
          </div>
          {/if}
        {/each}
      {/if}
      {#if shell.settings.showWorktrees}
      {@render sideHead("worktrees", shell.tr("chrome.worktrees"), shell.refs?.worktrees.length ?? (shell.mode === "sample" ? 1 : 0), [
        { label: shell.tr("chrome.add"), icon: "add", run: () => { shell.draft = ""; shell.draftExtra = shell.snapshot?.branch ?? ""; shell.dialog = "worktree"; } },
      ])}
      {#if shell.sideOpen("worktrees")}
        {#each (shell.refs?.worktrees ?? (shell.mode === "sample" ? [{ path: "this repository", branch: "main" }] : [])) as row (row.path)}
          {#if shell.matchesQuery(row.path, shell.sideQuery) || shell.matchesQuery(row.branch ?? "", shell.sideQuery)}
          <div class="side nested" class:selected={shell.sideSelected === `worktree:${row.path}`} role="group" oncontextmenu={(event) => shell.openMenu(event, [
            { label: shell.tr("menu.openFile"), disabled: shell.mode !== "live", run: () => shell.openRepo(row.path) },
            { label: shell.tr("menu.remove"), disabled: shell.mode !== "live", run: () => shell.ask(shell.tr("menu.remove"), row.path, () => void shell.mutate({ action: "removeWorktree", path: row.path })) },
          ])}>
            <button class="file-select" type="button" title={row.path} onclick={() => (shell.sideSelected = `worktree:${row.path}`)} ondblclick={() => shell.mode === "live" && shell.openRepo(row.path)}><span class="twist"></span>{@render glyph("worktree")}<span class="name">{row.branch ?? shell.folderName(row.path)}</span></button>
          </div>
          {/if}
        {/each}
      {/if}
      {/if}
      </div>
      {:else}
      <div class="side-search">
        <h2>{shell.tr("chrome.searchCommits")}</h2>
        <label class="side-filter">
          {@render glyph("search")}
          <input
            placeholder={shell.commitSearchField === "author" ? shell.tr("chrome.author") : shell.commitSearchField === "path" ? shell.tr("dialog.path") : shell.commitSearchField === "diff" ? shell.tr("chrome.diffContent") : shell.tr("chrome.commitMessage")}
            aria-label={shell.tr("chrome.searchCommits")}
            bind:this={shell.commitSearchEl}
            bind:value={shell.commitSearch}
            oninput={() => shell.queueCommitSearch()}
            onkeydown={(event) => { if (event.key === "Escape") { event.stopPropagation(); shell.commitSearchOpen = false; if (shell.commitSearch) { shell.commitSearch = ""; shell.queueCommitSearch(); } } }}
          />
          {#if shell.commitSearch}<button class="side-filter-clear" type="button" aria-label={shell.tr("chrome.clearFilter")} onclick={() => { shell.commitSearch = ""; shell.queueCommitSearch(); }}>×</button>{/if}
        </label>
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="side-search-pick" onclick={(event) => event.stopPropagation()}>
          <button class="side-search-toggle" type="button" aria-haspopup="menu" aria-expanded={shell.commitSearchOpen} onclick={() => (shell.commitSearchOpen = !shell.commitSearchOpen)}>
            <span>{shell.commitSearchField === "author" ? shell.tr("chrome.author") : shell.commitSearchField === "path" ? shell.tr("dialog.path") : shell.commitSearchField === "diff" ? shell.tr("chrome.diffContent") : shell.tr("chrome.commitMessage")}</span>
            <svg class="line-icon side-chevron" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>
          </button>
          {#if shell.commitSearchOpen}
            <div class="side-search-menu" role="menu">
              <button class="side-search-item" type="button" role="menuitemradio" aria-checked={shell.commitSearchField === "message"} onclick={() => shell.pickCommitSearch("message")}><span class="side-search-check">{#if shell.commitSearchField === "message"}{@render glyph("check")}{/if}</span>{shell.tr("chrome.commitMessage")}</button>
              <button class="side-search-item" type="button" role="menuitemradio" aria-checked={shell.commitSearchField === "author"} onclick={() => shell.pickCommitSearch("author")}><span class="side-search-check">{#if shell.commitSearchField === "author"}{@render glyph("check")}{/if}</span>{shell.tr("chrome.author")}</button>
              <button class="side-search-item" type="button" role="menuitemradio" aria-checked={shell.commitSearchField === "path"} onclick={() => shell.pickCommitSearch("path")}><span class="side-search-check">{#if shell.commitSearchField === "path"}{@render glyph("check")}{/if}</span>{shell.tr("dialog.path")}</button>
              <button class="side-search-item" type="button" role="menuitemradio" aria-checked={shell.commitSearchField === "diff"} onclick={() => shell.pickCommitSearch("diff")}><span class="side-search-check">{#if shell.commitSearchField === "diff"}{@render glyph("check")}{/if}</span>{shell.tr("chrome.diffContent")}</button>
              <div class="side-search-sep"></div>
              <div class="side-search-label">{shell.tr("chrome.searchIn")}</div>
              <button class="side-search-item" type="button" role="menuitemradio" aria-checked={shell.commitSearchScope === "repository"} onclick={() => shell.pickCommitScope("repository")}><span class="side-search-check">{#if shell.commitSearchScope === "repository"}{@render glyph("check")}{/if}</span>{shell.tr("chrome.repository")}</button>
              <button class="side-search-item" type="button" role="menuitemradio" aria-checked={shell.commitSearchScope === "branch"} onclick={() => shell.pickCommitScope("branch")}><span class="side-search-check">{#if shell.commitSearchScope === "branch"}{@render glyph("check")}{/if}</span>{shell.tr("chrome.currentBranch")}</button>
            </div>
          {/if}
        </div>
      </div>
      {/if}
    </aside>
