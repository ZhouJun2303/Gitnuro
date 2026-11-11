<script lang="ts">
  import { getShell } from "./context";
  import StashPlates from "./StashPlates.svelte";
  import RefChip from "./RefChip.svelte";
  import BinaryDiff from "./BinaryDiff.svelte";
  import "./CommitDetail.css";
  import SplitHandle from "../SplitHandle.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { diffs } from "../sample";
  import { onVirtualScroll, wrapPrefix, type SplitRow } from "../view";
  import type { Row, Side, TreeEntry } from "./types";

  const shell = getShell();
  const focusOnMount = (node: HTMLInputElement) => shell.focusOnMount(node);
  const splitScroller = (node: HTMLElement, columns?: { left: number; right: number }) => shell.splitScroller(node, columns);
  const onBlobScroll = onVirtualScroll((top) => (shell.blobTop = top));
  const patchWindow = (node: HTMLElement, spec: { file: string; count: number; rowHeight: number; prefix?: Uint32Array | null }) => shell.trackPatch(node, spec);
  const commitScroll = (node: HTMLElement) => shell.trackCommitScroll(node);
  const commitList = (node: HTMLElement) => shell.trackCommitList(node);
  const measureCommitPatch = (node: HTMLElement, path: string) => shell.measureCommitPatch(node, path);
  const loadCommitPatch = (node: HTMLElement, path: string) => shell.loadCommitPatch(node, path);
</script>

<section class="detail" style:height="{shell.historyDetailHeight}px">
          <div class="detail-tabs" role="tablist">
            <button type="button" role="tab" aria-selected={shell.historyTab === "commit"} class:on={shell.historyTab === "commit"} onclick={() => shell.openHistoryTab("commit")}>{shell.tr("chrome.commit")}</button>
            <button type="button" role="tab" aria-selected={shell.historyTab === "changes"} class:on={shell.historyTab === "changes"} onclick={() => shell.openHistoryTab("changes")}>{shell.tr("chrome.changes")}</button>
            <button type="button" role="tab" aria-selected={shell.historyTab === "tree"} class:on={shell.historyTab === "tree"} onclick={() => shell.openHistoryTab("tree")}>{shell.tr("chrome.fileTree")}</button>
            {#if shell.selectedCommit || shell.commitDetail}
              <div class="detail-tools" role="presentation">
                {#if shell.searchOpen.detail}
                  <input class="search detail-filter" placeholder={shell.tr("chrome.filterFiles")} aria-label={shell.tr("chrome.filterFiles")} bind:value={shell.detailQuery} use:focusOnMount onkeydown={(event) => shell.searchKeys("detail", event)} onblur={() => shell.searchBlur("detail")} />
                {/if}
                <button class="icon-btn search-toggle" class:on={shell.searchOpen.detail} type="button" aria-label={shell.tr("chrome.filterFiles")} aria-pressed={shell.searchOpen.detail} title={shell.tr("chrome.filterFiles")} onmousedown={(event) => event.preventDefault()} onclick={() => shell.toggleSearch("detail")}>
                  {#if shell.searchOpen.detail}
                    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/></svg>
                  {:else}
                    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><circle cx="7" cy="7" r="4.25"/><path d="M10.25 10.25L13.5 13.5"/></svg>
                  {/if}
                </button>
              </div>
            {/if}
          </div>
          {#if shell.selectedCommit || shell.commitDetail}
            {@const name = shell.commitAuthorName()}
            {@const plate = shell.stashParts(shell.commitSubject(), shell.commitRefs())}
            <div class="commit-strip">
              <span class="avatar tile strip" style:background={shell.avatarColor(name)}>{(name || "?").slice(0, 1).toUpperCase()}</span>
              <span class="strip-author" title={name}>{name}</span>
              <span class="strip-sha">{(shell.selectedCommit ?? "").slice(0, 7)}</span>
              <span class="strip-when">{shell.commitWhenShort()}</span>
              {#if plate}
                <StashPlates branch={plate.branch} message={plate.message} />
              {:else}
                <span class="strip-subject" title={shell.commitSubject()}>{@render linked(shell.commitSubject())}</span>
              {/if}
            </div>
          {/if}
          <div class="detail-body">
            {#if !shell.selectedCommit && !shell.commitDetail}
              <p class="empty">{shell.tr("chrome.selectCommitFile")}</p>
            {:else if shell.historyTab === "commit"}
              {@const plate = shell.stashParts(shell.commitSubject(), shell.commitRefs())}
              {@const route = shell.mergeRoute()}
              <div class="detail-scroll" use:commitScroll>
                <div class="fork-card" class:select-all={shell.textWhere === "detail"}>
                  {@render personBlock(shell.tr("chrome.author"), shell.commitDetail?.author || shell.commitAuthorName(), shell.commitDetail?.authorEmail ?? "", shell.commitDetail?.authorAt ?? 0)}
                  {#if shell.differentCommitter(shell.commitDetail)}
                    {@render personBlock(shell.tr("chrome.committer"), shell.commitDetail?.committer ?? "", shell.commitDetail?.committerEmail ?? "", shell.commitDetail?.committerAt ?? 0)}
                  {/if}
                  <div class="fork-row"><span class="k">{shell.tr("chrome.sha")}</span><button class="sha-link" type="button" title={shell.commitDetail?.id || shell.selectedCommit || ""} onclick={() => shell.copyText(shell.commitDetail?.id || shell.selectedCommit || "")}>{shell.commitDetail?.id || shell.selectedCommit}</button></div>
                  {#if (shell.commitDetail?.parents.length ?? 0) > 0}
                    <div class="fork-row"><span class="k">{shell.tr("chrome.parents")}</span>
                      {#each shell.commitDetail?.parents ?? [] as parent (parent)}
                        <button class="sha-link parent" type="button" onclick={() => { if (shell.mode === "live") void shell.selectCommit(parent); else shell.selectSample(parent); }}>{parent.slice(0, 7)}</button>
                      {/each}
                    </div>
                  {/if}
                  {#if route}
                    <div class="fork-row merge-route"><span class="k">{shell.tr("chrome.mergeRoute")}</span>
                      <span class="ref" title={route.source}>{route.source}</span>
                      {#if route.into}<span class="meta" aria-hidden="true">→</span><span class="ref" title={route.into}>{route.into}</span>{/if}
                    </div>
                  {/if}
                  {#if shell.commitRefs().length > 0}
                    {@const refs = [...shell.branchRefLabels(shell.commitRefs()), ...shell.plainRefLabels(shell.commitRefs())]}
                    <div class="fork-row"><span class="k">{shell.tr("chrome.refsLabel")}</span>
                      {#each refs as label (label)}<RefChip {label} kind={shell.refKind(label)} compact={shell.settings.compactBranchLabels} />{/each}
                    </div>
                  {/if}
                  {#if plate}
                    <p class="detail-message"><StashPlates branch={plate.branch} message={plate.message} /></p>
                  {:else}
                    <p class="detail-message">{@render linked(shell.commitSubject())}</p>
                  {/if}
                  {#if shell.commitDetail?.body}<pre class="detail-body-text">{@render linked(shell.commitDetail.body)}</pre>{/if}
                </div>
                <div class="patch-head commit-files-head">
                  <span class="meta">{shell.detailFiltering ? `${shell.commitShown.length} / ${shell.commitFiles.length}` : shell.commitFiles.length}</span>
                  {#if shell.commitShown.length > 0}
                    <button class="expand-all" type="button" onclick={() => void shell.toggleAllPatches()}>{shell.allPatchesOpen ? shell.tr("chrome.collapseAll") : shell.tr("chrome.expandAll")}</button>
                  {/if}
                </div>
                <div class="commit-files" use:commitList>
                  {#if shell.detailFiltering && shell.commitFiles.length > 0 && shell.commitShown.length === 0}
                    <p class="empty">{shell.tr("chrome.noMatch")}</p>
                  {/if}
                  {#if shell.commitWindow.topSpacer > 0}
                    <div style:height="{shell.commitWindow.topSpacer}px"></div>
                  {/if}
                  {#each shell.commitWindow.rows as entry (entry.key)}
                    {#if entry.kind === "file"}
                      <div class="patch-head">
                        <button class="patch-toggle" type="button" onclick={() => shell.togglePatch(entry.path)} oncontextmenu={(event) => shell.openMenu(event, shell.revisionFileMenu(entry.path))}>
                          {@render disclosure(shell.openPatchSet.has(entry.path))}
                          {@render statusBadge(shell.toRow(entry.file).letter)}
                          {@render typeIcon(entry.path)}
                          <span class="patch-path" title={entry.path}>{entry.path}</span>
                        </button>
                      </div>
                    {:else}
                      <div
                        class="commit-patch"
                        style:height={shell.patchHeights[entry.path] ? `${shell.patchHeights[entry.path]}px` : undefined}
                        use:measureCommitPatch={entry.path}
                        use:loadCommitPatch={entry.path}
                      >
                        {@render filePatch(entry.path, false)}
                      </div>
                    {/if}
                  {/each}
                  {#if shell.commitWindow.bottomSpacer > 0}
                    <div style:height="{shell.commitWindow.bottomSpacer}px"></div>
                  {/if}
                </div>
              </div>
            {:else if shell.historyTab === "changes"}
              <div class="detail-split">
                <div class="detail-tree" style:width="{shell.detailTreeWidth}px">
                  {#if shell.changeTree.length === 0}
                    <p class="empty">{shell.commitFiles.length === 0 ? shell.tr("chrome.noChanges") : shell.tr("chrome.noMatch")}</p>
                  {:else}
                    {@render detailTree(shell.changeTree, shell.detailFiltering ? [] : shell.commitCollapsed, shell.toggleCommitDir.bind(shell), shell.selectChangeFile.bind(shell))}
                  {/if}
                </div>
                <SplitHandle direction="vertical" bind:size={shell.detailTreeWidth} min={160} max={560} defaultSize={280} label={shell.tr("chrome.resizeTree")} onchange={shell.handleLayoutChange.bind(shell)} />
                <div class="detail-pane">
                  {#if shell.historyFile}
                    {@render filePatch(shell.historyFile, false)}
                  {:else}
                    <p class="diff-empty">{shell.tr("chrome.selectCommitFile")}</p>
                  {/if}
                </div>
              </div>
            {:else}
              <div class="detail-split">
                <div class="detail-tree" style:width="{shell.detailTreeWidth}px">
                  {#if shell.repoTree.length === 0}
                    <p class="empty">{shell.treePrepared === shell.selectedCommit ? shell.tr("chrome.noMatch") : shell.tr("chrome.reading")}</p>
                  {:else}
                    {@render detailTree(shell.repoTree, shell.detailFiltering ? [] : shell.treeCollapsed, shell.toggleRepoDir.bind(shell), shell.selectTreeFile.bind(shell))}
                  {/if}
                </div>
                <SplitHandle direction="vertical" bind:size={shell.detailTreeWidth} min={160} max={560} defaultSize={280} label={shell.tr("chrome.resizeTree")} onchange={shell.handleLayoutChange.bind(shell)} />
                <div class="detail-pane">
                  {#if shell.historyFile}
                    <div class="blob-tab">{@render typeIcon(shell.historyFile)}<span>{shell.fileName(shell.historyFile)}</span></div>
                    {#if shell.preview?.mime.startsWith("image/")}
                      <img class="blob-image" alt={shell.historyFile ?? ""} src={shell.preview.dataUrl} />
                    {:else if shell.preview?.mime === "application/pdf"}
                      <embed class="pdf-view" title={shell.historyFile} src={shell.preview.dataUrl} type="application/pdf" />
                    {:else if shell.treeBinary}
                      <p class="diff-empty">{shell.tr("chrome.binary")}</p>
                    {:else if shell.treeTruncated && shell.blobLines.length === 0}
                      <p class="diff-empty">{shell.tr("chrome.truncated")}</p>
                    {:else if shell.blobLines.length > 0}
                      {#if shell.treeTruncated}<p class="meta patch-note">{shell.tr("chrome.truncated")}</p>{/if}
                      <div class="blob-body" class:select-all={shell.textWhere === "blob"} onscroll={onBlobScroll}>
                        <div class="blob-window" style:height="{shell.blobLines.length * 18}px">
                          {#each shell.blobWindow.rows as line, index (`b-${shell.blobWindow.start + index}`)}
                            <div class="blob-line" style:top="{(shell.blobWindow.start + index) * 18}px">
                              <span class="blob-no">{shell.blobWindow.start + index + 1}</span>
                              <span class="blob-code">{#each shell.paint(line, shell.historyFile) as token, tokenIndex (`bt-${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
                            </div>
                          {/each}
                        </div>
                      </div>
                    {:else}
                      <p class="diff-empty">{shell.mode === "live" ? shell.tr("chrome.reading") : shell.tr("chrome.noChanges")}</p>
                    {/if}
                  {:else}
                    <p class="diff-empty">{shell.tr("chrome.selectCommitFile")}</p>
                  {/if}
                </div>
              </div>
            {/if}
          </div>
        </section>
{#snippet personBlock(label: string, name: string, email: string, at: number)}
    <div class="fork-person-wrap">
      <div class="k">{label}</div>
      <div class="fork-person">
        <span class="avatar tile lg" style:background={shell.avatarColor(name || "?")}>{(name || "?").slice(0, 1).toUpperCase()}</span>
        <div>
          <div class="fork-id"><span class="who">{name}</span>{#if email}<span class="meta">{email}</span>{/if}</div>
          <div class="meta">{at ? shell.forkWhenFull(at) : ""}</div>
        </div>
      </div>
    </div>
  {/snippet}

{#snippet linked(text: string)}
      {#each shell.linkParts(text) as part, index (`${index}-${part.text}`)}
        {#if part.href}
          <a class="issue" href={part.href} onclick={(event) => { event.preventDefault(); event.stopPropagation(); void openUrl(part.href); }}>{part.text}</a>
        {:else if shell.settings.highlightIssues && (/^#\d+$/.test(part.text) || part.text.startsWith("http"))}
          <span class="issue">{part.text}</span>
        {:else}
          {part.text}
        {/if}
      {/each}
    {/snippet}

{#snippet disclosure(open: boolean)}
    <span class="twist" aria-hidden="true">
      <svg class="twist-icon" viewBox="0 0 12 12">
        <path fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" d={open ? "M2.2 4.2 6 8l3.8-3.8" : "M4.2 2.2 8 6l-3.8 3.8"} />
      </svg>
    </span>
  {/snippet}

{#snippet folderIcon()}
    <svg class="tree-icon folder" viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M1.5 4.1A1.4 1.4 0 0 1 2.9 2.7h3.2l1.3 1.4H13a1.5 1.5 0 0 1 1.5 1.5v6.9A1.5 1.5 0 0 1 13 14H2.9a1.4 1.4 0 0 1-1.4-1.4z"/></svg>
  {/snippet}

{#snippet statusBadge(letter: string)}
    {#if letter === "A" || letter === "?"}
      <svg class="tree-icon status added" viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="6.2" fill="currentColor"/><path d="M8 4.9v6.2M4.9 8h6.2" stroke="#fff" stroke-width="1.5" stroke-linecap="round"/></svg>
    {:else if letter === "D"}
      <svg class="tree-icon status deleted" viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="6.2" fill="currentColor"/><path d="M4.9 8h6.2" stroke="#fff" stroke-width="1.5" stroke-linecap="round"/></svg>
    {:else if letter}
      <span class="tree-mark" class:alert={letter === "C"}>{letter}</span>
    {/if}
  {/snippet}

{#snippet typeIcon(path: string)}
    {@const kind = shell.fileKind(path)}
    <svg class="tree-icon type" class:code={kind === "code"} class:json={kind === "json"} class:markup={kind === "markup"} class:text={kind === "text"} class:image={kind === "image"} viewBox="0 0 16 16" aria-hidden="true">
      {#if kind === "image"}
        <rect x="1.8" y="3.1" width="12.4" height="9.8" rx="1.6" fill="none" stroke="currentColor" stroke-width="1.2"/>
        <circle cx="5.3" cy="6.3" r="1" fill="currentColor"/>
        <path fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" d="M2.4 11.3 5.7 8.4l2 1.7 2.2-1.5 3.5 2.7"/>
      {:else}
        <path fill="none" stroke="currentColor" stroke-width="1.2" d="M3.2 1.5h6L12.8 5v9.5H3.2z"/>
        <path fill="none" stroke="currentColor" stroke-width="1.2" d="M9.1 1.6V5.1h3.6"/>
        {#if kind === "json"}
          <path fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" d="M6.3 7.2c-.8.5-.8 1.1 0 1.6.8.5.8 1.1 0 1.6M9.7 7.2c.8.5.8 1.1 0 1.6-.8.5-.8 1.1 0 1.6"/>
        {:else if kind === "markup"}
          <path fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" d="M6.4 7.3 4.8 8.8l1.6 1.5M9.6 7.3l1.6 1.5-1.6 1.5M8.6 7.1 7.4 10.6"/>
        {:else}
          <path fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" d="M5.4 7.4h5.2M5.4 9.3h5.2M5.4 11.2h3.2"/>
        {/if}
      {/if}
    </svg>
  {/snippet}

{#snippet treeLine(dir: boolean, open: boolean, path: string, letter: string)}
    {#if dir}
      {@render disclosure(open)}
      {@render folderIcon()}
    {:else}
      <span class="twist"></span>
      {#if letter}{@render statusBadge(letter)}{/if}
      {@render typeIcon(path)}
    {/if}
    <span class="name">{shell.fileName(path)}</span>
  {/snippet}

{#snippet detailTree(entries: TreeEntry[], collapsed: string[], onDir: (path: string) => void, onFile: (path: string) => void)}
    {#each entries as entry (entry.key)}
      {#if entry.kind === "dir"}
        <button class="tree-row" type="button" style:padding-left="{8 + entry.depth * 16}px" onclick={() => onDir(entry.path)}>
          {@render treeLine(true, !collapsed.includes(entry.path), entry.path, "")}
        </button>
      {:else if entry.file}
        <button class="tree-row" class:selected={shell.historyFile === entry.file.path} type="button" style:padding-left="{8 + entry.depth * 16}px" onclick={() => onFile(entry.file?.path ?? "")} oncontextmenu={(event) => shell.openMenu(event, shell.revisionFileMenu(entry.file?.path ?? ""))}>
          {@render treeLine(false, false, entry.file.path, entry.file.letter)}
        </button>
      {/if}
    {/each}
  {/snippet}

{#snippet splitCell(row: SplitRow, side: "left" | "right", file: string, key: string, lineIndex: number | null = null)}
    {@const kind = side === "left" ? row.leftKind : row.rightKind}
    {@const mark = side === "left" ? "delete" : "add"}
    {@const text = side === "left" ? row.left : row.right}
    {@const other = side === "left" ? row.right : row.left}
    {@const paired = Boolean(row.left && row.right && row.left !== row.right && (row.leftKind === "delete" || row.rightKind === "add"))}
    {@const picked = lineIndex != null && shell.diffPicked.has(lineIndex)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span class="split-no" class:del={kind === "delete"} class:add={kind === "add"} class:meta={kind === "meta"} class:picked data-line={lineIndex ?? undefined} onmousedown={(event) => lineIndex != null && shell.dragDiffLine(lineIndex, event)} oncontextmenu={(event) => lineIndex != null && shell.openMenu(event, shell.diffLineMenu(lineIndex))}>{(side === "left" ? row.leftNo : row.rightNo) ?? ""}</span>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span class="split-code" data-side={side} data-line={lineIndex ?? undefined} class:del={kind === "delete"} class:add={kind === "add"} class:meta={kind === "meta"} class:picked onclick={(event) => lineIndex != null && shell.clickDiffLine(lineIndex, event)} oncontextmenu={(event) => lineIndex != null && shell.openMenu(event, shell.diffLineMenu(lineIndex))}><span class="split-text">{#if shell.settings.showDiffMarks && kind === mark}{side === "left" ? "-" : "+"}{/if}{#each shell.markedLine(text, paired ? other : "", file) as token, tokenIndex (`${key}-${tokenIndex}`)}<span class={token.cls} class:intra={token.hot}>{token.text}</span>{/each}</span></span>
  {/snippet}

{#snippet splitBar(actions = false)}
    <span class="split-bar-pad"></span>
    <div class="split-bar" data-bar="left"><div></div></div>
    <span class="split-bar-pad"></span>
    <div class="split-bar" data-bar="right"><div></div></div>
    {#if actions}<span class="split-bar-pad"></span>{/if}
  {/snippet}

{#snippet splitPatch(rows: SplitRow[], file: string, columns: { left: number; right: number })}
    {@const prefix = shell.settings.diffWordWrap ? wrapPrefix(rows) : null}
    {@const view = shell.patchView[file] ?? { start: 0, end: 0 }}
    {@const before = prefix ? (prefix[view.start] ?? 0) * shell.splitRowHeight : view.start * shell.splitRowHeight}
    {@const after = prefix ? ((prefix[rows.length] ?? 0) - (prefix[view.end] ?? 0)) * shell.splitRowHeight : Math.max(0, rows.length - view.end) * shell.splitRowHeight}
    <div class="diff-body split pick-{shell.splitPick}" class:virtual={!shell.settings.diffWordWrap} class:select-all={shell.textFile === file} data-file={file} data-select-side={shell.textFile === file ? shell.textSide : undefined} style:--split-row="{shell.splitRowHeight}px" role="presentation" onmousedown={shell.pickSplitSide.bind(shell)} use:splitScroller={columns} use:patchWindow={{ file, count: rows.length, rowHeight: shell.splitRowHeight, prefix }}>
      {#if before > 0}<div class="split-gap" style:height="{before}px"></div>{/if}
      {#each rows.slice(view.start, view.end) as row, offset (`${file}-s-${view.start + offset}`)}
        {#if row.meta}
          <div class="split-meta" data-line={row.lineIndex}>{row.left}</div>
        {:else}
          <div class="split-row">
            {@render splitCell(row, "left", file, `${file}-sl-${view.start + offset}`)}
            {@render splitCell(row, "right", file, `${file}-sr-${view.start + offset}`)}
          </div>
        {/if}
      {/each}
      {#if after > 0}<div class="split-gap" style:height="{after}px"></div>{/if}
      {@render splitBar()}
    </div>
  {/snippet}

{#snippet filePatch(file: string, embedded: boolean)}
    {#if shell.mode === "live"}
      {@const live = shell.patchOf(file)}
      {#if !live}
        <p class="diff-empty slim">{shell.tr("chrome.reading")}</p>
      {:else if live.binary}
        <BinaryDiff path={file} oldBytes={live.oldBytes} newBytes={live.newBytes} compact />
      {:else if live.lines.length === 0}
        <p class="diff-empty slim">{shell.tr("chrome.noChanges")}</p>
      {:else if shell.settings.diffStyle === "split"}
        {@const packed = shell.cachedSplit(file, live.lines)}
        {@render splitPatch(packed.rows, file, packed.columns)}
        {#if live.truncated}<p class="meta patch-note">{shell.tr("chrome.truncated")}</p>{/if}
      {:else}
        {@const prefix = shell.settings.diffWordWrap ? wrapPrefix(live.lines) : null}
        {@const view = shell.patchView[`${file}:uni`] ?? { start: 0, end: 0 }}
        {@const diffRow = shell.diffRowHeight}
        <div class="diff-body" class:wrap-window={shell.settings.diffWordWrap} class:flow={embedded} class:select-all={shell.textFile === file} data-file={file} data-select-side={shell.textFile === file ? shell.textSide : undefined}>
          <div class="commit-window" class:wrap-window={shell.settings.diffWordWrap} style:height={prefix ? `${(prefix[live.lines.length] ?? 0) * diffRow}px` : `${live.lines.length * diffRow}px`} use:patchWindow={{ file: `${file}:uni`, count: live.lines.length, rowHeight: diffRow, prefix }}>
            {#each live.lines.slice(view.start, view.end) as line, offset (`uni-${file}-${view.start + offset}`)}
              {@const index = view.start + offset}
              <span class="virtual-row {line.kind}" class:add={line.kind === "add"} class:del={line.kind === "delete"} class:hunk={line.kind === "hunk"} class:meta={line.kind === "meta"} style:top="{(prefix ? (prefix[index] ?? 0) : index) * diffRow}px">{#each shell.paint(line.text, file) as token, tokenIndex (`ut-${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}</span>
            {/each}
          </div>
        </div>
      {/if}
    {:else}
      {@const lines = diffs[file] ?? []}
      {#if lines.length === 0}
        <p class="diff-empty slim">{shell.tr("chrome.noChanges")}</p>
      {:else if shell.settings.diffStyle === "split"}
        {@const rows = shell.sampleSplitOf(lines)}
        {@render splitPatch(rows, file, shell.splitColumnsOf(rows))}
      {:else}
        <pre class="diff-body flow" class:select-all={shell.textFile === file} data-file={file} data-select-side={shell.textFile === file ? shell.textSide : undefined}>{#each lines as line, index (`sample-${file}-${index}`)}<span class:add={line.startsWith("+") && !line.startsWith("+++")} class:del={line.startsWith("-") && !line.startsWith("---")} class:hunk={line.startsWith("@@")}>{line + "\n"}</span>{/each}</pre>
      {/if}
    {/if}
  {/snippet}
