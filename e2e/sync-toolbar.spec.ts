import { expect, test } from "@playwright/test";
import { pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"] as const) {
  test(`busy toolbar sweep keeps its 300ms duration across motion preferences (${theme})`, async ({ page }) => {
    await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false });
    await page.goto(`/?scene=sync-busy&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="sync-busy"]')).toBeVisible();
    const sweep = page.locator(".identity-sweep");
    await expect(sweep).toBeVisible();

    for (const reducedMotion of ["no-preference", "reduce", "no-preference"] as const) {
      await page.emulateMedia({ reducedMotion });
      const timing = await sweep.evaluate((node) => {
        const style = getComputedStyle(node, "::before");
        const animation = node.getAnimations({ subtree: true }).find((item) => item instanceof CSSAnimation && item.animationName === "busy-flow");
        return {
          reducedMotion: matchMedia("(prefers-reduced-motion: reduce)").matches,
          duration: style.animationDuration,
          activeDuration: animation?.effect?.getTiming().duration,
          playState: animation?.playState,
        };
      });
      expect(timing.reducedMotion).toBe(reducedMotion === "reduce");
      expect(timing.duration, `${theme}: ${reducedMotion}`).toBe("0.3s");
      expect(timing.activeDuration).toBe(300);
      expect(timing.playState).toBe("running");
    }

    await page.screenshot({ path: `.lab/sweep-duration-${theme}.png` });
  });
}

for (const scene of ["rebase-progress", "sync-stash-conflict", "sync-busy"]) {
  test(`sync toolbar respects ${scene} in buttons, menus and shortcuts`, async ({ page }) => {
    const mutations: string[] = [];
    page.on("request", (request) => {
      if (!request.url().includes("/__lab/invoke") || request.method() !== "POST") return;
      const body = request.postDataJSON();
      if (body.cmd === "mutate") mutations.push(body.args.request.action);
    });
    await page.goto(`/?scene=${scene}`);
    await expect(page.locator(`[data-lab-scene="${scene}"]`)).toBeVisible();
    const toolbar = page.locator(".chrome-left");
    const fetch = toolbar.getByTitle(/^(获取|Fetch)$/);
    const pull = toolbar.getByTitle(/^(拉取|Pull)$/);
    const push = toolbar.getByTitle(/^(推送|Push)$/);
    const busy = scene === "sync-busy";
    await expect(fetch).toBeEnabled();
    if (busy) {
      // A task's activity indicator does not lock unrelated controls or browsing.
      await expect(pull).toBeEnabled();
      await expect(push).toBeEnabled();
      await pull.click();
      await expect(page.locator('[data-dialog="pull"]')).toBeVisible();
      await page.keyboard.press("Escape");
      await push.click();
      await expect(page.locator('[data-dialog="push"]')).toBeVisible();
      await page.keyboard.press("Escape");
      expect(mutations).toEqual([]);
      return;
    }
    await expect(pull).toBeDisabled();
    await expect(push).toBeDisabled();

    await page.locator(".menubar").getByRole("button", { name: /^(仓库|Repository)$/ }).click();
    const menu = page.locator('[data-layer="menu"]');
    await expect(menu.getByRole("menuitem", { name: /^(拉取|Pull)$/ })).toBeDisabled();
    await expect(menu.getByRole("menuitem", { name: /^(推送|Push)$/ })).toBeDisabled();
    await page.keyboard.press("Escape");
    await page.keyboard.press("Control+Shift+L");
    await page.keyboard.press("Control+Shift+P");
    await page.keyboard.press("Control+Alt+Shift+L");
    await page.keyboard.press("Control+Alt+Shift+P");
    if (busy) await page.keyboard.press("Control+Shift+F");
    // Let any accidentally queued shortcut invoke reach the request listener.
    await page.waitForTimeout(150);
    expect(mutations).toEqual([]);
    await expect(page.locator('[data-dialog="pull"], [data-dialog="push"]')).toHaveCount(0);

    if (!busy) {
      await fetch.click();
      await expect(page.locator('[data-dialog="fetch"]')).toBeVisible();
      await page.keyboard.press("Escape");
    }
    await page.screenshot({ path: `.lab/${scene}-sync-toolbar.png` });
  });
}

test("sync toolbar enables all actions when the repository has no unfinished operation", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  for (const [kind, label] of [["fetch", /^(获取|Fetch)$/], ["pull", /^(拉取|Pull)$/], ["push", /^(推送|Push)$/]] as const) {
    const button = page.locator(".chrome-left").getByTitle(label);
    await expect(button).toBeEnabled();
    await button.click();
    await expect(page.locator(`[data-dialog="${kind}"]`)).toBeVisible();
    await page.keyboard.press("Escape");
  }
});
