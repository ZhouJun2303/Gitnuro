import { expect, test, type Page } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

async function pressPrintShortcut(page: Page) {
  return page.evaluate(() => {
    const event = new KeyboardEvent("keydown", { key: "p", ctrlKey: true, bubbles: true, cancelable: true });
    (document.activeElement ?? window).dispatchEvent(event);
    return event.defaultPrevented;
  });
}

for (const theme of ["apple", "light", "dark"] as const) {
  test(`Ctrl+P mirrors quick launch without printing (${theme})`, async ({ page }) => {
    await pinLabSettings(page, { theme, locale: "en" });
    await openLabScene(page, "changes", ".summary-input");
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);

    await page.keyboard.press("Control+k");
    await expect(page.locator(".palette-input")).toBeFocused();
    expect(await pressPrintShortcut(page)).toBe(true);
    await expect(page.locator(".palette")).toHaveCount(0);

    const summary = page.locator(".summary-input");
    await summary.fill("draft");
    expect(await pressPrintShortcut(page)).toBe(true);
    await expect(page.locator(".palette-input")).toBeFocused();
    await expect(summary).toHaveValue("draft");

    await page.keyboard.press("Escape");
    await expect(page.locator(".palette")).toHaveCount(0);
    await page.locator(".chrome").click({ position: { x: 5, y: 5 } });
    await page.keyboard.press("Control+,");
    await expect(page.locator('[data-dialog="prefs"]')).toBeVisible();
    expect(await pressPrintShortcut(page)).toBe(true);
    await expect(page.locator(".palette")).toHaveCount(0);
    await expect(page.locator('[data-dialog="prefs"]')).toBeVisible();
  });

  test(`shortcut settings show all actions (${theme})`, async ({ page }) => {
    await pinLabSettings(page, { theme, locale: "en" });
    await openLabScene(page, "prefs-shortcuts", '[data-pref-tab="shortcuts"]');
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    await expect(page.locator('[data-shortcut="launch"] .shortcut-key')).toHaveText(["Ctrl+K", "Ctrl+P"]);
    await expect(page.locator('[data-shortcut="performance"] .shortcut-key')).toHaveText("Ctrl+Alt+P");
    await expect(page.locator(".shortcut-row")).toHaveCount(44);
  });
}

test("shortcut editor detects collisions and persists overrides across reloads", async ({ page }) => {
  await pinLabSettings(page, { locale: "en", theme: "light" });
  await openLabScene(page, "prefs-shortcuts", '[data-pref-tab="shortcuts"]');
  const search = page.getByRole("searchbox", { name: "Search commands or keys" });
  await search.fill("quick launch");
  const launch = page.locator('[data-shortcut="launch"]');
  await expect(page.locator(".shortcut-row")).toHaveCount(1);
  await launch.locator(".shortcut-key").first().click();
  await page.keyboard.press("Control+f");
  await expect(launch.getByRole("alert")).toContainText("Search commits");
  await page.keyboard.press("Control+j");
  await expect(launch.locator(".shortcut-key")).toHaveText(["Ctrl+J", "Ctrl+P"]);
  await page.reload();
  await expect(page.locator('[data-shortcut="launch"] .shortcut-key')).toHaveText(["Ctrl+J", "Ctrl+P"]);
  await expect(page.locator(".chrome-tools .tool-btn.stack").first()).toHaveAttribute("title", /Ctrl\+J/);
  await page.locator(".dialog-close").click();
  await page.keyboard.press("Control+j");
  await expect(page.locator(".palette-input")).toBeFocused();
  await page.keyboard.press("Escape");
  await openLabScene(page, "prefs-shortcuts", '[data-pref-tab="shortcuts"]');
  await page.getByRole("button", { name: "Restore all defaults" }).click();
  await expect(page.locator('[data-shortcut="launch"] .shortcut-key')).toHaveText(["Ctrl+K", "Ctrl+P"]);
  const resetLaunch = page.locator('[data-shortcut="launch"]');
  await resetLaunch.getByRole("button", { name: "Add shortcut: Quick launch" }).click();
  await page.keyboard.press("Control+i");
  await expect(resetLaunch.locator(".shortcut-key")).toHaveText(["Ctrl+K", "Ctrl+P", "Ctrl+I"]);
  await resetLaunch.getByRole("button", { name: "Clear shortcut: Quick launch Ctrl+P" }).click();
  await expect(resetLaunch.locator(".shortcut-key")).toHaveText(["Ctrl+K", "Ctrl+I"]);
  await page.locator(".dialog-close").click();
  expect(await pressPrintShortcut(page)).toBe(true);
  await expect(page.locator(".palette")).toHaveCount(0);
  await page.keyboard.press("Control+i");
  await expect(page.locator(".palette-input")).toBeFocused();
});

test("performance shortcut stays available while preferences are open", async ({ page }) => {
  await openLabScene(page, "prefs-shortcuts", '[data-pref-tab="shortcuts"]');
  await page.keyboard.press("Control+Alt+p");
  await expect(page.locator('[data-layer="perf"]')).toBeVisible();
  await page.keyboard.press("Control+Alt+p");
  await expect(page.locator('[data-layer="perf"]')).toHaveCount(0);
});
