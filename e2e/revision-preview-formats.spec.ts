import { expect, test } from "@playwright/test";
import { openLabScene } from "./helpers";

test("preferences list preview tools, executable requirements and supported extensions", async ({ page }) => {
  await openLabScene(page, "prefs-preview-formats", '[data-pref-tab="previewFormats"]', { locale: "en" });
  await expect(page.getByText("External executables")).toBeVisible();
  await expect(page.getByText("None. Previews create a temporary file beside the original; Unity.exe is not launched.")).toBeVisible();
  await expect(page.locator('[data-tool="unity"][data-scope="any"]')).toContainText(".prefab");
  await expect(page.locator('[data-tool="unity"][data-scope="unityAssets"]')).toContainText(".png");
  await expect(page.locator('[data-tool="unity"][data-scope="unityAssets"]')).toContainText(".cs");
  await expect(page.locator('[data-tool="common"]')).toContainText(".md");
  await expect(page.getByText("Cocos / Godot-specific formats are not yet supported; common formats still work.")).toBeVisible();
});
