import { execFileSync } from "node:child_process";
import { expect, test, type Page } from "@playwright/test";
import type { RefSnapshot } from "../src/lib/status";
import { pinLabSettings } from "./helpers";

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

for (const theme of ["apple", "light", "dark"] as const) {
  test(`${theme}: tags and stashes show ten more results independently`, async ({ page }) => {
    await pinLabSettings(page, { theme, locale: "en" });
    await page.route("**/__lab/invoke", async (route) => {
      if (route.request().postDataJSON()?.cmd !== "repository_refs") return route.fallback();
      const response = await route.fetch();
      const payload: { ok: boolean; value: RefSnapshot } = await response.json();
      const firstTag = payload.value.tags.find(({ name }) => name === "v2.0.1")!;
      payload.value.tags = [
        ...Array.from({ length: 10 }, (_, index) => ({ ...firstTag, name: `hidden/release-${index}` })),
        firstTag,
        ...Array.from({ length: 22 }, (_, index) => ({ ...firstTag, name: `page-${22 - index}` })),
      ];
      payload.value.stashes = Array.from({ length: 23 }, (_, index) => ({
        name: `stash@{${index}}`, summary: `page-${23 - index}`,
      }));
      await route.fulfill({ json: payload });
    });
    await openTags(page);

    const sidebar = page.locator(".sidebar");
    const tagRows = sidebar.locator('.file-select[title="v2.0.1"], .file-select[title^="page-"]');
    const stashRows = sidebar.locator('.file-select[title^="stash@"]');
    const moreTags = sidebar.getByRole("button", { name: "Show more (Tags)", exact: true });
    const moreStashes = sidebar.getByRole("button", { name: "Show more (Stashes)", exact: true });
    await sidebar.getByRole("button", { name: /^Stashes/, expanded: false }).click();
    await expect(sidebar.getByRole("button", { name: "hidden", expanded: false })).toBeVisible();
    await expect(tagRows).toHaveCount(10);
    await expect(stashRows).toHaveCount(10);

    await moreTags.click();
    await expect(tagRows).toHaveCount(20);
    await expect(stashRows).toHaveCount(10);
    await moreStashes.click();
    await expect(stashRows).toHaveCount(20);
    await moreTags.click();
    await expect(tagRows).toHaveCount(23);
    await expect(moreTags).toHaveCount(0);
    await moreStashes.click();
    await expect(stashRows).toHaveCount(23);
    await expect(moreStashes).toHaveCount(0);

    const filter = sidebar.locator(".side-filter input");
    await filter.fill("page-1");
    await expect(tagRows).toHaveCount(10);
    await expect(stashRows).toHaveCount(10);
    await moreStashes.click();
    await expect(stashRows.last()).toHaveAttribute("title", "stash@{22}");
    await filter.fill("page-2");
    await expect(tagRows).toHaveCount(4);
    await expect(stashRows).toHaveCount(5);
    await expect(moreTags.or(moreStashes)).toHaveCount(0);
    await filter.clear();
    await expect(tagRows).toHaveCount(10);
    await expect(stashRows).toHaveCount(10);
  });
}
