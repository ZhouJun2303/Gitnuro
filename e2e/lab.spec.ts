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
  const href = scenes.find((scene) => scene.id === id)?.href ?? `/?scene=${id}`;
  await page.goto(href, { waitUntil: "domcontentloaded" });
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

test("branch rows mark local-only and mirrored branches", async ({ page }) => {
  await openLabScene(page, "branch-badges", ".sidebar .track-dot.missing");
  const missing = page.locator(".sidebar .track-dot.missing").first();
  const mirrored = page.locator(".sidebar .track-dot.mirrored").first();
  await expect(mirrored).toBeVisible();
  expect(await missing.evaluate((node) => getComputedStyle(node).backgroundColor)).toBe("rgba(0, 0, 0, 0)");
  expect(await mirrored.evaluate((node) => getComputedStyle(node).borderTopWidth)).toBe("0px");
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

test("floating hunk actions stay inside the pane while scrolling", async ({ page }) => {
  await openLabScene(page, "diff-long", ".diff-body .split-code, .diff-body .virtual-row");
  const body = page.locator("section.diff .diff-body");
  await body.locator(".split-code, .virtual-row").nth(6).hover();
  const actions = page.locator(".diff-hunk-actions");
  await expect(actions).toBeVisible();
  const pane = await body.boundingBox();
  await body.evaluate((node) => {
    node.scrollTop = 1600;
    node.dispatchEvent(new Event("scroll"));
  });
  await page.waitForTimeout(300);
  await expect(actions).toBeVisible();
  const box = await actions.boundingBox();
  expect(pane).not.toBeNull();
  expect(box).not.toBeNull();
  expect(box!.y).toBeGreaterThanOrEqual(pane!.y - 1);
  expect(box!.y + box!.height).toBeLessThanOrEqual(pane!.y + pane!.height + 1);
});

test("dragging lines selects rows beneath the floating hunk actions", async ({ page }) => {
  await openLabScene(page, "diff-long", ".diff-body .split-code");
  const body = page.locator("section.diff .diff-body");
  await body.locator(".split-code").nth(20).hover();
  const actions = page.locator(".diff-hunk-actions");
  await expect(actions).toBeVisible();
  const bar = await actions.boundingBox();
  const start = await body.locator(".split-no").nth(40).boundingBox();
  expect(bar).not.toBeNull();
  expect(start).not.toBeNull();
  const firstCode = await page.evaluate(() =>
    Math.min(
      ...[...document.querySelectorAll<HTMLElement>("section.diff .split-code")]
        .map((node) => Number(node.dataset.line))
        .filter(Number.isFinite),
    ),
  );
  await page.mouse.move(start!.x + start!.width / 2, start!.y + start!.height / 2);
  await page.mouse.down();
  await page.mouse.move(bar!.x + bar!.width / 2, bar!.y + bar!.height / 2, { steps: 8 });
  const picked = await page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>("section.diff .split-code.picked")].map((node) => Number(node.dataset.line)),
  );
  await page.mouse.up();
  expect(picked.length).toBeGreaterThan(0);
  expect(Math.min(...picked)).toBe(firstCode);
});

test("wrapped hunk actions anchor to the hunk under the pointer", async ({ page }) => {
  const scenes: Array<[string, string]> = [
    ["diff-wrap", ".split-meta"],
    ["diff-wrap-unified", ".virtual-row.hunk"],
  ];
  for (const [scene, selector] of scenes) {
    await openLabScene(page, scene, selector);
    const body = page.locator("section.diff .diff-body");
    const header = body.locator(selector, { hasText: "@@" }).nth(1);
    await header.scrollIntoViewIfNeeded();
    await header.hover();
    const actions = page.locator(".diff-hunk-actions");
    await expect(actions).toBeVisible();
    const bar = await actions.boundingBox();
    const at = await header.boundingBox();
    expect(bar, scene).not.toBeNull();
    expect(at, scene).not.toBeNull();
    expect(bar!.y, scene).toBeGreaterThanOrEqual(at!.y - 4);
    expect(bar!.y, scene).toBeLessThanOrEqual(at!.y + at!.height + 24);
  }
});

test("Escape closes a dialog and a text merge panel", async ({ page }) => {
  await openLabScene(page, "dialog-prefs", '[data-dialog="prefs"]');
  await page.keyboard.press("Escape");
  await expect(page.locator('[data-dialog="prefs"]')).toHaveCount(0);
  await openLabScene(page, "text-merge", '[data-layer="text-merge"]');
  await page.keyboard.press("Escape");
  await expect(page.locator('[data-layer="text-merge"]')).toHaveCount(0);
});

test("open dialog keeps every source row readable", async ({ page }) => {
  await openLabScene(page, "dialog-open", '[data-dialog="open"] .open-col');
  const rows = page.locator('[data-dialog="open"] .open-col .text-button');
  expect(await rows.count()).toBeGreaterThan(10);
  const height = await rows.first().evaluate((node) => node.getBoundingClientRect().height);
  expect(height).toBeGreaterThanOrEqual(28);
});

test("long dialogs keep their title and first field in view", async ({ page }) => {
  await openLabScene(page, "dialog-accounts", '[data-dialog="accounts"]');
  await expect(page.locator('[data-dialog="accounts"] h2')).toBeInViewport();
  const scrolled = await page.locator('[data-dialog="accounts"] form').evaluate((node) => node.scrollTop);
  expect(scrolled).toBe(0);
  await openLabScene(page, "prefs-git", '[data-dialog="prefs"]');
  const prefScroll = await page.locator('[data-dialog="prefs"] .pref-page').evaluate((node) => node.scrollTop);
  expect(prefScroll).toBe(0);
  await openLabScene(page, "dialog-gitignore", '[data-dialog="gitignore"]');
  const area = await page.locator('[data-dialog="gitignore"] textarea').evaluate((node) => {
    const field = node as HTMLTextAreaElement;
    return { top: field.scrollTop, start: field.selectionStart };
  });
  expect(area.top).toBe(0);
  expect(area.start).toBe(0);
});

test("push and pull dialogs start with a branch", async ({ page }) => {
  await openLabScene(page, "dialog-push", '[data-dialog="push"]');
  const push = await page.locator('[data-dialog="push"] select').first().inputValue();
  expect(push).not.toBe("");
  await openLabScene(page, "dialog-pull", '[data-dialog="pull"]');
  const pull = await page.locator('[data-dialog="pull"] select').nth(1).inputValue();
  expect(pull).not.toBe("");
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

test("command log opens on the newest row even when a stale id matches an old error", async ({ page }) => {
  await openLabScene(page, "commands-errors", '[data-layer="commands"]');
  const state = await page.evaluate(() => {
    const box = document.querySelector(".command-items");
    const items = [...document.querySelectorAll(".command-item")];
    const current = document.querySelector(".command-item.current");
    return { scrollTop: box?.scrollTop ?? -1, index: current ? items.indexOf(current) : -1 };
  });
  expect(state.scrollTop).toBe(0);
  expect(state.index).toBe(0);
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

test("submenu stays open while the pointer moves through its items", async ({ page }) => {
  await openLabScene(page, "menu", '[data-layer="menu"]');
  await page.locator('[data-layer="menu"] button[aria-haspopup="menu"]').hover();
  await expect(page.locator('[data-layer="submenu"]')).toBeVisible();
  await page.locator('[data-layer="submenu"] button').nth(3).hover();
  await expect(page.locator('[data-layer="submenu"]')).toBeVisible();
  await expect(page.locator('[data-layer="submenu"] button')).toHaveCount(24);
});

test("a write blocked by local changes offers stash and retry", async ({ page }) => {
  await openLabScene(page, "action-error-retry", '.identity-floor[data-layer="error"]');
  await expect(page.locator('.identity-floor[data-layer="error"]')).toContainText("Stash and retry");
});

test("applying a stash asks before it touches the work tree", async ({ page }) => {
  await openLabScene(page, "stash-apply-confirm", '[data-layer="confirm"]');
  const confirm = page.locator('[data-layer="confirm"]');
  await expect(confirm).toContainText("working tree");
  await expect(confirm.locator(".commit")).toBeEnabled();
});

test("reverting a commit asks before it runs", async ({ page }) => {
  await openLabScene(page, "revert-confirm", '[data-layer="confirm"]');
  const confirm = page.locator('[data-layer="confirm"]');
  await expect(confirm).toContainText("Revert commit");
  await expect(confirm).toContainText("new revert commit");
  await expect(confirm.locator(".commit")).toBeEnabled();
});

test("popping a stash asks before it runs", async ({ page }) => {
  await openLabScene(page, "stash-pop-confirm", '[data-layer="confirm"]');
  const confirm = page.locator('[data-layer="confirm"]');
  await expect(confirm).toContainText("Pop stash");
  await expect(confirm).toContainText("remove it from the stash list");
  await expect(confirm.locator(".commit")).toBeEnabled();
});

test("resetting a branch asks before it moves", async ({ page }) => {
  await openLabScene(page, "reset-confirm", '[data-layer="confirm"]');
  const confirm = page.locator('[data-layer="confirm"]');
  await expect(confirm).toContainText("Mixed reset");
  await expect(confirm).toContainText("unstaged");
  await expect(confirm.locator(".commit")).toBeEnabled();
});

test("cherry-picking asks before it runs", async ({ page }) => {
  await openLabScene(page, "cherry-confirm", '[data-layer="confirm"]');
  const confirm = page.locator('[data-layer="confirm"]');
  await expect(confirm).toContainText("Cherry-pick (note source commit)");
  await expect(confirm).toContainText("records the source commit");
  await expect(confirm.locator(".commit")).toBeEnabled();
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

test("returning to changes reloads a diff cleared by a history refresh", async ({ page }) => {
  await openLabScene(page, "changes", "section.diff .diff-body");
  await page.locator('[data-section="history"]').click();
  await page.waitForTimeout(400);
  await page.locator(".menubar button", { hasText: "View" }).click();
  await page.locator(".menu button", { hasText: "Refresh" }).click();
  await page.waitForTimeout(1200);
  await page.locator('[data-section="changes"]').click();
  await expect(page.locator("section.diff .diff-body")).toBeVisible({ timeout: 20_000 });
  await expect(page.locator("section.diff .diff-empty")).toHaveCount(0);
});

test("switching repositories clears the previous commit detail", async ({ page }) => {
  const layout = (await lab("lab_layout")) as { even: string };
  await openLabScene(page, "history", ".commit-row");
  await page.locator(".commit-row").nth(1).click();
  await expect(page.locator(".detail").getByText("AweGit Lab").first()).toBeVisible();
  await page.locator(".tab-add").click();
  await page.locator(".repo-row", { hasText: "even" }).first().click();
  await expect(page.locator(".chrome-side .tab.active")).toContainText("even", { timeout: 30_000 });
  await expect(page.locator(".detail .empty")).toContainText(/Select a file|选择/, { timeout: 20_000 });
  expect(layout.even).not.toBe("");
});

test("interactive rebase from HEAD explains there is nothing newer", async ({ page }) => {
  await openLabScene(page, "history", ".commit-row");
  await page.locator(".commit-row").first().click({ button: "right" });
  await page.locator('[data-layer="menu"] button', { hasText: /Interactive rebase/ }).click();
  await expect(page.locator('[data-dialog="rebase"]')).toHaveCount(0);
  await expect(page.locator('.identity-floor[data-layer="error"]')).toContainText(/no newer commits/i);
});

test("changing one rebase verb leaves the other rows alone", async ({ page }) => {
  await openLabScene(page, "dialog-rebase", ".dialog-scroll");
  const selects = page.locator(".dialog-scroll select");
  expect(await selects.count()).toBeGreaterThan(1);
  await selects.first().selectOption("drop");
  await expect(selects.first()).toHaveValue("drop");
  await expect(selects.nth(1)).toHaveValue("pick");
});

test("shortcuts do not open layers behind a menu or the forge dialog", async ({ page }) => {
  await openLabScene(page, "history", ".commit-row");
  await page.locator(".menubar button", { hasText: "View" }).click();
  await page.keyboard.press("Control+,");
  await expect(page.locator('[data-dialog="prefs"]')).toHaveCount(0);
  await expect(page.locator('[data-layer="menu"]')).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.locator('[data-layer="menu"]')).toHaveCount(0);

  await openLabScene(page, "notes", '[data-layer="forge"]');
  await page.keyboard.press("Control+k");
  await expect(page.locator(".palette")).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(page.locator('[data-layer="forge"]')).toHaveCount(0);
});

test("merge stays available with local changes and stashes them", async ({ page }) => {
  await openLabScene(page, "dialog-merge", '[data-dialog="merge"]');
  const dialog = page.locator('[data-dialog="merge"]');
  await expect(dialog.locator(".commit")).toBeEnabled();
  await expect(dialog.locator(".dialog-error")).toHaveCount(0);
  await expect(dialog).toContainText("Local changes are stashed first.");
});

test("batch conflict actions keep one side for the selected files", async ({ page }) => {
  await openLabScene(page, "conflict-batch", ".file-list .file");
  await expect(page.locator(".composer .summary-input")).toHaveValue(/topic/);
  await expect(page.locator(".composer .summary-input")).toHaveValue(/main/);
  await expect(page.locator(".composer textarea")).not.toHaveValue(/conflict\/file-/);
  await expect(page.locator('.identity-floor[data-layer="conflicts"]')).toContainText("topic");
  await expect(page.locator('.identity-floor[data-layer="conflicts"]')).toContainText("main");
  const rows = page.locator(".unstaged-pane .file-list .file");
  const paths = await rows.evaluateAll((nodes) =>
    nodes.slice(0, 2).map((node) => node.querySelector<HTMLElement>(".file-select")?.title ?? ""),
  );
  expect(paths).toHaveLength(2);
  expect(paths.every((path) => path !== "")).toBe(true);
  await rows.nth(0).click();
  await rows.nth(1).click({ modifiers: ["Control"] });
  await expect(page.locator(".pick-bar")).toBeVisible();
  await expect(page.locator(".pick-count")).toHaveText(/2/);
  const pane = page.locator(".conflict-pane");
  await expect(pane.locator(".conflict-selection")).toContainText("2");
  await pane.locator("#conflict-theirs").click();
  await expect(pane.locator(".conflict-merge")).toContainText(/Keep main|保留 main/);
  await pane.locator("#conflict-theirs").click();
  await rows.nth(1).click({ button: "right" });
  const menu = page.locator('[data-layer="menu"]');
  await expect(menu).toBeVisible();
  const ours = menu.locator("button").filter({ hasText: /Keep main|保留 main/ });
  await expect(ours).toContainText("2");
  await expect(menu.locator("button").filter({ hasText: /Keep topic|保留 topic/ })).toBeVisible();
  await ours.click();
  for (const path of paths) {
    await expect(page.locator(`.unstaged-pane .file-list .file-select[title="${path}"]`)).toHaveCount(0);
  }
});

test("merge panel opens as a standalone page", async ({ page }) => {
  await openLabScene(page, "text-merge-window", ".merge-panel.standalone");
  const panel = page.locator(".merge-panel.standalone");
  const box = await panel.boundingBox();
  const viewport = page.viewportSize();
  expect(box).not.toBeNull();
  expect(viewport).not.toBeNull();
  expect(Math.round(box!.width)).toBe(viewport!.width);
  expect(Math.round(box!.height)).toBe(viewport!.height);
  await expect(page.locator(".merge-scrim")).toHaveCount(0);
});

test("unified diff keeps row nodes while scrolling", async ({ page }) => {
  await openLabScene(page, "prefs-git", '[data-dialog="prefs"]');
  const style = page.locator('[data-dialog="prefs"] label', { hasText: "Diff view" }).locator("select");
  await style.selectOption("unified");
  await page.locator('[data-dialog="prefs"] .commit').click();
  await page.waitForTimeout(500);
  await openLabScene(page, "diff-long", ".diff-body");
  await expect(page.locator("section.diff .diff-body")).not.toHaveClass(/split/);
  expect(await page.locator("section.diff .diff-body .virtual-row").count()).toBeGreaterThan(20);
  const result = await page.evaluate(async () => {
    const body = document.querySelector<HTMLElement>("section.diff .diff-body");
    const windowBox = body?.querySelector<HTMLElement>(".commit-window");
    if (!body || !windowBox) return null;
    let added = 0;
    const observer = new MutationObserver((records) => {
      for (const record of records) added += record.addedNodes.length;
    });
    observer.observe(windowBox, { childList: true });
    for (let step = 0; step < 3; step += 1) {
      body.scrollTop = 500 + step * 160;
      body.dispatchEvent(new Event("scroll"));
      await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
      await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
    }
    observer.disconnect();
    return added;
  });
  expect(result).not.toBeNull();
  expect(result ?? 0).toBeLessThan(80);
});
