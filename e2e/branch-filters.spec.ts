import { expect, test } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"] as const) {
  test(`${theme}: branch history supports multiple selections, removal and clearing`, async ({ page }) => {
    await pinLabSettings(page, { theme });
    await openLabScene(page, "history-branches", ".filter-bar", { locale: "en" });
    const heading = page.locator(".side.section").filter({ has: page.locator(".section-filter") }).locator(".name");
    await expect(heading).toHaveText("Branches");
    expect(await heading.evaluate((node) => node.scrollWidth - node.clientWidth)).toBeLessThanOrEqual(1);
    await expect(page.locator(".filter-bar")).toContainText("'main', 'topic'");
    await expect(page.locator('.history-filter-toggle[aria-pressed="true"]')).toHaveCount(2);
    await page.locator(".section-filter-clear").click();
    await expect(page.locator(".filter-bar")).toHaveCount(0);

    const selected: string[] = [];
    for (const name of ["main", "topic", "graph"]) {
      await page.locator(".side-filter input").fill(name);
      const toggle = page.locator(`.history-filter-toggle[data-branch="${name}"]`);
      await page.locator(".side.nested").filter({ has: toggle }).hover();
      const read = page.waitForResponse((response) => response.url().endsWith("/__lab/invoke") && response.request().postDataJSON()?.cmd === "commit_log"
        && response.request().postDataJSON().args.revs?.includes(`refs/heads/${name}`));
      await toggle.click();
      selected.push(name);
      const response = await read;
      expect(response.request().postDataJSON().args.revs).toEqual(expect.arrayContaining(selected.map((branch) => `refs/heads/${branch}`)));
      expect((await response.json()).ok).toBe(true);
      await expect(toggle).toHaveAttribute("aria-pressed", "true");
      for (const branch of selected) await expect(page.locator(".filter-bar")).toContainText(`'${branch}'`);
    }
    await page.locator(".side-filter input").fill("");
    await expect(page.locator('.history-filter-toggle[aria-pressed="true"]')).toHaveCount(3);
    await page.screenshot({ path: `.lab/screenshots/branch-filters-${theme}.png`, fullPage: true });
    await page.locator('.history-filter-toggle[data-branch="topic"]').click();
    await expect(page.locator(".filter-bar")).not.toContainText("'topic'");
    await expect(page.locator('.history-filter-toggle[aria-pressed="true"]')).toHaveCount(2);
    await page.locator(".section-filter-clear").click();
    await expect(page.locator(".filter-bar")).toHaveCount(0);
    await expect(page.locator('.history-filter-toggle[aria-pressed="true"]')).toHaveCount(0);
  });

  test(`${theme}: a deleted upstream does not enter the history filter`, async ({ page }) => {
    await pinLabSettings(page, { theme });
    const requests: { revs?: string[]; defaultBranch?: boolean }[] = [];
    page.on("request", (request) => {
      if (!request.url().endsWith("/__lab/invoke")) return;
      const body = request.postDataJSON();
      if (body?.cmd === "commit_log") requests.push(body.args);
    });
    await openLabScene(page, "history-gone-upstream", ".filter-bar", { locale: "en" });
    const filtered = requests.filter(({ defaultBranch }) => !defaultBranch);
    expect(filtered.length).toBeGreaterThan(0);
    for (const { revs } of filtered) expect(revs).toEqual(["refs/heads/Dev/EnterFast_2026_10_27"]);
    await expect(page.locator(".filter-bar")).toContainText("'Dev/EnterFast_2026_10_27'");
    await expect(page.locator('.identity-floor[data-layer="error"]')).toHaveCount(0);
    await expect(page.locator(".commit-row").first()).toBeVisible();
  });
}
