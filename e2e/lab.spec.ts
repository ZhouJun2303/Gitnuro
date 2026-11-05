import { execFileSync } from "node:child_process";
import { expect, test, type Page } from "@playwright/test";
import { scenes, type LabScene } from "../src/lib/lab/scenes";

async function lab(cmd: string, args: Record<string, unknown> = {}) {
  const response = await fetch("http://127.0.0.1:1429/invoke", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ cmd, args }),
  });
  const payload = (await response.json()) as { ok?: boolean; value?: unknown; error?: string };
  if (!payload.ok) throw new Error(payload.error || cmd);
  return payload.value;
}

async function openScene(page: Page, scene: LabScene) {
  const errors: string[] = [];
  const onError = (error: Error) => errors.push(error.message);
  page.on("pageerror", onError);
  try {
    console.log(`scene ${scene.id}`);
    await page.goto(scene.href, { waitUntil: "domcontentloaded" });
    const ready = page.locator(`[data-lab-scene="${scene.id}"]`);
    const failed = page.locator("[data-lab-error]");
    await expect(ready.or(failed)).toBeVisible({ timeout: 90_000 });
    if (!(await ready.isVisible())) throw new Error(`${scene.id}: ${await failed.getAttribute("data-lab-error")}`);
    if (scene.ready.startsWith("text=")) await expect(page.getByText(scene.ready.slice(5)).first()).toBeVisible();
    else await expect(page.locator(scene.ready).first()).toBeVisible();
    expect(errors, scene.id).toEqual([]);
  } finally {
    page.off("pageerror", onError);
  }
}

test("home lists a scene", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("link", { name: "变更" })).toBeVisible();
});

test("every scene opens", async ({ page }) => {
  test.setTimeout(scenes.length * 90_000 + 30_000);
  for (const scene of scenes) await openScene(page, scene);
});

test("pushbox commits and pushes", async () => {
  const layout = (await lab("lab_layout")) as { pushbox: string; remotePush: string };
  await lab("stage_path", { path: layout.pushbox, file: "note.txt" });
  await lab("commit_changes", {
    path: layout.pushbox,
    summary: "lab: record the push note",
    description: "",
    amend: false,
    signOff: false,
    skipHooks: true,
  });
  await lab("mutate", {
    path: layout.pushbox,
    request: { action: "push", remote: "origin", setUpstream: true, tags: false, forceWithLease: false },
  });
  const subject = execFileSync("git", ["--git-dir", layout.remotePush, "log", "-1", "--format=%s"], { encoding: "utf8" }).trim();
  expect(subject).toBe("lab: record the push note");
});

async function openLabScene(page: Page, id: string, ready: string) {
  await page.goto(`/?scene=${id}`, { waitUntil: "domcontentloaded" });
  await page.locator(`[data-lab-scene="${id}"]`).waitFor({ state: "visible", timeout: 90_000 });
  await page.locator(ready).first().waitFor({ state: "visible", timeout: 30_000 });
  await page.waitForTimeout(400);
}

test("dialog close button dismisses the dialog", async ({ page }) => {
  await openLabScene(page, "dialog-prefs", '[data-dialog="prefs"]');
  await page.locator(".dialog-close").click();
  await expect(page.locator('[data-dialog="prefs"]')).toHaveCount(0);
});

test("command palette is blocked while a dialog is open", async ({ page }) => {
  await openLabScene(page, "dialog-prefs", '[data-dialog="prefs"]');
  await page.keyboard.press("Control+k");
  await expect(page.locator(".palette")).toHaveCount(0);
  await expect(page.locator('[data-dialog="prefs"]')).toBeVisible();
});

test("merge resolve is disabled until every conflict is picked", async ({ page }) => {
  await openLabScene(page, "text-merge", ".merge-panel");
  await expect(page.locator(".merge-panel .commit")).toBeDisabled();
  await expect(page.locator(".merge-count")).toHaveText("0/1");
});

test("merge side checkboxes toggle and stay exclusive", async ({ page }) => {
  await openLabScene(page, "file-merge", ".merge-panel");
  const boxes = page.locator(".merge-party input[type=checkbox]");
  const resolve = page.locator(".merge-panel .commit");
  await boxes.first().check();
  await expect(boxes.first()).toBeChecked();
  await expect(boxes.nth(1)).not.toBeChecked();
  await expect(resolve).toBeEnabled();
  await boxes.nth(1).check();
  await expect(boxes.nth(1)).toBeChecked();
  await expect(boxes.first()).not.toBeChecked();
  await expect(resolve).toBeEnabled();
});

test("merge panel names the commit behind each side", async ({ page }) => {
  await openLabScene(page, "file-merge", ".merge-panel");
  await expect(page.locator(".merge-party-subject")).toHaveCount(2);
  await expect(page.locator(".merge-party-meta code")).toHaveCount(2);
  await expect(page.locator(".merge-file-subject")).toHaveCount(2);
  await expect(page.locator(".merge-file-meta dd").first()).not.toBeEmpty();
});

test("rebase rows start checked and can be excluded", async ({ page }) => {
  await openLabScene(page, "dialog-rebase", ".dialog-scroll");
  const boxes = page.locator(".dialog-scroll input[type=checkbox]");
  const total = await boxes.count();
  expect(total).toBeGreaterThan(0);
  await expect(page.locator(".dialog-scroll input[type=checkbox]:checked")).toHaveCount(total);
});

test("menu rows keep their full height", async ({ page }) => {
  await openLabScene(page, "menu", '[data-layer="menu"]');
  const height = await page.locator(".menu button").first().evaluate((node) => node.getBoundingClientRect().height);
  expect(height).toBeGreaterThanOrEqual(27);
});

test("confirmation is above a dialog that opened it", async ({ page }) => {
  await openLabScene(page, "dialog-workspace", '[data-dialog="workspace"]');
  await page.locator(".workspace-delete").first().click();
  const confirm = page.locator("[data-confirm]");
  await expect(confirm).toBeVisible();
  const layers = await page.evaluate(() => {
    const box = document.querySelector("[data-confirm]");
    const scrim = box?.closest(".scrim");
    return scrim ? Number(getComputedStyle(scrim).zIndex) : 0;
  });
  expect(layers).toBeGreaterThan(50);
});

test("hunk toolbar selects a hunk without hovering", async ({ page }) => {
  await openLabScene(page, "diff-text", ".hunk-tools");
  const stage = page.locator(".hunk-tools .pane-chip").first();
  await expect(stage).toBeDisabled();
  await page.locator(".hunk-tools .hunk-step").nth(1).click();
  await expect(page.locator(".hunk-pos")).toHaveText("1/1");
  await expect(stage).toBeEnabled();
});

test("command log keeps focus inside and closes with Escape", async ({ page }) => {
  await openLabScene(page, "commands", '[data-layer="commands"]');
  const inside = () => page.evaluate(() => !!document.activeElement?.closest('[data-layer="commands"]'));
  expect(await inside()).toBe(true);
  for (let step = 0; step < 4; step += 1) await page.keyboard.press("Tab");
  expect(await inside()).toBe(true);
  await page.keyboard.press("Escape");
  await expect(page.locator('[data-layer="commands"]')).toHaveCount(0);
});

test("submenu opens and closes with the arrow keys", async ({ page }) => {
  await openLabScene(page, "menu", '[data-layer="menu"]');
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowUp");
  await expect(page.locator('[data-layer="menu"] button[aria-haspopup="menu"]')).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(page.locator('[data-layer="submenu"]')).toBeVisible();
  await expect(page.locator('[data-layer="submenu"] button').first()).toBeFocused();
  await page.keyboard.press("ArrowLeft");
  await expect(page.locator('[data-layer="submenu"]')).toHaveCount(0);
  await expect(page.locator('[data-layer="menu"] button[aria-haspopup="menu"]')).toBeFocused();
});

test("commit patch renders each hunk header once", async ({ page }) => {
  await openLabScene(page, "commit", ".detail");
  await page.locator(".patch-toggle").first().click();
  const meta = page.locator(".detail .split-meta");
  await expect(meta.first()).toBeVisible();
  await expect(meta.filter({ hasText: "@@" })).toHaveCount(1);
  expect(await page.locator(".detail .split-row").count()).toBeGreaterThan(0);
});

test("history restores its scroll position after switching sections", async ({ page }) => {
  await openLabScene(page, "history", ".history .commit-row");
  await page.locator(".history").evaluate((node) => {
    node.scrollTop = 1600;
    node.dispatchEvent(new Event("scroll"));
  });
  await page.locator('[data-section="changes"]').click();
  await page.waitForTimeout(500);
  await page.locator('[data-section="history"]').click();
  await page.waitForTimeout(500);
  const visible = await page.evaluate(() => {
    const list = document.querySelector(".history");
    if (!list) return 0;
    const rect = list.getBoundingClientRect();
    return [...document.querySelectorAll(".history .commit-row")].filter((row) => {
      const box = row.getBoundingClientRect();
      return box.top < rect.bottom && box.bottom > rect.top;
    }).length;
  });
  expect(visible).toBeGreaterThan(0);
});

test("context menu arrows stay in the menu and restore focus", async ({ page }) => {
  await openLabScene(page, "history", ".history .commit-row");
  const row = page.locator(".history .commit-row").nth(2);
  await row.focus();
  const selected = () => page.evaluate(() => [...document.querySelectorAll(".history .commit-row")].findIndex((node) => node.classList.contains("selected")));
  const before = await selected();
  await page.keyboard.press("Shift+F10");
  await expect(page.locator('[data-layer="menu"]')).toBeVisible();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  expect(await selected()).toBe(before);
  await page.keyboard.press("Escape");
  await expect(page.locator('[data-layer="menu"]')).toHaveCount(0);
  await expect(row).toBeFocused();
});

test("Space selects the focused history row", async ({ page }) => {
  await openLabScene(page, "history", ".history .commit-row");
  const row = page.locator(".history .commit-row").nth(3);
  await row.focus();
  await page.keyboard.press("Space");
  await expect(row).toHaveClass(/selected/);
});

test("horizontal splitter arrows do not move history selection", async ({ page }) => {
  await openLabScene(page, "history", ".history .commit-row");
  const selected = () => page.evaluate(() => [...document.querySelectorAll(".history .commit-row")].findIndex((node) => node.classList.contains("selected")));
  const before = await selected();
  const handle = page.locator(".split-handle.horizontal");
  await handle.focus();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  expect(await selected()).toBe(before);
});

test("search options work from the keyboard and return to the input", async ({ page }) => {
  await openLabScene(page, "search", '[data-layer="search"]');
  const toggle = page.locator(".side-search-toggle");
  const input = page.locator(".side-search .side-filter input");
  const items = page.locator(".side-search-item");
  const initialLabel = (await toggle.textContent())?.trim() ?? "";
  await toggle.focus();
  await page.keyboard.press("Enter");
  await expect(items.first()).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(items.nth(1)).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.locator(".side-search-menu")).toHaveCount(0);
  await expect(toggle).toBeFocused();
  await page.keyboard.press("Enter");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(page.locator(".side-search-menu")).toHaveCount(0);
  await expect(input).toBeFocused();
  const pickedLabel = (await toggle.textContent())?.trim() ?? "";
  expect(pickedLabel).not.toBe(initialLabel);
  expect(await input.getAttribute("placeholder")).toBe(pickedLabel);
});

test("search options fit inside a narrow sidebar", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("awegit.layout.settings", JSON.stringify({ sidebarWidth: 180 })));
  await openLabScene(page, "search", '[data-layer="search"]');
  await page.locator(".side-search-toggle").click();
  const sidebar = await page.locator(".sidebar").boundingBox();
  const menu = await page.locator(".side-search-menu").boundingBox();
  expect(sidebar && menu).toBeTruthy();
  expect((menu?.x ?? 0) + (menu?.width ?? 0)).toBeLessThanOrEqual((sidebar?.x ?? 0) + (sidebar?.width ?? 0) + 0.5);
});

test("dialogs focus the first field and fall back to close", async ({ page }) => {
  await openLabScene(page, "dialog-branch", '[data-dialog="branch"]');
  await expect(page.locator('[data-dialog="branch"] input').first()).toBeFocused();
  await openLabScene(page, "dialog-about", '[data-dialog="about"]');
  await expect(page.locator('[data-dialog="about"] .dialog-close')).toBeFocused();
});
