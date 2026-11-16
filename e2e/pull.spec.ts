import { expect, test } from "@playwright/test";
import { pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"]) {
  test(`empty pull keeps local files and skips history refresh (${theme})`, async ({ page, request }, testInfo) => {
    test.setTimeout(90_000);
    const restored = await request.post("http://127.0.0.1:1429/invoke", {
      data: { cmd: "lab_restore", args: { name: "pull-noop" } },
    });
    expect((await restored.json()).ok).toBe(true);
    await pinLabSettings(page, { theme, locale: "zh", pullRebase: true, fetchAutomatically: false, fileLayout: "list" });
    await page.goto(`/?scene=pull-noop&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="pull-noop"]')).toBeVisible();
    await expect(page.locator(".staged-pane")).toContainText("note.txt");
    await expect(page.locator(".unstaged-pane")).toContainText("1001");
    const calls: string[] = [];
    page.on("request", (item) => {
      if (item.url().includes("/__lab/invoke") && item.method() === "POST") calls.push(item.postDataJSON().cmd);
    });
    await page.locator(".chrome-left").getByTitle("拉取", { exact: true }).click();
    const result = page.waitForResponse((item) => item.url().includes("/__lab/invoke") && item.request().method() === "POST" && item.request().postDataJSON().cmd === "mutate");
    calls.length = 0;
    const start = Date.now();
    await page.locator('[data-dialog="pull"] button.commit').click();
    const payload = await (await result).json();
    expect(payload.ok).toBe(true);
    expect(payload.value.outcome).toBe("completed");
    expect(payload.value.refreshScope).toBe("status");
    expect(payload.value.staged.find((file: { path: string }) => file.path === "note.txt")).toBeTruthy();
    expect(payload.value.unstaged.filter((file: { kind: string }) => file.kind === "untracked")).toHaveLength(1000);
    expect(payload.value.unstaged.find((file: { path: string }) => file.path === "note.txt")).toBeTruthy();
    expect(payload.value.log).toContain("$ git fetch ");
    expect(payload.value.log).not.toMatch(/\$ git (stash|pull|merge) /);
    await expect(page.locator('[data-dialog="pull"]')).toHaveCount(0);
    await expect(page.locator(".chrome-left").getByTitle("拉取", { exact: true })).toBeEnabled();
    expect(calls.filter((cmd) => ["repository_refs", "commit_log", "workspace_status"].includes(cmd))).toEqual([]);
    await testInfo.attach("empty-pull-timing", { body: JSON.stringify({ theme, clickToReadyMs: Date.now() - start }), contentType: "application/json" });
    await expect(page.locator(".staged-pane")).toContainText("note.txt");
    await page.locator(".unstaged-pane .file-list").evaluate((element) => { element.scrollTop = 5000; });
    await expect(page.locator(".unstaged-pane .file-list")).toBeVisible();
    await page.locator(".history-btn").click();
    await page.locator(".command-filters").getByRole("button", { name: "用户", exact: true }).click();
    await expect(page.locator(".command-detail")).toContainText("total=");
    await expect(page.locator(".command-detail strong.git-command").filter({ hasText: "$ git fetch " })).toHaveCount(1);
    await page.screenshot({ path: `.lab/pull-noop-${theme}.png` });
  });

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
    await page.locator(".history-btn").click();
    await page.locator(".command-filters").getByRole("button", { name: "用户", exact: true }).click();
    const detail = page.locator(".command-detail");
    const pull = detail.locator("strong.git-command").filter({ hasText: "--no-autostash" });
    await expect(pull).toHaveCount(1);
    await expect(pull).toContainText("--no-autostash");
    await expect(pull).toContainText("--no-rebase");
    await expect(pull).toContainText(/\[\d+ ms, exit 0\]/);
    await expect(pull).toHaveCSS("font-weight", "700");
    await expect(detail).toContainText("[AweGit] backend=");
    await expect(detail).toContainText("total=");
    await page.screenshot({ path: `.lab/pull-deleted-${theme}.png` });
  });
}
