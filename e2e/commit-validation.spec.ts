import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

const themes = [
  { name: "apple", locale: "zh", width: 1440, height: 900 },
  { name: "light", locale: "zh", width: 1440, height: 900 },
  { name: "dark", locale: "en", width: 960, height: 640 },
] as const;
const audit = ".lab/commit-feedback-20261118/screenshots";

async function watchWrites(page: Page) {
  const calls: { cmd: string; args: { request?: Record<string, unknown> } }[] = [];
  await page.route("**/__lab/invoke", async (route) => {
    const body = route.request().postDataJSON();
    if (body.cmd !== "commit_changes" && body.cmd !== "mutate") return route.fallback();
    calls.push(body);
    // A broken guard must never change the shared fixture repository.
    return route.fulfill({ json: { ok: false, error: "fixture: commit hook rejected the message" } });
  });
  return calls;
}

for (const theme of themes) {
  test.describe(theme.name, () => {
    test.beforeEach(async ({ page }) => {
      await page.setViewportSize({ width: theme.width, height: theme.height });
      await pinLabSettings(page, {
        theme: theme.name, locale: theme.locale, messageHigh: 70, messageLow: 50,
        fetchAutomatically: false, statusInterval: 0, pushOnCommit: false,
      });
    });

    test(`commit validation explains the limit and preserves drafts (${theme.name})`, async ({ page }) => {
      const calls = await watchWrites(page);
      await openLabScene(page, "commit-too-long", "#commit-feedback");
      const subject = page.locator(".composer .summary-input");
      const feedback = page.locator("#commit-feedback");
      const commit = page.locator(".composer button.commit");
      await expect(feedback).toContainText("86");
      await expect(feedback).toContainText("70");
      await expect(feedback).toContainText("16");
      await expect(subject).toHaveAttribute("aria-invalid", "true");
      await expect(commit).toBeDisabled();
      await expect(commit).toHaveAttribute("title", await feedback.innerText());
      await expect(page.locator(".summary-count")).toHaveText("86/70");

      await subject.fill("x".repeat(140));
      await expect(subject).toHaveValue("x".repeat(140));
      await expect(feedback).toContainText("146");
      await page.locator(".type-chip").click();
      await page.getByRole("option", { name: /^refactor/ }).click();
      await expect(subject).toHaveValue("x".repeat(140));
      await expect(feedback).toContainText("150");
      await page.keyboard.press("Control+Enter");
      await expect(page.locator('.identity-floor[data-layer="error"]')).toContainText("150");
      await page.keyboard.press("Control+Shift+Enter");
      await page.locator(".composer").dispatchEvent("submit");
      expect(calls).toEqual([]);

      mkdirSync(audit, { recursive: true });
      await page.screenshot({ path: `${audit}/commit-${theme.name}.png` });
      await expect(commit).toBeInViewport();
      await subject.fill("x".repeat(60));
      await expect(page.locator(".summary-count")).toHaveText("70/70");
      await expect(commit).toBeEnabled();
      await expect(feedback).toHaveCount(0);
      await expect(page.locator('.identity-floor[data-layer="error"]')).toHaveCount(0);
      await subject.fill("x".repeat(61));
      await expect(feedback).toContainText("71");
      await expect(commit).toBeDisabled();
      await subject.fill("");
      await expect(feedback).toHaveText(theme.locale === "zh" ? "请填写提交标题。" : "Enter a commit subject.");
      await subject.fill("a valid subject");
      await page.locator(".composer-desc").fill("Long body details. ".repeat(80));
      await expect(commit).toBeEnabled();
    });

    test(`commit validation explains staging and allows amend (${theme.name})`, async ({ page }) => {
      const calls = await watchWrites(page);
      await openLabScene(page, "commit-no-staged", "#commit-feedback");
      const feedback = page.locator("#commit-feedback");
      const commit = page.locator(".composer button.commit");
      await expect(feedback).toContainText(theme.locale === "zh" ? "请先暂存" : "Stage the changes");
      await expect(commit).toBeDisabled();
      await page.keyboard.press("Control+Enter");
      await expect(page.locator('.identity-floor[data-layer="error"]')).toContainText(theme.locale === "zh" ? "请先暂存" : "Stage the changes");
      expect(calls).toEqual([]);
      await page.locator('.composer .check input[type="checkbox"]').check();
      await expect(commit).toBeEnabled();
      await expect(feedback).toHaveCount(0);
    });

    test(`continue validation checks edited subjects and keeps original messages usable (${theme.name})`, async ({ page }) => {
      const calls = await watchWrites(page);
      await openLabScene(page, "continue-too-long", "#dialog-subject-error");
      const dialog = page.locator('[data-dialog="continue"]');
      const subject = dialog.locator('input:not([type="checkbox"])');
      const feedback = dialog.locator("#dialog-subject-error");
      const next = dialog.locator("button.commit");
      await expect(feedback).toContainText("86");
      await expect(feedback).toContainText("70");
      await expect(feedback).toContainText("16");
      await expect(next).toBeDisabled();
      await expect(subject).toHaveAttribute("aria-invalid", "true");
      await subject.press("Enter");
      await dialog.locator("form").dispatchEvent("submit");
      expect(calls).toEqual([]);
      mkdirSync(audit, { recursive: true });
      await page.screenshot({ path: `${audit}/continue-${theme.name}.png` });

      await subject.fill("x".repeat(70));
      await expect(next).toBeEnabled();
      await expect(feedback).toHaveCount(0);
      await subject.fill(" ");
      await expect(next).toBeDisabled();
      await expect(feedback).toHaveText(theme.locale === "zh" ? "请填写提交标题。" : "Enter a commit subject.");
      await dialog.locator('input[type="checkbox"]').uncheck();
      await expect(subject).toHaveCount(0);
      await expect(feedback).toHaveCount(0);
      await expect(next).toBeEnabled();
      await next.click();
      await expect.poll(() => calls.length).toBe(1);
      expect(calls[0].args.request).toMatchObject({ action: "continue", message: "" });
      // The paused flow's identity banner must not hide a failed continuation.
      await expect(page.locator('.composer [role="alert"]')).toContainText("fixture: commit hook rejected the message");
      await expect(page.locator('.identity-floor[data-layer="progress"]')).toBeVisible();
    });

    test(`reword validation explains invalid subjects before running Git (${theme.name})`, async ({ page }) => {
      const calls = await watchWrites(page);
      await openLabScene(page, "reword-too-long", "#dialog-subject-error");
      const dialog = page.locator('[data-dialog="reword"]');
      const subject = dialog.locator("input");
      const next = dialog.locator("button.commit");
      await expect(next).toBeDisabled();
      await expect(dialog.locator("#dialog-subject-error")).toContainText("86");
      await dialog.locator("form").dispatchEvent("submit");
      expect(calls).toEqual([]);
      await subject.fill("");
      await expect(next).toBeDisabled();
      await expect(dialog.locator("#dialog-subject-error")).toHaveText(theme.locale === "zh" ? "请填写提交标题。" : "Enter a commit subject.");
      await subject.fill("a valid replacement subject");
      await expect(next).toBeEnabled();
      await expect(dialog.locator("#dialog-subject-error")).toHaveCount(0);
    });

    test(`paused flow explains unresolved conflicts (${theme.name})`, async ({ page }) => {
      const calls = await watchWrites(page);
      await openLabScene(page, "conflict-banner", '.identity-floor[data-layer="conflicts"]');
      await expect(page.locator("#commit-feedback")).toContainText(theme.locale === "zh" ? "解决并暂存所有冲突" : "Resolve and stage all conflicted files");
      await expect(page.locator(".composer button.commit")).toBeDisabled();
      await page.keyboard.press("Control+Enter");
      await expect(page.locator('.composer [role="alert"]')).toContainText(theme.locale === "zh" ? "解决并暂存所有冲突" : "Resolve and stage all conflicted files");
      expect(calls).toEqual([]);
    });

    test(`commit validation explains pending staging (${theme.name})`, async ({ page }) => {
      const calls = await watchWrites(page);
      await openLabScene(page, "changes", ".composer");
      await page.locator(".composer .summary-input").fill("ready after staging");
      const commit = page.locator(".composer button.commit");
      await expect(commit).toBeEnabled();
      let release!: () => void;
      const gate = new Promise<void>((done) => { release = done; });
      await page.route("**/__lab/invoke", async (route) => {
        if (route.request().postDataJSON().cmd !== "stage_path") return route.fallback();
        await gate;
        return route.fulfill({ json: { ok: false, error: "fixture: staging cancelled" } });
      });
      try {
        await page.locator('.unstaged-pane .file-select[title="README.md"]').dblclick();
        await expect(page.locator(".stage-pending").first()).toBeVisible();
        await expect(page.locator("#commit-feedback")).toContainText(theme.locale === "zh" ? "正在更新暂存区" : "The index is being updated");
        await expect(commit).toBeDisabled();
        await page.keyboard.press("Control+Enter");
        expect(calls).toEqual([]);
      } finally { release(); }
      await expect(page.locator(".stage-pending")).toHaveCount(0);
      await expect(commit).toBeEnabled();
    });

    test(`continue validation explains an operation already in progress (${theme.name})`, async ({ page }) => {
      const calls = await watchWrites(page);
      await openLabScene(page, "rebase-progress", ".composer");
      await page.locator(".composer button.commit").click();
      const dialog = page.locator('[data-dialog="continue"]');
      await expect(dialog).toBeVisible();
      async function updateTask(state: "running" | "completed") {
        await page.evaluate(async ({ path, state }) => {
          const modulePath = "/src/lib/tasks.ts";
          const { acceptTask } = await import(/* @vite-ignore */ modulePath);
          acceptTask({ id: "validation-pending-continue", path, action: "continue", label: "Continue rebase", targets: [], state, dependsOn: [],
            revision: 0, queuedMs: 0, executeMs: 0, verifyMs: 0 });
        }, { path: resolve(".lab/repos/rebase"), state });
      }
      await updateTask("running");
      await expect(dialog.locator("#dialog-subject-error")).toContainText(theme.locale === "zh" ? "正在处理仓库操作" : "A repository operation is in progress");
      await expect(dialog.locator("button.commit")).toBeDisabled();
      await dialog.locator("form").dispatchEvent("submit");
      expect(calls).toEqual([]);
      await updateTask("completed");
      await expect(dialog.locator("button.commit")).toBeEnabled();
      await expect(dialog.locator("#dialog-subject-error")).toHaveCount(0);
    });
  });
}
