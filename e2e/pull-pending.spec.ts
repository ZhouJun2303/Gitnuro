import { expect, test } from "@playwright/test";
import type { OperationTask } from "../src/lib/tasks";
import { pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"]) {
  for (const unchanged of [true, false]) {
    test(`pull clears the branch spinner ${unchanged ? "when references stay unchanged" : "after the reference snapshot arrives"} (${theme})`, async ({ page }) => {
      let status: Record<string, unknown>, refs: Record<string, unknown>;
      let task: OperationTask | undefined;
      let deliveries = 0, refreshing = false;
      let releaseResult!: () => void, releaseRefs!: () => void;
      const resultGate = new Promise<void>((done) => { releaseResult = done; });
      const refsGate = new Promise<void>((done) => { releaseRefs = done; });
      let receivedTask!: () => void;
      const taskResponse = new Promise<void>((done) => { receivedTask = done; });

      await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, statusInterval: 0 });
      await page.route("**/__lab/invoke", async (route) => {
        const { cmd, args } = route.request().postDataJSON();
        if (cmd === "operation_tasks") {
          // Keep replaying unchanged tasks; otherwise send one event so a later
          // poll cannot mask a missing reaction to the reference revision.
          const tasks = task && (unchanged || deliveries === 0) ? [task] : [];
          await route.fulfill({ json: { ok: true, value: tasks } });
          if (tasks.length) { deliveries++; receivedTask(); }
          return;
        }
        if (cmd === "workspace_status" && !task) {
          const response = await route.fetch();
          status = (await response.json()).value;
          return route.fulfill({ response });
        }
        if (cmd === "repository_refs") {
          if (!task) {
            const response = await route.fetch();
            refs = (await response.json()).value;
            return route.fulfill({ response });
          }
          refreshing = true;
          await refsGate;
          return route.fulfill({ json: { ok: true, value: { ...refs, revision: task.revision } } });
        }
        if (cmd === "mutate" && ["pull", "pullRef"].includes(args.request?.action)) {
          task = {
            id: args.__task, path: args.path, action: args.request.action, label: "Pull origin/main", targets: ["main", "origin"],
            state: "completed", dependsOn: [], revision: Number(refs.revision ?? 0) + 1, queuedMs: 0, executeMs: 10, verifyMs: 1,
          };
          await resultGate;
          return route.fulfill({ json: { ok: true, value: {
            ...status, revision: task.revision, outcome: "completed", error: "", log: "$ git fetch origin\nAlready up to date.",
            refreshAreas: ["refs", "history", "flow"], ...(unchanged ? { refreshScope: "status" } : {}),
          } } });
        }
        return route.fallback();
      });
      await page.goto(`/?scene=pull-noop&theme=${theme}&locale=zh`);
      await expect(page.locator('[data-lab-scene="pull-noop"]')).toBeVisible();
      const spinner = page.locator('.sidebar .file-select[title="main"] .reference-pending');
      await expect(spinner).toHaveCount(0);
      await page.locator(".chrome-left").getByTitle("拉取", { exact: true }).click();
      await page.locator('[data-dialog="pull"] button.commit').click();
      try {
        await taskResponse;
        await expect(spinner).toBeVisible();
        releaseResult();
        await expect(page.locator(".chrome-left").getByTitle("拉取", { exact: true })).toBeEnabled();
        await page.locator(".history-btn").click();
        const summary = page.locator(".command-filters .task-queue summary");
        await expect(summary.locator("span")).toHaveText("0");
        await summary.click();
        await expect(page.locator('.task-row[data-task-state="completed"]').filter({ hasText: "Pull origin/main" })).toBeVisible();
        await page.locator(".command-log .dialog-close").click();
        if (unchanged) {
          expect(refreshing).toBe(false);
        } else {
          await expect.poll(() => refreshing).toBe(true);
          await expect(spinner).toBeVisible();
          releaseRefs();
        }
        await expect(spinner).toHaveCount(0, { timeout: 3000 });
        if (unchanged) {
          await expect.poll(() => deliveries).toBeGreaterThan(1);
          await expect(spinner).toHaveCount(0);
        }
      } finally { releaseResult(); releaseRefs(); }
    });
  }
}
