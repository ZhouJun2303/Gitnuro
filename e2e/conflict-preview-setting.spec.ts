import { expect, test } from "@playwright/test";
import { openLabScene } from "./helpers";

const themes = [
  { theme: "apple" as const, locale: "zh" as const, label: "冲突预览" },
  { theme: "light" as const, locale: "zh" as const, label: "冲突预览" },
  { theme: "dark" as const, locale: "en" as const, label: "Conflict preview" },
];

for (const item of themes) {
  test(`conflict panel hides the full preview (${item.theme})`, async ({ page }) => {
    test.setTimeout(90_000);
    await openLabScene(page, "conflict", ".conflict-pane", item);
    await expect(page.locator(".conflict-title")).toBeVisible();
    await expect(page.locator(".conflict-preview")).toHaveCount(0);
  });

  test(`diff preferences keep conflict preview off (${item.theme})`, async ({ page }) => {
    test.setTimeout(90_000);
    await openLabScene(page, "prefs-diff", "[data-pref=\"show-conflict-preview\"]", item);
    const toggle = page.locator("[data-pref=\"show-conflict-preview\"]");
    await expect(toggle).toBeVisible();
    await expect(toggle).not.toBeChecked();
    await expect(page.locator(".pref-page")).toContainText(item.label);
  });
}

test("turning conflict preview on loads the blocks and turning it off removes them", async ({ page }) => {
  test.setTimeout(90_000);
  await openLabScene(page, "conflict", ".conflict-pane", { theme: "apple", locale: "zh" });
  await expect(page.locator(".conflict-preview")).toHaveCount(0);

  await page.locator(".sidebar").getByRole("button", { name: "偏好设置" }).click();
  await page.locator(".pref-nav-item", { hasText: "差异" }).click();
  await page.locator("[data-pref=\"show-conflict-preview\"]").check();
  await page.locator(".dialog-close").click();
  await expect(page.locator(".conflict-preview-block").first()).toBeVisible();

  await page.locator(".sidebar").getByRole("button", { name: "偏好设置" }).click();
  await page.locator(".pref-nav-item", { hasText: "差异" }).click();
  await page.locator("[data-pref=\"show-conflict-preview\"]").uncheck();
  await page.locator(".dialog-close").click();
  await expect(page.locator(".conflict-preview")).toHaveCount(0);
});

test("the preview scene still shows every block when the preference is on", async ({ page }) => {
  test.setTimeout(90_000);
  await openLabScene(page, "conflict-preview", ".conflict-preview-block");
  await expect(page.locator(".conflict-preview")).toBeVisible();
  await expect(page.locator(".conflict-preview-block").first()).toBeVisible();
});
