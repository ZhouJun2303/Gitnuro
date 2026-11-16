import { mkdirSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { scenes } from "../src/lib/lab/scenes";
import { openScene, pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"] as const) {
  test(`revert panel ${theme} previews and submits the edited commit message`, async ({ page }) => {
    const requests: Record<string, unknown>[] = [];
    await pinLabSettings(page, { locale: "en", theme });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd, args } = route.request().postDataJSON();
      if (cmd !== "mutate" || args.request?.action !== "revert") return route.continue();
      requests.push(args.request);
      return route.fulfill({ json: { ok: false, error: "Revert request captured in UI test" } });
    });
    await openScene(page, scenes.find((scene) => scene.id === "revert-confirm")!, `&locale=en&theme=${theme}`);
    const panel = page.getByRole("alertdialog");
    const checkbox = panel.getByRole("checkbox", { name: "Commit the changes" });
    const message = panel.getByRole("textbox", { name: "Commit message" });
    const submit = panel.getByRole("button", { name: "Revert", exact: true });
    const subject = (await panel.locator("[data-confirm-subject]").innerText()).trim();
    const original = await message.inputValue();
    const revision = original.match(/This reverts commit ([0-9a-f]{40,64})\./)![1];
    expect(original).toBe(`revert: ${subject.slice(8)}\n\nThis reverts commit ${revision}.`);
    expect(revision.startsWith(subject.slice(0, 7))).toBe(true);
    await expect(checkbox).toBeChecked();
    await expect(message).toBeEnabled();
    await expect(submit).toBeEnabled();
    await message.focus();
    await message.press("End");
    await message.press("Enter");
    await expect(panel).toBeVisible();
    expect(requests).toEqual([]);
    await message.fill("   \n");
    await expect(submit).toBeDisabled();
    await checkbox.uncheck();
    await expect(submit).toBeEnabled();
    await expect(message).toBeDisabled();
    await checkbox.check();
    await expect(submit).toBeDisabled();
    const edited = `revert: restore the previous behavior\n\nReason for this revert.\n\nThis reverts commit ${revision}.`;
    await message.fill(edited);
    await checkbox.uncheck();
    await checkbox.check();
    await expect(message).toHaveValue(edited);
    mkdirSync(`.lab/ui-audit/en-${theme}`, { recursive: true });
    await page.screenshot({ path: `.lab/ui-audit/en-${theme}/revert-message.png` });
    await submit.click();
    await expect(panel).toHaveCount(0);
    await expect.poll(() => requests.length).toBe(1);
    expect(requests[0]).toEqual({ action: "revert", rev: revision, noCommit: false, message: edited });
  });

  test(`revert panel ${theme} can leave changes staged without committing`, async ({ page }) => {
    const requests: Record<string, unknown>[] = [];
    await pinLabSettings(page, { locale: "zh", theme });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd, args } = route.request().postDataJSON();
      if (cmd !== "mutate" || args.request?.action !== "revert") return route.continue();
      requests.push(args.request);
      return route.fulfill({ json: { ok: false, error: "Revert request captured in UI test" } });
    });
    await openScene(page, scenes.find((scene) => scene.id === "revert-no-commit")!, `&locale=zh&theme=${theme}`);
    const panel = page.getByRole("alertdialog");
    await expect(panel.getByRole("checkbox", { name: "提交更改" })).not.toBeChecked();
    const message = panel.getByRole("textbox", { name: "提交说明" });
    await expect(message).toBeDisabled();
    await expect(message).toHaveValue(/^revert: /);
    await expect(panel).toContainText("解决冲突后继续也不会自动提交");
    const original = await message.inputValue();
    for (const viewport of [{ width: 1440, height: 900 }, { width: 480, height: 560 }]) {
      await page.setViewportSize(viewport);
      await expect(message).toBeInViewport();
      await expect(panel.getByRole("button", { name: "还原", exact: true })).toBeInViewport();
      const box = (await panel.boundingBox())!;
      expect(box.x).toBeGreaterThanOrEqual(0);
      expect(box.x + box.width).toBeLessThanOrEqual(viewport.width);
      expect(box.y).toBeGreaterThanOrEqual(0);
      expect(box.y + box.height).toBeLessThanOrEqual(viewport.height);
    }
    await panel.getByRole("button", { name: "还原", exact: true }).click();
    await expect.poll(() => requests.length).toBe(1);
    expect(requests[0]).toMatchObject({ action: "revert", noCommit: true, message: original });
  });
}

test("revert cancellation never writes the repository", async ({ page }) => {
  const requests: Record<string, unknown>[] = [];
  page.on("request", (request) => {
    if (!request.url().endsWith("/__lab/invoke")) return;
    const { cmd, args } = request.postDataJSON();
    if (cmd === "mutate") requests.push(args.request);
  });
  await pinLabSettings(page, { locale: "en", theme: "apple" });
  for (const id of ["revert-confirm", "revert-no-commit"]) {
    await openScene(page, scenes.find((scene) => scene.id === id)!, "&locale=en&theme=apple");
    await page.getByRole("alertdialog").getByRole("button", { name: "Cancel" }).click();
    await expect(page.getByRole("alertdialog")).toHaveCount(0);
    await openScene(page, scenes.find((scene) => scene.id === id)!, "&locale=en&theme=apple");
    await page.keyboard.press("Escape");
    await expect(page.getByRole("alertdialog")).toHaveCount(0);
  }
  expect(requests).toEqual([]);
});
