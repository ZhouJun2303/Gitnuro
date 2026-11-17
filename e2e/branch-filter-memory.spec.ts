import { expect, test, type Page } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

async function toggleBranch(page: Page, name: string) {
  const toggle = page.locator(`.history-filter-toggle[data-branch="${name}"]`);
  await page.locator(".side.nested").filter({ has: toggle }).hover();
  await toggle.click();
}

async function clearFilter(page: Page) {
  const clear = page.locator(".section-filter-clear");
  if (await clear.count()) await clear.click();
  await expect(page.locator('.history-filter-toggle[aria-pressed="true"]')).toHaveCount(0);
}

for (const theme of ["apple", "light", "dark"] as const) {
  test(`${theme}: repositories remember separate branch filters after reopening and clearing`, async ({ page, browser }) => {
    await pinLabSettings(page, { theme });
    await openLabScene(page, "history", ".sidebar", { locale: "en" });
    await clearFilter(page);
    for (const name of ["topic", "graph"]) await toggleBranch(page, name);
    await expect(page.locator('.history-filter-toggle[aria-pressed="true"]')).toHaveCount(2);

    await openLabScene(page, "history-remote", ".sidebar", { locale: "en" });
    await expect(page.locator('.history-filter-toggle[aria-pressed="true"]')).toHaveCount(0);
    await clearFilter(page);
    await toggleBranch(page, "main");

    await openLabScene(page, "history", ".sidebar", { locale: "en" });
    for (const name of ["topic", "graph"]) {
      await expect(page.locator(`.history-filter-toggle[data-branch="${name}"]`)).toHaveAttribute("aria-pressed", "true");
    }
    await expect(page.locator('.history-filter-toggle[aria-pressed="true"]')).toHaveCount(2);
    await expect(page.locator(".section-filter-clear")).toBeVisible();

    const origin = new URL(page.url()).origin;
    const reopened = await browser.newContext({ baseURL: origin, storageState: await page.context().storageState() });
    let releaseReopen!: () => void;
    const reopening = new Promise<void>((resolve) => { releaseReopen = resolve; });
    try {
      const next = await reopened.newPage();
      await pinLabSettings(next, { theme });
      await next.route("**/__lab/invoke", async (route) => {
        if (route.request().postDataJSON()?.cmd === "commit_log") await reopening;
        await route.fallback();
      });
      await next.goto("/?scene=history&locale=en");
      await expect(next.locator(".section-filter-clear")).toBeVisible();
      await expect(next.locator('.history-filter-toggle[aria-pressed="true"]')).toHaveCount(2);
      releaseReopen();
      await expect(next.locator('[data-lab-scene="history"]')).toBeVisible();
      await clearFilter(next);
      await openLabScene(next, "history", ".sidebar", { locale: "en" });
      await expect(next.locator('.history-filter-toggle[aria-pressed="true"]')).toHaveCount(0);
      await expect(next.locator(".section-filter-clear")).toHaveCount(0);
      await openLabScene(next, "history-remote", ".sidebar", { locale: "en" });
      await expect(next.locator('.history-filter-toggle[data-branch="main"]')).toHaveAttribute("aria-pressed", "true");
    } finally {
      releaseReopen();
      for (const next of reopened.pages()) await next.unrouteAll({ behavior: "wait" });
      await reopened.close();
    }
  });

  test(`${theme}: cancel filter icons update while history loading is blocked`, async ({ page }) => {
    await pinLabSettings(page, { theme });
    await openLabScene(page, "history", ".sidebar", { locale: "en" });
    await clearFilter(page);
    let release!: () => void;
    const pending = new Promise<void>((resolve) => { release = resolve; });
    await page.route("**/__lab/invoke", async (route) => {
      const body = route.request().postDataJSON();
      if (body?.cmd === "repository_refs") await pending;
      await route.fallback();
    });
    try {
      await toggleBranch(page, "topic");
      const toggle = page.locator('.history-filter-toggle[data-branch="topic"]');
      await page.locator(".side-filter input").hover();
      await expect(toggle).toHaveAttribute("aria-pressed", "true");
      await expect(toggle).toHaveAttribute("title", "Remove branch from history filter");
      await expect(toggle.locator("..")).toHaveCSS("opacity", "1");
      const clear = page.locator(".section-filter-clear");
      await expect(clear).toBeVisible();
      await clear.click();
      await expect(clear).toHaveCount(0);
      await expect(toggle).toHaveAttribute("aria-pressed", "false");
      await expect(toggle).toHaveAttribute("title", "Include branch in history filter (multiple selection)");
    } finally {
      release();
      await page.unrouteAll({ behavior: "wait" });
    }
  });

  test(`${theme}: saved remote filters stay separate from matching local branch names`, async ({ page }) => {
    await pinLabSettings(page, { theme });
    await openLabScene(page, "history", ".sidebar", { locale: "en" });
    await page.evaluate(() => {
      const key = Object.keys(localStorage).find((key) => key.startsWith("awegit.history-filter."));
      if (!key) throw new Error("History filter was not saved");
      localStorage.setItem(key, JSON.stringify({ allBranches: false, branches: ["main", "origin/main"] }));
    });
    const log = page.waitForRequest((request) => request.url().endsWith("/__lab/invoke") && request.postDataJSON()?.cmd === "commit_log");
    await openLabScene(page, "history", ".sidebar", { locale: "en" });
    expect((await log).postDataJSON().args.revs).toEqual(expect.arrayContaining(["refs/heads/main", "refs/remotes/origin/main"]));
    await expect(page.locator(".filter-bar")).toContainText("'main', 'origin/main'");
    await expect(page.locator(".section-filter-clear")).toBeVisible();
  });
}
