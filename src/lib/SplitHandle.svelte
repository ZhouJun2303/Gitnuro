<script lang="ts">
  import { onDestroy } from "svelte";
  type Direction = "horizontal" | "vertical";

  let {
    direction = "vertical",
    size = $bindable(260),
    min = 160,
    max = 800,
    defaultSize = 260,
    collapsible = false,
    collapsed = $bindable(false),
    reverse = false,
    label,
    onchange,
  }: {
    direction?: Direction;
    size?: number;
    min?: number;
    max?: number;
    defaultSize?: number;
    collapsible?: boolean;
    collapsed?: boolean;
    reverse?: boolean;
    label?: string;
    onchange?: (size: number) => void;
  } = $props();

  let dragging = $state(false);
  let stopDrag: (() => void) | undefined;
  onDestroy(() => stopDrag?.());

  function commit(next: number) {
    if (collapsible && next < min * 0.6) {
      collapsed = true;
      size = 0;
    } else {
      collapsed = false;
      size = Math.max(min, Math.min(max, next));
    }
    onchange?.(collapsed ? 0 : size);
  }

  function handleKey(e: KeyboardEvent) {
    const negative = direction === "vertical" ? "ArrowLeft" : "ArrowUp";
    const positive = direction === "vertical" ? "ArrowRight" : "ArrowDown";
    const step = e.shiftKey ? 32 : 8;
    if (e.key === negative || e.key === positive) {
      e.preventDefault();
      if (collapsed) {
        collapsed = false;
        size = defaultSize;
        onchange?.(size);
        return;
      }
      const delta = (e.key === negative ? -step : step) * (reverse ? -1 : 1);
      commit(size + delta);
      return;
    }
    if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      commit(e.key === "Home" ? min : max);
      return;
    }
    if ((e.key === "Enter" || e.key === " ") && collapsible) {
      e.preventDefault();
      commit(collapsed ? defaultSize : 0);
    }
  }

  function handlePointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    stopDrag?.();
    e.preventDefault();
    dragging = true;
    const startPos = direction === "vertical" ? e.clientX : e.clientY;
    const startSize = collapsed ? 0 : size;
    let current = startSize;
    let pending = startSize;
    let frame = 0;
    const cursor = document.body.style.cursor;
    const selection = document.body.style.userSelect;

    function apply() {
      frame = 0;
      let newSize = pending;
      if (collapsible && newSize < min * 0.6) {
        collapsed = true;
        size = 0;
        current = 0;
        return;
      }
      if (collapsed) collapsed = false;
      newSize = Math.max(min, Math.min(max, newSize));
      size = newSize;
      current = newSize;
    }

    function handlePointerMove(moveEvent: PointerEvent) {
      if (moveEvent.pointerId !== e.pointerId) return;
      const currentPos = direction === "vertical" ? moveEvent.clientX : moveEvent.clientY;
      pending = startSize + (reverse ? startPos - currentPos : currentPos - startPos);
      if (!frame) frame = requestAnimationFrame(apply);
    }

    function cleanup() {
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
      dragging = false;
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      window.removeEventListener("pointercancel", handlePointerUp);
      window.removeEventListener("blur", finish);
      document.body.style.cursor = cursor;
      document.body.style.userSelect = selection;
      stopDrag = undefined;
    }

    function finish() {
      if (frame) {
        cancelAnimationFrame(frame);
        apply();
      }
      cleanup();
      onchange?.(current);
    }

    function handlePointerUp(event: PointerEvent) {
      if (event.pointerId === e.pointerId) finish();
    }

    stopDrag = cleanup;
    document.body.style.cursor = direction === "vertical" ? "col-resize" : "row-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
    window.addEventListener("pointercancel", handlePointerUp);
    window.addEventListener("blur", finish);
  }

  function handleDoubleClick() {
    if (collapsed) {
      collapsed = false;
      size = defaultSize;
    } else if (Math.abs(size - defaultSize) < 5 && collapsible) {
      collapsed = true;
      size = 0;
    } else {
      size = defaultSize;
    }
    onchange?.(size);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="split-handle {direction}"
  class:dragging
  class:is-collapsed={collapsed}
  onpointerdown={handlePointerDown}
  ondblclick={handleDoubleClick}
  onkeydown={handleKey}
  role="separator"
  aria-label={label}
  aria-orientation={direction}
  aria-valuenow={collapsed ? 0 : size}
  aria-valuemin={collapsible ? 0 : min}
  aria-valuemax={max}
  tabindex="0"
>
  <div class="line"></div>
</div>

<style>
  .split-handle {
    position: relative;
    z-index: 10;
    flex: none;
    user-select: none;
    transition: background 150ms ease;
  }

  .split-handle.vertical {
    width: 7px;
    margin: 0 -3px;
    cursor: col-resize;
    display: flex;
    justify-content: center;
    align-items: stretch;
  }

  .split-handle.vertical .line {
    width: 1px;
    height: 100%;
    background: var(--line);
    transition: background 120ms ease, width 120ms ease;
  }

  .split-handle.horizontal {
    height: 7px;
    margin: -3px 0;
    cursor: row-resize;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  .split-handle.horizontal .line {
    height: 1px;
    width: 100%;
    background: var(--line);
    transition: background 120ms ease, height 120ms ease;
  }

  .split-handle:hover .line,
  .split-handle.dragging .line,
  .split-handle:focus-visible .line {
    background: var(--accent);
  }

  .split-handle:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
    border-radius: 3px;
  }

  .split-handle.vertical:hover .line,
  .split-handle.vertical.dragging .line {
    width: 2px;
  }

  .split-handle.horizontal:hover .line,
  .split-handle.horizontal.dragging .line {
    height: 2px;
  }
</style>
