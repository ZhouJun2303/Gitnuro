import { existsSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { expect, test } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"] as const) {
  test(`${theme}: Excel history previews preserve the extension and historical bytes`, async ({ page, context }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await pinLabSettings(page, { theme });
    await openLabScene(page, "file-history-excel", ".preview-revision", { locale: "en" });
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    await expect(page.locator(".history-file")).toContainText("data/sheet.xlsx");
    const repo = process.cwd() + "/.lab/repos/full";
    const original = readFileSync(`${repo}/data/sheet.xlsx`);
    const row = page.locator(".history-item").first();
    const sha = (await row.getAttribute("data-rev"))!;
    await row.locator(".preview-revision").click();
    await expect(page.locator(".copy-result")).toContainText(`sheet.awegit-${sha.slice(0, 12)}.xlsx`);
    const path = (await page.locator(".copy-result").getAttribute("title"))!;
    expect(path).toMatch(/\.xlsx$/);
    expect(readFileSync(path)).toEqual(execFileSync("git", ["-C", repo, "show", `${sha}:data/sheet.xlsx`]));
    expect(readFileSync(`${repo}/data/sheet.xlsx`)).toEqual(original);
    await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(path.split(/[\\/]/).pop());
    await page.screenshot({ path: `.lab/screenshots/file-history-excel-${theme}.png`, fullPage: true });
    await row.getByRole("button", { name: "Cancel preview and delete temporary files" }).click();
    await expect(row.locator(".preview-revision")).toBeVisible();
    expect(existsSync(path)).toBe(false);
    expect(readFileSync(`${repo}/data/sheet.xlsx`)).toEqual(original);
  });
}

test("file history previews each revision, copies its filename, and cancels safely", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await openLabScene(page, "file-history", ".history-commit", { locale: "en" });
  const rows = page.locator(".history-item");
  await expect(rows.nth(1).locator(".preview-revision")).toBeVisible();
  await expect(page.getByRole("checkbox", { name: "Clean previews on close" })).toBeChecked();
  const selected = await page.locator(".history-item.selected .sha").textContent();
  const repo = process.cwd() + "/.lab/repos/full";

  for (const index of [1, 0]) {
    const row = rows.nth(index);
    const sha = (await row.getAttribute("data-rev"))!;
    await row.locator(".preview-revision").click();
    await expect(page.locator(".copy-result")).toContainText(`README.awegit-${sha.slice(0, 12)}`);
    const path = await page.locator(".copy-result").getAttribute("title");
    expect(path).toMatch(/\.md$/);
    expect(readFileSync(path!)).toEqual(execFileSync("git", ["-C", repo, "show", `${sha}:README.md`]));
    await expect(row.locator(".preview-revision")).toHaveCount(0);
    await expect(row.getByRole("button", { name: "Copy preview filename" })).toBeVisible();
    await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(path!.split(/[\\/]/).pop());
    await page.evaluate(() => navigator.clipboard.writeText("different name"));
    await row.getByRole("button", { name: "Copy preview filename" }).click();
    await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(path!.split(/[\\/]/).pop());
    await expect(page.locator(".history-item.selected .sha")).toHaveText(selected!);
    const meta = `${path}.meta`;
    writeFileSync(meta, "Unity-generated metadata");
    await row.getByRole("button", { name: "Cancel preview and delete temporary files" }).click();
    await expect(row.locator(".preview-revision")).toBeVisible();
    expect(existsSync(path!)).toBe(false);
    expect(existsSync(meta)).toBe(false);
  }
});

test("Unity previews copy names without the file extension", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await openLabScene(page, "file-history-unity", ".history-commit", { locale: "en" });
  const prefab = page.locator(".history-item").first();
  await prefab.locator(".preview-revision").click();
  await expect(page.locator(".copy-result")).toContainText("UIChapter.awegit-");
  const prefabPath = (await page.locator(".copy-result").getAttribute("title"))!;
  expect(prefabPath).toMatch(/UIChapter\.awegit-[0-9a-f]{12}\.prefab$/);
  const prefabName = prefabPath.split(/[\\/]/).pop()!.replace(/\.prefab$/, "");
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(prefabName);
  await page.evaluate(() => navigator.clipboard.writeText("different name"));
  await prefab.getByRole("button", { name: "Copy preview filename" }).click();
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(prefabName);
  await prefab.getByRole("button", { name: "Cancel preview and delete temporary files" }).click();
  await expect(prefab.locator(".preview-revision")).toBeVisible();
  expect(existsSync(prefabPath)).toBe(false);

  await page.goto("/?scene=file-history-unity&view=history&file=Assets/Scripts/Player.cs&locale=en");
  await expect(page.locator('[data-lab-scene="file-history-unity"] .history-commit').first()).toBeVisible();
  const script = page.locator(".history-item").first();
  await script.locator(".preview-revision").click();
  await expect(page.locator(".copy-result")).toContainText("Player.awegit-");
  const scriptPath = (await page.locator(".copy-result").getAttribute("title"))!;
  expect(scriptPath).toMatch(/Player\.awegit-[0-9a-f]{12}\.cs\.txt$/);
  const scriptName = scriptPath.split(/[\\/]/).pop()!.replace(/\.cs\.txt$/, "");
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(scriptName);
  await script.getByRole("button", { name: "Copy preview filename" }).click();
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(scriptName);
  await script.getByRole("button", { name: "Cancel preview and delete temporary files" }).click();
  await expect(script.locator(".preview-revision")).toBeVisible();
  expect(existsSync(scriptPath)).toBe(false);
});

for (const clean of [true, false]) {
  test(`file history ${clean ? "cleans" : "retains"} previews on close`, async ({ page }) => {
    await openLabScene(page, "file-history", ".history-commit", { locale: "en" });
    const row = page.locator(".history-item").first();
    await row.locator(".preview-revision").click();
    await expect(row.getByRole("button", { name: "Copy preview filename" })).toBeVisible();
    const path = (await page.locator(".copy-result").getAttribute("title"))!;
    const meta = `${path}.meta`;
    writeFileSync(meta, "Unity-generated metadata");
    if (!clean) await page.getByRole("checkbox", { name: "Clean previews on close" }).uncheck();
    await page.getByRole("button", { name: "Close" }).click();
    await expect(page.locator(".file-history")).toHaveCount(0);
    expect(existsSync(path)).toBe(!clean);
    expect(existsSync(meta)).toBe(!clean);
    if (!clean) { unlinkSync(path); unlinkSync(meta); }
  });
}

test("closing asks whether to keep a modified preview", async ({ page }) => {
  await openLabScene(page, "file-history", ".history-commit", { locale: "en" });
  const rows = page.locator(".history-item");
  await rows.nth(0).locator(".preview-revision").click();
  await expect(rows.nth(0).getByRole("button", { name: "Copy preview filename" })).toBeVisible();
  const modified = (await page.locator(".copy-result").getAttribute("title"))!;
  try {
    writeFileSync(modified, "modified preview");
    await rows.nth(1).locator(".preview-revision").click();
    await expect(rows.nth(1).getByRole("button", { name: "Copy preview filename" })).toBeVisible();
    const other = (await page.locator(".copy-result").getAttribute("title"))!;
    await page.getByRole("button", { name: "Close" }).click();
    const dialog = page.getByRole("dialog", { name: "Preview was modified" });
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText(modified.split(/[\\/]/).pop()!);
    await dialog.getByRole("button", { name: "Keep and close" }).click();
    await expect(page.locator(".file-history")).toHaveCount(0);
    expect(existsSync(modified)).toBe(true);
    expect(existsSync(other)).toBe(false);
  } finally {
    if (existsSync(modified)) unlinkSync(modified);
  }
});

test("force deleting a modified preview removes it on close", async ({ page }) => {
  await openLabScene(page, "file-history", ".history-commit", { locale: "en" });
  const row = page.locator(".history-item").first();
  await row.locator(".preview-revision").click();
  await expect(row.getByRole("button", { name: "Copy preview filename" })).toBeVisible();
  const modified = (await page.locator(".copy-result").getAttribute("title"))!;
  try {
    writeFileSync(modified, "modified preview");
    await page.getByRole("button", { name: "Close" }).click();
    const dialog = page.getByRole("dialog", { name: "Preview was modified" });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "Force delete and close" }).click();
    await expect(page.locator(".file-history")).toHaveCount(0);
    expect(existsSync(modified)).toBe(false);
  } finally {
    if (existsSync(modified)) unlinkSync(modified);
  }
});

test("unsupported file types do not show the copy action", async ({ page }) => {
  await page.route("**/__lab/invoke", async (route) => {
    const body = route.request().postDataJSON() as { cmd?: string };
    if (body.cmd === "revision_copy_support") return route.fulfill({ json: { ok: true, value: null } });
    return route.continue();
  });
  await openLabScene(page, "file-history", ".history-commit");
  await expect(page.locator(".preview-revision")).toHaveCount(0);
});

test("file history shows a support lookup failure instead of silently hiding the action", async ({ page }) => {
  await page.route("**/__lab/invoke", async (route) => {
    const body = route.request().postDataJSON() as { cmd?: string };
    if (body.cmd === "revision_copy_support") return route.fulfill({ json: { ok: false, error: "revision_copy_support not allowed" } });
    return route.continue();
  });
  await openLabScene(page, "file-history", ".history-commit");
  await expect(page.locator(".copy-error")).toHaveText("revision_copy_support not allowed");
});
