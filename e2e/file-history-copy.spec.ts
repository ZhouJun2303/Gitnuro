import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { expect, test } from "@playwright/test";
import { openLabScene } from "./helpers";

test("file history creates separate copies for each revision without changing the selection", async ({ page }) => {
  await openLabScene(page, "file-history", ".history-commit");
  const rows = page.locator(".history-item");
  await expect(rows.nth(1).locator(".copy-revision")).toBeVisible();
  const selected = await page.locator(".history-item.selected .sha").textContent();
  const repo = process.cwd() + "/.lab/repos/full";

  for (const index of [1, 0]) {
    const row = rows.nth(index);
    const sha = (await row.getAttribute("data-rev"))!;
    await row.locator(".copy-revision").click();
    await expect(page.locator(".copy-result")).toContainText(`README.awegit-${sha.slice(0, 12)}`);
    const path = await page.locator(".copy-result").getAttribute("title");
    expect(path).toMatch(/\.md$/);
    expect(readFileSync(path!)).toEqual(execFileSync("git", ["-C", repo, "show", `${sha}:README.md`]));
    await expect(page.locator(".history-item.selected .sha")).toHaveText(selected!);
  }
});

test("unsupported file types do not show the copy action", async ({ page }) => {
  await page.route("**/__lab/invoke", async (route) => {
    const body = route.request().postDataJSON() as { cmd?: string };
    if (body.cmd === "revision_copy_support") return route.fulfill({ json: { ok: true, value: null } });
    return route.continue();
  });
  await openLabScene(page, "file-history", ".history-commit");
  await expect(page.locator(".copy-revision")).toHaveCount(0);
});
