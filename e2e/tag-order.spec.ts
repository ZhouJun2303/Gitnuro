import { execFileSync } from "node:child_process";
import { expect, test, type Page } from "@playwright/test";

test.beforeAll(() => {
  execFileSync(process.execPath, ["scripts/lab-seed.mjs", "tags"], { encoding: "utf8" });
});

async function openTags(page: Page) {
  await page.goto("/?scene=tag-order", { waitUntil: "domcontentloaded" });
  await expect(page.locator('[data-lab-scene="tag-order"]')).toBeVisible({ timeout: 90_000 });
  await expect(page.locator('.sidebar .file-select[title="v2.0.1"]')).toBeVisible();
}

const tags = (page: Page) => page.locator('.sidebar .side.nested[role="group"] .file-select');

test("tags keep newest dates first across lightweight and annotated releases", async ({ page }) => {
  await openTags(page);
  const releases = tags(page).filter({ has: page.locator('span.name', { hasText: /^v2\./ }) });
  await expect(releases).toHaveText(["v2.0.1", "v2.0.0", "v2.0.10", "v2.0.6"]);
  await page.locator(".side-pin .side-filter input").fill("v2.0.");
  await expect(releases).toHaveText(["v2.0.1", "v2.0.0", "v2.0.10", "v2.0.6"]);
  await page.locator(".side-pin .side-filter input").fill("v2.0.1");
  await expect(releases).toHaveText(["v2.0.1", "v2.0.10"]);
});

test("tags keep interleaved folders contiguous and ordered by their newest tag", async ({ page }) => {
  await openTags(page);
  await expect.poll(() => tags(page).evaluateAll((nodes) => nodes.map((node) => node.getAttribute("title")))).toEqual([
    "release/v1", "release/v8", "archive/v1", "archive/v9", "v2.0.1", "v2.0.0", "v2.0.10", "v2.0.6",
  ]);
  const folders = page.locator(".sidebar button.side.nested[aria-expanded]");
  await expect(folders).toHaveText(["release", "archive"]);
  await folders.first().click();
  await expect(tags(page)).toHaveCount(6);
  await folders.first().click();
  await expect(tags(page)).toHaveCount(8);
});
