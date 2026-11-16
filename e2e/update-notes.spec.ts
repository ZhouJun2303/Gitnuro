import { expect, test } from "@playwright/test";
import { scenes } from "../src/lib/lab/scenes";
import { openScene } from "./helpers";

function updateScene() {
  const scene = scenes.find((item) => item.id === "update-long");
  if (!scene) throw new Error("update-long");
  return scene;
}

test("update notes give every change its own line and scroll inside the panel", async ({ page }) => {
  await openScene(page, updateScene());
  const dialog = page.locator('[data-dialog="update"]');
  await expect(dialog.locator(".update-notes-section")).toHaveCount(3);
  await expect(dialog.locator(".update-notes-section").first()).toHaveText("✨ 新功能");
  const notes = dialog.locator(".update-notes");
  const metrics = await notes.evaluate((node) => ({ clientHeight: node.clientHeight, scrollHeight: node.scrollHeight }));
  expect(metrics.scrollHeight).toBeGreaterThan(metrics.clientHeight);
  await notes.evaluate((node) => {
    node.scrollTop = node.scrollHeight;
  });
  await expect.poll(() => notes.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
});

for (const theme of ["apple", "light", "dark"] as const) {
  test(`update notes fit the dialog in ${theme}`, async ({ page }) => {
    await openScene(page, updateScene(), `&theme=${theme}`);
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    const notes = page.locator('[data-dialog="update"] .update-notes');
    const box = await notes.boundingBox();
    expect(box?.height ?? 0).toBeGreaterThan(80);
    expect(box?.height ?? 0).toBeLessThanOrEqual(300);
    const item = await page.locator('[data-dialog="update"] .update-notes-item').first().boundingBox();
    expect(item?.width ?? 0).toBeLessThan(box?.width ?? 0);
    await expect(page.locator('[data-dialog="update"] .update-actions')).toBeInViewport();
  });
}
