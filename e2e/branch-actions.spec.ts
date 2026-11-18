import { expect, test, type Locator } from "@playwright/test";
import type { RefSnapshot } from "../src/lib/status";
import { openLabScene, pinLabSettings } from "./helpers";

test.afterEach(async ({ page }) => {
  await page.unrouteAll({ behavior: "wait" });
});

async function expectSyncBesideActions(row: Locator, buttonCount = 3) {
  await expect(row.locator(".ahead.up")).toBeVisible();
  await expect(row.locator(".ahead.up")).toHaveText("12");
  await expect(row.locator(".ahead.down")).toBeVisible();
  await expect(row.locator(".ahead.down")).toHaveText("34");
  await expect(row.locator(".branch-switches")).toBeVisible();
  const bounds = await row.evaluate((node) => {
    const rect = (element: Element) => {
      const { left, right, top, bottom } = element.getBoundingClientRect();
      return { left, right, top, bottom };
    };
    return {
      row: rect(node),
      name: rect(node.querySelector(".name")!),
      sync: rect(node.querySelector(".sync")!),
      actions: rect(node.querySelector(".branch-switches")!),
      buttons: Array.from(node.querySelectorAll(".branch-switches button"))
        .filter((button) => button.getClientRects().length > 0).map(rect),
    };
  });
  expect(bounds.sync.left).toBeGreaterThanOrEqual(bounds.name.right - 0.5);
  expect(bounds.sync.right).toBeLessThanOrEqual(bounds.actions.left);
  expect(bounds.actions.right).toBeLessThanOrEqual(bounds.row.right);
  expect(bounds.buttons).toHaveLength(buttonCount);
  for (const button of bounds.buttons) {
    expect(button.right - button.left).toBe(22);
    expect(button.top).toBeGreaterThanOrEqual(bounds.row.top);
    expect(button.bottom).toBeLessThanOrEqual(bounds.row.bottom);
  }
}

for (const theme of ["apple", "light", "dark"] as const) {
  test(`${theme}: branch sync arrows stay visible beside hovered, focused and active filter actions`, async ({ page }) => {
    await page.addInitScript(() => localStorage.setItem("awegit.layout.settings", JSON.stringify({ sidebarWidth: 240 })));
    await pinLabSettings(page, { theme, pinnedRefs: [] });
    await page.route("**/__lab/invoke", async (route) => {
      if (route.request().postDataJSON()?.cmd !== "repository_refs") return route.fallback();
      const response = await route.fetch();
      const payload: { ok: boolean; value: RefSnapshot } = await response.json();
      for (const branch of payload.value.branches) {
        if (branch.name === "main") {
          branch.ahead = 12;
          branch.behind = 34;
        }
      }
      await route.fulfill({ json: payload });
    });
    await openLabScene(page, "branch-badges", '.side.nested:has(.branch-switches[data-branch="main"])', { locale: "en" });
    const row = page.locator(".side.nested").filter({ has: page.locator('.branch-switches[data-branch="main"]') });
    const select = row.locator(".file-select");
    const pin = row.getByRole("button", { name: "Pin: main", exact: true });
    const filter = row.locator(".history-filter-toggle");
    const search = page.locator(".side-filter input");

    await search.hover();
    await expect(row.locator(".sync")).toBeVisible();
    await row.hover();
    await expectSyncBesideActions(row);
    await page.locator(".sidebar").screenshot({ path: `.lab/screenshots/branch-sync-${theme}-hover.png`, animations: "disabled" });

    await search.hover();
    await select.focus();
    await expectSyncBesideActions(row);
    await page.keyboard.press("Tab");
    await expect(pin).toBeFocused();
    await expectSyncBesideActions(row);

    await pin.click();
    await row.hover();
    await filter.click();
    await expect(filter).toHaveAttribute("aria-pressed", "true");
    await search.focus();
    await search.hover();
    await expectSyncBesideActions(row, 1);
    expect(await row.locator(".name").evaluate((node) => node.scrollWidth - node.clientWidth)).toBeLessThanOrEqual(1);
    await expect(filter).toHaveAttribute("title", "Remove branch from history filter");
    await expect(filter.locator("..")).toHaveCSS("opacity", "1");
    await page.locator(".sidebar").screenshot({ path: `.lab/screenshots/branch-sync-${theme}-filtered.png`, animations: "disabled" });

    await row.hover();
    await expectSyncBesideActions(row);
    await page.locator(".sidebar").screenshot({ path: `.lab/screenshots/branch-sync-${theme}-filtered-hover.png`, animations: "disabled" });

    await filter.click();
    await expect(filter).toHaveAttribute("aria-pressed", "false");
    await search.focus();
    await search.hover();
    await expect(row.locator(".sync")).toBeVisible();
    await expect(row.locator(".branch-switches")).toBeHidden();
  });

  for (const locale of ["zh", "en"] as const) {
    test(`${theme} ${locale}: branch actions show hide, pin and history filter in order`, async ({ page }) => {
      await pinLabSettings(page, { theme, pinnedRefs: [] });
      await openLabScene(page, "history", '.side.nested:has(.branch-switches[data-branch="main"])', { locale });

      const actions = page.locator('.branch-switches[data-branch="main"]');
      const buttons = actions.locator("button");
      const hide = buttons.nth(0);
      const pin = buttons.nth(1);
      const filter = buttons.nth(2);
      const labels = locale === "zh"
        ? { hide: "隐藏", pin: "固定", unpin: "取消固定", include: "将分支加入历史筛选（可多选）", remove: "将分支移出历史筛选" }
        : { hide: "Hide", pin: "Pin", unpin: "Unpin", include: "Include branch in history filter (multiple selection)", remove: "Remove branch from history filter" };

      await actions.locator("..").hover();
      await expect(hide).toBeDisabled();
      await expect(hide).toHaveAccessibleName(labels.hide);
      const [hideX, pinX, filterX] = await buttons.evaluateAll((nodes) => nodes.map((node) => node.getBoundingClientRect().left));
      expect(hideX).toBeLessThan(pinX);
      expect(pinX).toBeLessThan(filterX);
      await expect(pin).toHaveAttribute("aria-pressed", "false");
      await expect(pin).toHaveAttribute("title", labels.pin);
      await expect(pin).toHaveAccessibleName(`${labels.pin}: main`);
      const pinIcon = await pin.locator("svg").innerHTML();
      const inactiveBackground = await pin.evaluate((node) => getComputedStyle(node).backgroundColor);
      await pin.click();
      await actions.locator("..").hover();
      await expect(pin).toHaveAttribute("aria-pressed", "true");
      await expect(pin).toHaveAttribute("title", labels.unpin);
      await expect(pin).toHaveAccessibleName(`${labels.unpin}: main`);
      expect(await pin.locator("svg").innerHTML()).not.toBe(pinIcon);
      expect(await pin.evaluate((node) => getComputedStyle(node).backgroundColor)).not.toBe(inactiveBackground);

      await actions.locator("..").hover();
      if (await filter.getAttribute("aria-pressed") === "true") await filter.click();
      await expect(filter).toHaveAttribute("title", labels.include);
      await expect(filter).toHaveAccessibleName(`${labels.include}: main`);
      const includeIcon = await filter.locator("svg").innerHTML();
      await filter.click();
      await expect(filter).toHaveAttribute("aria-pressed", "true");
      await expect(filter).toHaveAttribute("title", labels.remove);
      await expect(filter).toHaveAccessibleName(`${labels.remove}: main`);
      expect(await filter.locator("svg").innerHTML()).not.toBe(includeIcon);
      await expect(page.locator(".filter-bar")).toContainText("main");
      for (const button of [pin, filter]) {
        const colors = await button.evaluate((node) => ({
          icon: getComputedStyle(node.querySelector("svg")!).color,
          text: getComputedStyle(node).color,
        }));
        expect(colors.icon).toBe(colors.text);
      }
      await page.locator(".sidebar").screenshot({ path: `.lab/screenshots/branch-actions-${theme}-${locale}-active.png`, animations: "disabled" });

      await filter.click();
      await expect(filter).toHaveAttribute("aria-pressed", "false");
      await expect(filter).toHaveAttribute("title", labels.include);
      expect(await filter.locator("svg").innerHTML()).toBe(includeIcon);
      await actions.locator("..").hover();
      await pin.click();
      await expect(pin).toHaveAttribute("aria-pressed", "false");
      await expect(pin).toHaveAttribute("title", labels.pin);
      expect(await pin.locator("svg").innerHTML()).toBe(pinIcon);
      await actions.locator("..").hover();
      await page.locator(".sidebar").screenshot({ path: `.lab/screenshots/branch-actions-${theme}-${locale}-inactive.png`, animations: "disabled" });
    });
  }
}
