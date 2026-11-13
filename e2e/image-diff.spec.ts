import { expect, test } from "@playwright/test";
import { mkdirSync } from "node:fs";
import { openScene } from "./helpers";
import { scenes } from "../src/lib/lab/scenes";

test("pixel comparison computes a 4096 image in a worker while the UI keeps running", async ({ page }, info) => {
  test.setTimeout(60_000);
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const path = "/src/lib/pixel-diff.ts";
    const { comparePixels } = await import(/* @vite-ignore */ path);
    const source = document.createElement("canvas");
    source.width = source.height = 4096;
    const draw = source.getContext("2d")!;
    draw.fillStyle = "rgb(20,40,60)";
    draw.fillRect(0, 0, 4096, 4096);
    const before = source.toDataURL();
    draw.fillStyle = "rgb(100,80,20)";
    draw.fillRect(0, 0, 4096, 4096);
    draw.fillStyle = "rgb(20,40,60)";
    draw.fillRect(4095, 4095, 1, 1);
    const after = source.toDataURL();
    await new Promise((resolve) => setTimeout(resolve, 0));
    const original = CanvasRenderingContext2D.prototype.getImageData;
    const bitmap = window.createImageBitmap;
    let mainReads = 0;
    let mainBitmaps = 0;
    window.createImageBitmap = ((...args: any[]) => { mainBitmaps++; return (bitmap as any)(...args); }) as typeof createImageBitmap;
    CanvasRenderingContext2D.prototype.getImageData = function (...args: Parameters<typeof original>) { mainReads++; return original.apply(this, args); };
    let ticks = 0;
    const timer = setInterval(() => ticks++, 0);
    const longTasks: number[] = [];
    const observer = new PerformanceObserver((list) => { longTasks.push(...list.getEntries().map((entry) => entry.duration)); });
    observer.observe({ type: "longtask" });
    const start = performance.now();
    try {
      const blob = await comparePixels(before, after, new AbortController().signal);
      const elapsedMs = performance.now() - start;
      await new Promise((resolve) => setTimeout(resolve, 0));
      observer.disconnect();
      clearInterval(timer);
      CanvasRenderingContext2D.prototype.getImageData = original;
      window.createImageBitmap = bitmap;
      const image = await createImageBitmap(blob);
      draw.clearRect(0, 0, 4096, 4096);
      draw.drawImage(image, 0, 0);
      const first = [...draw.getImageData(0, 0, 1, 1).data];
      const last = [...draw.getImageData(4095, 4095, 1, 1).data];
      const size = [image.width, image.height];
      image.close();
      return { first, last, size, mainReads, mainBitmaps, ticks, elapsedMs, longTasks };
    } finally {
      clearInterval(timer);
      observer.disconnect();
      CanvasRenderingContext2D.prototype.getImageData = original;
      window.createImageBitmap = bitmap;
    }
  });
  expect(result.first).toEqual([80, 40, 40, 255]);
  expect(result.last).toEqual([0, 0, 0, 255]);
  expect(result.size).toEqual([4096, 4096]);
  expect(result.mainReads).toBe(0);
  expect(result.mainBitmaps).toBe(0);
  expect(result.ticks).toBeGreaterThan(1);
  await info.attach("pixel-performance", { body: JSON.stringify(result, null, 2), contentType: "application/json" });
  console.log("pixel-performance", JSON.stringify(result));
});

test("pixel comparison fallback preserves unequal image bounds and limits every pixel read", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const path = "/src/lib/pixel-diff.ts";
    const { comparePixels } = await import(/* @vite-ignore */ path);
    const image = (width: number, height: number, color: string) => {
      const canvas = document.createElement("canvas");
      canvas.width = width; canvas.height = height;
      const context = canvas.getContext("2d")!;
      context.fillStyle = color; context.fillRect(0, 0, width, height);
      return canvas.toDataURL();
    };
    const left = image(513, 257, "rgb(20,40,60)");
    const right = image(257, 129, "rgb(100,80,20)");
    const worker = window.Worker;
    const original = CanvasRenderingContext2D.prototype.getImageData;
    let largestRead = 0;
    (window as any).Worker = undefined;
    CanvasRenderingContext2D.prototype.getImageData = function (...args: Parameters<typeof original>) { largestRead = Math.max(largestRead, args[2] * args[3]); return original.apply(this, args); };
    try {
      const blob = await comparePixels(left, right, new AbortController().signal);
      CanvasRenderingContext2D.prototype.getImageData = original;
      const bitmap = await createImageBitmap(blob);
      const canvas = document.createElement("canvas");
      canvas.width = bitmap.width; canvas.height = bitmap.height;
      const context = canvas.getContext("2d")!;
      context.drawImage(bitmap, 0, 0); bitmap.close();
      return { first: [...context.getImageData(0, 0, 1, 1).data], outside: [...context.getImageData(512, 256, 1, 1).data], largestRead };
    } finally { window.Worker = worker; CanvasRenderingContext2D.prototype.getImageData = original; }
  });
  expect(result.first).toEqual([80, 40, 40, 255]);
  expect(result.outside).toEqual([20, 40, 60, 255]);
  expect(result.largestRead).toBeLessThanOrEqual(256 * 256);
});

test("pixel comparison reports an unreadable image and allows a successful retry", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const path = "/src/lib/shell/diff.svelte.ts";
    const { paintPixel, clearPixel } = await import(/* @vite-ignore */ path);
    const sample = "/src/lib/sample.ts";
    const { sampleImages } = await import(/* @vite-ignore */ sample);
    const shell: any = { repoPath: () => "repo", openGen: 0, selectedPath: "image.png", imageOld: null, imageNew: null,
      imageMode: "pixel", pixelUrl: "", pixelStatus: "idle" };
    try {
      await paintPixel(shell, "data:image/png;base64,broken", sampleImages.after);
      const failed = shell.pixelStatus === "error" && shell.pixelUrl === "";
      await paintPixel(shell, sampleImages.before, sampleImages.after);
      return { failed, retried: shell.pixelStatus === "idle" && shell.pixelUrl.startsWith("blob:") };
    } finally { clearPixel(shell); }
  });
  expect(result).toEqual({ failed: true, retried: true });
});

for (const change of ["repository", "selection", "newer", "cancelled"] as const) {
  test(`pixel comparison discards ${change} jobs and releases previews`, async ({ page }) => {
    await page.goto("/?scene=clean");
    await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
    const result = await page.evaluate(async (change) => {
      const path = "/src/lib/shell/diff.svelte.ts";
      const { paintPixel, clearPixel } = await import(/* @vite-ignore */ path);
      const sample = "/src/lib/sample.ts";
      const { sampleImages } = await import(/* @vite-ignore */ sample);
      const worker = window.Worker;
      const revoke = URL.revokeObjectURL;
      const jobs: any[] = [];
      const revoked: string[] = [];
      let terminations = 0;
      (window as any).Worker = class {
        onmessage: any; onerror: any;
        postMessage() { jobs.push(this); }
        terminate() { terminations++; }
      };
      URL.revokeObjectURL = (url) => { revoked.push(url); revoke(url); };
      const shell: any = { path: "old-repo", repoPath() { return this.path; }, openGen: 0, selectedPath: "old.png", selectedSide: "unstaged",
        selectedCommit: null, historyFile: null, imageOld: sampleImages.before, imageNew: sampleImages.after, imageMode: "pixel", pixelUrl: "", pixelStatus: "idle" };
      const waitForJobs = async (count: number) => { while (jobs.length < count) await new Promise((resolve) => setTimeout(resolve, 0)); };
      const finish = (index: number) => jobs[index].onmessage({ data: { blob: new Blob(["preview"], { type: "image/png" }) } });
      try {
        const first = paintPixel(shell, sampleImages.before, sampleImages.after);
        await waitForJobs(1);
        if (change === "repository") { shell.path = "new-repo"; shell.openGen++; }
        if (change === "selection") shell.selectedPath = "new.png";
        if (change === "cancelled") clearPixel(shell);
        let retained = "";
        if (change === "newer") {
          const next = paintPixel(shell, sampleImages.after, sampleImages.before);
          await waitForJobs(2); finish(1); await next; retained = shell.pixelUrl;
        }
        finish(0); await first;
        const url = shell.pixelUrl;
        clearPixel(shell);
        return { hasPreview: url.startsWith("blob:"), keptNew: url === retained, released: !url || revoked.includes(url), terminations, cleared: shell.pixelUrl === "" };
      } finally { clearPixel(shell); window.Worker = worker; URL.revokeObjectURL = revoke; }
    }, change);
    expect(result.hasPreview).toBe(change === "newer");
    expect(result.keptNew).toBe(true);
    expect(result.released).toBe(true);
    expect(result.cleared).toBe(true);
    expect(result.terminations).toBeGreaterThan(0);
  });
}

for (const theme of ["apple", "light", "dark"] as const) {
  const locale = theme === "dark" ? "en" : "zh";
  const pixel = locale === "zh" ? "像素差分" : "Pixel";
  const side = locale === "zh" ? "并排" : "Side by side";
  test(`image panel ${theme} shows pixel results and releases them on mode changes`, async ({ page }) => {
    await openScene(page, scenes.find((scene) => scene.id === "diff-image")!, `&locale=${locale}&theme=${theme}`);
    await page.getByRole("button", { name: pixel, exact: true }).click();
    const preview = page.locator(".image-stage img");
    await expect(preview).toHaveCount(1);
    await expect(preview).toHaveAttribute("src", /^blob:/);
    await expect(preview).toBeVisible();
    mkdirSync(".lab/fix-audit", { recursive: true });
    await page.screenshot({ path: `.lab/fix-audit/pixel-${theme}.png` });
    await page.getByRole("button", { name: side, exact: true }).click();
    await expect(page.locator(".image-stage img")).toHaveCount(2);
  });

  test(`image panel ${theme} keeps mode buttons usable while a comparison is pending`, async ({ page }) => {
    await page.addInitScript(() => {
      (window as any).__pixelTerminated = 0;
      (window as any).__pixelStarted = false;
      (window as any).Worker = class {
        onmessage: any; onerror: any;
        postMessage() { (window as any).__pixelStarted = true; }
        terminate() { (window as any).__pixelTerminated++; }
      };
    });
    await openScene(page, scenes.find((scene) => scene.id === "diff-image")!, `&locale=${locale}&theme=${theme}`);
    await page.getByRole("button", { name: pixel, exact: true }).click();
    await page.waitForFunction(() => (window as any).__pixelStarted);
    await expect(page.locator(".image-stage [role=status]")).toBeVisible();
    await page.getByRole("button", { name: side, exact: true }).click();
    await expect(page.locator(".image-stage img")).toHaveCount(2);
    expect(await page.evaluate(() => (window as any).__pixelTerminated)).toBe(1);
  });

  test(`image panel ${theme} resets a pixel preview when its image refreshes`, async ({ page }) => {
    await openScene(page, scenes.find((scene) => scene.id === "diff-image")!, `&locale=${locale}&theme=${theme}`);
    await page.getByRole("button", { name: pixel, exact: true }).click();
    await expect(page.locator(".image-stage img")).toHaveAttribute("src", /^blob:/);
    const reloaded = page.waitForResponse((response) => response.url().endsWith("/__lab/invoke")
      && response.request().postDataJSON()?.cmd === "revision_preview");
    await page.keyboard.press("F5");
    await reloaded;
    await expect(page.getByRole("button", { name: side, exact: true })).toHaveAttribute("aria-pressed", "true", { timeout: 3000 });
    await expect(page.locator(".image-stage img")).toHaveCount(2);
    await page.getByRole("button", { name: pixel, exact: true }).click();
    await expect(page.locator(".image-stage img")).toHaveAttribute("src", /^blob:/);
  });

  test(`image panel ${theme} rejects excessive combined dimensions before allocating pixels`, async ({ page }) => {
    let bitmaps = 0;
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd } = route.request().postDataJSON();
      if (cmd !== "file_preview" && cmd !== "revision_preview") return route.continue();
      const response = await route.fetch();
      const payload = await response.json();
      const [width, height] = cmd === "file_preview" ? [8000, 2000] : [2000, 8000];
      const dataUrl = `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}"></svg>`)}`;
      return route.fulfill({ json: { ...payload, value: { ...payload.value, dataUrl } } });
    });
    await page.addInitScript(() => {
      const original = window.createImageBitmap;
      (window as any).__pixelBitmaps = 0;
      window.createImageBitmap = ((...args: any[]) => { (window as any).__pixelBitmaps++; return (original as any)(...args); }) as typeof createImageBitmap;
    });
    await openScene(page, scenes.find((scene) => scene.id === "diff-image")!, `&locale=${locale}&theme=${theme}`);
    await page.getByRole("button", { name: pixel, exact: true }).click();
    await expect(page.locator(".image-stage [role=status]")).toContainText(locale === "zh" ? "图片过大" : "too large");
    bitmaps = await page.evaluate(() => (window as any).__pixelBitmaps);
    expect(bitmaps).toBe(0);
    mkdirSync(".lab/fix-audit", { recursive: true });
    await page.screenshot({ path: `.lab/fix-audit/pixel-limit-${theme}.png` });
    await page.getByRole("button", { name: side, exact: true }).click();
    await expect(page.locator(".image-stage img")).toHaveCount(2);
  });
}
