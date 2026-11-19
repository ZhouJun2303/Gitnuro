import { expect, test } from "@playwright/test";
import { pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"] as const) {
  test(`killer push uses a solid push icon (${theme})`, async ({ page }) => {
    await pinLabSettings(page, { theme, locale: "zh", killerPush: true, fetchAutomatically: false });
    await page.goto(`/?scene=ahead&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="ahead"]')).toBeVisible();

    const push = page.locator(".chrome-left").getByTitle(/推送|Push/);
    await expect(push.locator("svg.push-icon.solid")).toBeVisible();
    await expect(push.locator("svg.push-icon.outline")).toHaveCount(0);

    // The ahead count keeps the familiar top-right corner, clear of the icon.
    const count = push.locator(".sync-count");
    await expect(count).toHaveText("24");
    const button = await push.boundingBox();
    const mark = await count.boundingBox();
    expect(button && mark).toBeTruthy();
    expect(mark!.x + mark!.width).toBeGreaterThan(button!.x + button!.width - 6);
    expect(mark!.y).toBeLessThan(button!.y + 6);

    await page.screenshot({ path: `.lab/killer-push-icon-${theme}.png` });
  });
}

test("killer push is off by default and saves from Git preferences", async ({ page }) => {
  await pinLabSettings(page, { locale: "zh", fetchAutomatically: false });
  await page.goto("/?scene=ahead&locale=zh");
  await expect(page.locator('[data-lab-scene="ahead"]')).toBeVisible();
  const push = page.locator(".chrome-left").getByTitle(/推送|Push/);
  await expect(push.locator("svg.push-icon.outline")).toBeVisible();
  await expect(push.locator("svg.push-icon.solid")).toHaveCount(0);
  await expect(push.locator(".sync-count")).toBeVisible();
  await page.screenshot({ path: ".lab/killer-push-off.png" });

  await page.goto("/?scene=prefs-git&locale=zh");
  await expect(page.locator('[data-lab-scene="prefs-git"]')).toBeVisible();
  const dialog = page.locator('[data-dialog="prefs"]');
  const toggle = dialog.getByLabel("Killer 推送");
  await expect(toggle).not.toBeChecked();

  const saved = page.waitForResponse((response) => {
    if (!response.url().endsWith("/__lab/invoke") || response.request().method() !== "POST") return false;
    const body = response.request().postDataJSON();
    return body.cmd === "save_settings" && body.args.values.killerPush === true;
  });
  await toggle.check();
  await dialog.getByRole("button", { name: "完成", exact: true }).click();
  expect((await (await saved).json()).ok).toBe(true);

  await page.goto("/?scene=ahead&locale=zh");
  await expect(page.locator('[data-lab-scene="ahead"]')).toBeVisible();
  await expect(page.locator(".chrome-left").getByTitle(/推送|Push/).locator("svg.push-icon.solid")).toBeVisible();
});

test("killer push can be turned off again from preferences", async ({ page }) => {
  await pinLabSettings(page, { locale: "zh", killerPush: true, fetchAutomatically: false });
  await page.goto("/?scene=prefs-git&locale=zh");
  await expect(page.locator('[data-lab-scene="prefs-git"]')).toBeVisible();
  const dialog = page.locator('[data-dialog="prefs"]');
  const toggle = dialog.getByLabel("Killer 推送");
  await expect(toggle).toBeChecked();

  const saved = page.waitForResponse((response) => {
    if (!response.url().endsWith("/__lab/invoke") || response.request().method() !== "POST") return false;
    const body = response.request().postDataJSON();
    return body.cmd === "save_settings" && body.args.values.killerPush === false;
  });
  await toggle.uncheck();
  await dialog.getByRole("button", { name: "完成", exact: true }).click();
  expect((await (await saved).json()).ok).toBe(true);

  await page.goto("/?scene=ahead&locale=zh");
  await expect(page.locator('[data-lab-scene="ahead"]')).toBeVisible();
  await expect(page.locator(".chrome-left").getByTitle(/推送|Push/).locator("svg.push-icon.solid")).toHaveCount(0);
  await expect(page.locator(".chrome-left").getByTitle(/推送|Push/).locator("svg.push-icon.outline")).toBeVisible();
});
