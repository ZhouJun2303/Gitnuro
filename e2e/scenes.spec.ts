import { expect, test } from "@playwright/test";
import { scenes } from "../src/lib/lab/scenes";
import { openScene } from "./helpers";
import { mkdirSync, writeFileSync } from "node:fs";

for (const variant of [
  { name: "zh-apple", width: 1440, height: 900, locale: "zh", theme: "apple" },
  { name: "zh-light", width: 1440, height: 900, locale: "zh", theme: "light" },
  { name: "en-dark-compact", width: 960, height: 640, locale: "en", theme: "dark" },
]) {
  test.describe(variant.name, () => {
    test.use({ viewport: { width: variant.width, height: variant.height } });
    for (const scene of scenes) test(`scene ${scene.id}`, async ({ page }, info) => {
      test.setTimeout(100_000);
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));
      // Preferences fixtures can autosave while being initialized. Keep each audit case isolated.
      let settings: Record<string, unknown> | undefined;
      await page.route("**/__lab/invoke", async (route) => {
        const { cmd, args } = route.request().postDataJSON();
        if (cmd === "load_settings") {
          if (!settings) {
            const response = await route.fetch();
            const payload = await response.json();
            settings = { ...payload.value, flowAutoClose: false };
          }
          return route.fulfill({ json: { ok: true, value: settings } });
        }
        if (cmd === "save_settings") {
          settings = args.patch ? { ...settings, ...args.patch } : args.values;
          return route.fulfill({ json: { ok: true, value: settings } });
        }
        return route.continue();
      });
      await page.addInitScript(() => {
        const metrics = { longTasks: 0, maxTaskMs: 0 };
        (window as any).__uiAuditMetrics = metrics;
        new PerformanceObserver((list) => {
          for (const entry of list.getEntries()) { metrics.longTasks++; metrics.maxTaskMs = Math.max(metrics.maxTaskMs, entry.duration); }
        }).observe({ type: "longtask", buffered: true });
      });
      await openScene(page, scene, `&locale=${variant.locale}&theme=${variant.theme}`);
      await expect(page.locator("html")).toHaveAttribute("lang", variant.locale === "zh" ? "zh-CN" : "en");
      await expect(page.locator("html")).toHaveAttribute("data-theme", variant.theme);
      const layout = await page.evaluate(() => {
        const surfaces = [...document.querySelectorAll<HTMLElement>(".dialog, .merge-panel, .flow-window")];
        return { overflow: document.documentElement.scrollWidth - innerWidth,
          outside: surfaces.filter((node) => {
            const box = node.getBoundingClientRect();
            return box.width > 0 && box.height > 0 && (box.left < -1 || box.right > innerWidth + 1 || box.top < -1 || box.bottom > innerHeight + 1);
          }).map((node) => node.className) };
      });
      const directory = `.lab/ui-audit/${variant.name}`;
      mkdirSync(directory, { recursive: true });
      const screenshot = `${directory}/${scene.id}.png`;
      await page.evaluate(() => Promise.allSettled(document.getAnimations()
        .filter((animation) => animation.effect?.getTiming().iterations !== Infinity).map((animation) => animation.finished)));
      await page.screenshot({ path: screenshot });
      await info.attach("interface", { path: screenshot, contentType: "image/png" });
      const metrics = await page.evaluate(() => ({ nodes: document.querySelectorAll("*").length,
        loadMs: performance.now(), ...((window as any).__uiAuditMetrics ?? {}) }));
      writeFileSync(`${directory}/${scene.id}.json`, JSON.stringify(metrics, null, 2));
      expect(errors).toEqual([]);
      expect(layout.overflow).toBeLessThanOrEqual(1);
      expect(layout.outside).toEqual([]);
    });
  });
}
