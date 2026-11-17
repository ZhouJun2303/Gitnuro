import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "@playwright/test";
import { pinLabSettings } from "./helpers";

const repo = resolve(".lab/repos/stage-dense");
const git = (...args: string[]) => execFileSync("git", args, { cwd: repo, encoding: "utf8", stdio: "pipe" }).trim();

test.beforeAll(() => {
  execFileSync(process.execPath, ["scripts/lab-seed.mjs", "stage-dense"], { stdio: "pipe" });
});

for (const theme of ["apple", "light", "dark"]) for (const style of ["split", "unified"]) {
  test(`single files with thousands of changed lines stage independently of previews (${theme}, ${style})`, async ({ page }) => {
    test.setTimeout(90_000);
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await pinLabSettings(page, { theme, diffStyle: style, diffWordWrap: true, fetchAutomatically: false, statusInterval: 0, fileLayout: "list" });
    // Retains a way to compare older lab backends against the same fixture.
    await page.route("**/__lab/invoke", async (route) => {
      if (route.request().postDataJSON().cmd !== "lab_layout") return route.fallback();
      const response = await route.fetch();
      const payload = await response.json();
      payload.value["stage-dense"] = repo;
      return route.fulfill({ json: payload });
    });
    await page.goto(`/?scene=stage-dense&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="stage-dense"]')).toBeVisible();
    await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
    await page.evaluate(() => {
      (window as any).__denseLongTasks = [];
      new PerformanceObserver((list) => {
        (window as any).__denseLongTasks.push(...list.getEntries().map((entry) => entry.duration));
      }).observe({ entryTypes: ["longtask"] });
    });
    const samples: { file: string; stage: number; unstage: number }[] = [];
    const unstaged = page.locator(".unstaged-pane");
    const staged = page.locator(".staged-pane");
    for (const file of ["a-many.ts", "b-large.asset"]) {
      const working = git("hash-object", "--", file);
      await unstaged.locator(`.file-select[title="${file}"]`).click();
      if (file === "b-large.asset") {
        await expect(page.locator(".diff-load")).toBeVisible();
        await page.locator(".diff-load button").click();
      }
      const rows = page.locator("section.diff .wrapped-row");
      await expect.poll(() => rows.count()).toBeGreaterThan(1);
      expect(await rows.count()).toBeLessThan(200);
      const diff = page.locator("section.diff .diff-body");
      await diff.evaluate((node) => { node.scrollTop = 20_000; });
      await expect.poll(() => diff.evaluate((node) => node.scrollTop)).toBeGreaterThan(1000);

      let release!: () => void;
      let preview!: () => void;
      let blocked = false;
      const gate = new Promise<void>((done) => { release = done; });
      const reading = new Promise<void>((done) => { preview = done; });
      await page.route("**/__lab/invoke", async (route) => {
        const body = route.request().postDataJSON();
        if (body.cmd !== "file_diff" || body.args.file !== file || !body.args.staged || blocked) return route.fallback();
        blocked = true;
        preview();
        await gate;
        return route.fallback();
      });
      try {
        const start = performance.now();
        await unstaged.locator(`.file-select[title="${file}"]`).dblclick();
        await expect(staged.locator(`.file-select[title="${file}"]`)).toBeVisible();
        const stage = performance.now() - start;
        await reading;
        expect(git("rev-parse", `:${file}`)).toBe(working);
        const list = unstaged.locator(".file-list");
        await list.evaluate((node) => { node.scrollTop = 3000; });
        await expect.poll(() => list.evaluate((node) => node.scrollTop)).toBeGreaterThan(1000);
        await expect(list.locator(".file").first()).toBeVisible();
        expect(await list.locator(".file").count()).toBeLessThan(200);
        await page.locator(".summary-input").fill("still responsive");
        await expect(page.locator(".summary-input")).toHaveValue("still responsive");
        const reset = performance.now();
        const resetDone = page.waitForResponse((response) => response.url().includes("/__lab/invoke")
          && response.request().postDataJSON()?.cmd === "unstage_path"
          && response.request().postDataJSON()?.args.file === file);
        await staged.locator(`.file-select[title="${file}"]`).dblclick();
        await expect(staged.locator(`.file-select[title="${file}"]`)).toHaveCount(0);
        samples.push({ file, stage, unstage: performance.now() - reset });
        // The row now moves before index confirmation, while the preview remains held.
        expect((await (await resetDone).json()).ok).toBe(true);
        expect(git("diff", "--cached", "--name-only")).toBe("");
        expect(git("hash-object", "--", file)).toBe(working);
      } finally { release(); }
      await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
      await unstaged.locator(".file-list").evaluate((node) => { node.scrollTop = 0; });
    }
    expect(errors).toEqual([]);
    const directory = `.lab/ui-audit/${process.env.AWEGIT_UI_AUDIT_RUN || "current"}`;
    mkdirSync(directory, { recursive: true });
    const longTasks = await page.evaluate(() => (window as any).__denseLongTasks as number[]);
    writeFileSync(`${directory}/stage-dense-${theme}-${style}.json`, JSON.stringify({ samples, longTasks }, null, 2));
    await page.screenshot({ path: `${directory}/stage-dense-${theme}-${style}.png` });
    console.log(theme, style, JSON.stringify({ samples, longTasks }));
  });
}
