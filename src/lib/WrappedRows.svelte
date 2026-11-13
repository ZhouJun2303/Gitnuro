<script lang="ts" generics="T extends { text?: string; left?: string; right?: string }">
  import { onMount, tick, untrack, type Snippet } from "svelte";
  import { wrapPrefix } from "./view";
  import { RowHeights } from "./rowHeights";

  let { items, rowHeight, split = false, children }: {
    items: T[]; rowHeight: number; split?: boolean; children: Snippet<[T, number]>;
  } = $props();
  let root: HTMLDivElement;
  let scroller: HTMLElement | undefined;
  let observer: ResizeObserver | undefined;
  let frame = 0;
  let disposed = false;
  let atBottom = false;
  let offset = $state(0);
  let viewport = $state(600);
  let width = $state(0);
  let revision = $state(0);
  const mounted = new Map<HTMLElement, number>();
  const heights = $derived.by(() => {
    void width;
    return new RowHeights(wrapPrefix(items), rowHeight);
  });
  const view = $derived.by(() => {
    void revision;
    const start = Math.max(0, heights.at(Math.max(0, offset - 300)) - 2);
    const end = Math.min(items.length, heights.at(offset + viewport + 300) + 3);
    return { start, end, before: heights.top(start), after: Math.max(0, heights.total - heights.top(end)), total: heights.total };
  });

  function measure() {
    cancelAnimationFrame(frame);
    frame = 0;
    if (!scroller || !root.isConnected || disposed) return;
    const bounds = root.getBoundingClientRect();
    const scrollBounds = scroller.getBoundingClientRect();
    viewport = scroller.clientHeight;
    offset = Math.max(0, scrollBounds.top - bounds.top);
    if (width !== bounds.width) width = bounds.width;
    const anchor = heights.at(offset);
    const previous = heights.top(anchor);
    let changed = false;
    for (const [node, index] of mounted) changed = heights.set(index, node.getBoundingClientRect().height) || changed;
    if (changed) {
      const shift = heights.top(anchor) - previous;
      // Keep the same visible row when estimates above it become measured heights.
      const top = scroller.scrollTop;
      const bottom = atBottom;
      if (offset > 0) offset += shift;
      revision++;
      // New spacer heights must reach the DOM before moving beyond the old scroll limit.
      void tick().then(() => {
        if (disposed || !scroller) return;
        scroller.scrollTop = bottom ? scroller.scrollHeight : top + shift;
        schedule();
      });
    }
  }

  function schedule() {
    if (!frame && !disposed) frame = requestAnimationFrame(measure);
  }

  function scrolled() {
    if (scroller) atBottom = scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2;
    schedule();
  }

  function row(node: HTMLElement, index: number) {
    mounted.set(node, index);
    observer?.observe(node);
    schedule();
    return { destroy() { mounted.delete(node); observer?.unobserve(node); } };
  }

  $effect(() => { heights; untrack(schedule); });

  onMount(() => {
    scroller = root.closest<HTMLElement>(".detail-scroll") ?? undefined;
    for (let parent = root.parentElement; !scroller && parent; parent = parent.parentElement) {
      if (/auto|scroll/.test(getComputedStyle(parent).overflowY)) scroller = parent;
    }
    if (!scroller) return;
    observer = new ResizeObserver(schedule);
    observer.observe(root); observer.observe(scroller);
    for (const node of mounted.keys()) observer.observe(node);
    scroller.addEventListener("scroll", scrolled, { passive: true });
    measure();
    return () => {
      disposed = true;
      cancelAnimationFrame(frame);
      observer?.disconnect();
      scroller?.removeEventListener("scroll", scrolled);
    };
  });

  export async function scrollTo(index: number) {
    if (!scroller || index < 0 || index >= items.length) return;
    atBottom = false;
    scroller.scrollTop += root.getBoundingClientRect().top - scroller.getBoundingClientRect().top + heights.top(index) - 8;
    measure();
    await tick();
    if (disposed) return;
    measure();
    await tick();
    const node = [...mounted].find(([, at]) => at === index)?.[0];
    if (node && scroller) scroller.scrollTop += node.getBoundingClientRect().top - scroller.getBoundingClientRect().top - 8;
  }
</script>

<div class="wrapped-rows" style:min-height="{view.total}px" bind:this={root}>
  {#if view.before > 0}<div style:height="{view.before}px"></div>{/if}
  {#each items.slice(view.start, view.end) as item, offset (view.start + offset)}
    <div class="wrapped-row" class:split data-wrapped-row={view.start + offset} use:row={view.start + offset}>
      {@render children(item, view.start + offset)}
    </div>
  {/each}
  {#if view.after > 0}<div style:height="{view.after}px"></div>{/if}
</div>

<style>
  .wrapped-rows { grid-column: 1 / -1; min-width: 0; width: 100%; overflow-anchor: none; }
  .wrapped-row { min-width: 0; }
  .wrapped-row.split { display: grid; grid-template-columns: var(--split-no) minmax(0, 1fr) var(--split-no) minmax(0, 1fr); }
  .wrapped-row :global(.virtual-row), .wrapped-row :global(.split-row > span), .wrapped-row :global(.split-meta) {
    content-visibility: visible;
  }
</style>
