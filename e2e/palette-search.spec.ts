import { expect, test, type Page } from "@playwright/test";
import { openLabScene } from "./helpers";

function named(page: Page, label: string) {
  return page.locator(".palette-list .name").filter({ hasText: new RegExp(`^${label}$`) });
}

test("palette search matches commands in either language", async ({ page }) => {
  await openLabScene(page, "launch", ".palette-input", { locale: "en" });
  const en = page.locator(".palette-input");
  await en.fill("推送");
  await expect(named(page, "Push")).toBeVisible();
  await expect(named(page, "Push tags")).toBeVisible();

  await openLabScene(page, "launch", ".palette-input", { locale: "zh" });
  const zh = page.locator(".palette-input");
  await zh.fill("push");
  await expect(named(page, "推送")).toBeVisible();
  await expect(named(page, "推送标签")).toBeVisible();
});
