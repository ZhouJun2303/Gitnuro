import { expect, test, type Page } from "@playwright/test";
import { scenes } from "../src/lib/lab/scenes";
import { openScene, pinLabSettings } from "./helpers";

const scene = scenes.find((item) => item.id === "workspace-recent")!;

async function recentLimit(page: Page) {
  await page.locator(".menubar").getByRole("button", { name: /^(视图|View)$/ }).click();
  await page.getByRole("menuitem", { name: /偏好设置|首选项|Preferences/ }).click();
  const prefs = page.locator('[data-dialog="prefs"]');
  await prefs.getByRole("button", { name: /^(工作区|Workspace)$/ }).click();
  return prefs.getByRole("spinbutton", { name: /最多保留的项目数|Maximum repositories/ });
}

async function saveLimit(page: Page, value: string) {
  const limit = await recentLimit(page);
  await limit.fill(value);
  await page.locator('[data-dialog="prefs"] .commit').click();
}

async function openFromPicker(page: Page, name: string) {
  await page.locator(".tab-add").click();
  await page.locator('[data-layer="repos"]').getByRole("button", { name, exact: true }).first().click();
}

for (const theme of ["apple", "light", "dark"] as const) {
  test(`${theme} recent workspace keeps pinned projects in the configured slots`, async ({ page }) => {
    test.setTimeout(60_000);
    await pinLabSettings(page, { recentWorkspaceLimit: undefined, recentRepoPins: undefined });
    const query = `&theme=${theme}&locale=${theme === "dark" ? "en" : "zh"}`;
    await openScene(page, scene, query);
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);

    const tabs = page.locator(".chrome-tabs .tab:has(.file-select)");
    const tab = (name: string) => tabs.filter({
      has: page.getByRole("button", { name: new RegExp(`^(关闭|Close) ${name}$`) }),
    });
    const menu = page.locator('[data-layer="menu"]');
    await expect(tabs).toHaveCount(5);
    const limit = await recentLimit(page);
    await expect(limit).toHaveValue("5");
    await limit.fill("14");
    await page.locator('[data-dialog="prefs"] .commit').click();
    await expect(tabs).toHaveCount(14);

    await tab("r04").locator(".file-select").click({ button: "right" });
    await menu.getByRole("menuitem", { name: /^(固定|Pin)$/ }).click();
    await expect(tab("r04").locator(".tab-pin")).toBeVisible();
    await saveLimit(page, "3");
    await expect(tabs).toHaveCount(3);
    await expect(tab("r04")).toBeVisible();
    await expect(tab("r01")).toBeVisible();

    await Promise.all([
      page.waitForResponse((response) => {
        const request = response.request();
        if (!request.url().endsWith("/__lab/invoke") || request.method() !== "POST") return false;
        const { cmd, args } = request.postDataJSON();
        return cmd === "save_settings" && args.values.recent[0].endsWith("r08");
      }),
      openFromPicker(page, "r08"),
    ]);
    await expect(tab("r08")).toHaveClass(/active/);
    await expect(tabs).toHaveCount(3);
    await expect(tab("full")).toHaveCount(0);
    await expect(tab("r04").locator(".tab-pin")).toBeVisible();

    await openScene(page, scene, query);
    await expect(tabs).toHaveCount(3);
    await expect(tab("r04").locator(".tab-pin")).toBeVisible();
    await saveLimit(page, "1");
    await expect(tab("r04").locator(".tab-pin")).toBeVisible();

    await openFromPicker(page, "r09");
    await expect(tab("r09")).toHaveClass(/active/);
    await tab("r09").locator(".file-select").click({ button: "right" });
    await expect(menu.getByRole("menuitem", { name: /最近工作区已满|Recent workspace is full/ })).toBeDisabled();
    await page.keyboard.press("Escape");

    await tab("r04").locator(".file-select").click({ button: "right" });
    await menu.getByRole("menuitem", { name: /^(取消固定|Unpin)$/ }).click();
    await expect(tabs).toHaveCount(1);
    await expect(tab("r09")).toHaveClass(/active/);
    await expect(tab("r04")).toHaveCount(0);
  });
}
