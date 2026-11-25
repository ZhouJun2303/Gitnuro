import { mkdirSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { scenes } from "../src/lib/lab/scenes";
import { openScene, pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"] as const) {
  test(`history multiselect ${theme} toggles commits and keeps a stable Shift anchor`, async ({ page }) => {
    await pinLabSettings(page, { locale: "en", theme });
    await openScene(page, scenes.find((scene) => scene.id === "history")!, `&locale=en&theme=${theme}`);
    const rows = page.locator(".history .commit-row");
    const selected = page.locator(".history .commit-row.selected");
    await rows.nth(1).click();
    await rows.nth(3).click({ modifiers: ["Control"] });
    await expect(selected).toHaveCount(2);
    await expect(rows.nth(1)).toHaveAttribute("aria-pressed", "true");
    await rows.nth(3).click({ modifiers: ["Control"] });
    await expect(selected).toHaveCount(1);
    await expect(rows.nth(1)).toHaveClass(/selected/);
    await rows.nth(1).click({ modifiers: ["Control"] });
    await expect(selected).toHaveCount(0);

    await rows.nth(1).click();
    await rows.nth(4).click({ modifiers: ["Shift"] });
    await expect(selected).toHaveCount(4);
    await rows.nth(3).click({ modifiers: ["Shift"] });
    await expect(selected).toHaveCount(3);
    await rows.nth(5).click({ modifiers: ["Meta"] });
    await expect(selected).toHaveCount(4);
    await rows.nth(2).click();
    await expect(selected).toHaveCount(1);
    await rows.nth(4).click({ modifiers: ["Control"] });
    await expect(selected).toHaveCount(2);
    await rows.nth(4).press("ArrowDown");
    await expect(selected).toHaveCount(1);
    const selectedId = await selected.getAttribute("data-commit-id");
    const summary = (await rows.first().locator(".subject").innerText()).trim();
    const beforeSearch = await rows.count();
    await page.getByRole("button", { name: "Find commits", exact: true }).click();
    const query = page.getByRole("textbox", { name: "Find commits", exact: true });
    await query.fill(summary);
    await expect(rows).toHaveCount(beforeSearch);
    await expect(page.locator(".history .commit-row.match")).toHaveCount(1);
    await expect(selected).toHaveCount(1);
    await expect(selected).toHaveAttribute("data-commit-id", selectedId!);
    await query.fill("");
    await expect(rows.nth(5)).toBeVisible();
    await expect(selected).toHaveCount(1);
    await expect(selected).toHaveAttribute("data-commit-id", selectedId!);
  });

  test(`history multiselect ${theme} shows the count and submits exactly the selected commits`, async ({ page }) => {
    const requests: Record<string, unknown>[] = [];
    await pinLabSettings(page, { locale: "zh", theme });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd, args } = route.request().postDataJSON();
      if (cmd !== "mutate" || args.request?.action !== "cherryPick") return route.fallback();
      requests.push(args.request);
      return route.fulfill({ json: { ok: false, error: "Cherry-pick captured in UI test" } });
    });
    await openScene(page, scenes.find((scene) => scene.id === "history")!, `&locale=zh&theme=${theme}`);
    const rows = page.locator(".history .commit-row");
    const first = await rows.nth(1).getAttribute("data-commit-id");
    const last = await rows.nth(4).getAttribute("data-commit-id");
    await rows.nth(1).click();
    await rows.nth(4).click({ modifiers: ["Control"] });
    await rows.nth(1).click({ button: "right" });
    const menu = page.getByRole("menu");
    await expect(menu.getByRole("menuitem", { name: "已选中 2 个提交", exact: true })).toBeDisabled();
    await expect(page.locator(".history .commit-row.selected")).toHaveCount(2);
    await expect(menu.getByRole("menuitem", { name: "重置", exact: true })).toHaveCount(0);
    await menu.getByRole("menuitem", { name: "拣选选中的 2 个提交", exact: true }).click();
    const confirm = page.getByRole("alertdialog");
    await expect(confirm).toContainText("祖先提交会先于后续提交应用");
    await expect(confirm).toContainText(first!.slice(0, 7));
    await expect(confirm).toContainText(last!.slice(0, 7));
    await confirm.getByRole("button", { name: "取消", exact: true }).click();
    expect(requests).toEqual([]);
    await rows.nth(4).click({ button: "right" });
    await menu.getByRole("menuitem", { name: "拣选选中的 2 个提交并注明来源", exact: true }).click();
    await expect(confirm).toContainText("新提交的说明里会注明来源提交");
    mkdirSync(`.lab/ui-audit/history-multiselect-${theme}`, { recursive: true });
    await page.evaluate(() => Promise.allSettled(document.getAnimations()
      .filter((animation) => animation.effect?.getTiming().iterations !== Infinity).map((animation) => animation.finished)));
    await page.screenshot({ path: `.lab/ui-audit/history-multiselect-${theme}/cherry-selection.png` });
    await confirm.getByRole("button", { name: "确认", exact: true }).click();
    await expect.poll(() => requests.length).toBe(1);
    expect(requests[0]).toEqual({ action: "cherryPick", revs: [first, last], record: true });
  });

  test(`history multiselect ${theme} right-clicking an unselected commit replaces the selection`, async ({ page }) => {
    await pinLabSettings(page, { locale: "en", theme });
    await openScene(page, scenes.find((scene) => scene.id === "history-multiselect")!, `&locale=en&theme=${theme}`);
    const selected = page.locator(".history .commit-row.selected");
    await expect(selected).toHaveCount(3);
    await page.locator(".history .commit-row").nth(4).click({ button: "right" });
    await expect(selected).toHaveCount(1);
    await expect(page.getByRole("menuitem", { name: "Selected commits: 1", exact: true })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Cherry-pick", exact: true })).toBeEnabled();
    await expect(page.getByRole("menuitem", { name: "Cherry-pick 3 selected commits", exact: true })).toHaveCount(0);
    await page.keyboard.press("Escape");
  });
}
