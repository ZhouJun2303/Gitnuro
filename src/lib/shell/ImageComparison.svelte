<script lang="ts">
  import { formatBytes, formatInt } from "../format";
  import { getShell } from "./context";

  type Dimensions = { width: number; height: number };
  type Mode = "side" | "swipe" | "onion" | "pixel";
  let {
    before, after, oldBytes = null, newBytes = null,
    mode = "side", position = $bindable(50), pixelUrl = "", pixelStatus = "idle",
    onmodechange, onerror,
  }: {
    before: string | null;
    after: string | null;
    oldBytes?: number | null;
    newBytes?: number | null;
    mode?: Mode;
    position?: number;
    pixelUrl?: string;
    pixelStatus?: "idle" | "working" | "too-large" | "error";
    onmodechange?: (mode: Mode) => void;
    onerror?: () => void;
  } = $props();

  const shell = getShell();
  let oldSize = $state<Dimensions | null>(null);
  let newSize = $state<Dimensions | null>(null);
  let oldFailed = $state(false);
  let newFailed = $state(false);
  const locale = $derived(shell.activeLocale);
  const percent = $derived(new Intl.NumberFormat(locale === "zh" ? "zh-CN" : "en-US", { maximumFractionDigits: 1 }));
  const width = $derived(Math.max(oldSize?.width ?? 0, newSize?.width ?? 0, 1));
  const height = $derived(Math.max(oldSize?.height ?? 0, newSize?.height ?? 0, 1));
  // One scale and one origin for both revisions, including small icons and overlays.
  const scale = $derived(Math.max(width, height) < 128 ? Math.floor(128 / Math.max(width, height)) : 1);
  const dimensionsChanged = $derived(oldSize && newSize && (oldSize.width !== newSize.width || oldSize.height !== newSize.height));
  const byteDelta = $derived(oldBytes != null && newBytes != null ? newBytes - oldBytes : null);

  $effect(() => {
    const sources = [before, after];
    oldSize = newSize = null;
    oldFailed = newFailed = false;
    let cancelled = false;
    // Reuse the browser's image cache. No canvas, pixel reads or extra backend calls.
    const images = sources.map((source, side) => {
      if (!source) return null;
      const image = new Image();
      image.decoding = "async";
      image.onload = () => {
        if (cancelled) return;
        const size = { width: image.naturalWidth, height: image.naturalHeight };
        if (side === 0) oldSize = size;
        else newSize = size;
      };
      image.onerror = () => {
        if (cancelled) return;
        if (side === 0) oldFailed = true;
        else newFailed = true;
        onerror?.();
      };
      image.src = source;
      return image;
    });
    return () => {
      cancelled = true;
      for (const image of images) if (image) {
        image.onload = image.onerror = null;
        image.removeAttribute("src");
      }
    };
  });

  function signed(value: number, bytes = false) {
    return `${value > 0 ? "+" : value < 0 ? "−" : ""}${bytes ? formatBytes(Math.abs(value), locale) : formatInt(Math.abs(value), locale)}`;
  }
</script>

<div class="image-comparison" style:--image-width="{width * scale}px" style:--image-ratio={width / height}>
  {#if onmodechange}
    <div class="image-toolbar">
      {#each ["side", "swipe", "onion", "pixel"] as option}
        <button type="button" class:on={mode === option} aria-pressed={mode === option} disabled={option === "pixel" && (!before || !after)} onclick={() => onmodechange?.(option as Mode)}>{shell.tr(option === "side" ? "dialog.imageSide" : option === "swipe" ? "dialog.imageSwipe" : option === "onion" ? "dialog.imageOnion" : "dialog.imagePixel")}</button>
      {/each}
      {#if mode === "swipe" || mode === "onion"}
        <input type="range" min="0" max="100" bind:value={position} aria-label={mode} />
      {/if}
    </div>
  {/if}
  <div class="image-content">
    <div class="image-metadata">
      {@render metadata("old", oldSize, oldBytes)}
      {@render metadata("new", newSize, newBytes)}
    </div>
    <div class="image-change-summary" aria-live="polite">
      {#if oldSize && newSize}
        <span class:changed={dimensionsChanged} class="image-dimension-change">
          {#if dimensionsChanged}
            {shell.tr("dialog.imageDimensionsChange")}:
            <span>{shell.tr("dialog.imageWidth")} {signed(newSize.width - oldSize.width)} px</span> ·
            <span>{shell.tr("dialog.imageHeight")} {signed(newSize.height - oldSize.height)} px</span>
          {:else}{shell.tr("dialog.imageDimensionsUnchanged")}{/if}
        </span>
      {/if}
      {#if byteDelta != null}
        <span class:changed={byteDelta !== 0} class="image-byte-change" title={`${signed(byteDelta)} B`}>
          {#if byteDelta === 0}{shell.tr("dialog.imageSizeUnchanged")}
          {:else}
            {shell.tr("dialog.imageSizeChange")}: {signed(byteDelta, true)}
            {#if oldBytes != null && oldBytes > 0}({byteDelta > 0 ? "+" : "−"}{percent.format(Math.abs(byteDelta) / oldBytes * 100)}%){/if}
          {/if}
        </span>
      {/if}
    </div>
    {#if oldSize || newSize}<p class="image-scale-note">{shell.tr("dialog.imageScale")}</p>{/if}
    <div class="image-stage" class:pair={mode === "side"}>
      {#if mode === "pixel"}
        <figure class="image-frame"><div class="image-box">
          {#if pixelUrl}
            <div class="image-canvas" style:aspect-ratio={`${width} / ${height}`}><img class="pixel-preview" alt={shell.tr("dialog.imagePixel")} src={pixelUrl} decoding="async" /></div>
          {:else if pixelStatus !== "idle"}
            <p class="meta" role="status">{shell.tr(pixelStatus === "working" ? "dialog.imageWorking" : pixelStatus === "too-large" ? "dialog.imageTooLarge" : "dialog.imageFailed")}</p>
          {/if}
        </div></figure>
      {:else if mode === "side"}
        {#each [{ source: before, size: oldSize, failed: oldFailed, bytes: oldBytes, label: "dialog.imageBefore" }, { source: after, size: newSize, failed: newFailed, bytes: newBytes, label: "dialog.imageAfter" }] as side (side.label)}
          <figure class="image-frame" aria-label={shell.tr(side.label)}>
            <div class="image-box">
              {#if side.source && side.size}
                <div class="image-canvas" style:aspect-ratio={`${width} / ${height}`}>
                  {@render sourceImage(side.source, side.size, side.label)}
                </div>
              {:else}<span class="meta">{shell.tr(side.source && !side.failed ? "chrome.reading" : side.bytes != null || side.failed ? "dialog.imageUnavailable" : "dialog.imageNoFile")}</span>{/if}
            </div>
          </figure>
        {/each}
      {:else}
        <figure class="image-frame"><div class="image-box">
          <div class="image-canvas image-overlay" style:aspect-ratio={`${width} / ${height}`}>
            {#if before && oldSize}{@render sourceImage(before, oldSize, "dialog.imageBefore")}{/if}
            {#if after && newSize}
              <div class="image-layer over" style:opacity={mode === "onion" ? position / 100 : 1} style:clip-path={mode === "swipe" ? `inset(0 ${100 - position}% 0 0)` : "none"}>
                {@render sourceImage(after, newSize, "dialog.imageAfter")}
              </div>
            {/if}
          </div>
        </div></figure>
      {/if}
    </div>
  </div>
</div>

{#snippet sourceImage(source: string, size: Dimensions, label: string)}
  <img class="image-source" alt={shell.tr(label)} src={source} decoding="async" style:width="{size.width / width * 100}%" style:height="{size.height / height * 100}%" style:image-rendering={scale > 1 ? "pixelated" : "auto"} />
{/snippet}

{#snippet metadata(side: "old" | "new", size: Dimensions | null, bytes: number | null)}
  <div class="image-meta-side {side}">
    <span class="image-side-label">{shell.tr(side === "old" ? "dialog.imageBefore" : "dialog.imageAfter")}</span>
    <dl>
      <div><dt>{shell.tr("dialog.imageDimensions")}</dt><dd class="image-dimensions">{size ? `${formatInt(size.width, locale)} × ${formatInt(size.height, locale)} px` : "—"}</dd></div>
      <div><dt>{shell.tr("dialog.imageFileSize")}</dt><dd class="image-file-size">{bytes == null ? "—" : formatBytes(bytes, locale)}{#if bytes != null && bytes >= 1024} <span class="image-exact-bytes">({formatInt(bytes, locale)} B)</span>{/if}</dd></div>
    </dl>
  </div>
{/snippet}

<style>
  .image-comparison { display: flex; flex: 1; flex-direction: column; min-width: 0; min-height: 0; color: var(--text); font-size: 12px; }
  .image-toolbar { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; padding: 8px 12px 0; }
  .image-toolbar button { border: 0; background: transparent; font: inherit; color: inherit; padding: 4px 8px; border-radius: var(--radius-sm); }
  .image-toolbar button.on { background: var(--selection); }
  .image-toolbar button:hover:not(.on):not(:disabled) { background: var(--field); }
  .image-toolbar input { flex: 1; min-width: 80px; max-width: 180px; }
  .image-content { flex: 1; min-height: 0; overflow: auto; padding: 12px; }
  .image-metadata { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
  .image-meta-side { min-width: 0; padding: 10px 12px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--elevated); }
  .image-side-label { display: inline-block; padding: 1px 7px; margin-bottom: 8px; border-radius: var(--radius-sm); background: var(--diff-del); font-weight: 500; }
  .new .image-side-label { background: var(--diff-add); }
  dl { display: grid; gap: 4px; margin: 0; }
  dl > div { display: flex; flex-wrap: wrap; align-items: baseline; gap: 2px 10px; }
  dt { color: var(--text-secondary); }
  dd { margin: 0; font-variant-numeric: tabular-nums; overflow-wrap: anywhere; }
  .image-exact-bytes { margin-inline-start: 4px; color: var(--text-secondary); font-size: 11px; }
  .image-change-summary { display: flex; flex-wrap: wrap; gap: 4px 20px; margin-top: 10px; color: var(--text-secondary); font-variant-numeric: tabular-nums; }
  .image-change-summary:empty { display: none; }
  .image-change-summary .changed { color: var(--text); font-weight: 500; }
  .image-dimension-change > span { white-space: nowrap; }
  .image-scale-note { margin: 6px 0 10px; color: var(--text-secondary); font-size: 11px; }
  .image-stage { display: grid; grid-template-columns: minmax(0, 1fr); gap: 12px; margin-top: 10px; }
  .image-stage.pair { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .image-frame { margin: 0; min-width: 0; display: flex; }
  .image-box { display: flex; flex: 1; align-items: center; justify-content: center; min-width: 0; min-height: 160px; padding: 12px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: repeating-conic-gradient(var(--image-checker-a) 0% 25%, var(--image-checker-b) 0% 50%) 50% / 16px 16px; }
  .image-canvas { position: relative; flex: none; width: min(100%, var(--image-width), calc(55vh * var(--image-ratio))); }
  .image-source, .pixel-preview { position: absolute; top: 0; left: 0; display: block; max-width: none; max-height: none; outline: 1px solid var(--line); }
  .pixel-preview { width: 100%; height: 100%; }
  .image-layer { position: absolute; inset: 0; }
</style>
