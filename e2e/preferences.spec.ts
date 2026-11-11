import { expect, test, type Page } from "@playwright/test";

async function openPreferences(page: Page) {
  await page.goto("/?scene=prefs-language");
  await expect(page.locator('[data-lab-scene="prefs-language"]')).toBeVisible();
  await expect(page.locator('[data-dialog="prefs"]')).toBeVisible();
  await expect(page.locator('[data-dialog="prefs"] select').first()).toHaveValue("zh");
}

async function saveLocale(page: Page, locale: string, action: () => Promise<unknown>) {
  const saved = page.waitForResponse((response) => {
    if (!response.url().endsWith("/__lab/invoke") || response.request().method() !== "POST") return false;
    const body = response.request().postDataJSON();
    return body.cmd === "save_settings" && body.args.values.locale === locale;
  });
  await action();
  expect((await (await saved).json()).ok).toBe(true);
  const response = await page.request.post("/__lab/invoke", { data: { cmd: "load_settings", args: {} } });
  const payload = await response.json();
  expect(payload.ok).toBe(true);
  expect(payload.value.locale).toBe(locale);
}

test("language shortcut switches and saves from preferences, text fields and the main window", async ({ page }) => {
  await openPreferences(page);
  const dialog = page.locator('[data-dialog="prefs"]');
  const language = dialog.locator("select").first();
  await language.focus();
  await saveLocale(page, "en", () => page.keyboard.press("Control+Alt+l"));
  await expect(language).toHaveValue("en");
  await expect(dialog.getByRole("button", { name: "General", exact: true })).toBeVisible();

  await language.dispatchEvent("keydown", { key: "l", ctrlKey: true, altKey: true, repeat: true });
  await expect(language).toHaveValue("en");
  const alias = dialog.locator("input.alias").first();
  await alias.fill("language shortcut");
  await saveLocale(page, "zh", () => page.keyboard.press("Control+Alt+l"));
  await expect(alias).toHaveValue("language shortcut");
  await expect(language).toHaveValue("zh");
  await expect(dialog.getByRole("button", { name: "通用", exact: true })).toBeVisible();

  await dialog.locator(".dialog-close").click();
  await expect(dialog).toHaveCount(0);
  await saveLocale(page, "en", () => page.keyboard.press("Control+Alt+l"));
  await expect(page.getByRole("button", { name: "Appearance", exact: true })).toBeVisible();
});

test("language shortcut switches away from the resolved system language", async ({ page }) => {
  await openPreferences(page);
  const language = page.locator('[data-dialog="prefs"] select').first();
  await saveLocale(page, "system", () => language.selectOption("system"));
  const next = await page.evaluate(() => navigator.language.toLowerCase().startsWith("zh") ? "en" : "zh");
  await saveLocale(page, next, () => page.keyboard.press("Control+Alt+l"));
  await expect(language).toHaveValue(next);
});

test("preferences backdrop keeps the panel open and explicit controls close it", async ({ page }) => {
  await openPreferences(page);
  const dialog = page.locator('[data-dialog="prefs"]');
  await dialog.getByRole("button", { name: "提交", exact: true }).click();
  const name = dialog.locator(".commit-type-name").first();
  await expect(name).toBeVisible();
  await page.locator(".scrim").click({ position: { x: 8, y: 8 } });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("button", { name: "提交", exact: true })).toHaveClass("on");
  await dialog.getByRole("button", { name: "完成", exact: true }).click();
  await expect(dialog).toHaveCount(0);

  await page.keyboard.press("Control+,");
  await expect(dialog).toBeVisible();
  await dialog.locator(".dialog-close").click();
  await expect(dialog).toHaveCount(0);
  await page.keyboard.press("Control+,");
  await expect(dialog).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
});

test("appearance menu shows the language shortcut and switches language", async ({ page }) => {
  await openPreferences(page);
  await page.locator(".dialog-close").click();
  await page.getByRole("button", { name: "外观", exact: true }).click();
  const item = page.locator('[data-layer="menu"] button', { hasText: "切换语言" });
  await expect(item).toContainText("Ctrl+Alt+L");
  await saveLocale(page, "en", () => item.click());
  await expect(page.getByRole("button", { name: "Appearance", exact: true })).toBeVisible();
  await expect(page.locator('[data-layer="menu"]')).toHaveCount(0);
});

for (const scenario of [
  { scene: "suggest-unconfigured", field: "apiKey", tab: "commit", requests: 1 },
  { scene: "suggest-no-url", field: "baseUrl", tab: "commit", requests: 0 },
  { scene: "suggest-disabled", field: "enabled", tab: "git", requests: 0 },
]) {
  test(`generation button opens and focuses missing AI configuration (${scenario.field})`, async ({ page }) => {
    await page.setViewportSize({ width: 900, height: 600 });
    let suggestions = 0;
    await page.route("**/__lab/invoke", async (route) => {
      if (route.request().postDataJSON().cmd !== "suggest_commit_message") return route.continue();
      suggestions += 1;
      await route.fulfill({ json: { ok: false, error: "Add an API key in Preferences, or set XAI_API_KEY or OPENAI_API_KEY." } });
    });
    await page.goto(`/?scene=${scenario.scene}`);
    await expect(page.locator(`[data-lab-scene="${scenario.scene}"]`)).toBeVisible();
    const summary = page.locator(".summary-input");
    await summary.fill("Keep this draft");
    await page.locator(".suggest-message").click();
    const dialog = page.locator('[data-dialog="prefs"]');
    const target = dialog.locator(`[data-ai-field="${scenario.field}"]`);
    await expect(dialog).toBeVisible();
    await expect(target).toBeFocused();
    await expect(target).toBeInViewport();
    await expect(dialog.locator(".pref-tabs > button.on")).toHaveText(scenario.tab === "git" ? "Git" : /提交|Commit/);
    expect(suggestions).toBe(scenario.requests);
    await dialog.locator(".dialog-close").click();
    await expect(summary).toHaveValue("Keep this draft");
  });
}

test("generation button still accepts a key configured through the backend", async ({ page }) => {
  await page.route("**/__lab/invoke", async (route) => {
    if (route.request().postDataJSON().cmd !== "suggest_commit_message") return route.continue();
    await route.fulfill({ json: { ok: true, value: { summary: "feat: Generated message", description: "Generated description" } } });
  });
  await page.goto("/?scene=suggest-unconfigured");
  await expect(page.locator('[data-lab-scene="suggest-unconfigured"]')).toBeVisible();
  await page.locator(".suggest-message").click();
  await expect(page.locator(".summary-input")).toHaveValue("Generated message");
  await expect(page.locator('[data-dialog="prefs"]')).toHaveCount(0);
});

test("independent windows update language immediately and retain merge edits", async ({ page, context }) => {
  test.setTimeout(60_000);
  await openPreferences(page);
  const flow = await context.newPage();
  const history = await context.newPage();
  const blob = await context.newPage();
  const merge = await context.newPage();
  try {
    await Promise.all([
      flow.goto("/?scene=operation-flow&view=flow&file=live"),
      history.goto("/?scene=file-history&view=history&file=README.md"),
      blob.goto("/?scene=blob&view=blob&file=README.md&rev=HEAD"),
      merge.goto("/?scene=text-merge-window&view=merge"),
    ]);
    await expect(flow.locator('[data-lab-scene="operation-flow"]')).toBeVisible();
    await expect(history.locator('[data-lab-scene="file-history"]')).toBeVisible();
    await expect(blob.locator('[data-lab-scene="blob"]')).toBeVisible();
    await expect(merge.locator('[data-lab-scene="text-merge-window"]')).toBeVisible();
    await merge.locator(".merge-edit-entry").click();
    const result = merge.locator(".merge-result-edit");
    await result.fill("Keep my independent merge draft\n");
    const language = page.locator('[data-dialog="prefs"] select').first();
    await saveLocale(page, "zh", () => language.selectOption("zh"));
    await expect(flow.locator("h1")).toHaveText("操作与冲突流程");

    await saveLocale(page, "en", () => page.keyboard.press("Control+Alt+l"));
    await expect(flow.locator("h1")).toHaveText("Operations & conflicts", { timeout: 2000 });
    await expect(history.getByRole("button", { name: "Close", exact: true })).toBeVisible();
    await expect(blob.getByRole("button", { name: "Close", exact: true })).toBeVisible();
    await expect(merge.locator(".merge-close")).toHaveAttribute("aria-label", "Close");
    await expect(result).toHaveValue("Keep my independent merge draft\n");

    // A settings reply from before the switch must not restore the old language.
    let releaseSave!: () => void;
    let saveArrived!: () => void;
    const heldSave = new Promise<void>((resolve) => { releaseSave = resolve; });
    const arrived = new Promise<void>((resolve) => { saveArrived = resolve; });
    await page.route("**/__lab/invoke", async (route) => {
      const body = route.request().postDataJSON();
      if (body.cmd !== "save_settings" || body.args.values.locale !== "en") return route.continue();
      const response = await route.fetch();
      saveArrived();
      await heldSave;
      await route.fulfill({ response });
    });
    const theme = page.locator('[data-dialog="prefs"] select').nth(1);
    await theme.selectOption(await theme.inputValue() === "dark" ? "light" : "dark");
    await arrived;
    await saveLocale(flow, "zh", () => flow.keyboard.press("Control+Alt+l"));
    await expect(flow.locator("h1")).toHaveText("操作与冲突流程");
    await expect(language).toHaveValue("zh", { timeout: 2000 });
    await expect(history.getByRole("button", { name: "关闭", exact: true })).toBeVisible();
    await expect(blob.getByRole("button", { name: "关闭", exact: true })).toBeVisible();
    await expect(merge.locator(".merge-close")).toHaveAttribute("aria-label", "关闭");
    await expect(result).toHaveValue("Keep my independent merge draft\n");
    await saveLocale(page, "zh", async () => { releaseSave(); });
    await expect(language).toHaveValue("zh");
  } finally {
    await Promise.all([flow.close(), history.close(), blob.close(), merge.close()]);
  }
});
