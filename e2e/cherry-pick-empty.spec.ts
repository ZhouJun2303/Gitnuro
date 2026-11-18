import { mkdirSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

const emptyNotice = "The previous cherry-pick is now empty, possibly due to conflict resolution.\nIf you wish to commit it anyway, use: git commit --allow-empty\nOtherwise, please use 'git cherry-pick --skip'";

for (const theme of ["apple", "light", "dark"] as const) {
  test(`empty cherry-pick ${theme} continues without a false conflict dialog`, async ({ page }) => {
    const writes: Record<string, unknown>[] = [];
    await pinLabSettings(page, { locale: "zh", theme, statusInterval: 0, fetchAutomatically: false });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd, args } = route.request().postDataJSON();
      if (cmd === "in_progress") return route.fulfill({ json: { ok: true, value: "cherryPick" } });
      if (cmd !== "mutate") return route.fallback();
      writes.push(args.request);
      return route.fulfill({ json: { ok: false, error: emptyNotice } });
    });
    await openLabScene(page, "cherry-pick-paused", "#commit-feedback");
    const next = page.locator(".composer button.commit");
    await expect(next).toHaveText("继续拣选");
    await expect(next).toBeEnabled();
    await expect(page.locator("#commit-feedback")).toContainText("没有改动的提交会自动跳过");
    await expect(page.locator("#commit-feedback")).not.toContainText("解决冲突");
    mkdirSync(".lab/cherry-pick-empty/screenshots", { recursive: true });
    await page.screenshot({ path: `.lab/cherry-pick-empty/screenshots/${theme}.png` });
    await next.click();
    await expect.poll(() => writes.length).toBe(1);
    expect(writes[0]).toEqual({ action: "continue" });
    await expect(page.locator('.composer [role="alert"]')).toContainText("The previous cherry-pick is now empty");
    await expect(page.locator('[data-dialog="mergeConflict"]')).toHaveCount(0);
    await expect(next).toBeEnabled();
  });

  test(`empty cherry-pick ${theme} keeps actual conflicts blocking continuation`, async ({ page }) => {
    const writes: unknown[] = [];
    await pinLabSettings(page, { locale: "en", theme, statusInterval: 0, fetchAutomatically: false });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd, args } = route.request().postDataJSON();
      if (cmd !== "mutate") return route.fallback();
      writes.push(args.request);
      return route.fulfill({ json: { ok: false, error: "fixture: must not run" } });
    });
    await openLabScene(page, "conflict-banner", "#commit-feedback");
    await expect(page.locator("#commit-feedback")).toContainText("Resolve and stage all conflicted files");
    await expect(page.locator(".composer button.commit")).toBeDisabled();
    await page.keyboard.press("Control+Enter");
    expect(writes).toEqual([]);
  });
}
