<script lang="ts">
  import { getShell } from "./context";
  import "./DiffPane.css";
  import { sampleImages } from "../sample";
  import { onVirtualScroll, type SplitRow } from "../view";
  import type { Row, Side } from "./types";
  import { applyConflictChoice, mergeToolLabel, openConflictTool } from "./diff.svelte";
  import type { ConflictParty } from "../status";

  const shell = getShell();
  const splitScroller = (node: HTMLElement, columns?: { left: number; right: number }) => shell.splitScroller(node, columns);
  const onDiffScroll = onVirtualScroll((top) => (shell.diffTop = top));
  const onBlameScroll = onVirtualScroll((top) => (shell.blameTop = top));
  let pickOurs = $state(true);
  let pickTheirs = $state(true);
  let pickedPath = "";

  $effect(() => {
    const path = shell.selectedPath ?? "";
    if (path === pickedPath) return;
    pickedPath = path;
    pickOurs = true;
    pickTheirs = true;
  });

  const sideText = (party: ConflictParty) => {
    if (party.state === "added") return shell.tr("chrome.sideAdded");
    if (party.state === "deleted") return shell.tr("chrome.sideDeleted");
    if (party.state === "modified") return shell.tr("chrome.sideModified");
    return "";
  };
</script>

<section class="diff flex-fill">
          {#if shell.blameLines}
            <header class="diff-head">
              <span>{shell.tr("chrome.blameOf", { name: shell.selectedPath ?? "" })}</span>
              <button class="text-button" type="button" onclick={() => (shell.blameLines = null)}>{shell.tr("chrome.close")}</button>
            </header>
            <div class="diff-body" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} onscroll={onBlameScroll}>
              <div class="commit-window" style:height="{(shell.blameLines?.length ?? 0) * 18}px">
                {#each shell.blameWindow.rows as line, index (`${line.line}-${line.id}`)}
                  <button class="virtual-row" type="button" style:top="{(shell.blameWindow.start + index) * 18}px" title={line.summary} onclick={() => { shell.section = "history"; void shell.selectCommit(line.id); }}>{line.line} {line.shortId} {line.author} {line.summary} {line.text}</button>
                {/each}
              </div>
            </div>
          {:else if (shell.mode === "live" || shell.mode === "sample") && shell.section === "changes" && shell.selectedPath && shell.selectedConflict}
            <header class="diff-head">
              <span>{shell.selectedPath}</span>
            </header>
            {#if shell.activeConflict}
              {@const view = shell.activeConflict}
              {@const tool = shell.mode === "sample" ? (view.binary ? "ExcelMergeFork" : "") : mergeToolLabel(shell)}
              {@const only = pickTheirs !== pickOurs ? (pickTheirs ? view.theirs.label : view.ours.label) : ""}
              <div class="conflict-pane">
                <div class="conflict-title">
                  <svg class="conflict-warn" viewBox="0 0 16 16" aria-hidden="true"><path fill="currentColor" d="M8 1.4 14.8 13.4H1.2L8 1.4z"/><path d="M8 6.1v3.3" stroke="#fff" stroke-width="1.4" stroke-linecap="round"/><circle cx="8" cy="11.4" r=".75" fill="#fff"/></svg>
                  <strong>{shell.tr("chrome.mergeConflict")}</strong>
                </div>
                <p class="conflict-hint">{shell.tr("chrome.mergeConflictHint")}</p>
                <div class="conflict-file">
                  <svg viewBox="0 0 16 16" aria-hidden="true"><path fill="none" stroke="currentColor" stroke-width="1.2" d="M3.2 1.5h6L12.8 5v9.5H3.2z"/><path fill="none" stroke="currentColor" stroke-width="1.2" d="M9.1 1.6V5.1h3.6"/></svg>
                  <span>{shell.fileName(shell.selectedPath)}</span>
                </div>
                <div class="conflict-cluster">
                <div class="conflict-row" class:pick-one={pickTheirs !== pickOurs}>
                  <div class="conflict-end left">{@render conflictCard(view.theirs, "theirs")}</div>
                  <div class="conflict-bridge">
                    <input id="conflict-theirs" type="checkbox" bind:checked={pickTheirs} aria-label={view.theirs.label} />
                    <span class="conflict-link" aria-hidden="true"></span>
                    <input id="conflict-ours" type="checkbox" bind:checked={pickOurs} aria-label={view.ours.label} />
                  </div>
                  <div class="conflict-end right">{@render conflictCard(view.ours, "ours")}</div>
                </div>
                <div class="conflict-actions">
                  <button class="conflict-merge" type="button" disabled={(!pickOurs && !pickTheirs) || (shell.mode === "live" && shell.busy)} onclick={() => void applyConflictChoice(shell, pickOurs, pickTheirs)}>{only ? shell.tr("chrome.chooseSide", { name: only }) : shell.tr("chrome.mergeAction")}</button>
                  {#if tool && pickOurs && pickTheirs}
                    <button class="conflict-tool" type="button" disabled={shell.mode === "live" && shell.busy} onclick={() => { if (shell.mode === "live") void openConflictTool(shell); }}>
                      {shell.tr("chrome.mergeInTool", { name: tool })}
                      <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4.5 6.25 8 9.75l3.5-3.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
                    </button>
                  {/if}
                  {#if shell.mode === "live" && tool && !shell.settings.mergeToolPath.trim()}
                    <p class="conflict-note">{shell.tr("chrome.toolPathMissing", { name: tool })}</p>
                  {/if}
                </div>
                </div>
              </div>
            {:else if shell.diffError}
              <p class="diff-empty">{shell.diffError}</p>
            {:else}
              <p class="diff-empty">{shell.tr("chrome.reading")}</p>
            {/if}
          {:else if shell.mode === "live" && shell.section === "changes" && shell.selectedPath}
            <header class="diff-head">
              <span>{shell.selectedPath}</span>
              {#if shell.discardableSelected.length > 1}
                <button class="pane-chip danger" type="button" disabled={shell.busy} onclick={() => shell.ask(shell.tr("menu.discardLines"), shell.tr("dialog.discardLinesBody", { count: String(shell.discardableSelected.length) }), () => void shell.applyLineOps("discardLine", shell.discardableSelected))}>{shell.tr("menu.discardLines")}</button>
              {/if}
            </header>
            {#if shell.diff && shell.diff.lines.length > 0}
              <div class="minimap">
                {#each shell.minimapMarks as mark (`m-${mark.index}`)}
                  <button class="mark {mark.kind}" type="button" aria-label={shell.tr("menu.nextHunk")} style:top="{mark.top}%" onclick={() => { const node = document.querySelector<HTMLElement>(".diff-body"); if (node) node.scrollTop = mark.index * 18; shell.diffTop = mark.index * 18; }}></button>
                {/each}
              </div>
            {/if}
            {#if shell.diffError}
              <p class="diff-empty">{shell.diffError}</p>
            {:else if !shell.diff}
              <p class="diff-empty">{shell.tr("chrome.reading")}</p>
            {:else if shell.preview?.mime === "application/pdf"}
              <embed class="pdf-view" title={shell.selectedPath} src={shell.preview.dataUrl} type="application/pdf" />
            {:else if shell.diff.binary}
              {#if shell.imageOld || shell.imageNew}
                {@render imageCompare(shell.imageOld, shell.imageNew)}
              {:else}
                <p class="diff-empty">{shell.tr("chrome.binary")}</p>
              {/if}
            {:else if shell.diff.lines.length === 0}
              <p class="diff-empty">{shell.tr("chrome.noChanges")}</p>
            {:else if shell.settings.diffStyle === "split"}
              <div class="diff-body split pick-{shell.splitPick}" class:virtual={!shell.settings.diffWordWrap} class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} style:--split-row="{shell.splitRowHeight}px" role="presentation" bind:this={shell.diffEl} bind:clientHeight={shell.diffHeight} onmousedown={shell.pickSplitSide.bind(shell)} onscroll={onDiffScroll} use:splitScroller={shell.splitColumns}>
                {#if shell.splitWindow.start > 0}<div class="split-gap" style:height="{shell.splitWindow.start * shell.splitRowHeight}px"></div>{/if}
                {#each shell.splitWindow.rows as row, offset (shell.splitWindow.start + offset)}
                  {@const index = shell.splitWindow.start + offset}
                  <div class="split-row">
                    {@render splitCell(row, "left", shell.selectedPath ?? "", `l${index}`, shell.splitLineAt(row, "left"))}
                    {@render splitCell(row, "right", shell.selectedPath ?? "", `r${index}`, shell.splitLineAt(row, "right"))}
                  </div>
                {/each}
                {#if shell.splitWindow.after > 0}<div class="split-gap" style:height="{shell.splitWindow.after * shell.splitRowHeight}px"></div>{/if}
                {@render splitBar()}
              </div>
            {:else}
              <div class="diff-body" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} bind:this={shell.diffEl} onscroll={onDiffScroll}>
                <div class="commit-window" style:height="{shell.diffLines.length * 18}px">
                  {#each shell.diffWindow.rows as line, index (`${shell.diffWindow.start}-${index}`)}
                    {@const at = shell.diffWindow.start + index}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <span class="virtual-row {line.kind}" class:add={line.kind === "add"} class:del={line.kind === "delete"} class:hunk={line.kind === "hunk"} class:meta={line.kind === "meta"} class:picked={shell.diffPicked.has(at)} data-line={at} style:top="{at * 18}px" onclick={(event) => shell.clickDiffLine(at, event)} oncontextmenu={(event) => shell.openMenu(event, shell.diffLineMenu(at))}>
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <span class="gutter" onmousedown={(event) => shell.dragDiffLine(at, event)}>{shell.shownDiffLines[at]?.oldNo ?? ""}</span>
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <span class="gutter" onmousedown={(event) => shell.dragDiffLine(at, event)}>{shell.shownDiffLines[at]?.newNo ?? ""}</span>
                      {#each shell.paint(line.text, shell.selectedPath ?? "") as token, tokenIndex (`${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}
                    </span>
                  {/each}
                </div>
                {#if shell.diff.truncated}<span class="meta">{shell.tr("chrome.truncated")}</span>{/if}
              </div>
            {/if}
          {:else if shell.section === "changes" && shell.selectedPath && shell.selectedDiff.length > 0}
            <header class="diff-head">
              <span>{shell.selectedPath}</span>
            </header>
            {#if shell.settings.diffStyle === "split"}
              <div class="diff-body split pick-{shell.splitPick}" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} role="presentation" onmousedown={shell.pickSplitSide.bind(shell)} use:splitScroller={shell.splitColumnsOf(shell.sampleSplit)}>
                {#each shell.sampleSplit as row, index (index)}
                  <div class="split-row">
                    {@render splitCell(row, "left", shell.selectedPath ?? "", `sl${index}`)}
                    {@render splitCell(row, "right", shell.selectedPath ?? "", `sr${index}`)}
                  </div>
                {/each}
                {@render splitBar()}
              </div>
            {:else}
              <pre class="diff-body" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined}>{#each shell.selectedDiff as line, index (index)}<span class:add={line.startsWith("+") && !line.startsWith("+++")} class:del={line.startsWith("-") && !line.startsWith("---")} class:hunk={line.startsWith("@@")}>{line + "\n"}</span>{/each}</pre>
            {/if}
          {:else if shell.selectedPath === "art/mark.png"}
            {@render imageCompare(sampleImages.before, sampleImages.after)}
          {:else}
            <p class="diff-empty">{shell.tr("chrome.selectFile")}</p>
          {/if}
        </section>
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

{#snippet conflictCard(party: ConflictParty, side: "ours" | "theirs")}
  {@const on = side === "ours" ? pickOurs : pickTheirs}
  <label class="conflict-card" class:on for="conflict-{side}">
    <svg class="branch" viewBox="0 0 16 16" aria-hidden="true"><path fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" d="M4 2.2v7.2a2.2 2.2 0 1 0 2.2 2.2"/><circle cx="4" cy="12.2" r="1.35"/><circle cx="12" cy="3.6" r="1.35"/><path fill="none" stroke="currentColor" stroke-width="1.3" d="M12 5a4.2 4.2 0 0 1-6 2.6"/></svg>
    <span class="name" title={party.label}>{party.label}</span>
    {#if sideText(party)}<span class="conflict-pill {party.state}">{sideText(party)}</span>{/if}
  </label>
{/snippet}

{#snippet imageCompare(before: string | null, after: string | null)}
    <div class="image-toolbar">
      <button type="button" class:on={shell.imageMode === "side"} onclick={() => (shell.imageMode = "side")}>{shell.tr("dialog.imageSide")}</button>
      <button type="button" class:on={shell.imageMode === "swipe"} onclick={() => (shell.imageMode = "swipe")}>{shell.tr("dialog.imageSwipe")}</button>
      <button type="button" class:on={shell.imageMode === "onion"} onclick={() => (shell.imageMode = "onion")}>{shell.tr("dialog.imageOnion")}</button>
      <button type="button" class:on={shell.imageMode === "pixel"} onclick={() => { shell.imageMode = "pixel"; if (before && after) void shell.paintPixel(before, after); }}>{shell.tr("dialog.imagePixel")}</button>
      {#if shell.imageMode === "swipe" || shell.imageMode === "onion"}
        <input type="range" min="0" max="100" bind:value={shell.imagePos} aria-label={shell.imageMode} />
      {/if}
    </div>
    <div class="image-stage" class:pair={shell.imageMode === "side"}>
      {#if shell.imageMode === "pixel" && shell.pixelUrl}
        <figure class="image-frame"><div class="image-box"><img alt="" src={shell.pixelUrl} onload={shell.fitImage.bind(shell)} /></div></figure>
      {:else if shell.imageMode === "side"}
        <figure class="image-frame">
          <figcaption class="del">{shell.tr("dialog.imageBefore")}</figcaption>
          <div class="image-box">{#if before}<img alt="" src={before} onload={shell.fitImage.bind(shell)} />{:else}<span class="meta">—</span>{/if}</div>
        </figure>
        <figure class="image-frame">
          <figcaption class="add">{shell.tr("dialog.imageAfter")}</figcaption>
          <div class="image-box">{#if after}<img alt="" src={after} onload={shell.fitImage.bind(shell)} />{:else}<span class="meta">—</span>{/if}</div>
        </figure>
      {:else}
        <figure class="image-frame">
          <div class="image-box">
            <div class="image-overlay">
              {#if before}<img alt="" src={before} onload={shell.fitImage.bind(shell)} />{/if}
              {#if after}<img class="over" alt="" src={after} style:opacity={shell.imageMode === "onion" ? shell.imagePos / 100 : 1} style:clip-path={shell.imageMode === "swipe" ? `inset(0 ${100 - shell.imagePos}% 0 0)` : "none"} />{/if}
            </div>
          </div>
        </figure>
      {/if}
    </div>
  {/snippet}
