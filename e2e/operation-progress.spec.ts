import { expect, test, type Page, type Route } from "@playwright/test";
import type { OperationTask } from "../src/lib/tasks";
import { pinLabSettings } from "./helpers";

const fixtures = [
  ["fetch-progress", 33, "获取远程提交"],
  ["pull-progress", 50, "整合提交"],
  ["push-progress", 42, "执行命令"],
  ["fetch-background-progress", 33, "获取远程提交"],
  ["checkout-progress", 20, "整合提交"],
  ["clone-waiting", 33, "执行命令"],
  ["clone-progress", 42, ""],
] as const;

/** Hold the command response while delivering real-shaped task phase snapshots. */
async function pendingCommand(page: Page) {
  let task: OperationTask | undefined;
  let release!: () => void;
  const gate = new Promise<void>((done) => { release = done; });
  let result = "completed";
  let status: Record<string, unknown> = {};
  const handler = async (route: Route) => {
    const body = route.request().postDataJSON();
    if (body.cmd === "workspace_status") {
      const response = await route.fetch();
      status = (await response.json()).value;
      return route.fulfill({ response });
    }
    if (body.cmd === "operation_tasks") return route.fulfill({ json: { ok: true, value: task ? [task] : [] } });
    if (body.cmd !== "mutate") return route.fallback();
    task = { id: body.args.__task, path: body.args.path, action: body.args.request.action, label: body.args.__taskLabel,
      targets: [], state: "queued", dependsOn: [], revision: 0, queuedMs: 0, executeMs: 0, verifyMs: 0, percent: 0 };
    await gate;
    return route.fulfill({ json: result === "cancelled" ? { ok: false, error: "awegit:cancelled:" } : {
      ok: true, value: { ...status, outcome: result, error: result === "failed" ? "progress fixture failure" : "", output: "", log: "", refreshScope: "status" },
    } });
  };
  await page.route("**/__lab/invoke", handler);
  return {
    started: () => !!task,
    phase: (phase: string, percent: number, state: OperationTask["state"] = "running") => { task = { ...task!, phase, percent, state }; },
    external: () => {
      task = { id: "external-push-progress", path: String(status.path), action: "push", label: "推送", targets: [], state: "running", phase: "operation", percent: 33,
        dependsOn: [], revision: 0, queuedMs: 0, executeMs: 0, verifyMs: 0 };
      return task.path;
    },
    release: (outcome = "completed") => { result = outcome; release(); },
  };
}

for (const theme of ["apple", "light", "dark"] as const) {
  for (const [scene, percent, phase] of fixtures) {
    test(`command progress scene ${scene} shows a percentage (${theme})`, async ({ page }) => {
      await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, statusInterval: 0 });
      await page.goto(`/?scene=${scene}&theme=${theme}&locale=zh`);
      await expect(page.locator(`[data-lab-scene="${scene}"]`)).toBeVisible();
      const bar = page.getByRole("progressbar");
      await expect(bar).toBeVisible();
      await expect(bar).toHaveAttribute("aria-valuenow", String(percent));
      await expect(page.locator(".identity-pct")).toHaveText(`${percent}%`);
      if (phase) await expect(page.locator(".identity-branch")).toContainText(phase);
      expect(await bar.evaluate((node) => {
        const track = node.getBoundingClientRect(), fill = node.firstElementChild!.getBoundingClientRect();
        return Math.round(fill.width / track.width * 100);
      })).toBe(percent);
      await expect(page.locator('.chrome-left [title="获取"]')).toBeEnabled();
      await page.screenshot({ path: `.lab/${scene}-${theme}.png` });
    });
  }

  for (const [action, label] of [["fetch", "获取"], ["pull", "拉取"], ["push", "推送"]] as const) {
    test(`${action} advances by command phases without Git transfer percentages (${theme})`, async ({ page }) => {
      await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, statusInterval: 0 });
      const command = await pendingCommand(page);
      await page.goto(`/?scene=clean&theme=${theme}&locale=zh`);
      await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
      await page.locator(".chrome-left").getByTitle(label, { exact: true }).click();
      await page.locator(`[data-dialog="${action}"] button.commit`).click();
      try {
        await expect.poll(command.started).toBe(true);
        const bar = page.getByRole("progressbar");
        await expect(bar).toHaveAttribute("aria-valuenow", "0");
        const stages = action === "pull" ? [["fetch", 16], ["saved", 33], ["integration", 50], ["restore", 66], ["finished", 83]] as const
          : [[action === "fetch" ? "fetch" : "operation", 33], ["finished", 66]] as const;
        for (const [phase, percent] of stages) {
          command.phase(phase, percent, phase === "finished" ? "verifying" : "running");
          await expect(bar).toHaveAttribute("aria-valuenow", String(percent));
          await expect(page.locator(".identity-pct")).toHaveText(`${percent}%`);
          await page.locator(".summary-input").fill(`draft during ${action} ${percent}`);
          await expect(page.locator(".summary-input")).toHaveValue(`draft during ${action} ${percent}`);
          await expect(page.locator(".chrome-left").getByTitle(label, { exact: true })).toBeEnabled();
        }
      } finally { command.release(); }
      await expect(page.getByRole("progressbar")).toHaveCount(0);
    });
  }

  for (const outcome of ["failed", "cancelled"]) {
    test(`command progress clears after ${outcome} (${theme})`, async ({ page }) => {
      await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, statusInterval: 0 });
      const command = await pendingCommand(page);
      await page.goto(`/?scene=clean&theme=${theme}&locale=zh`);
      await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
      await page.locator(".chrome-left").getByTitle("获取", { exact: true }).click();
      await page.locator('[data-dialog="fetch"] button.commit').click();
      try {
        await expect.poll(command.started).toBe(true);
        command.phase("fetch", 33);
        await expect(page.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "33");
      } finally { command.release(outcome); }
      await expect(page.getByRole("progressbar")).toHaveCount(0);
      await expect(page.locator('.chrome-left [title="获取"]')).toBeEnabled();
    });
  }

  test(`automatic fetch reports command progress and keeps browsing available (${theme})`, async ({ page }) => {
    await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, statusInterval: 0 });
    const command = await pendingCommand(page);
    await page.goto(`/?scene=clean&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
    await page.clock.install();
    await page.keyboard.press("Control+,");
    await page.locator('[data-dialog="prefs"]').getByRole("button", { name: "Git", exact: true }).click();
    await page.getByRole("checkbox", { name: "自动获取远程", exact: true }).check();
    const saved = page.waitForResponse((response) => response.url().endsWith("/__lab/invoke") && response.request().postDataJSON()?.cmd === "save_settings");
    await page.locator('[data-dialog="prefs"] button.commit').click();
    await page.clock.runFor(100);
    expect((await (await saved).json()).value.fetchAutomatically).toBe(true);
    await page.clock.fastForward(10 * 60 * 1000);
    await page.clock.runFor(1100);
    await page.clock.resume();
    try {
      await expect.poll(command.started).toBe(true);
      command.phase("fetch", 33);
      await expect(page.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "33");
      await expect(page.locator('.tab.active .tab-job')).toBeVisible();
      await page.locator('.chrome-left [title="拉取"]').click();
      await expect(page.locator('[data-dialog="pull"]')).toBeVisible();
      await page.keyboard.press("Escape");
      await page.locator(".summary-input").fill("draft during automatic fetch");
      await expect(page.locator(".summary-input")).toHaveValue("draft during automatic fetch");
    } finally { command.release(); }
    await expect(page.getByRole("progressbar")).toHaveCount(0);
  });

  test(`a repository task shows progress without a foreground job (${theme})`, async ({ page }) => {
    await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, statusInterval: 0 });
    const command = await pendingCommand(page);
    await page.goto(`/?scene=clean&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
    await expect(page.getByRole("progressbar")).toHaveCount(0);
    const path = command.external();
    await expect(page.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "33");
    await expect(page.locator(".identity-branch")).toContainText("推送");
    await page.evaluate(async (path) => {
      const modulePath = "/src/lib/tasks.ts";
      const { acceptTask } = await import(/* @vite-ignore */ modulePath);
      acceptTask({ id: "queued-after-push", path, action: "fetch", label: "queued fetch", targets: [], state: "queued", dependsOn: ["external-push-progress"], percent: 0,
        revision: 0, queuedMs: 0, executeMs: 0, verifyMs: 0 });
    }, path);
    await expect(page.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "33");
    command.phase("finished", 66, "verifying");
    await expect(page.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "66");
    await page.evaluate(async (path) => {
      const modulePath = "/src/lib/tasks.ts";
      const { acceptTask } = await import(/* @vite-ignore */ modulePath);
      acceptTask({ id: "queued-after-push", path, action: "fetch", label: "queued fetch", targets: [], state: "cancelled", dependsOn: [], percent: 0,
        revision: 0, queuedMs: 0, executeMs: 0, verifyMs: 0 });
    }, path);
    command.phase("finished", 100, "completed");
    await expect(page.getByRole("progressbar")).toHaveCount(0);
  });
}
