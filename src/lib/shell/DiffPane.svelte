<script lang="ts">
  import { getShell } from "./context";
  import "./DiffPane.css";
  import { sampleImages } from "../sample";
  import { type SplitRow } from "../view";
  import type { Row, Side } from "./types";

  const shell = getShell();
  const splitScroller = (node: HTMLElement, columns?: { left: number; right: number }) => shell.splitScroller(node, columns);
</script>

<section class="diff flex-fill">
          {#if shell.blameLines}
            <header class="diff-head">
              <span>{shell.tr("chrome.blameOf", { name: shell.selectedPath ?? "" })}</span>
              <button class="text-button" type="button" onclick={() => (shell.blameLines = null)}>{shell.tr("chrome.close")}</button>
            </header>
            <div class="diff-body" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} onscroll={(event) => (shell.blameTop = (event.currentTarget as HTMLElement).scrollTop)}>
              <div class="commit-window" style:height="{(shell.blameLines?.length ?? 0) * 18}px">
                {#each shell.blameWindow.rows as line, index (`${line.line}-${line.id}`)}
                  <button class="virtual-row" type="button" style:top="{(shell.blameWindow.start + index) * 18}px" title={line.summary} onclick={() => { shell.section = "history"; void shell.selectCommit(line.id); }}>{line.line} {line.shortId} {line.author} {line.summary} {line.text}</button>
                {/each}
              </div>
            </div>
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
              <div class="diff-body split virtual pick-{shell.splitPick}" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} style:--split-row="{shell.splitRowHeight}px" role="presentation" bind:this={shell.diffEl} bind:clientHeight={shell.diffHeight} onmousedown={shell.pickSplitSide.bind(shell)} onscroll={(event) => (shell.diffTop = (event.currentTarget as HTMLElement).scrollTop)} use:splitScroller={shell.splitColumns}>
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
              <div class="diff-body" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} bind:this={shell.diffEl} onscroll={(event) => (shell.diffTop = (event.currentTarget as HTMLElement).scrollTop)}>
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
