import { invoke } from "./perf";
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import catalog from "./font-catalog.json";

export const uiFonts = catalog.filter((font) => font.kind === "ui");
export const codeFonts = catalog.filter((font) => font.kind === "code");
export interface FontOption { id: string; name: string; bytes: number; variable: boolean; script?: string }
const fontById = new Map<string, FontOption>(catalog.map((font) => [font.id, font]));
export interface FontCacheInfo {
  directory: string;
  fonts: { id: string; path: string; bytes: number | null }[];
  customFonts: FontOption[];
}
export interface FontMatch { id: string; name: string; bytes: number | null; cached: boolean }
export interface FontProgress { requestId: string; id: string; phase: string; received: number; total: number; percent: number | null }
export function registerCustomFonts(fonts: FontOption[]) {
  for (const font of fonts) if (/^google-[a-z0-9]+$/.test(font.id)) fontById.set(font.id, { ...font, script: "custom" });
}
export async function readFontCache(): Promise<FontCacheInfo> {
  const info = await invoke<FontCacheInfo>("font_cache_info");
  registerCustomFonts(info.customFonts ?? []);
  return info;
}
export function fontSize(bytes: number): string {
  return bytes >= 1024 * 1024 ? `${(bytes / (1024 * 1024)).toFixed(1)} MB` : `${Math.ceil(bytes / 1024)} KB`;
}
export type FontStatus = "system" | "loading" | "missing" | "ready" | "error";

const loaded = new Map<string, Promise<FontFace | null>>();
const selectedFonts = new Map<"ui" | "code", string>();
let customRegistry: Promise<FontCacheInfo> | null = null;

function decodeFont(font: FontOption, source: string): Promise<FontFace> {
  if (source === `awegit-font:${font.id}`) source = isTauri() ? convertFileSrc(font.id, "awegit-font") : `/__lab/font/${font.id}`;
  return new FontFace(`AweGit ${font.id}`, `url("${source}")`, { weight: font.variable ? "100 900" : "400", display: "swap" }).load().then((face) => {
    document.fonts.add(face);
    return face;
  });
}

function remember(id: string, task: Promise<FontFace | null>) {
  loaded.delete(id);
  loaded.set(id, task);
  void task.then((face) => {
    if (!face) { if (loaded.get(id) === task) loaded.delete(id); return; }
    // Keep a few recently used faces, plus the two currently applied fonts.
    const root = document.documentElement.style;
    for (const [key, value] of loaded) {
      if (loaded.size <= 4) break;
      if (key === id || [...selectedFonts.values()].includes(key) || ["--font-user", "--mono-user"].some((property) => root.getPropertyValue(property).includes(`AweGit ${key}"`))) continue;
      loaded.delete(key);
      void value.then((old) => { if (old) document.fonts.delete(old); }, () => {});
    }
  }, () => { if (loaded.get(id) === task) loaded.delete(id); });
  return task;
}

export function useDownloadedFont(font: FontOption, source: string): Promise<FontFace | null> {
  registerCustomFonts([font]);
  return remember(font.id, decodeFont(font, source));
}

/** Reuse a decoded face across UI/code controls. Missing files never trigger a network request. */
export function loadFont(id: string, download = false, requestId?: string): Promise<FontFace | null> {
  const cached = loaded.get(id);
  if (cached) {
    loaded.delete(id); loaded.set(id, cached);
    return download ? cached.then((face) => face ?? loadFont(id, true, requestId)) : cached;
  }
  const task = (async () => {
    if (!fontById.has(id) && /^google-[a-z0-9]+$/.test(id)) {
      customRegistry ??= readFontCache().catch((error) => { customRegistry = null; throw error; });
      await customRegistry;
      if (!fontById.has(id)) await readFontCache();
    }
    const font = fontById.get(id);
    if (!font) return null;
    const source = await invoke<string | null>(download ? "download_font" : "font_data", download ? { id, requestId } : { id });
    if (!source) return null;
    return decodeFont(font, source);
  })();
  return remember(id, task);
}

/** Accept a family name or a specimen/share URL copied from the official website. */
export function googleFamily(input: string): string | null {
  let name = input.trim();
  if (/^https?:/i.test(name)) {
    try {
      const url = new URL(name);
      if (url.protocol !== "https:" || url.hostname !== "fonts.google.com" || url.username || url.password || url.port) return null;
      name = url.searchParams.get("family")?.split(":")[0] ?? decodeURIComponent(url.pathname.match(/^\/specimen\/(.+?)(?:\/|$)/)?.[1] ?? "");
    } catch { return null; }
  }
  name = name.replace(/\+/g, " ").trim();
  return name.length <= 100 && /^[a-z0-9 _-]+$/i.test(name) && /[a-z0-9]/i.test(name) ? name : null;
}

export function applyFont(id: string, kind: "ui" | "code", backend: boolean, status: (state: FontStatus, error?: unknown) => void): () => void {
  selectedFonts.set(kind, id);
  const root = document.documentElement;
  const variable = kind === "ui" ? "--font-user" : "--mono-user";
  root.style.removeProperty(variable);
  let active = true;
  if (!id) status("system");
  else if (!backend) status("missing");
  else {
    status("loading");
    void loadFont(id).then((face) => {
      if (!active) return;
      if (face) root.style.setProperty(variable, `"AweGit ${id}",`);
      status(face ? "ready" : "missing");
    }).catch((error) => { if (active) status("error", error); });
  }
  return () => { active = false; selectedFonts.delete(kind); root.style.removeProperty(variable); };
}
