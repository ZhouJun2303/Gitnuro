import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "@playwright/test";
import { pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"]) {
  test(`sync skips unchanged context and reloads new remote refs (${theme})`, async ({ page, request }) => {
    test.setTimeout(90_000);
    const restored = await request.post("http://127.0.0.1:1429/invoke", { data: { cmd: "lab_restore", args: { name: "pull-noop" } } });
    expect((await restored.json()).ok).toBe(true);
    await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, fileLayout: "list" });
    await page.goto(`/?scene=pull-noop&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="pull-noop"]')).toBeVisible();
    const calls: string[] = [];
    page.on("request", (item) => { if (item.url().includes("/__lab/invoke") && item.method() === "POST") calls.push(item.postDataJSON().cmd); });
    for (const kind of ["fetch", "push", "updated-fetch"]) {
      const action = kind === "push" ? "push" : "fetch";
      if (kind === "updated-fetch") execFileSync("git", ["branch", "fresh-remote"], { cwd: resolve(".lab/repos/pull-noop-upstream") });
      const button = page.locator(".chrome-left").getByTitle(action === "fetch" ? "获取" : "推送", { exact: true });
      await button.click();
      const dialog = page.locator(`[data-dialog="${action}"]`);
      await expect(dialog).toBeVisible();
      const result = page.waitForResponse((item) => item.url().includes("/__lab/invoke") && item.request().method() === "POST" && item.request().postDataJSON().cmd === "mutate");
      calls.length = 0;
      await dialog.locator("button.commit").click();
      const payload = await (await result).json();
      expect(payload.ok).toBe(true);
      expect(payload.value.outcome).toBe("completed");
      expect(payload.value.refreshScope).toBe(kind === "updated-fetch" ? undefined : "status");
      await expect(dialog).toHaveCount(0);
      await expect(button).toBeEnabled();
      expect(calls.filter((cmd) => cmd === "repository_refs")).toHaveLength(kind === "updated-fetch" ? 1 : 0);
      expect(calls.filter((cmd) => cmd === "commit_log")).toHaveLength(kind === "updated-fetch" ? 1 : 0);
      expect(calls.filter((cmd) => cmd === "workspace_status")).toHaveLength(0);
      await expect(page.locator(".staged-pane")).toContainText("note.txt");
    }
    await page.screenshot({ path: `.lab/sync-performance-${theme}.png` });
  });

  test(`fast-forward pull keeps many tracked edits and remains scrollable (${theme})`, async ({ page, request }) => {
    test.setTimeout(90_000);
    const restored = await request.post("http://127.0.0.1:1429/invoke", { data: { cmd: "lab_restore", args: { name: "pull-disjoint" } } });
    expect((await restored.json()).ok).toBe(true);
    const repo = resolve(".lab/repos/pull-disjoint");
    const git = (...args: string[]) => execFileSync("git", args, { cwd: repo, encoding: "utf8" });
    const before = git("status", "--porcelain", "-z");
    await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, fileLayout: "list" });
    await page.goto(`/?scene=pull-disjoint&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="pull-disjoint"]')).toBeVisible();
    await page.locator(".chrome-left").getByTitle("拉取", { exact: true }).click();
    // Keep the response pending after the real write to check the waiting UI.
    let release!: () => void;
    const waiting = new Promise<void>((done) => { release = done; });
    await page.route("**/__lab/invoke", async (route) => {
      if (route.request().postDataJSON().cmd !== "mutate") return route.fallback();
      const response = await route.fetch();
      await waiting;
      return route.fulfill({ response });
    });
    const result = page.waitForResponse((item) => item.url().includes("/__lab/invoke") && item.request().method() === "POST" && item.request().postDataJSON().cmd === "mutate");
    await page.locator('[data-dialog="pull"] button.commit').click();
    try {
      const list = page.locator(".unstaged-pane .file-list");
      await list.evaluate((element) => { element.scrollTop = 5000; });
      await expect.poll(() => list.evaluate((element) => element.scrollTop)).toBeGreaterThan(1000);
      await expect(list.locator(".file").first()).toBeVisible();
      expect(await list.locator(".file").count()).toBeLessThan(200);
    } finally { release(); }
    const payload = await (await result).json();
    expect(payload.ok).toBe(true); expect(payload.value.outcome).toBe("completed");
    expect(payload.value.log).toContain("$ git merge --ff-only --no-autostash");
    expect(payload.value.log).not.toContain("$ git stash ");
    await expect(page.locator('[data-dialog="pull"]')).toHaveCount(0);
    await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
    expect(git("status", "--porcelain", "-z")).toBe(before);
    expect(git("show", ":note.txt")).toBe("staged\n");
    expect(readFileSync(resolve(repo, "note.txt"), "utf8")).toBe("working\n");
    expect(readFileSync(resolve(repo, "incoming.txt"), "utf8")).toBe("incoming\n");
    expect(payload.value.unstaged.filter((file: { path: string }) => file.path.startsWith("tracked/"))).toHaveLength(1000);
    await page.screenshot({ path: `.lab/pull-disjoint-${theme}.png` });
  });
}
