import { expect, test } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"] as const) {
  test(`${theme}: commit Changes and expanded patches display images directly`, async ({ page }) => {
    await pinLabSettings(page, { theme });
    await openLabScene(page, "commit-image", ".detail-pane .revision-images", { locale: "en" });
    const images = page.locator(".detail-pane .revision-images img");
    await expect(images).toHaveCount(2);
    await expect.poll(() => images.evaluateAll((items) => items.every((item) => (item as HTMLImageElement).naturalWidth > 0))).toBe(true);
    await expect(page.locator(".detail-pane .binary-summary")).toHaveCount(0);
    await page.screenshot({ path: `.lab/screenshots/commit-image-${theme}.png`, fullPage: true });
    // The added image has only the new side, including in the exact panel from the report.
    await page.locator(".history .commit-row").last().click();
    await page.getByRole("tab", { name: "Changes", exact: true }).click();
    await expect(page.locator(".detail-pane .revision-images img")).toHaveCount(1);
    await page.getByRole("tab", { name: "Commit", exact: true }).click();
    await page.locator(".patch-toggle").first().click();
    await expect(page.locator(".commit-patch .revision-images img")).toHaveCount(1);
  });

  test(`${theme}: file history displays historical image pairs and added images`, async ({ page }) => {
    await pinLabSettings(page, { theme });
    await openLabScene(page, "file-history-image", ".revision-images", { locale: "en" });
    const images = page.locator(".revision-images img");
    await expect(images).toHaveCount(2);
    await expect.poll(() => images.evaluateAll((items) => items.every((item) => (item as HTMLImageElement).naturalWidth > 0))).toBe(true);
    const latest = await images.last().getAttribute("src");
    await page.locator(".history-commit").last().click();
    await expect(images).toHaveCount(1);
    await expect(images).toHaveAttribute("alt", "After");
    expect(await images.first().getAttribute("src")).not.toBe(latest);
    await page.locator(".history-commit").first().click();
    await expect(images).toHaveCount(2);
    await page.screenshot({ path: `.lab/screenshots/history-image-${theme}.png`, fullPage: true });
  });

  test(`${theme}: deleted images show the previous revision and oversized files stay bounded`, async ({ page }) => {
    await pinLabSettings(page, { theme, locale: "en" });
    await page.goto("/?scene=file-history-image&view=history&file=art/deleted.png&locale=en");
    await expect(page.locator('[data-lab-scene="file-history-image"]')).toBeVisible();
    await expect(page.locator(".revision-images img")).toHaveCount(1);
    await expect(page.locator(".revision-images img")).toHaveAttribute("alt", "Before");
    const previewRequests: string[] = [];
    page.on("request", (request) => {
      if (request.url().endsWith("/__lab/invoke") && request.postDataJSON()?.cmd === "revision_preview") previewRequests.push(request.postDataJSON().args.file);
    });
    await page.goto("/?scene=file-history-image&view=history&file=art/large.png&locale=en");
    await expect(page.locator('[data-lab-scene="file-history-image"]')).toBeVisible();
    await expect(page.locator(".binary-diff")).toBeVisible();
    expect(previewRequests).toEqual([]);
  });
}

test("a late image preview cannot overwrite a newly selected revision", async ({ page }) => {
  await pinLabSettings(page, { locale: "en" });
  let release!: () => void;
  const delayed = new Promise<void>((resolve) => { release = resolve; });
  let delayedId = "";
  let pending = 0;
  await page.route("**/__lab/invoke", async (route) => {
    const request = route.request().postDataJSON();
    if (request.cmd !== "revision_preview") return route.fallback();
    if (!delayedId) delayedId = request.args.rev.replace(/\^$/, "");
    if (request.args.rev.replace(/\^$/, "") === delayedId) {
      pending++;
      await delayed;
    }
    return route.fallback();
  });
  await page.goto("/?scene=file-history-image&view=history&file=art/mark.png&locale=en");
  await expect(page.locator('[data-lab-scene="file-history-image"]')).toBeVisible();
  await expect.poll(() => pending).toBeGreaterThan(0);
  await page.locator(".history-commit").last().click();
  await expect(page.locator(".revision-images img")).toHaveCount(1);
  const source = await page.locator(".revision-images img").getAttribute("src");
  release();
  await expect(page.locator(".revision-images img")).toHaveCount(1);
  await expect(page.locator(".revision-images img")).toHaveAttribute("src", source!);
});
