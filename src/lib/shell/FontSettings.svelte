<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { invoke } from "../perf";
  import { fontSize, googleFamily, readFontCache, type FontCacheInfo, type FontMatch } from "../fonts";
  import { getShell } from "./context";
  import FontPicker from "./FontPicker.svelte";

  const shell = getShell();
  let cache = $state.raw<FontCacheInfo | null>(null);
  let active = true;
  let family = $state("");
  let target = $state<"ui" | "code">("ui");
  let wasDownloading = false;
  let recognition = $state<FontMatch | null>(null);
  let recognizing = $state(false);
  let recognitionError = $state("");
  let recognitionGeneration = 0;
  const matches = new Map<string, FontMatch>();
  const recognized = $derived(!!recognition && recognition.id === `google-${(googleFamily(family) ?? "").replace(/[^a-z0-9]/gi, "").toLowerCase()}`);
  const used = $derived(cache?.fonts.reduce((sum, font) => sum + (font.bytes ?? 0), 0) ?? 0);

  async function refresh() {
    if (!shell.backend()) return;
    try {
      const info = await readFontCache();
      if (active) cache = info;
    } catch (error) { if (active) shell.fontError = shell.message(error); }
  }
  async function download(kind: "ui" | "code") {
    await shell.downloadFont(kind);
  }
  async function addGoogleFont() {
    const name = googleFamily(family);
    if (!name) { shell.fontError = shell.tr("dialog.googleFontInvalid"); return; }
    if (!recognized || !recognition) return;
    await shell.importGoogleFont(recognition.name, target);
  }
  async function identify(name: string, generation: number) {
    if (generation !== recognitionGeneration || !active) return;
    recognizing = true;
    recognition = null;
    recognitionError = "";
    try {
      const match = matches.get(name) ?? await invoke<FontMatch>("recognize_google_font", { name });
      if (generation !== recognitionGeneration || !active) return;
      if (matches.size >= 16) matches.delete(matches.keys().next().value!);
      matches.set(name, match);
      recognition = match;
    } catch (error) { if (generation === recognitionGeneration && active) recognitionError = shell.message(error); }
    finally { if (generation === recognitionGeneration && active) recognizing = false; }
  }
  function defaults() {
    shell.fontChoiceGeneration++;
    shell.settings.uiFont = "";
    shell.settings.codeFont = "";
    shell.settings.uiScale = 14;
    shell.settings.diffFontSize = 14;
    shell.settings.uiFontWeight = 400;
    shell.settings.codeFontWeight = 400;
    shell.fontError = "";
  }
  $effect(() => {
    const downloading = !!shell.fontDownloading;
    if (wasDownloading && !downloading) untrack(() => void refresh());
    wasDownloading = downloading;
  });
  $effect(() => {
    const name = googleFamily(family);
    const generation = ++recognitionGeneration;
    recognition = null;
    recognizing = false;
    recognitionError = family.trim() && !name ? shell.tr("dialog.googleFontInvalid") : "";
    if (!name) return;
    const timer = setTimeout(() => void identify(name, generation), 600);
    return () => { clearTimeout(timer); recognitionGeneration++; };
  });
  async function openFolder() {
    try { await invoke("open_font_folder"); }
    catch (error) { shell.fontError = shell.message(error); }
  }
  onMount(() => { void refresh(); return () => { active = false; }; });
</script>

<div class="font-defaults">
  <p class="pref-setting-hint">{shell.tr("dialog.fontDefaultsHint")}</p>
  <button class="text-button" type="button" onclick={defaults}>{shell.tr("dialog.fontDefaults")}</button>
</div>
{#if shell.fontProgress}
  <div class="font-download-progress" role="status">
    <div><span>{shell.tr("dialog.fontProgressTitle")}</span><span>{shell.tr(shell.fontProgress.phase === "ready" ? "dialog.fontReady" : shell.fontProgress.phase === "error" ? "dialog.fontFailed" : shell.fontProgress.phase === "downloading" ? "dialog.fontDownloading" : "dialog.fontConnecting")}</span></div>
    <progress aria-label={shell.tr("dialog.fontProgressTitle")} max="100" value={shell.fontProgress.percent ?? undefined}></progress>
    <div class="meta"><span>{fontSize(shell.fontProgress.received)}{shell.fontProgress.total > 0 ? ` / ${fontSize(shell.fontProgress.total)}` : ""}</span><span>{shell.fontProgress.percent == null ? "…" : `${shell.fontProgress.percent}%`}</span></div>
  </div>
{/if}
<section class="pref-card google-font-import">
  <h3 class="pref-card-title">{shell.tr("dialog.googleFontAdd")}</h3>
  <div class="pref-card-body">
    <p class="pref-setting-hint">{shell.tr("dialog.googleFontHint")}</p>
    <a class="text-button google-font-link" href="https://fonts.google.com/" target="_blank" rel="noreferrer" onclick={(event) => {
      if (shell.inApp()) { event.preventDefault(); void openUrl("https://fonts.google.com/").catch((error) => { shell.fontError = shell.message(error); }); }
    }}>{shell.tr("dialog.googleFontBrowse")} ↗</a>
    <label class="google-font-label" for="google-font-family">{shell.tr("dialog.googleFontName")}</label>
    <input id="google-font-family" type="text" maxlength="1000" placeholder="Noto Sans SC / https://fonts.google.com/specimen/…" bind:value={family} onkeydown={(event) => {
      if (event.key === "Enter") { event.preventDefault(); event.stopPropagation(); const name = googleFamily(family); if (recognized && !shell.fontDownloading) void addGoogleFont(); else if (name) void identify(name, ++recognitionGeneration); }
    }} />
    <div class="font-recognition" class:recognized={!!recognition} class:failed={!!recognitionError} role="status">
      {#if recognizing}<span>{shell.tr("dialog.googleFontChecking")}</span>
      {:else if recognition}<span>✓ {shell.tr(recognition.cached ? "dialog.fontCached" : "dialog.googleFontRecognized")} · {recognition.name}{recognition.bytes ? ` · ${fontSize(recognition.bytes)}` : ""}</span>
      {:else if recognitionError}<span>{recognitionError}</span>
      {:else}<span>{shell.tr("dialog.googleFontCheckHint")}</span>{/if}
      <button class="text-button" type="button" disabled={!googleFamily(family) || recognizing || !!shell.fontDownloading} onclick={() => { const name = googleFamily(family); if (name) void identify(name, ++recognitionGeneration); }}>{shell.tr("dialog.googleFontCheck")}</button>
    </div>
    <div class="google-font-actions">
      <label for="google-font-target">{shell.tr("dialog.googleFontTarget")}</label>
      <select id="google-font-target" bind:value={target}>
        <option value="ui">{shell.tr("dialog.uiFont")}</option>
        <option value="code">{shell.tr("dialog.codeFont")}</option>
      </select>
      <button class="text-button" type="button" disabled={!recognized || recognizing || !!shell.fontDownloading} onclick={() => void addGoogleFont()}>{shell.tr(shell.fontDownloading === "google" ? "dialog.fontDownloading" : "dialog.googleFontDownload")}</button>
    </div>
  </div>
</section>
{#if shell.fontError}<p class="font-error" role="alert">{shell.fontError}</p>{/if}
<FontPicker kind="ui" {cache} ondownload={download} />
<FontPicker kind="code" {cache} ondownload={download} />
<section class="pref-card font-storage">
  <h3 class="pref-card-title">{shell.tr("dialog.fontStorage")}</h3>
  <div class="pref-card-body">
    <p class="pref-setting-hint">{shell.tr("dialog.fontHint")}</p>
    <code class="font-cache-path">{cache?.directory || shell.tr("chrome.reading")}</code>
    <div class="font-storage-actions">
      <span class="meta">{shell.tr("dialog.fontStorageUsed", { size: fontSize(used) })}</span>
      <button class="text-button" type="button" disabled={!cache} onclick={() => void openFolder()}>{shell.tr("dialog.fontOpenFolder")}</button>
    </div>
  </div>
</section>
