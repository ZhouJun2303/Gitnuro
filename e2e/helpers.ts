import { expect, type Page } from "@playwright/test";
import { scenes, type LabScene } from "../src/lib/lab/scenes";

const pinnedSettings = new WeakMap<Page, Record<string, unknown>>();

/** Keep locale and diff style in this page. Saves stay in memory, so `.lab/settings.json` is left alone. */
export async function pinLabSettings(page: Page, patch: Record<string, unknown>) {
  const first = !pinnedSettings.has(page);
  pinnedSettings.set(page, { ...pinnedSettings.get(page), ...patch });
  if (!first) return;
  let settings: Record<string, unknown> | undefined;
  await page.route("**/__lab/invoke", async (route) => {
    const raw = route.request().postData();
    if (!raw) return route.continue();
    let body: { cmd?: string; args?: { values?: Record<string, unknown> } };
    try {
      body = JSON.parse(raw) as { cmd?: string; args?: { values?: Record<string, unknown> } };
    } catch {
      return route.continue();
    }
    if (body.cmd === "load_settings") {
      if (!settings) {
        const response = await route.fetch();
        const payload = await response.json();
        settings = { ...(payload.value ?? {}), ...pinnedSettings.get(page) };
      }
      return route.fulfill({ json: { ok: true, value: settings } });
    }
    if (body.cmd === "save_settings") {
      settings = body.args?.values ?? settings;
      return route.fulfill({ json: { ok: true, value: settings } });
    }
    return route.continue();
  });
}

export async function openScene(page: Page, scene: LabScene, query = "") {
  const errors: string[] = [];
  const onError = (error: Error) => errors.push(error.message);
  page.on("pageerror", onError);
  try {
    const response = await page.goto(scene.href + query, { waitUntil: "domcontentloaded" });
    expect(response?.status(), scene.id).toBeLessThan(400);
    const ready = page.locator(`[data-lab-scene="${scene.id}"]`);
    const failed = page.locator("[data-lab-error]");
    await expect(ready.or(failed)).toBeVisible({ timeout: 90_000 });
    if (!(await ready.isVisible())) throw new Error(`${scene.id}: ${await failed.getAttribute("data-lab-error")}`);
    if (scene.ready.startsWith("text=")) await expect(page.getByText(scene.ready.slice(5)).first()).toBeVisible();
    else await expect(page.locator(scene.ready).first()).toBeVisible();
    expect(errors, scene.id).toEqual([]);
  } finally { page.off("pageerror", onError); }
}

export async function openLabScene(
  page: Page,
  id: string,
  ready: string,
  options?: { locale?: "en" | "zh"; diffStyle?: "split" | "unified" },
) {
  const scene = scenes.find((item) => item.id === id);
  if (!scene) throw new Error(`Unknown lab scene: ${id}`);
  const patch: Record<string, unknown> = {};
  if (options?.locale) patch.locale = options.locale;
  if (options?.diffStyle) patch.diffStyle = options.diffStyle;
  if (Object.keys(patch).length > 0) await pinLabSettings(page, patch);
  await openScene(page, scene, options?.locale ? `&locale=${options.locale}` : "");
  await expect(page.locator(ready).first()).toBeVisible();
}
