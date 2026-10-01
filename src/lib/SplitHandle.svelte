<script lang="ts">
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
    onchange?: (size: number) => void;
  } = $props();

  let dragging = $state(false);

  function handlePointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    dragging = true;

    const startPos = direction === "vertical" ? e.clientX : e.clientY;
    const startSize = collapsed ? 0 : size;

    function handlePointerMove(moveEvent: PointerEvent) {
      const currentPos = direction === "vertical" ? moveEvent.clientX : moveEvent.clientY;
      const delta = reverse ? startPos - currentPos : currentPos - startPos;
      let newSize = startSize + delta;

      if (collapsible && newSize < min * 0.6) {
        collapsed = true;
        size = 0;
        onchange?.(0);
        return;
      }

      if (collapsed) {
        collapsed = false;
      }

      newSize = Math.max(min, Math.min(max, newSize));
      size = newSize;
      onchange?.(newSize);
    }

    function handlePointerUp() {
      dragging = false;
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      document.body.style.removeProperty("cursor");
      document.body.style.removeProperty("user-select");
    }

    document.body.style.cursor = direction === "vertical" ? "col-resize" : "row-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
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

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="split-handle {direction}"
  class:dragging
  class:is-collapsed={collapsed}
  onpointerdown={handlePointerDown}
  ondblclick={handleDoubleClick}
  role="separator"
  aria-orientation={direction}
  aria-valuenow={collapsed ? 0 : size}
  tabindex="-1"
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
  .split-handle.dragging .line {
    background: var(--accent);
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
