<script lang="ts">
  import { tick, untrack } from "svelte";
  import { getShell } from "./context";
  import { hunkIncludes } from "./dialogs.svelte";
  import { mergeBlocks, MergeLayout, type MergeSide } from "./mergeLayout";

  let { side, onpick, onscroll }: { side: MergeSide; onpick: (index: number, side: "ours" | "theirs") => void; onscroll?: (event: Event) => void } = $props();
  const shell = getShell();
  let viewport: HTMLDivElement;
  let top = $state(0), height = $state(0), width = $state(0), revision = $state(0);
  let scrollFrame = 0, measureFrame = 0;
  const blocks = $derived(mergeBlocks(side === "result" ? shell.mergePieces : shell.mergeSource, side,
    shell.tr("dialog.mergeEmpty"), shell.tr("dialog.mergeConflictMark"), side === "result" && shell.mergeEdited ? shell.mergeResult : undefined));
  const layout = $derived(new MergeLayout(blocks, width));
  const range = $derived.by(() => {
    revision;
    if (blocks.length <= 80) return { start: 0, end: blocks.length };
    return { start: Math.max(0, layout.at(top) - 4), end: Math.min(blocks.length, layout.at(top + height) + 5) };
  });
  const visible = $derived(blocks.slice(range.start, range.end));
  const before = $derived.by(() => { revision; return layout.offset(range.start); });
  const after = $derived.by(() => { revision; return Math.max(0, layout.height - layout.offset(range.end)); });

  function scrolled(event: Event) {
    onscroll?.(event);
    if (!scrollFrame) scrollFrame = requestAnimationFrame(() => { scrollFrame = 0; top = viewport.scrollTop; });
  }
  function measure(node: HTMLElement, value: { index: number; layout: MergeLayout }) {
    let current = value;
    const observer = new ResizeObserver(() => {
      if (current.layout !== untrack(() => layout)) return;
      if (current.layout.measure(current.index, node.getBoundingClientRect().height) && !measureFrame) {
        measureFrame = requestAnimationFrame(() => { measureFrame = 0; revision++; });
      }
    });
    observer.observe(node);
    return { update(next: typeof value) {
      if (next.layout === current.layout && next.index === current.index) return;
      current = next;
      if (current.layout.measure(current.index, node.getBoundingClientRect().height) && !measureFrame) {
        measureFrame = requestAnimationFrame(() => { measureFrame = 0; revision++; });
      }
    }, destroy() { observer.disconnect(); } };
  }
  export function element() { return viewport; }
  export async function reveal(piece: number) {
    // Selection changes the result's block heights. Read the new layout after Svelte flushes it.
    await tick();
    const index = blocks.findIndex((block) => block.piece === piece);
    if (index < 0) return;
    const start = layout.offset(index), end = layout.offset(index + 1);
    if (start < viewport.scrollTop || end > viewport.scrollTop + viewport.clientHeight) {
      viewport.scrollTop = start;
      top = viewport.scrollTop;
      await tick();
    }
    const node = viewport.querySelector<HTMLElement>(`[data-merge-piece="${piece}"]`);
    if (node) {
      const box = node.getBoundingClientRect(), pane = viewport.getBoundingClientRect();
      if (box.top < pane.top || box.bottom > pane.bottom) {
        viewport.scrollTop += box.top - pane.top;
        top = viewport.scrollTop;
      }
    }
  }
  $effect(() => {
    layout;
    untrack(() => { if (viewport) top = viewport.scrollTop; });
  });
  $effect(() => () => { cancelAnimationFrame(scrollFrame); cancelAnimationFrame(measureFrame); });
</script>

<div class={side === "result" ? "merge-result-scroll" : `merge-side ${side === "theirs" ? "left" : "right"}`}
  bind:this={viewport} bind:clientHeight={height} bind:clientWidth={width} onscroll={scrolled}>
  {#if before > 0}<div class="merge-spacer" style:height="{before}px"></div>{/if}
  {#each visible as block, offset (block.key)}
    {@const piece = shell.mergePieces[block.piece]}
    {@const selected = piece?.kind === "hunk" && side !== "result" && hunkIncludes(piece.pick, side)}
    <div class="merge-block" data-merge-piece={block.piece} use:measure={{ index: range.start + offset, layout }}>
      {#if side !== "result" && block.kind === "hunk"}
        <div class="merge-cell hunk" class:on={selected} data-merge-hunk={block.piece} role="group">
          <pre class:empty={block.empty}>{block.text}</pre>
          <button class="merge-hover-btn" class:remove={selected} type="button" disabled={shell.busy}
            aria-pressed={selected} onclick={() => onpick(block.piece, side === "theirs" ? "theirs" : "ours")}>
            {shell.tr(selected ? side === "theirs" ? "dialog.removeLeft" : "dialog.removeRight" : side === "theirs" ? "dialog.selectLeft" : "dialog.selectRight")}
          </button>
        </div>
      {:else if side !== "result"}
        <pre class="merge-cell context">{block.text}</pre>
      {:else if block.kind === "pending"}
        <div class="merge-conflict-mark" data-merge-result={block.piece}>{block.text}</div>
      {:else}
        <pre class={block.kind === "hunk" && piece?.kind === "hunk" ? `merge-result-hunk pick-${piece.pick}` : "merge-result-text"}
          data-merge-result={block.kind === "hunk" ? block.piece : undefined} class:merge-manual-preview={block.kind === "manual"}>{block.text}</pre>
      {/if}
    </div>
  {/each}
  {#if after > 0}<div class="merge-spacer" style:height="{after}px"></div>{/if}
</div>
