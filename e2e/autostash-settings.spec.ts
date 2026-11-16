import { execFileSync, spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "@playwright/test";

for (const theme of ["apple", "light", "dark"]) {
  test(`automatic stash setting persists and controls new files (${theme})`, async ({ page, request }) => {
    test.setTimeout(90_000);
    const invoke = async (cmd: string, args = {}) => {
      const response = await request.post("http://127.0.0.1:1429/invoke", { data: { cmd, args } });
      const payload = await response.json();
      expect(payload.ok).toBe(true);
      return payload.value;
    };
    const original = await invoke("load_settings");
    const locale = theme === "dark" ? "en" : "zh";
    const repo = resolve(".lab", "repos", "checkout-index");
    const git = (...args: string[]) => execFileSync("git", args, { cwd: repo, encoding: "utf8" });
    const checkbox = page.locator('input[name="autostashIncludeUntracked"]');
    await page.setViewportSize({ width: theme === "dark" ? 960 : 1440, height: theme === "dark" ? 640 : 900 });
    try {
      await invoke("save_settings", { values: {
        ...original, theme, locale, autostashIncludeUntracked: false, stashIncludeUntracked: true,
        fetchAutomatically: false, flowAutoOpen: false,
      } });
      await invoke("lab_restore", { name: "checkout-index" });
      await page.goto(`/?scene=prefs-autostash&theme=${theme}&locale=${locale}`);
      await expect(page.locator('[data-lab-scene="prefs-autostash"]')).toBeVisible();
      await expect(checkbox).not.toBeChecked();
      // The existing manual stash preference is independent from automatic protection.
      await expect(page.getByRole("checkbox", { name: locale === "en" ? "Include untracked files" : "包含未跟踪文件", exact: true })).toBeChecked();
      for (const enabled of [true, false]) {
        await checkbox.scrollIntoViewIfNeeded();
        await checkbox.setChecked(enabled);
        await expect(page.locator("#autostash-untracked-hint")).toBeInViewport();
        const saved = page.waitForResponse((response) => response.url().includes("/__lab/invoke")
          && response.request().method() === "POST"
          && response.request().postDataJSON().cmd === "save_settings");
        await page.locator('[data-dialog="prefs"] button.commit').click();
        const payload = await (await saved).json();
        expect(payload.ok).toBe(true);
        expect(payload.value.autostashIncludeUntracked).toBe(enabled);
        expect((await invoke("load_settings")).autostashIncludeUntracked).toBe(enabled);

        // Reload the UI and use an actual write, rather than faking the mutation result.
        await page.goto("about:blank");
        await invoke("lab_restore", { name: "checkout-index" });
        await page.goto(`/?scene=prefs-autostash&theme=${theme}&locale=${locale}`);
        await expect(page.locator('[data-lab-scene="prefs-autostash"]')).toBeVisible();
        if (enabled) await expect(checkbox).toBeChecked();
        else await expect(checkbox).not.toBeChecked();
        await checkbox.scrollIntoViewIfNeeded();
        await expect(page.locator("#autostash-untracked-hint")).toBeInViewport();
        await page.screenshot({ path: `.lab/autostash-settings-${theme}-${enabled}.png`, animations: "disabled" });
        await page.locator('[data-dialog="prefs"] .dialog-close').click();
        const result = page.waitForResponse((response) => response.url().includes("/__lab/invoke")
          && response.request().method() === "POST"
          && response.request().postDataJSON().cmd === "mutate");
        await page.locator('.sidebar .file-select[title="topic"]').dblclick();
        const report = await (await result).json();
        expect(report.ok).toBe(true);
        expect(report.value.outcome).toBe("completed");
        await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
        const push = report.value.log.split("\n").find((line: string) => line.startsWith("$ git stash push "));
        expect(push.includes("--include-untracked")).toBe(enabled);
        const untracked = spawnSync("git", ["cat-file", "-e", "stash@{0}^3:loose.txt"], { cwd: repo });
        expect(untracked.status === 0).toBe(enabled);
        expect(git("show", "stash@{0}^2:keep.txt")).toBe("staged\n");
        expect(readFileSync(resolve(repo, "loose.txt"), "utf8")).toBe("untracked\n");
        if (enabled) {
          await page.keyboard.press("Control+,");
          await page.locator('[data-dialog="prefs"]').getByRole("button", { name: "Git", exact: true }).click();
        }
      }
    } finally {
      await page.goto("about:blank");
      await invoke("save_settings", { values: original });
    }
  });
}
