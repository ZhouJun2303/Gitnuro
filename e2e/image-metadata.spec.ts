import { expect, test, type Locator, type Page } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

async function expectSharedScale(images: Locator, before: [number, number], after: [number, number], overlay = false) {
  await expect(images).toHaveCount(2);
  const sizes = await images.evaluateAll((items) => items.map((item) => {
    const image = item as HTMLImageElement;
    const bounds = image.getBoundingClientRect();
    return { width: image.naturalWidth, height: image.naturalHeight, x: bounds.x, y: bounds.y, drawnWidth: bounds.width, drawnHeight: bounds.height };
  }));
  expect(sizes.map((size) => [size.width, size.height])).toEqual([before, after]);
  expect(sizes[0].drawnWidth).toBeGreaterThan(0);
  expect(sizes[0].drawnWidth / sizes[1].drawnWidth).toBeCloseTo(before[0] / after[0], 2);
  expect(sizes[0].drawnHeight / sizes[1].drawnHeight).toBeCloseTo(before[1] / after[1], 2);
  expect(sizes[0].drawnWidth / sizes[0].drawnHeight).toBeCloseTo(before[0] / before[1], 2);
  if (overlay) {
    expect(sizes[0].x).toBeCloseTo(sizes[1].x, 1);
    expect(sizes[0].y).toBeCloseTo(sizes[1].y, 1);
  }
}

async function expectFileSizes(page: Page, open: () => Promise<void>, locale: string, staged = false) {
  const [response] = await Promise.all([
    page.waitForResponse((response) => {
      if (!response.url().endsWith("/__lab/invoke")) return false;
      const request = response.request().postDataJSON();
      return request?.cmd === "file_diff" && request.args?.staged === staged;
    }),
    open(),
  ]);
  const { value: diff } = await response.json();
  const format = new Intl.NumberFormat(locale === "zh" ? "zh-CN" : "en-US");
  for (const [side, bytes] of [["old", diff.oldBytes], ["new", diff.newBytes]] as const) {
    expect(bytes).toBeGreaterThan(1024);
    await expect(page.locator(`.image-meta-side.${side} .image-exact-bytes`)).toHaveText(`(${format.format(bytes)} B)`);
  }
  await expect(page.locator(".image-byte-change")).toContainText("%");
  await expect(page.locator(".image-byte-change")).toHaveAttribute("title", `${diff.newBytes > diff.oldBytes ? "+" : "−"}${format.format(Math.abs(diff.newBytes - diff.oldBytes))} B`);
}

for (const theme of ["apple", "light", "dark"] as const) {
  const locale = theme === "dark" ? "en" : "zh";

  test(`${theme}: historical images show dimensions, exact file sizes and changes without normalizing their size`, async ({ page }) => {
    await pinLabSettings(page, { theme, locale });
    await openLabScene(page, "commit-image", ".revision-images");
    await expect(page.locator(".image-meta-side.old .image-dimensions")).toHaveText("240 × 144 px");
    await expect(page.locator(".image-meta-side.new .image-dimensions")).toHaveText("360 × 216 px");
    await expect(page.locator(".image-dimension-change")).toContainText("+120 px");
    await expect(page.locator(".image-dimension-change")).toContainText("+72 px");
    await expect(page.locator(".image-byte-change")).toContainText("%");
    await expectSharedScale(page.locator(".revision-images img"), [240, 144], [360, 216]);
    await page.screenshot({ path: `.lab/screenshots/commit-image-metadata-${theme}.png`, fullPage: true });

    await openLabScene(page, "file-history-image", ".revision-images");
    await expect(page.locator(".image-meta-side.new .image-dimensions")).toHaveText("360 × 216 px");
    await expectSharedScale(page.locator(".revision-images img"), [240, 144], [360, 216]);
    await page.screenshot({ path: `.lab/screenshots/history-image-metadata-${theme}.png`, fullPage: true });
    await page.locator(".history-commit").last().click();
    await expect(page.locator(".image-meta-side.old .image-dimensions")).toHaveText("—");
    await expect(page.locator(".image-meta-side.old .image-file-size")).toHaveText("—");
    await expect(page.locator(".image-meta-side.new .image-dimensions")).toHaveText("240 × 144 px");
    await expect(page.locator(".image-change-summary")).toBeHidden();
    await expect(page.locator(".revision-images img")).toHaveCount(1);
  });

  test(`${theme}: image diff uses index and work tree sizes and keeps real bounds in every mode`, async ({ page }) => {
    await pinLabSettings(page, { theme, locale });
    await expectFileSizes(page, () => openLabScene(page, "diff-image-size", ".image-metadata"), locale);
    await expect(page.locator(".image-meta-side.old .image-dimensions")).toHaveText("480 × 144 px");
    await expect(page.locator(".image-meta-side.new .image-dimensions")).toHaveText("240 × 288 px");
    await expect(page.locator(".image-dimension-change")).toContainText("−240 px");
    await expect(page.locator(".image-dimension-change")).toContainText("+144 px");
    await expectSharedScale(page.locator(".image-stage img"), [480, 144], [240, 288]);
    await page.screenshot({ path: `.lab/screenshots/image-size-diff-${theme}.png`, fullPage: true });

    for (const mode of locale === "zh" ? ["洋葱皮", "滑动"] : ["Onion", "Swipe"]) {
      await page.getByRole("button", { name: mode, exact: true }).click();
      await expectSharedScale(page.locator(".image-stage img"), [480, 144], [240, 288], true);
      await expect(page.locator(".image-meta-side.old .image-dimensions")).toHaveText("480 × 144 px");
    }
    await page.screenshot({ path: `.lab/screenshots/image-size-overlay-${theme}.png`, fullPage: true });
    await page.getByRole("button", { name: locale === "zh" ? "像素差分" : "Pixel", exact: true }).click();
    await expect(page.locator(".image-stage img")).toHaveAttribute("src", /^blob:/);
    await expect.poll(() => page.locator(".image-stage img").evaluate((item) => [(item as HTMLImageElement).naturalWidth, (item as HTMLImageElement).naturalHeight])).toEqual([480, 288]);
    await expect(page.locator(".image-meta-side.new .image-dimensions")).toHaveText("240 × 288 px");

    await page.setViewportSize({ width: 960, height: 640 });
    await expectFileSizes(page, () => openLabScene(page, "diff-image-staged", ".image-metadata"), locale, true);
    await expect(page.locator(".image-meta-side.old .image-dimensions")).toHaveText("360 × 216 px");
    await expect(page.locator(".image-meta-side.new .image-dimensions")).toHaveText("480 × 144 px");
    await expect(page.locator(".image-dimension-change")).toContainText("+120 px");
    await expect(page.locator(".image-dimension-change")).toContainText("−72 px");
    await expectSharedScale(page.locator(".image-stage img"), [360, 216], [480, 144]);
    expect(await page.locator(".image-content").evaluate((node) => node.scrollWidth <= node.clientWidth + 1)).toBe(true);
    await page.screenshot({ path: `.lab/screenshots/image-size-staged-${theme}.png`, fullPage: true });
  });
}
