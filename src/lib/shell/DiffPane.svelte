<script lang="ts">
  import { getShell } from "./context";
  import "./DiffPane.css";
  import { sampleImages } from "../sample";
  import { onVirtualScroll, type SplitRow } from "../view";
  import type { Row, Side } from "./types";
  import { applyConflictChoice, mergeToolLabel, openConflictTool } from "./diff.svelte";
  import type { ConflictParty } from "../status";
  import type { DiffLine } from "../status";

  const shell = getShell();
  const splitScroller = (node: HTMLElement, columns?: { left: number; right: number }) => shell.splitScroller(node, columns);
  const onDiffScroll = onVirtualScroll((top) => (shell.diffTop = top));
  const onBlameScroll = onVirtualScroll((top) => (shell.blameTop = top));
  let pickOurs = $state(true);
  let pickTheirs = $state(true);
  let pickedPath = "";
  let hoveredHunk = $state<number | null>(null);
  let activeHunk = $state<number | null>(null);

  $effect(() => {
    const path = shell.selectedPath ?? "";
    if (path === pickedPath) return;
    pickedPath = path;
    pickOurs = true;
    pickTheirs = true;
    hoveredHunk = null;
    activeHunk = null;
    shell.pixelUrl = "";
    shell.imageMode = "side";
    shell.imagePos = 50;
  });

  const hunkList = $derived(shell.mode === "live" && shell.section === "changes" ? shell.hunkRanges() : []);
  const activeAt = $derived(hunkList.findIndex((range) => range.index === activeHunk));

  $effect(() => {
    if (activeHunk != null && !hunkList.some((range) => range.index === activeHunk)) activeHunk = null;
  });
  const litHunk = $derived(hoveredHunk ?? activeHunk);
  const hoveredRange = $derived(hoveredHunk == null ? null : hunkList.find((range) => range.index === hoveredHunk) ?? null);
  const hoveredSelected = $derived(
    hoveredRange
      ? shell.diffSelected.filter((index) => index >= hoveredRange.start && index < hoveredRange.end)
      : [],
  );
  const hoveredLineOps = $derived.by(() => {
    if (hoveredSelected.length === 0) return [] as { line: DiffLine; index: number }[];
    return hoveredSelected
      .map((index) => ({ line: shell.diffLines[index], index }))
      .filter((entry): entry is { line: DiffLine; index: number } => !!entry.line && (entry.line.kind === "add" || entry.line.kind === "delete") && entry.line.stageAt != null);
  });
  const hoveredDiscardOps = $derived(hoveredSelected.length > 0 ? shell.discardableEntries(hoveredSelected) : []);
  const useLineOps = $derived(hoveredLineOps.length > 0 || hoveredDiscardOps.length > 0);
  const rowHeight = $derived(shell.settings.diffStyle === "split" ? shell.splitRowHeight : shell.diffRowHeight);
  const hunkActionTop = $derived.by(() => {    if (!hoveredRange) return 0;
    if (shell.settings.diffStyle !== "split") return hoveredRange.start * rowHeight;
    for (let at = 0; at < shell.splitRows.length; at += 1) {
      const row = shell.splitRows[at];
      if (!row) continue;
      const left = shell.splitLineAt(row, "left");
      const right = shell.splitLineAt(row, "right");
      if ((left != null && left >= hoveredRange.start && left < hoveredRange.end) || (right != null && right >= hoveredRange.start && right < hoveredRange.end)) {
        return at * rowHeight;
      }
    }
    return hoveredRange.start * rowHeight;
  });

  const sideText = (party: ConflictParty) => {
    if (party.state === "added") return shell.tr("chrome.sideAdded");
    if (party.state === "deleted") return shell.tr("chrome.sideDeleted");
    if (party.state === "modified") return shell.tr("chrome.sideModified");
    return "";
  };

  function hoverDiffLine(event: MouseEvent) {
    const hit = event.target instanceof Element ? event.target.closest<HTMLElement>("[data-line]") : null;
    const line = Number(hit?.dataset.line);
    if (!Number.isInteger(line)) {
      if (!(event.target instanceof Element) || !event.target.closest(".diff-hunk-actions")) hoveredHunk = null;
      return;
    }
    hoveredHunk = shell.hunkOf(line);
  }

  function clearHunkHover() {
    hoveredHunk = null;
  }

  function goMark(index: number) {
    activeHunk = index;
    hoveredHunk = index;
    const node = shell.diffEl;
    if (!node) return;
    if (shell.settings.diffStyle === "split") {
      node.querySelector<HTMLElement>(`[data-line="${index}"]`)?.scrollIntoView({ block: "center" });
      return;
    }
    node.scrollTop = Math.max(0, index * shell.diffRowHeight);
    shell.diffTop = node.scrollTop;
  }

  function scrollToHunk(index: number) {
    const node = shell.diffEl;
    const range = hunkList.find((item) => item.index === index);
    if (!node || !range) return;
    if (shell.settings.diffStyle === "split") {
      const row = shell.splitRows.findIndex((item) => {
        const left = shell.splitLineAt(item, "left");
        const right = shell.splitLineAt(item, "right");
        return (left != null && left >= range.start && left < range.end) || (right != null && right >= range.start && right < range.end);
      });
      node.scrollTop = Math.max(0, (row < 0 ? 0 : row) * shell.splitRowHeight - 8);
    } else {
      node.scrollTop = Math.max(0, range.start * shell.diffRowHeight - 8);
    }
    shell.diffTop = node.scrollTop;
  }

  function selectHunk(index: number) {
    activeHunk = index;
    hoveredHunk = index;
    scrollToHunk(index);
  }

  function stepHunk(delta: number) {
    if (hunkList.length === 0) return;
    const at = activeAt < 0 ? (delta > 0 ? -1 : 0) : activeAt;
    const next = (at + delta + hunkList.length) % hunkList.length;
    const range = hunkList[next];
    if (range) selectHunk(range.index);
  }

  function stageActive() {
    const file = shell.selectedPath;
    if (!file || shell.busy || activeHunk == null) return;
    void shell.mutate({ action: "stageHunk", file, index: activeHunk, unstage: shell.selectedSide === "staged" });
  }

  function discardActive() {
    const file = shell.selectedPath;
    if (!file || shell.busy || activeHunk == null || shell.selectedSide === "staged") return;
    const index = activeHunk;
    shell.ask(shell.tr("menu.discardHunk"), shell.tr("dialog.discardBody", { name: file }), () => {
      void shell.mutate({ action: "discardHunk", file, index });
    });
  }

  function stageHovered() {
    const file = shell.selectedPath;
    if (!file || shell.busy) return;
    const index = hoveredHunk;
    const lineMode = useLineOps;
    const lineOps = hoveredLineOps;
    hoveredHunk = null;
    if (lineMode) {
      if (lineOps.length === 0) return;
      void shell.applyLineOps("stageLine", lineOps);
      return;
    }
    if (index == null) return;
    void shell.mutate({ action: "stageHunk", file, index, unstage: shell.selectedSide === "staged" });
  }

  function discardHovered() {
    const file = shell.selectedPath;
    if (!file || shell.busy || shell.selectedSide === "staged") return;
    const index = hoveredHunk;
    const lineMode = useLineOps;
    const discardOps = hoveredDiscardOps;
    hoveredHunk = null;
    if (lineMode) {
      if (discardOps.length === 0) return;
      if (discardOps.length === 1) {
        void shell.applyLineOps("discardLine", discardOps);
        return;
      }
      shell.ask(shell.tr("menu.discardLines"), shell.tr("dialog.discardLinesBody", { count: String(discardOps.length) }), () => void shell.applyLineOps("discardLine", discardOps));
      return;
    }
    if (index == null) return;
    shell.ask(shell.tr("menu.discardHunk"), shell.tr("dialog.discardBody", { name: file }), () => {
      void shell.mutate({ action: "discardHunk", file, index });
    });
  }

  function stageLabel() {
    if (useLineOps) {
      const many = hoveredLineOps.length > 1;
      return shell.selectedSide === "staged"
        ? shell.tr(many ? "menu.unstageLines" : "menu.unstageLine")
        : shell.tr(many ? "menu.stageLines" : "menu.stageLine");
    }
    return shell.selectedSide === "staged" ? shell.tr("menu.unstageHunk") : shell.tr("menu.stage");
  }

  function discardLabel() {
    if (useLineOps) return shell.tr(hoveredDiscardOps.length > 1 ? "menu.discardLines" : "menu.discardLine");
    return shell.tr("menu.discard");
  }
</script>

<section class="diff flex-fill">
          {#if shell.blameLines}
            <header class="diff-head" data-layer="blame">
              <span>{shell.tr("chrome.blameOf", { name: shell.selectedPath ?? "" })}</span>
              <button class="text-button" type="button" onclick={() => (shell.blameLines = null)}>{shell.tr("chrome.close")}</button>
            </header>
            <div class="diff-body" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} onscroll={onBlameScroll}>
              <div class="commit-window" style:height="{(shell.blameLines?.length ?? 0) * shell.diffRowHeight}px">
                {#each shell.blameWindow.rows as line, index (`${line.line}-${line.id}`)}
                  {@const pending = !line.id || /^0+$/.test(line.id)}
                  <button class="virtual-row blame-line" class:blame-pending={pending} type="button" style:top="{(shell.blameWindow.start + index) * shell.diffRowHeight}px" title={pending ? shell.tr("dialog.notCommitted") : `${line.author} · ${line.summary}`} onclick={() => { if (pending) return; shell.section = "history"; void shell.selectCommit(line.id); }}>
                    <span class="blame-gutter">{line.line}</span>
                    <span class="blame-who">{pending ? shell.tr("dialog.notCommitted") : line.author}</span>
                    <span class="blame-code">{line.text}</span>
                  </button>
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
                    <input id="conflict-theirs" type="checkbox" bind:checked={pickTheirs} aria-label={shell.tr("dialog.mergeKeepSide", { name: view.theirs.label })} title={shell.tr("dialog.mergeKeepSide", { name: view.theirs.label })} />
                    <span class="conflict-link" aria-hidden="true"></span>
                    <input id="conflict-ours" type="checkbox" bind:checked={pickOurs} aria-label={shell.tr("dialog.mergeKeepSide", { name: view.ours.label })} title={shell.tr("dialog.mergeKeepSide", { name: view.ours.label })} />
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
              <span title={shell.selectedPath}>{shell.selectedPath}</span>
              {#if hunkList.length > 0}
                <div class="hunk-tools" role="group" aria-label={shell.tr("chrome.hunkActions")}>
                  <button class="hunk-step" type="button" disabled={shell.busy} aria-label={shell.tr("menu.prevHunk")} title={shell.tr("menu.prevHunk")} onclick={() => stepHunk(-1)}>‹</button>
                  <span class="hunk-pos" aria-live="polite">{activeAt >= 0 ? activeAt + 1 : "–"}/{hunkList.length}</span>
                  <button class="hunk-step" type="button" disabled={shell.busy} aria-label={shell.tr("menu.nextHunk")} title={shell.tr("menu.nextHunk")} onclick={() => stepHunk(1)}>›</button>
                  <button class="pane-chip" type="button" disabled={shell.busy || activeHunk == null} title={shell.tr(shell.selectedSide === "staged" ? "menu.unstageHunk" : "menu.stageHunk")} onclick={stageActive}>{shell.tr(shell.selectedSide === "staged" ? "menu.unstageHunk" : "menu.stageHunk")}</button>
                  {#if shell.selectedSide === "unstaged"}
                    <button class="pane-chip danger" type="button" disabled={shell.busy || activeHunk == null} title={shell.tr("menu.discardHunk")} onclick={discardActive}>{shell.tr("menu.discardHunk")}</button>
                  {/if}
                </div>
              {/if}
              {#if shell.discardableSelected.length > 1}
                <button class="pane-chip danger" type="button" disabled={shell.busy} onclick={() => shell.ask(shell.tr("menu.discardLines"), shell.tr("dialog.discardLinesBody", { count: String(shell.discardableSelected.length) }), () => void shell.applyLineOps("discardLine", shell.discardableSelected))}>{shell.tr("menu.discardLines")}</button>
              {/if}
            </header>
            {#if shell.diff && shell.diff.lines.length > 0}
              <div class="minimap" role="presentation">
                {#each shell.minimapMarks as mark (`m-${mark.index}`)}
                  <button class="mark {mark.kind}" type="button" aria-label={`${shell.tr("menu.nextHunk")} ${mark.index + 1}`} title={`${shell.tr("menu.nextHunk")} ${mark.index + 1}`} style:left="{mark.top}%" onclick={() => goMark(mark.index)}></button>
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
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="diff-body split pick-{shell.splitPick}" class:virtual={!shell.settings.diffWordWrap} class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} style:--split-row="{shell.splitRowHeight}px" role="presentation" bind:this={shell.diffEl} bind:clientHeight={shell.diffHeight} onmousedown={shell.pickSplitSide.bind(shell)} onscroll={(event) => { onDiffScroll(event); hoveredHunk = null; }} onmousemove={hoverDiffLine} onmouseleave={clearHunkHover} use:splitScroller={shell.splitColumns}>
                {#if shell.splitWindow.start > 0}<div class="split-gap" style:height="{shell.splitWindow.start * shell.splitRowHeight}px"></div>{/if}
                {#each shell.splitWindow.rows as row, offset (shell.splitWindow.start + offset)}
                  {@const index = shell.splitWindow.start + offset}
                  {@const leftAt = shell.splitLineAt(row, "left")}
                  {@const rightAt = shell.splitLineAt(row, "right")}
                  {@const rowHunk = row.hunkIndex ?? (leftAt != null ? shell.hunkOf(leftAt) : rightAt != null ? shell.hunkOf(rightAt) : null)}
                  {#if row.meta}
                    <div class="split-meta" class:hunk-hover={litHunk != null && rowHunk === litHunk} data-line={row.lineIndex}>{row.left}</div>
                  {:else}
                    <div class="split-row" class:hunk-hover={litHunk != null && rowHunk === litHunk}>
                      {@render splitCell(row, "left", shell.selectedPath ?? "", `l${index}`, leftAt)}
                      {@render splitCell(row, "right", shell.selectedPath ?? "", `r${index}`, rightAt)}
                    </div>
                  {/if}
                {/each}
                {#if shell.splitWindow.after > 0}<div class="split-gap" style:height="{shell.splitWindow.after * shell.splitRowHeight}px"></div>{/if}
                {@render splitBar()}
                {@render hunkActions(true)}
              </div>
            {:else}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="diff-body" class:select-all={shell.textWhere === "changes"} data-select-side={shell.textWhere === "changes" ? shell.textSide : undefined} bind:this={shell.diffEl} onscroll={(event) => { onDiffScroll(event); hoveredHunk = null; }} onmousemove={hoverDiffLine} onmouseleave={clearHunkHover}>
                <div class="commit-window" style:height="{shell.diffLines.length * shell.diffRowHeight}px">
                  {#each shell.diffWindow.rows as line, index (`${shell.diffWindow.start}-${index}`)}
                    {@const at = shell.diffWindow.start + index}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <span class="virtual-row {line.kind}" class:add={line.kind === "add"} class:del={line.kind === "delete"} class:hunk={line.kind === "hunk"} class:meta={line.kind === "meta"} class:picked={shell.diffPicked.has(at)} class:hunk-hover={litHunk != null && shell.hunkOf(at) === litHunk} data-line={at} style:top="{at * shell.diffRowHeight}px" onclick={(event) => shell.clickDiffLine(at, event)} oncontextmenu={(event) => shell.openMenu(event, shell.diffLineMenu(at))}>
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <span class="gutter" onmousedown={(event) => shell.dragDiffLine(at, event)}>{shell.shownDiffLines[at]?.oldNo ?? ""}</span>
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <span class="gutter" onmousedown={(event) => shell.dragDiffLine(at, event)}>{shell.shownDiffLines[at]?.newNo ?? ""}</span>
                      {#each shell.paint(line.text, shell.selectedPath ?? "") as token, tokenIndex (`${index}-${tokenIndex}`)}<span class={token.cls}>{token.text}</span>{/each}
                    </span>
                  {/each}
                  {@render hunkActions(false)}
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
                  {#if row.meta}
                    <div class="split-meta" data-line={row.lineIndex}>{row.left}</div>
                  {:else}
                    <div class="split-row">
                      {@render splitCell(row, "left", shell.selectedPath ?? "", `sl${index}`)}
                      {@render splitCell(row, "right", shell.selectedPath ?? "", `sr${index}`)}
                    </div>
                  {/if}
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

{#snippet hunkActions(split: boolean)}
  {#if hoveredRange && shell.mode === "live"}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="diff-hunk-actions" class:split-float={split} style:top="{hunkActionTop}px" onmouseenter={() => (hoveredHunk = hoveredRange.index)}>
      {#if !useLineOps || hoveredLineOps.length > 0}
        <button type="button" disabled={shell.busy} onclick={(event) => { event.stopPropagation(); stageHovered(); }}>{stageLabel()}</button>
      {/if}
      {#if shell.selectedSide === "unstaged" && (!useLineOps || hoveredDiscardOps.length > 0)}
        <button class="danger" type="button" disabled={shell.busy} onclick={(event) => { event.stopPropagation(); discardHovered(); }}>{discardLabel()}</button>
      {/if}
    </div>
  {/if}
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
      <button type="button" class:on={shell.imageMode === "side"} aria-pressed={shell.imageMode === "side"} onclick={() => (shell.imageMode = "side")}>{shell.tr("dialog.imageSide")}</button>
      <button type="button" class:on={shell.imageMode === "swipe"} aria-pressed={shell.imageMode === "swipe"} onclick={() => (shell.imageMode = "swipe")}>{shell.tr("dialog.imageSwipe")}</button>
      <button type="button" class:on={shell.imageMode === "onion"} aria-pressed={shell.imageMode === "onion"} onclick={() => (shell.imageMode = "onion")}>{shell.tr("dialog.imageOnion")}</button>
      <button type="button" class:on={shell.imageMode === "pixel"} aria-pressed={shell.imageMode === "pixel"} onclick={() => { shell.imageMode = "pixel"; if (before && after) void shell.paintPixel(before, after); }}>{shell.tr("dialog.imagePixel")}</button>
      {#if shell.imageMode === "swipe" || shell.imageMode === "onion"}
        <input type="range" min="0" max="100" bind:value={shell.imagePos} aria-label={shell.imageMode} />
      {/if}
    </div>
    <div class="image-stage" class:pair={shell.imageMode === "side"}>
      {#if shell.imageMode === "pixel" && shell.pixelUrl}
        <figure class="image-frame"><div class="image-box"><img alt={shell.tr("dialog.imagePixel")} src={shell.pixelUrl} onload={shell.fitImage.bind(shell)} /></div></figure>
      {:else if shell.imageMode === "side"}
        <figure class="image-frame">
          <figcaption class="del">{shell.tr("dialog.imageBefore")}</figcaption>
          <div class="image-box">{#if before}<img alt={shell.tr("dialog.imageBefore")} src={before} onload={shell.fitImage.bind(shell)} />{:else}<span class="meta">—</span>{/if}</div>
        </figure>
        <figure class="image-frame">
          <figcaption class="add">{shell.tr("dialog.imageAfter")}</figcaption>
          <div class="image-box">{#if after}<img alt={shell.tr("dialog.imageAfter")} src={after} onload={shell.fitImage.bind(shell)} />{:else}<span class="meta">—</span>{/if}</div>
        </figure>
      {:else}
        <figure class="image-frame">
          <div class="image-box">
            <div class="image-overlay">
              {#if before}
                <img alt={shell.tr("dialog.imageBefore")} src={before} onload={shell.fitImage.bind(shell)} />
                {#if after}
                  <img class="over" alt={shell.tr("dialog.imageAfter")} src={after} style:opacity={shell.imageMode === "onion" ? shell.imagePos / 100 : 1} style:clip-path={shell.imageMode === "swipe" ? `inset(0 ${100 - shell.imagePos}% 0 0)` : "none"} />
                {/if}
              {:else if after}
                <img alt={shell.tr("dialog.imageAfter")} src={after} onload={shell.fitImage.bind(shell)} />
              {/if}
            </div>
          </div>
        </figure>
      {/if}
    </div>
  {/snippet}
