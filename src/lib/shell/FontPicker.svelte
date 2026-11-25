<script lang="ts">
  import { getShell } from "./context";
  import { uiFonts, codeFonts, fontSize, type FontCacheInfo, type FontOption } from "../fonts";

  let { kind, cache, ondownload }: { kind: "ui" | "code"; cache: FontCacheInfo | null; ondownload: (kind: "ui" | "code") => Promise<void> } = $props();
  const shell = getShell();
  const status = $derived(kind === "ui" ? shell.uiFontStatus : shell.codeFontStatus);
  const value = $derived(kind === "ui" ? shell.settings.uiFont : shell.settings.codeFont);
  const size = $derived(kind === "ui" ? shell.settings.uiScale : shell.settings.diffFontSize);
  const weight = $derived(kind === "ui" ? shell.settings.uiFontWeight : shell.settings.codeFontWeight);
  const previewFontSize = $derived(kind === "ui" ? shell.effectiveUiScale : shell.effectiveDiffFontSize);
  const previewFontWeight = $derived(kind === "ui" ? shell.effectiveUiFontWeight : shell.effectiveCodeFontWeight);
  const fonts = $derived<FontOption[]>([...(kind === "ui" ? uiFonts : codeFonts), ...(cache?.customFonts ?? []).map((font) => ({ ...font, script: "custom" }))]);
  const selected = $derived(fonts.find((font) => font.id === value));
  const cached = $derived(cache?.fonts.find((font) => font.id === value));
  const label = $derived(shell.tr(kind === "ui" ? "dialog.uiFont" : "dialog.codeFont"));
  const previewStatus = $derived(status === "ready" ? shell.tr("dialog.fontUsing", { name: selected?.name ?? value })
    : shell.tr(!value ? "dialog.fontSystemPreview" : status === "loading" ? "dialog.fontLoading" : "dialog.fontNotDownloaded"));

  function setSize(input: HTMLInputElement) {
    const next = Math.min(kind === "ui" ? 30 : 32, Math.max(kind === "ui" ? 11 : 8, Math.round(Number(input.value) || (kind === "ui" ? 14 : 13))));
    if (kind === "ui") shell.settings.uiScale = next;
    else shell.settings.diffFontSize = next;
    input.value = String(next);
  }
  function beginSlider() {
    shell.prefSliderDragging = true;
  }
  function setWeight(input: HTMLInputElement) {
    const next = Math.min(600, Math.max(400, Math.round(Number(input.value) / 100) * 100));
    if (kind === "ui") shell.settings.uiFontWeight = next;
    else shell.settings.codeFontWeight = next;
    input.value = String(next);
  }
</script>

<section class="pref-card font-card" data-font-kind={kind}>
  <h3 class="pref-card-title">{label}</h3>
  <div class="pref-card-body">
    <div class="pref-row font-row">
      <label class="pref-label" for={`font-${kind}`}>{label}</label>
      <span class="pref-control">
        <select id={`font-${kind}`} value={value} onchange={(event) => {
          shell.fontChoiceGeneration++;
          shell.fontError = "";
          if (kind === "ui") shell.settings.uiFont = event.currentTarget.value;
          else shell.settings.codeFont = event.currentTarget.value;
        }}>
          <option value="">{shell.tr("dialog.systemFont")}</option>
          {#each ["cjk", "latin", "mono", "custom"] as group}
            {@const choices = fonts.filter((font) => font.script === group)}
            {#if choices.length}
              <optgroup label={shell.tr(group === "cjk" ? "dialog.fontGroupChinese" : group === "latin" ? "dialog.fontGroupLatin" : group === "mono" ? "dialog.fontGroupCode" : "dialog.fontGroupCustom")}>
                {#each choices as font (font.id)}<option value={font.id}>{font.name}</option>{/each}
              </optgroup>
            {/if}
          {/each}
        </select>
        <button class="text-button" type="button" disabled={!value || !!shell.fontDownloading || status === "loading" || status === "ready"} onclick={() => void ondownload(kind)}>
          {shell.tr(shell.fontDownloading === value && value ? "dialog.fontDownloading" : status === "ready" ? "dialog.fontCached" : "dialog.downloadFont")}
        </button>
      </span>
    </div>
    {#if selected}
      <p class="pref-setting-hint font-download-size">Google Fonts · {shell.tr("dialog.fontDownloadSize", { size: fontSize(cached?.bytes ?? selected.bytes) })}</p>
      {#if cached}<p class="font-file-path"><span>{shell.tr("dialog.fontFilePath")}</span><code>{cached.path}</code></p>{/if}
    {/if}
    <div class="font-metrics">
      <label for={`font-size-${kind}`}>{shell.tr(kind === "ui" ? "dialog.uiFontSize" : "dialog.codeFontSize")}</label>
      <div class="font-size-control">
        <input id={`font-size-${kind}`} type="range" aria-label={shell.tr(kind === "ui" ? "dialog.uiFontSize" : "dialog.codeFontSize")} min={kind === "ui" ? 11 : 8} max={kind === "ui" ? 30 : 32} value={size} onpointerdown={beginSlider} oninput={(event) => setSize(event.currentTarget)} />
        <output>{size} px</output>
      </div>
      <label for={`font-weight-${kind}`}>{shell.tr("dialog.fontWeight")}</label>
      <div class="font-size-control font-weight-control">
        <input id={`font-weight-${kind}`} type="range" aria-label={shell.tr("dialog.fontWeight")} min="400" max="600" step="100" value={weight || 400} onpointerdown={beginSlider} oninput={(event) => setWeight(event.currentTarget)} />
        <output>{weight || 400}</output>
      </div>
    </div>
    <div class="font-preview-label"><span>{shell.tr("dialog.fontPreview")}</span><span role="status">{previewStatus}</span></div>
    <div class="font-preview" data-font-preview={kind} style:font-family={kind === "ui" ? "var(--font)" : "var(--mono)"} style:font-size={`${previewFontSize}px`} style:font-weight={previewFontWeight}>
      {#if kind === "ui"}
        <p>AweGit · 让每一次提交清晰可见</p>
        <p>The quick brown fox jumps over the lazy dog.</p>
        <p>中文界面与文件历史 · 0123456789 · Il1 O0 () {"{}"}</p>
      {:else}
        <pre><span class="font-code-keyword">const</span> branch = <span class="font-code-string">"feat/字体预览"</span>;
<span class="font-code-comment">// 清楚区分 I l 1 和 O 0</span>
<span class="font-code-keyword">if</span> (ready) {'{'} commit(branch, 123); {'}'}</pre>
      {/if}
    </div>
  </div>
</section>
