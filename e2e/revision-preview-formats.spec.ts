import { expect, test } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"] as const) {
  test(`${theme}: preferences list preview tools, executable requirements and supported extensions`, async ({ page }) => {
    await pinLabSettings(page, { theme });
    await openLabScene(page, "prefs-preview-formats", '[data-pref-tab="previewFormats"]', { locale: "en" });
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    await expect(page.getByText("External executables")).toBeVisible();
    await expect(page.getByText("None. Previews create a temporary file beside the original; Unity.exe is not launched.")).toBeVisible();
    await expect(page.locator('[data-tool="unity"][data-scope="any"]')).toContainText(".prefab");
    await expect(page.locator('[data-tool="unity"][data-scope="unityAssets"]')).toContainText(".png");
    await expect(page.locator('[data-tool="unity"][data-scope="unityAssets"]')).toContainText(".cs");
    await expect(page.locator('[data-tool="common"]')).toContainText(".md");
    const sheets = page.locator('[data-tool="common"][data-scope="any"] .preview-format-row').filter({ has: page.locator(".preview-kind", { hasText: /^sheet$/ }) });
    await expect(sheets).toBeVisible();
    await expect(sheets.locator(".preview-extensions")).toHaveText(".xls  .xlsx  .xlsm  .xlsb  .xlt  .xltx  .xltm  .csv  .tsv  .ods  .numbers");
    await expect(page.getByText("Cocos / Godot-specific formats are not yet supported; common formats still work.")).toBeVisible();
    await sheets.scrollIntoViewIfNeeded();
    await expect(sheets).toBeInViewport();
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
    await page.screenshot({ path: `.lab/screenshots/preview-formats-${theme}.png`, fullPage: true });
  });
}
