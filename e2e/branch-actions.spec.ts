import { expect, test } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"] as const) {
  for (const locale of ["zh", "en"] as const) {
    test(`${theme} ${locale}: branch action icons and labels follow pin and history filter state`, async ({ page }) => {
      await pinLabSettings(page, { theme, pinnedRefs: [] });
      await openLabScene(page, "history", '.branch-switches[data-branch="main"]', { locale });

      const actions = page.locator('.branch-switches[data-branch="main"]');
      const pin = actions.locator("button").nth(0);
      const filter = actions.locator("button").nth(1);
      const labels = locale === "zh"
        ? { pin: "固定", unpin: "取消固定", include: "将分支加入历史筛选（可多选）", remove: "将分支移出历史筛选" }
        : { pin: "Pin", unpin: "Unpin", include: "Include branch in history filter (multiple selection)", remove: "Remove branch from history filter" };

      await actions.locator("..").hover();
      await expect(pin).toHaveAttribute("aria-pressed", "false");
      await expect(pin).toHaveAttribute("title", labels.pin);
      await expect(pin).toHaveAccessibleName(`${labels.pin}: main`);
      const pinIcon = await pin.locator("svg").innerHTML();
      const inactiveBackground = await pin.evaluate((node) => getComputedStyle(node).backgroundColor);
      await pin.click();
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
