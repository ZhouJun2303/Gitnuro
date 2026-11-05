import { expect, test, type Page } from "@playwright/test";

async function openLabScene(page: Page, id: string, ready: string) {
  await page.goto(`/?scene=${id}`, { waitUntil: "domcontentloaded" });
  await page.locator(`[data-lab-scene="${id}"]`).waitFor({ state: "visible", timeout: 90_000 });
  await page.locator(ready).first().waitFor({ state: "visible", timeout: 30_000 });
  await page.waitForTimeout(400);
}

async function manyHunkTotal(page: Page) {
  const text = (await page.locator(".merge-count").innerText()).trim();
  const total = Number(text.split("/")[1]);
  expect(total).toBeGreaterThanOrEqual(10);
  return total;
}

test("merge panel splits one file into many separate hunks", async ({ page }) => {
  await openLabScene(page, "text-merge-many", ".merge-panel");
  const total = await manyHunkTotal(page);
  await expect(page.locator(".merge-count")).toHaveText(`0/${total}`);
  await expect(page.locator(".merge-side.left .merge-cell.hunk")).toHaveCount(total);
  await expect(page.locator(".merge-side.right .merge-cell.hunk")).toHaveCount(total);
  await expect(page.locator(".merge-conflict-mark")).toHaveCount(total);
  expect(await page.locator(".merge-side.left .merge-cell.context").count()).toBeGreaterThan(0);
});

test("merge panel keeps empty sides and blank lines inside a hunk", async ({ page }) => {
  await openLabScene(page, "text-merge-many", ".merge-panel");
  expect(await page.locator(".merge-side.left .merge-cell.hunk pre.empty").count()).toBeGreaterThan(0);
  expect(await page.locator(".merge-side.right .merge-cell.hunk pre.empty").count()).toBeGreaterThan(0);
  const blank = await page.locator(".merge-side.left .merge-cell.hunk pre").evaluateAll((nodes) =>
    nodes.some((node) => (node.textContent ?? "").includes("\n\n")),
  );
  expect(blank).toBe(true);
});

test("merge panel resolves every hunk with the header checkboxes", async ({ page }) => {
  await openLabScene(page, "text-merge-many", ".merge-panel");
  const total = await manyHunkTotal(page);
  const boxes = page.locator(".merge-party input[type=checkbox]");
  const resolve = page.locator(".merge-panel .commit");
  await expect(resolve).toBeDisabled();
  await boxes.first().check();
  await expect(page.locator(".merge-count")).toHaveText(`${total}/${total}`);
  await expect(page.locator(".merge-conflict-mark")).toHaveCount(0);
  await expect(resolve).toBeEnabled();
  const first = page.locator(".merge-result-hunk").first();
  await expect(first).toContainText("topic");
  await expect(first).not.toContainText("main");
  await boxes.nth(1).check();
  await expect(page.locator(".merge-count")).toHaveText(`${total}/${total}`);
  await expect(first).toContainText("topic");
  await expect(first).toContainText("main");
  await boxes.first().uncheck();
  await expect(page.locator(".merge-count")).toHaveText(`${total}/${total}`);
  await expect(page.locator(".merge-result-hunk").first()).toContainText("main");
  await expect(page.locator(".merge-result-hunk").first()).not.toContainText("topic");
});

test("merge panel selects one hunk without touching the rest", async ({ page }) => {
  await openLabScene(page, "text-merge-many", ".merge-panel");
  const total = await manyHunkTotal(page);
  const hunk = page.locator(".merge-side.left .merge-cell.hunk").first();
  await hunk.hover();
  await hunk.locator(".merge-hover-btn").click();
  await expect(page.locator(".merge-count")).toHaveText(`1/${total}`);
  await expect(page.locator(".merge-conflict-mark")).toHaveCount(total - 1);
  await expect(page.locator(".merge-panel .commit")).toBeDisabled();
});

test("merge panel paints picked hunks differently from pending ones", async ({ page }) => {
  await openLabScene(page, "text-merge-many", ".merge-panel");
  const left = page.locator(".merge-side.left .merge-cell.hunk").first();
  const right = page.locator(".merge-side.right .merge-cell.hunk").first();
  const color = (locator: typeof left) => locator.evaluate((node) => getComputedStyle(node).backgroundColor);
  const pending = await color(left);
  await left.hover();
  await left.locator(".merge-hover-btn").click();
  const pickedLeft = await color(left);
  expect(pickedLeft).not.toBe(pending);
  await right.hover();
  await right.locator(".merge-hover-btn").click();
  const pickedRight = await color(right);
  expect(pickedRight).not.toBe(pending);
  expect(pickedRight).not.toBe(pickedLeft);
});

test("merge panel keeps both sides scrolled together", async ({ page }) => {
  await openLabScene(page, "text-merge-many", ".merge-panel");
  const left = page.locator(".merge-side.left");
  const right = page.locator(".merge-side.right");
  await left.evaluate((node) => {
    node.scrollTop = 200;
    node.dispatchEvent(new Event("scroll"));
  });
  await expect.poll(() => right.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
  const [leftTop, rightTop] = await Promise.all([
    left.evaluate((node) => node.scrollTop),
    right.evaluate((node) => node.scrollTop),
  ]);
  expect(Math.abs(leftTop - rightTop)).toBeLessThanOrEqual(1);
});

test("merge panel scrolls the result to the picked hunk", async ({ page }) => {
  await openLabScene(page, "text-merge-many", ".merge-panel");
  const resultPane = page.locator(".merge-result-scroll");
  const target = page.locator(".merge-side.left .merge-cell.hunk").nth(9);
  await target.hover();
  await target.locator(".merge-hover-btn").click();
  const mark = page.locator(".merge-result-scroll [data-merge-result]").nth(9);
  await expect(mark).toHaveClass(/pick-theirs/);
  await expect.poll(() => resultPane.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
  const paneBox = await resultPane.boundingBox();
  const markBox = await mark.boundingBox();
  expect(paneBox).not.toBeNull();
  expect(markBox).not.toBeNull();
  expect(markBox!.y).toBeGreaterThanOrEqual(paneBox!.y - 1);
  expect(markBox!.y + markBox!.height).toBeLessThanOrEqual(paneBox!.y + paneBox!.height + 1);
});
