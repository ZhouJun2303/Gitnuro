import { expect, test, type Page } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { openLabScene, pinLabSettings } from "./helpers";

const baseline = process.env.AWEGIT_UI_AUDIT_BASELINE === "1";
const count = 4_000;

async function observe(page: Page) {
  await page.addInitScript(() => {
    const tasks: { startMs: number; ms: number }[] = [];
    (window as any).__dialogTasks = tasks;
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) tasks.push({ startMs: entry.startTime, ms: entry.duration });
    }).observe({ type: "longtask", buffered: true });
  });
}

async function report(page: Page, theme: string, name: string, extra: Record<string, unknown>) {
  await page.evaluate(() => new Promise(requestAnimationFrame));
  const metrics = await page.evaluate(() => ({
    readyMs: performance.now(), nodes: document.querySelectorAll("*").length,
    tasks: (window as any).__dialogTasks,
  }));
  const directory = `.lab/ui-audit/${process.env.AWEGIT_UI_AUDIT_RUN || "current"}/dialogs`;
  mkdirSync(directory, { recursive: true });
  writeFileSync(`${directory}/${theme}-${name}.json`, JSON.stringify({ theme, name, count, ...metrics, ...extra }, null, 2));
  await page.evaluate(() => Promise.allSettled(document.getAnimations()
    .filter((animation) => animation.playState === "running" && animation.effect?.getTiming().iterations !== Infinity)
    .map((animation) => animation.finished)));
  await page.screenshot({ path: `${directory}/${theme}-${name}.png` });
}

for (const theme of ["apple", "light", "dark"]) test.describe(theme, () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: theme === "dark" ? 960 : 1440, height: theme === "dark" ? 640 : 900 });
  });
  for (const item of [
    { label: "Mailmap", command: "blob_view", dialog: "mailmap", value: { text: "Lab <lab@example.com>" } },
    { label: "Review branch", command: "review_branch", dialog: "output", value: "Review completed" },
  ]) test(`${item.dialog} stays closable during a slow read and ignores its late result`, async ({ page }) => {
    await observe(page);
    await pinLabSettings(page, { locale: "en", theme });
    await openLabScene(page, item.dialog === "output" ? "diff-text" : "clean", ".file-list");
    let release!: () => void;
    const pending = new Promise<void>((resolve) => { release = resolve; });
    let requested!: () => void;
    const started = new Promise<void>((resolve) => { requested = resolve; });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd } = route.request().postDataJSON();
      if (cmd !== item.command) return route.fallback();
      requested();
      await pending;
      return route.fulfill({ json: { ok: true, value: item.value } });
    });
    await page.locator(".menubar").getByRole("button", { name: "Repository", exact: true }).click();
    const startMs = await page.evaluate(() => performance.now());
    await page.getByRole("menuitem", { name: item.label, exact: true }).click();
    await started;
    const dialog = page.locator(`[data-dialog="${item.dialog}"]`);
    if (baseline) {
      const visibleWhilePending = await dialog.isVisible();
      release();
      await expect(dialog).toBeVisible();
      const openMs = await page.evaluate((start) => performance.now() - start, startMs);
      await report(page, theme, item.dialog, { visibleWhilePending, openMs });
      return;
    }
    await expect(dialog).toBeVisible();
    const openMs = await page.evaluate((start) => performance.now() - start, startMs);
    await expect(dialog.getByRole("status")).toContainText("Reading");
    await report(page, theme, item.dialog, { visibleWhilePending: true, openMs });
    if (item.dialog === "mailmap") {
      await expect(dialog.getByRole("textbox")).toBeDisabled();
      await expect(dialog.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
    }
    await dialog.locator(".dialog-close").click();
    await expect(dialog).toHaveCount(0);
    if (item.dialog === "output") {
      // A different command can use the same output surface after the read was dismissed.
      await page.route("**/__lab/invoke", async (route) => {
        if (route.request().postDataJSON().cmd !== "pick_history") return route.fallback();
        return route.fulfill({ json: { ok: true, value: "New line history result" } });
      });
      await page.locator(".diff-body .virtual-row.add, .diff-body .split-code.add").first().click({ button: "right" });
      await page.getByRole("menuitem", { name: "History for this line", exact: true }).click();
      await expect(dialog.locator("pre")).toHaveText("New line history result");
    }
    const response = page.waitForResponse((response) => response.url().endsWith("/__lab/invoke")
      && response.request().postDataJSON()?.cmd === item.command);
    release();
    await (await response).finished();
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    if (item.dialog === "output") await expect(dialog.locator("pre")).toHaveText("New line history result");
    else await expect(dialog).toHaveCount(0);
  });

  test("long rebase mounts bounded rows and preserves offscreen edits", async ({ page }) => {
    test.setTimeout(90_000);
    await observe(page);
    await pinLabSettings(page, { locale: "en", theme });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd } = route.request().postDataJSON();
      if (cmd === "commit_log") {
        const rows = Array.from({ length: count }, (_, index) => ({
          id: index.toString(16).padStart(40, "0"), shortId: index.toString(16).padStart(7, "0"),
          summary: `Commit ${index}`, author: "Lab", email: "lab@example.com", when: "now",
          at: 1_700_000_000 - index, parents: index + 1 < count ? [(index + 1).toString(16).padStart(40, "0")] : [], refs: [], lane: 0,
        }));
        return route.fulfill({ json: { ok: true, value: rows } });
      }
      if (cmd === "rebase_conflicts") return route.fulfill({ json: { ok: true, value: [] } });
      return route.fallback();
    });
    await openLabScene(page, "dialog-rebase", ".dialog-scroll");
    const list = page.locator('[data-dialog="rebase"] .dialog-scroll');
    const mounted = await list.locator('[role="listitem"]').count();
    await report(page, theme, "rebase", { mounted });
    if (!baseline) expect(mounted).toBeLessThan(100);
    await list.evaluate((node) => { node.scrollTop = node.scrollHeight; });
    const select = list.getByRole("combobox", { name: "Action Commit 0", exact: true });
    await expect(select).toBeVisible();
    await select.selectOption("reword");
    const message = list.getByRole("textbox", { name: "Message", exact: true });
    await message.fill("Keep this offscreen edit");
    await list.getByRole("checkbox", { name: "Action Commit 0", exact: true }).uncheck();
    await list.evaluate((node) => { node.scrollTop = 0; });
    await expect(list.getByRole("combobox").first()).toHaveValue("pick");
    await list.evaluate((node) => { node.scrollTop = node.scrollHeight; });
    await expect(select).toHaveValue("reword");
    await expect(message).toHaveValue("Keep this offscreen edit");
    await expect(list.getByRole("checkbox", { name: "Action Commit 0", exact: true })).not.toBeChecked();
    const source = list.locator('[role="listitem"]').filter({ has: page.getByRole("combobox", { name: "Action Commit 1", exact: true }) });
    const target = list.locator('[role="listitem"]').filter({ has: page.getByRole("combobox", { name: "Action Commit 0", exact: true }) });
    await source.dragTo(target);
    await expect(list.getByRole("combobox").last()).toHaveAttribute("aria-label", "Action Commit 1");
    await expect(select).toHaveValue("reword");
    await expect(message).toHaveValue("Keep this offscreen edit");
    await expect(page.locator('[data-dialog="rebase"] .dialog-close')).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(list).toHaveCount(0);
  });

  test("merged branches opens before a slow read and keeps rows bounded", async ({ page }) => {
    test.setTimeout(90_000);
    await observe(page);
    await pinLabSettings(page, { locale: "en", theme });
    await openLabScene(page, "clean", ".file-list");
    let release!: () => void;
    const pending = new Promise<void>((resolve) => { release = resolve; });
    let requested!: () => void;
    const started = new Promise<void>((resolve) => { requested = resolve; });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd } = route.request().postDataJSON();
      if (cmd !== "merged_branches") return route.fallback();
      requested();
      await pending;
      return route.fulfill({ json: { ok: true, value: Array.from({ length: count }, (_, index) => `merged/${String(index).padStart(5, "0")}`) } });
    });
    await page.locator(".menubar").getByRole("button", { name: "Repository", exact: true }).click();
    const startMs = await page.evaluate(() => performance.now());
    await page.getByRole("menuitem", { name: /merged branches/i }).click();
    await started;
    const dialog = page.locator('[data-dialog="stale"]');
    if (!baseline) await expect(dialog).toBeVisible();
    const visibleWhilePending = await dialog.isVisible();
    const initialOpenMs = visibleWhilePending ? await page.evaluate((start) => performance.now() - start, startMs) : null;
    release();
    await expect(dialog).toBeVisible();
    const list = dialog.locator(".branch-pick-list");
    await expect(list.getByRole("checkbox").first()).toBeVisible();
    const mounted = await list.getByRole("checkbox").count();
    const openMs = initialOpenMs ?? await page.evaluate((start) => performance.now() - start, startMs);
    await report(page, theme, "stale", { openMs, visibleWhilePending, mounted });
    if (!baseline) expect(mounted).toBeLessThan(100);
    await list.evaluate((node) => { node.scrollTop = node.scrollHeight; });
    const last = list.getByRole("checkbox", { name: "merged/03999", exact: true });
    await expect(last).toBeVisible();
    await last.check();
    await list.evaluate((node) => { node.scrollTop = 0; });
    await expect(list.getByRole("checkbox").first()).not.toBeChecked();
    await list.evaluate((node) => { node.scrollTop = node.scrollHeight; });
    await expect(last).toBeChecked();
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
  });
});
