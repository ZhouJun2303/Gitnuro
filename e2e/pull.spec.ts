import { expect, test } from "@playwright/test";
import { pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"]) {
  test(`pull preserves local deletions and staged content (${theme})`, async ({ page, request }) => {
    test.setTimeout(90_000);
    const response = await request.post("http://127.0.0.1:1429/invoke", {
      data: { cmd: "lab_restore", args: { name: "pull-deleted" } },
    });
    expect((await response.json()).ok).toBe(true);
    await pinLabSettings(page, { theme, locale: "zh", pullRebase: false, fetchAutomatically: false, fileLayout: "list" });
    await page.goto(`/?scene=pull-deleted&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="pull-deleted"]')).toBeVisible();
    // Check the actual backend result as well as the visible list: an added
    // file can look plausible while its original worktree deletion was lost.
    const result = page.waitForResponse((item) => {
      if (!item.url().includes("/__lab/invoke") || item.request().method() !== "POST") return false;
      const body = item.request().postDataJSON();
      return body.cmd === "mutate" && ["pull", "pullRef"].includes(body.args.request.action);
    });
    await page.locator(".chrome-left").getByTitle("拉取", { exact: true }).click();
    await page.locator('[data-dialog="pull"] button.commit').click();
    const payload = await (await result).json();
    expect(payload.ok).toBe(true);
    expect(payload.value.outcome).toBe("completed");
    await expect(page.locator('[data-dialog="pull"]')).toHaveCount(0);
    await expect(page.locator('.identity-floor[data-layer="error"]')).toHaveCount(0);
    await expect(page.locator(".chrome-left").getByTitle("拉取", { exact: true })).toBeEnabled();
    await expect(page.locator(".staged-pane")).toContainText("new.txt");
    await expect(page.locator(".unstaged-pane")).toContainText("new.txt");
    await expect(page.locator(".unstaged-pane")).toContainText("deleted.txt");
    await page.screenshot({ path: `.lab/pull-deleted-${theme}.png` });
  });
}
