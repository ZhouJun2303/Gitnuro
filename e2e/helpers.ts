import { expect, type Page } from "@playwright/test";
import { scenes, type LabScene } from "../src/lib/lab/scenes";

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

export async function openLabScene(page: Page, id: string, ready: string) {
  const scene = scenes.find((item) => item.id === id);
  if (!scene) throw new Error(`Unknown lab scene: ${id}`);
  await openScene(page, scene);
  await expect(page.locator(ready).first()).toBeVisible();
}
