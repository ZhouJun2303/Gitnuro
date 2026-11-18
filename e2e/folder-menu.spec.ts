import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

for (const variant of [
  { theme: "apple", locale: "zh" },
  { theme: "light", locale: "zh" },
  { theme: "dark", locale: "en" },
] as const) {
  test.describe(variant.theme, () => {
    test.setTimeout(60_000);
    test.beforeEach(async ({ page }) => {
      await pinLabSettings(page, { ...variant, fileLayout: "tree", fetchAutomatically: false, statusInterval: 0 });
    });

    const labels = variant.locale === "zh"
      ? ["打开文件夹", "在文件管理器中显示", "复制绝对路径", "复制相对路径"]
      : ["Open folder", "Reveal in file manager", "Copy absolute path", "Copy relative path"];

    for (const side of ["unstaged", "staged"] as const) {
      test(`${side} folder menu copies the clicked folder paths and opens the folder`, async ({ page, context }, info) => {
        await context.grantPermissions(["clipboard-read", "clipboard-write"]);
        const calls: { path: string; openDirectory: boolean }[] = [];
        await page.route("**/__lab/invoke", (route) => {
          const { cmd, args } = route.request().postDataJSON();
          if (cmd === "reveal_path") {
            calls.push({ path: args.path, openDirectory: args.openDirectory });
            return route.fulfill({ json: { ok: true, value: null } });
          }
          return route.fallback();
        });
        await openLabScene(page, "folder-menu", '[data-layer="menu"]', { locale: variant.locale });
        await expect(page.locator("html")).toHaveAttribute("data-theme", variant.theme);
        await page.keyboard.press("Escape");
        const folder = page.locator(`.${side}-pane .dir-row .file-select`).first();
        await expect(folder).toBeVisible();
        const relative = (await folder.getAttribute("title"))!;
        const response = await page.request.post("/__lab/invoke", { data: { cmd: "lab_layout", args: {} } });
        const { value: layout } = await response.json();
        const absolute = join(layout.full, ...relative.split(/[\\/]/));

        await folder.click({ button: "right" });
        const menu = page.locator('[data-layer="menu"]');
        for (const [index, label] of labels.entries()) {
          await expect(menu.getByRole("menuitem").nth(index)).toHaveText(label);
        }
        const screenshot = `.lab/screenshots/folder-menu-${variant.theme}-${side}.png`;
        await page.screenshot({ path: screenshot });
        await info.attach("folder-menu", { path: screenshot, contentType: "image/png" });

        await menu.getByRole("menuitem", { name: labels[3], exact: true }).click();
        await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(relative);
        await expect(menu).toHaveCount(0);

        await folder.click({ button: "right" });
        await menu.getByRole("menuitem", { name: labels[2], exact: true }).click();
        await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(absolute);

        await folder.click({ button: "right" });
        await menu.getByRole("menuitem", { name: labels[0], exact: true }).click();
        await expect.poll(() => calls).toEqual([{ path: absolute, openDirectory: true }]);

        await folder.click({ button: "right" });
        await menu.getByRole("menuitem", { name: labels[1], exact: true }).click();
        await expect.poll(() => calls).toEqual([
          { path: absolute, openDirectory: true },
          { path: absolute, openDirectory: false },
        ]);
      });
    }

    test("a folder opening failure is visible and leaves path copying usable", async ({ page, context }) => {
      await context.grantPermissions(["clipboard-read", "clipboard-write"]);
      await page.route("**/__lab/invoke", (route) => {
        if (route.request().postDataJSON().cmd === "reveal_path") {
          return route.fulfill({ json: { ok: false, error: "Folder is no longer available" } });
        }
        return route.fallback();
      });
      await openLabScene(page, "folder-menu", '[data-layer="menu"]', { locale: variant.locale });
      await page.getByRole("menuitem", { name: labels[0], exact: true }).click();
      await expect(page.getByText("Folder is no longer available", { exact: true }).first()).toBeVisible();
      const folder = page.locator('.unstaged-pane .file-select[title="pending"]');
      await folder.click({ button: "right" });
      await page.getByRole("menuitem", { name: labels[3], exact: true }).click();
      await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe("pending");
    });
  });
}
