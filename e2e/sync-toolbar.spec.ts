import { expect, test } from "@playwright/test";

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
    if (busy) await expect(fetch).toBeDisabled();
    else await expect(fetch).toBeEnabled();
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
