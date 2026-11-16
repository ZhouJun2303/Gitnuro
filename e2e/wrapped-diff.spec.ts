import { expect, test, type Locator } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

async function checkRows(list: Locator) {
  await expect.poll(() => list.locator(".wrapped-row").count()).toBeGreaterThan(1);
  expect(await list.locator(".wrapped-row").count()).toBeLessThan(160);
  await expect.poll(() => list.locator(".wrapped-row").evaluateAll((nodes) => nodes.flatMap((node, index) => {
    const box = node.getBoundingClientRect();
    const previous = nodes[index - 1]?.getBoundingClientRect();
    const line = node.querySelector(".virtual-row")?.getBoundingClientRect();
    return box.height < 18 || (previous && previous.bottom > box.top + 1) || (line && line.bottom > box.bottom + 1)
      ? [{ index, height: box.height }] : [];
  }))).toEqual([]);
}

for (const theme of ["apple", "light", "dark"]) for (const style of ["split", "unified"] as const) {
  for (const surface of ["changes", "commit", "file-history"] as const) {
    test(`${theme} wrapped ${style} ${surface} keeps rows separate and reaches the tail after resizing`, async ({ page }) => {
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));
      await page.setViewportSize({ width: 960, height: 640 });
      await pinLabSettings(page, { theme, diffStyle: style, diffWordWrap: true, statusInterval: 0, fetchAutomatically: false });
      const lines = [{ kind: "hunk", text: "@@ -1,1200 +1,1200 @@" }, ...Array.from({ length: 1200 }, (_, index) => ({
        kind: "context", text: ` row-${index} ${index % 3 === 0 ? "中文\t".repeat(35) : "long words ".repeat(24)}`,
      }))];
      await page.route("**/__lab/invoke", async (route) => {
        const { cmd, args } = route.request().postDataJSON();
        if (cmd !== "file_diff" && cmd !== "show_commit_file") return route.fallback();
        await route.fulfill({ json: { ok: true, value: { path: args.file, lines, hunks: [], binary: false, truncated: false } } });
      });
      const scene = surface === "changes" ? style === "split" ? "diff-wrap" : "diff-wrap-unified" : surface;
      await openLabScene(page, scene, surface === "file-history" ? ".history-diff" : surface === "commit" ? ".detail" : "section.diff");
      if (surface === "commit") await page.locator(".patch-toggle").first().click();
      const scroller = page.locator(surface === "changes" ? "section.diff .diff-body" : surface === "commit" ? ".detail-scroll" : ".history-diff");
      const list = scroller.locator(".wrapped-rows").first();
      await checkRows(list);
      await scroller.evaluate((node) => { node.scrollTop = node.scrollHeight; });
      await expect(list.locator('[data-wrapped-row="1200"]')).toContainText("row-1199");
      await expect(list.locator('[data-wrapped-row="1200"]')).toBeInViewport();
      await checkRows(list);
      await page.setViewportSize({ width: 1440, height: 900 });
      await scroller.evaluate((node) => { node.scrollTop = node.scrollHeight / 2; });
      await expect.poll(() => list.locator(".wrapped-row").first().getAttribute("data-wrapped-row")).not.toBe("1200");
      await checkRows(list);
      await scroller.evaluate((node) => { node.scrollTop = 0; });
      if (surface === "commit") await list.locator('[data-wrapped-row="0"]').scrollIntoViewIfNeeded();
      await expect(list.locator('[data-wrapped-row="0"]')).toBeInViewport();
      await checkRows(list);
      expect(errors).toEqual([]);
    });
  }
}

test("measured height index preserves exact row boundaries after scattered updates", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const path = "/src/lib/rowHeights.ts";
    const { RowHeights } = await import(/* @vite-ignore */ path);
    const heights = Array.from({ length: 20000 }, () => 19);
    const index = new RowHeights(Uint32Array.from({ length: heights.length + 1 }, (_, i) => i), 19);
    for (let at = 0; at < heights.length; at += 37) { heights[at] = 63.5; index.set(at, 63.5); }
    let sum = 0;
    for (let at = 0; at < heights.length; at++) {
      if (index.top(at) !== sum || index.at(sum) !== at || index.at(sum + heights[at] - 0.1) !== at) return false;
      sum += heights[at];
    }
    return index.total === sum && index.at(sum + 1000) === heights.length - 1;
  });
  expect(result).toBe(true);
});
