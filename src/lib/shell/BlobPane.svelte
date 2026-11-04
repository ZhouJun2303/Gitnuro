<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getShell } from "./context";
  import { onVirtualScroll, windowSlice } from "../view";

  const shell = getShell();
  let top = $state(0);
  const lines = $derived(shell.blobText ? shell.blobText.split("\n") : []);
  const view = $derived(windowSlice(lines, top, 18));
  const onScroll = onVirtualScroll((value) => (top = value));

  function closePane() {
    if (shell.inApp()) {
      void getCurrentWindow().close();
      return;
    }
    window.location.href = "/";
  }
</script>

<div class="blob-pane">
  <header>
    <strong>{shell.blobFile || shell.tr("chrome.reading")}</strong>
    <button class="text-button" type="button" aria-label={shell.tr("chrome.close")} onclick={closePane}>{shell.tr("chrome.close")}</button>
  </header>
  {#if shell.blobPreview}
    <img alt="" src={shell.blobPreview.dataUrl} />
  {:else if shell.blobBinary}
    <p class="empty">{shell.tr("chrome.binary")}</p>
  {:else if lines.length === 0}
    <p class="empty">{shell.tr("chrome.reading")}</p>
  {:else}
    <div class="blob-scroll" onscroll={onScroll}>
      <div class="commit-window" style:height="{lines.length * 18}px">
        {#each view.rows as line, index (`b-${view.start + index}`)}
          <div class="blob-row" style:top="{(view.start + index) * 18}px">
            <span>{view.start + index + 1}</span>
            <span>{line}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .blob-pane {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--canvas);
    color: var(--text);
  }

  header {
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 0 10px 0 12px;
    border-bottom: 1px solid var(--line);
    flex: none;
    font-size: 12px;
  }

  header strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  header :global(.text-button) {
    margin-left: 0;
    flex: none;
  }

  img {
    max-width: 100%;
    margin: 16px auto;
  }

  .blob-scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    font-family: var(--mono);
    font-size: 12.5px;
  }

  .blob-row {
    position: absolute;
    left: 0;
    right: 0;
    height: 18px;
    display: flex;
    gap: 12px;
    padding: 0 12px;
    white-space: pre;
  }

  .blob-row span:first-child {
    width: 40px;
    flex: none;
    text-align: right;
    color: var(--text-secondary);
  }
</style>
