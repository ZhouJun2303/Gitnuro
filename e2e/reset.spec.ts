import { mkdirSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { scenes } from "../src/lib/lab/scenes";
import { openScene, pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"] as const) {
  test(`reset panel ${theme} opens from history with Soft selected every time`, async ({ page }) => {
    await pinLabSettings(page, { locale: "en", theme });
    await openScene(page, scenes.find((scene) => scene.id === "history")!, `&locale=en&theme=${theme}`);
    const row = page.locator(".history .commit-row").nth(1);
    const summary = await row.locator(".subject").innerText();
    const shortId = await row.locator(".sha").innerText();
    const panel = page.locator('[data-dialog="reset"]');
    const combo = panel.getByRole("combobox", { name: "Reset type" });
    const openReset = async () => {
      await row.click({ button: "right" });
      await page.getByRole("menuitem", { name: "Reset", exact: true }).click();
      await expect(panel).toBeVisible();
      await expect(combo).toContainText("Soft");
      await expect(combo).toBeFocused();
    };
    await openReset();
    await expect.poll(async () => Math.round((await panel.boundingBox())!.width)).toBe(680);
    await expect(panel.locator('[aria-labelledby="reset-branch-label"]')).not.toBeEmpty();
    await expect(panel.locator('[aria-labelledby="reset-target-label"]')).toContainText(shortId.slice(0, 7));
    await expect(panel.locator('[aria-labelledby="reset-target-label"]')).toContainText(summary);
    await expect(combo).toContainText("Keep all changes. Stage differences");

    await combo.press("ArrowDown");
    await expect(panel.getByRole("option")).toHaveCount(3);
    await expect(panel.getByRole("option", { name: /^Soft/ })).toHaveAttribute("aria-selected", "true");
    for (const hint of ["Keep all changes. Stage differences", "Keep all changes. Unstage differences", "Discard all local changes"]) {
      await expect(panel.getByRole("listbox")).toContainText(hint);
    }
    const layout = await panel.getByRole("listbox").evaluate((list) => {
      const options = [...list.querySelectorAll<HTMLElement>('[role="option"]')];
      return {
        colors: options.map((option) => getComputedStyle(option.querySelector(".reset-dot")!).backgroundColor),
        visible: options.every((option) => {
          const box = option.getBoundingClientRect();
          const hit = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
          return box.left >= 0 && box.right <= innerWidth && box.top >= 0 && box.bottom <= innerHeight && !!hit && option.contains(hit);
        }),
      };
    });
    expect(new Set(layout.colors).size).toBe(3);
    expect(layout.visible).toBe(true);
    mkdirSync(`.lab/ui-audit/en-${theme}`, { recursive: true });
    await page.evaluate(() => Promise.allSettled(document.getAnimations()
      .filter((animation) => animation.effect?.getTiming().iterations !== Infinity).map((animation) => animation.finished)));
    await page.screenshot({ path: `.lab/ui-audit/en-${theme}/reset-options.png` });

    await combo.press("ArrowDown");
    await combo.press("Enter");
    await expect(combo).toContainText("Mixed");
    await expect(panel.getByRole("listbox")).toHaveCount(0);
    await expect(page.getByRole("alertdialog")).toHaveCount(0);
    await combo.press("ArrowDown");
    await combo.press("Escape");
    await expect(panel.getByRole("listbox")).toHaveCount(0);
    await expect(panel).toBeVisible();
    await combo.press("h");
    await expect(combo).toContainText("Hard");
    await panel.getByRole("button", { name: "Cancel", exact: true }).click();
    await openReset();
  });

  test(`reset panel ${theme} confirms and sends the selected mode and revision`, async ({ page }) => {
    const requests: Record<string, unknown>[] = [];
    await pinLabSettings(page, { locale: "en", theme });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd, args } = route.request().postDataJSON();
      if (cmd !== "mutate" || args.request?.action !== "reset") return route.continue();
      requests.push(args.request);
      // Verify dispatch without changing the shared lab repository.
      return route.fulfill({ json: { ok: false, error: "Reset request captured in UI test" } });
    });
    await openScene(page, scenes.find((scene) => scene.id === "dialog-reset")!, `&locale=en&theme=${theme}`);
    const panel = page.locator('[data-dialog="reset"]');
    const target = panel.locator('[aria-labelledby="reset-target-label"]');
    const rev = (await target.getAttribute("title"))!.split(" ")[0];
    const summary = await target.locator("span").innerText();
    const combo = panel.getByRole("combobox", { name: "Reset type" });
    for (const [index, mode] of ["soft", "mixed", "hard"].entries()) {
      await combo.click();
      await panel.getByRole("option", { name: new RegExp(`^${mode}`, "i") }).click();
      await panel.getByRole("button", { name: "Reset", exact: true }).click();
      const confirm = page.getByRole("alertdialog");
      await expect(confirm).toBeVisible();
      await expect(confirm.locator("h2")).toHaveText(new RegExp(`^${mode} reset$`, "i"));
      await expect(confirm.locator("[data-confirm-subject]")).toContainText(`${rev.slice(0, 7)} "${summary}"`);
      expect(requests).toHaveLength(index);
      if (mode === "hard") {
        await expect(confirm).toHaveClass(/danger/);
        await expect(confirm.getByRole("button", { name: "Cancel", exact: true })).toBeFocused();
        await confirm.getByRole("button", { name: "Cancel", exact: true }).click();
        await expect(combo).toContainText("Hard");
        expect(requests).toHaveLength(index);
        await panel.getByRole("button", { name: "Reset", exact: true }).click();
      }
      await confirm.getByRole("button", { name: "Confirm", exact: true }).click();
      await expect.poll(() => requests.length).toBe(index + 1);
      expect(requests[index]).toEqual({ action: "reset", rev, mode });
      await expect(panel.locator(".dialog-error")).toContainText("Reset request captured in UI test");
    }
  });
}
