import { expect, test, type Page } from "@playwright/test";
import { openLabScene } from "./helpers";

// Keep theme selection local to this test; the shared lab settings must stay intact.
test.beforeEach(async ({ context }) => {
  let settings: Record<string, unknown> | undefined;
  await context.route("**/__lab/invoke", async (route) => {
    const { cmd, args } = route.request().postDataJSON();
    if (cmd === "load_settings") {
      if (!settings) {
        const response = await route.fetch();
        const payload = await response.json();
        settings = { ...payload.value, theme: "apple", locale: "zh", verticalTabs: false };
      }
      return route.fulfill({ json: { ok: true, value: settings } });
    }
    if (cmd === "save_settings") {
      settings = args.patch ? { ...settings, ...args.patch } : args.values;
      return route.fulfill({ json: { ok: true, value: settings } });
    }
    return route.continue();
  });
});

async function appearance(page: Page) {
  return page.evaluate(() => {
    const style = getComputedStyle(document.querySelector(".dialog")!);
    const surface = getComputedStyle(document.querySelector(".pref-card")!).backgroundColor;
    return { radius: parseFloat(style.borderRadius), surface: surface.match(/[\d.]+/g)!.slice(0, 3).map(Number) };
  });
}

test("three independent themes default to Apple and the selection survives reload", async ({ page }) => {
  await openLabScene(page, "prefs-appearance", ".theme-styles");
  const code = page.getByRole("button", { name: /VS Code 黑色/ });
  const apple = page.getByRole("button", { name: /Apple Liquid Glass/ });
  const white = page.getByRole("button", { name: /经典白色/ });
  await expect(page.locator(".theme-style")).toHaveCount(3);
  await expect(apple).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "apple");
  await code.click();
  await expect(code).toHaveAttribute("aria-pressed", "true");
  const dark = await appearance(page);
  expect(Math.max(...dark.surface)).toBeLessThan(60);
  expect(Math.max(...dark.surface) - Math.min(...dark.surface)).toBeLessThanOrEqual(2);

  const saved = page.waitForResponse((response) => response.url().endsWith("/__lab/invoke")
    && response.request().postDataJSON().cmd === "save_settings"
    && response.request().postDataJSON().args.values?.theme === "apple");
  await apple.click();
  await saved;
  await expect(page.locator("html")).toHaveAttribute("data-theme", "apple");
  await expect(apple).toHaveAttribute("aria-pressed", "true");
  const light = await appearance(page);
  expect(Math.min(...light.surface)).toBeGreaterThan(230);
  expect(await page.locator(".dialog").evaluate((node) => getComputedStyle(node).backdropFilter)).toContain("blur(");
  expect(light.radius).toBeGreaterThan(dark.radius);

  await white.click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await expect(white).toHaveAttribute("aria-pressed", "true");
  expect(await page.locator(".dialog").evaluate((node) => getComputedStyle(node).backdropFilter)).toBe("none");
  expect(await page.locator(".shell").evaluate((node) => getComputedStyle(node).backgroundImage)).toBe("none");
  await apple.click();

  await page.reload();
  await expect(page.locator('[data-lab-scene="prefs-appearance"]')).toBeVisible();
  await expect(apple).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "apple");
  await code.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(code).toHaveAttribute("aria-pressed", "true");
});

for (const theme of ["apple", "light", "dark"]) {
  test(`welcome actions remain usable in a narrow ${theme} window`, async ({ page }) => {
    await page.setViewportSize({ width: 560, height: 640 });
    await page.goto(`/?scene=welcome&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="welcome"]')).toBeVisible();
    const actions = page.locator(".welcome-btn");
    await expect(actions).toHaveCount(3);
    await actions.nth(1).focus();
    await page.keyboard.press("Enter");
    await expect(page.locator('[data-dialog="clone"]')).toBeVisible();
    const outside = await page.evaluate(() => {
      const box = document.querySelector(".dialog")!.getBoundingClientRect();
      return box.left < 0 || box.right > innerWidth || box.top < 0 || box.bottom > innerHeight;
    });
    expect(outside).toBe(false);
    await page.keyboard.press("Escape");
    await expect(page.locator('[data-dialog="clone"]')).toHaveCount(0);
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
  });
}

test("Liquid Glass keeps history, navigation and preferences usable at 960px", async ({ page }, info) => {
  await page.setViewportSize({ width: 960, height: 640 });
  await page.goto("/?scene=history&theme=apple&locale=zh");
  await expect(page.locator('[data-lab-scene="history"]')).toBeVisible();
  const history = page.locator(".history");
  await history.evaluate((node) => { node.scrollTop = node.scrollHeight; });
  const last = history.locator(".commit-row").last();
  await last.click();
  await expect(last).toHaveClass(/selected/);
  await info.attach("apple-history-compact", { body: await page.screenshot(), contentType: "image/png" });

  await page.locator('[data-section="changes"]').click();
  await expect(page.locator(".file-list").first()).toBeVisible();
  await expect(page.locator(".composer .summary-input")).toBeVisible();
  await info.attach("apple-changes-compact", { body: await page.screenshot(), contentType: "image/png" });
  await page.locator(".side-brand .icon-btn").click();
  await expect(page.locator('[data-dialog="prefs"]')).toBeVisible();
  await expect(page.getByRole("button", { name: /Apple Liquid Glass/ })).toBeVisible();
  const layout = await page.evaluate(() => {
    const box = document.querySelector(".dialog")!.getBoundingClientRect();
    return { overflow: document.documentElement.scrollWidth - innerWidth,
      outside: box.left < 0 || box.top < 0 || box.right > innerWidth || box.bottom > innerHeight };
  });
  expect(layout.overflow).toBeLessThanOrEqual(1);
  expect(layout.outside).toBe(false);
});

test("appearance switches in all independent windows without losing the merge draft", async ({ page, context }) => {
  test.setTimeout(90_000);
  await openLabScene(page, "prefs-appearance", ".theme-styles");
  const children = await Promise.all(Array.from({ length: 4 }, () => context.newPage()));
  const [flow, history, blob, merge] = children;
  try {
    await Promise.all([
      flow.goto("/?scene=operation-flow&view=flow&file=live&theme=dark"),
      history.goto("/?scene=file-history&view=history&file=README.md&theme=dark"),
      blob.goto("/?scene=blob&view=blob&file=README.md&rev=HEAD&theme=dark"),
      merge.goto("/?scene=text-merge-window&view=merge&theme=dark"),
    ]);
    await expect(flow.locator(".flow-node.active")).toBeVisible();
    await expect(history.locator(".history-commit").first()).toBeVisible();
    await expect(blob.locator(".blob-row").first()).toBeVisible();
    await expect(merge.locator(".merge-panel")).toBeVisible();
    await merge.locator(".merge-edit-entry").click();
    const draft = merge.locator(".merge-result-edit");
    await draft.fill("Keep this draft when switching appearance\n");
    for (const theme of ["dark", "light", "apple"]) {
      await page.getByRole("combobox", { name: "主题", exact: true }).selectOption(theme);
      for (const child of children) {
        await expect(child.locator("html")).toHaveAttribute("data-theme", theme);
      }
      await expect(draft).toHaveValue("Keep this draft when switching appearance\n");
    }
    for (const child of children) {
      await child.setViewportSize({ width: 960, height: 640 });
      expect(await child.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
    }
    expect(await flow.locator(".flow-overview").evaluate((node) => getComputedStyle(node).backdropFilter)).toContain("blur(");
    expect(await history.locator(".history-file").evaluate((node) => getComputedStyle(node).backdropFilter)).toContain("blur(");
    expect(await blob.locator("header").evaluate((node) => getComputedStyle(node).backdropFilter)).toContain("blur(");
    expect(await merge.locator(".merge-panel-title").evaluate((node) => getComputedStyle(node).backdropFilter)).toContain("blur(");
  } finally {
    await Promise.all(children.map((child) => child.close()));
  }
});

for (const scene of ["text-merge-many", "file-merge"]) {
  test(`Liquid Glass merge controls fit the minimum window: ${scene}`, async ({ page }, info) => {
    await page.setViewportSize({ width: 480, height: 360 });
    await openLabScene(page, scene, ".merge-panel");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "apple");
    for (const control of await page.locator(".merge-close, .merge-party input, .merge-panel-foot button").all()) {
      await expect(control).toBeInViewport({ ratio: 1 });
    }
    await page.locator(".merge-party input").first().check();
    await expect(page.locator(".merge-panel-foot .commit")).toBeEnabled();
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
    const screenshot = `.lab/ui-audit/apple-${scene}-minimum.png`;
    await page.screenshot({ path: screenshot });
    await info.attach("apple-merge-minimum", { path: screenshot, contentType: "image/png" });
  });
}
