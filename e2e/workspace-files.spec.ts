import { expect, test, type Page } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

type FileCall = { cmd: string; path: string };

async function fileCalls(page: Page) {
  const calls: FileCall[] = [];
  await page.route("**/__lab/invoke", async (route) => {
    const { cmd, args } = route.request().postDataJSON();
    if (cmd === "open_file" || cmd === "reveal_path" || cmd === "open_editor") {
      calls.push({ cmd, path: args.path });
      return route.fulfill({ json: { ok: true, value: null } });
    }
    return route.fallback();
  });
  return calls;
}

async function expectFilledTabs(page: Page) {
  await expect.poll(() => page.locator(".chrome-tabs .tabs").evaluate((strip) => {
    const tabs = [...strip.querySelectorAll<HTMLElement>(".tab")];
    const bounds = strip.getBoundingClientRect();
    return Math.max(
      Math.abs(tabs[0].getBoundingClientRect().left - bounds.left),
      Math.abs(tabs.at(-1)!.getBoundingClientRect().right - bounds.right),
      strip.scrollWidth - strip.clientWidth,
    );
  })).toBeLessThanOrEqual(1);
  const widths = await page.locator(".chrome-tabs .tab").evaluateAll((tabs) => tabs.map((tab) => tab.getBoundingClientRect().width));
  // The classic themes put the remaining whole pixels into the last tab.
  expect(Math.max(...widths) - Math.min(...widths)).toBeLessThanOrEqual(Math.max(1, widths.length - 1));
  await expect(page.locator(".tab-add")).toBeInViewport({ ratio: 1 });
}

for (const theme of ["apple", "light", "dark"] as const) {
  test.describe(theme, () => {
    test.setTimeout(60_000);
    test.beforeEach(async ({ page }) => {
      await pinLabSettings(page, { theme, locale: "en", fetchAutomatically: false, statusInterval: 0, verticalTabs: false });
    });

    test("workspace tabs fill the strip after resizing and closing projects", async ({ page }, info) => {
      await openLabScene(page, "workspace-tabs", ".chrome-tabs .tab");
      await expect(page.locator(".chrome-tabs .tab")).toHaveCount(5);
      for (const width of [1440, 960, 640, 1800]) {
        await page.setViewportSize({ width, height: 900 });
        await expectFilledTabs(page);
      }
      for (const count of [4, 3, 2, 1]) {
        const last = page.locator(".chrome-tabs .tab").last();
        await last.hover();
        await last.locator(".tab-close").click();
        await expect(page.locator(".chrome-tabs .tab")).toHaveCount(count);
        await expectFilledTabs(page);
      }
      const screenshot = `.lab/ui-audit/workspace-tabs-${theme}.png`;
      await page.screenshot({ path: screenshot });
      await info.attach("workspace-tabs", { path: screenshot, contentType: "image/png" });
    });

    test("project names shrink with the tab and ellipsize at the minimum", async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await openLabScene(page, "workspace-tabs", ".chrome-tabs .tab");
      const fontSize = () => page.locator(".chrome-tabs .tab-name").first().evaluate((node) => parseFloat(getComputedStyle(node).fontSize));
      await expect.poll(fontSize).toBe(13);

      await page.setViewportSize({ width: 640, height: 900 });
      await expect.poll(fontSize).toBeLessThan(13);
      expect(await fontSize()).toBeGreaterThanOrEqual(10);
      const long = page.locator(".chrome-tabs .tab-name", { hasText: "AweGit-performance-branch" });
      const metrics = await long.evaluate((node) => ({
        overflow: getComputedStyle(node).textOverflow,
        clipped: node.scrollWidth > node.clientWidth,
      }));
      expect(metrics.overflow).toBe("ellipsis");
      expect(metrics.clipped).toBe(true);
    });

    test("project tabs keep the dragged order in the workspace", async ({ page }) => {
      await page.setViewportSize({ width: 1280, height: 900 });
      await openLabScene(page, "workspace-recent", ".chrome-tabs .tab .file-select");
      const tabs = page.locator(".chrome-tabs .tab:has(.file-select)");
      await expect(tabs).toHaveCount(5);
      const names = () => tabs.locator(".tab-name").allTextContents();
      const before = await names();
      const paths = await tabs.evaluateAll((nodes) => nodes.map((node) => (node as HTMLElement).dataset.repo!));
      const active = await page.locator(".chrome-tabs .tab.active .tab-name").textContent();
      const rebuilt = (list: string[]) => [list[list.length - 1], ...list.slice(0, -1)];
      const expected = rebuilt(before);
      const expectedPaths = rebuilt(paths);
      const source = tabs.last();
      const target = tabs.first();
      const box = (await target.boundingBox())!;
      const saved = page.waitForRequest((request) => {
        if (!request.url().endsWith("/__lab/invoke") || request.method() !== "POST") return false;
        const body = request.postDataJSON();
        const recent = body.args?.values?.workspaces?.find((item: { id: string }) => item.id === "awegit-recent");
        return body.cmd === "save_settings" && recent?.openTabs?.[0] === expectedPaths[0];
      });
      await source.dragTo(target, { targetPosition: { x: 4, y: box.height / 2 } });
      await expect.poll(names).toEqual(expected);
      await expect(page.locator(".chrome-tabs .tab.active .tab-name")).toHaveText(active!);
      const request = await saved;
      const body = request.postDataJSON();
      const recent = body.args.values.workspaces.find((item: { id: string }) => item.id === "awegit-recent");
      expect(recent.openTabs).toEqual(expectedPaths);
      expect(recent.repositories).toEqual(expectedPaths);
    });

    test("many workspace tabs keep readable widths and scroll inside the strip", async ({ page }) => {
      await page.setViewportSize({ width: 640, height: 720 });
      await openLabScene(page, "workspace-tabs-many", ".chrome-tabs .tab");
      const tabs = page.locator(".chrome-tabs .tabs");
      expect(await tabs.locator(".tab").count()).toBeGreaterThan(6);
      const before = await tabs.evaluate((strip) => ({
        scrollWidth: strip.scrollWidth, clientWidth: strip.clientWidth,
        widths: [...strip.querySelectorAll(".tab")].map((tab) => tab.getBoundingClientRect().width),
      }));
      expect(before.scrollWidth).toBeGreaterThan(before.clientWidth);
      expect(Math.min(...before.widths)).toBeGreaterThanOrEqual(96);
      expect(await tabs.locator(".file-select").first().evaluate((node) => parseFloat(getComputedStyle(node).fontSize))).toBe(10);
      await tabs.evaluate((strip) => { strip.scrollLeft = strip.scrollWidth; });
      await expect(tabs.locator(".tab").last()).toBeInViewport({ ratio: 1 });
      await expect(page.locator(".tab-add")).toBeInViewport({ ratio: 1 });
      expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
    });

    test("vertical workspace tabs retain their sidebar layout", async ({ page }) => {
      await openLabScene(page, "vertical-tabs", ".vertical-tabs");
      const strip = page.locator(".chrome-tabs .tabs");
      const layout = await strip.evaluate((node) => ({
        direction: getComputedStyle(node).flexDirection,
        width: node.clientWidth,
        tab: node.querySelector(".tab")!.getBoundingClientRect().width,
      }));
      expect(layout.direction).toBe("column");
      expect(layout.tab).toBeLessThanOrEqual(layout.width);
      expect(layout.tab).toBeGreaterThan(96);
    });

    test("change file menus open spreadsheets text and images in the default application", async ({ page }) => {
      const calls = await fileCalls(page);
      await openLabScene(page, "changes", ".file-list");
      for (const [index, file] of ["data/sheet.xlsx", "notes/long.txt", "art/mark.png"].entries()) {
        const row = page.locator(".unstaged-pane .file-select").and(page.getByTitle(file, { exact: true }));
        await row.click({ button: "right" });
        await page.getByRole("menuitem", { name: "Open file", exact: true }).click();
        await expect.poll(() => calls.length).toBe(index + 1);
        await expect.poll(() => calls.at(-1)).toMatchObject({ cmd: "open_file" });
        expect(calls.at(-1)!.path.replaceAll("\\", "/")).toMatch(new RegExp(`/full/${file.replaceAll(".", "\\.")}$`));
      }
      expect(calls.map((call) => call.cmd)).toEqual(["open_file", "open_file", "open_file"]);
      await page.locator(".unstaged-pane .file-select").and(page.getByTitle("notes/long.txt", { exact: true })).click({ button: "right" });
      await page.getByRole("menuitem", { name: "Reveal in file manager", exact: true }).click();
      await expect.poll(() => calls.at(-1)?.cmd).toBe("reveal_path");
    });

    for (const item of [
      { scene: "commit", row: ".patch-toggle" },
      { scene: "tree", row: ".detail-tree .tree-row:has-text('README.md')" },
      { scene: "file-history", row: ".history-commit" },
    ]) {
      test(`${item.scene} file menu uses the default application and preserves reveal`, async ({ page }) => {
        const calls = await fileCalls(page);
        await openLabScene(page, item.scene, item.row);
        const row = page.locator(item.row).first();
        await row.click({ button: "right" });
        await page.getByRole("menuitem", { name: "Open", exact: true }).click();
        await expect.poll(() => calls.length).toBe(1);
        expect(calls[0].cmd).toBe("open_file");
        await row.click({ button: "right" });
        await page.getByRole("menuitem", { name: "Show in File Explorer", exact: true }).click();
        await expect.poll(() => calls.length).toBe(2);
        expect(calls[1]).toEqual({ cmd: "reveal_path", path: calls[0].path });
      });
    }

    test("a default application failure is visible and the file menu remains usable", async ({ page }) => {
      await page.route("**/__lab/invoke", (route) => {
        if (route.request().postDataJSON().cmd === "open_file") {
          return route.fulfill({ json: { ok: false, error: "No application is associated with this file" } });
        }
        return route.fallback();
      });
      await openLabScene(page, "changes", ".file-list");
      const row = page.locator(".unstaged-pane .file-select").and(page.getByTitle("notes/long.txt", { exact: true }));
      await row.click({ button: "right" });
      await page.getByRole("menuitem", { name: "Open file", exact: true }).click();
      await expect(page.getByText("No application is associated with this file", { exact: true }).first()).toBeVisible();
      await row.click({ button: "right" });
      await expect(page.getByRole("menuitem", { name: "Open file", exact: true })).toBeEnabled();
    });
  });
}
