import { expect, test } from "@playwright/test";
import { scenes } from "../src/lib/lab/scenes";
import { openScene } from "./helpers";

function updateScene() {
  const scene = scenes.find((item) => item.id === "update-long");
  if (!scene) throw new Error("update-long");
  return scene;
}

for (const recheck of [false, true]) for (const fail of [false, true]) {
  test(`dismissed update checks ignore late ${fail ? "errors" : "results"}${recheck ? " after rechecking" : ""}`, async ({ page }) => {
    await page.goto("/?scene=clean");
    await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
    const result = await page.evaluate(async ({ recheck, fail }) => {
      const modulePath = "/src/lib/shell/state.svelte.ts";
      const { ShellState } = await import(/* @vite-ignore */ modulePath);
      const original = window.fetch;
      let release!: () => void, calls = 0;
      const pending = new Promise<Response>((resolve) => {
        release = () => resolve(new Response(JSON.stringify(fail ? { ok: false, error: "old error" } : { ok: true, value: { appVersion: "old" } })));
      });
      window.fetch = (input, init) => {
        const body = typeof init?.body === "string" ? JSON.parse(init.body) : null;
        if (body?.cmd !== "check_for_update") return original(input, init);
        return ++calls === 1 ? pending : Promise.resolve(new Response(JSON.stringify({ ok: true, value: { appVersion: "new" } })));
      };
      const shell: any = { updatePhase: "idle", updateBusy: false, updateCheckGen: 0, updateNotice: null, pendingUpdate: null,
        updateCheckedAt: 0, settings: {}, dialog: "update", tr: String, message: String, savePrefs: async () => {},
        updateDay: () => {
          const now = new Date();
          return `${now.getFullYear()}-${now.getMonth()}-${now.getDate()}`;
        },
        finishUpdateCheck(generation = this.updateCheckGen) {
          if (generation !== this.updateCheckGen) return;
          this.updateCheckInFlight = false;
        } };
      try {
        const checking = ShellState.prototype.checkForUpdate.call(shell);
        ShellState.prototype.dismissUpdate.call(shell);
        if (recheck) await ShellState.prototype.checkForUpdate.call(shell);
        release(); await checking;
        return { phase: shell.updatePhase, version: shell.updateNotice?.appVersion ?? null, pending: shell.pendingUpdate?.appVersion ?? null };
      } finally { release(); window.fetch = original; }
    }, { recheck, fail });
    expect(result).toEqual(recheck ? { phase: "available", version: "new", pending: "new" } : { phase: "idle", version: null, pending: null });
  });
}

test("update notes give every change its own line and scroll inside the panel", async ({ page }) => {
  await openScene(page, updateScene());
  const dialog = page.locator('[data-dialog="update"]');
  await expect(dialog.locator(".update-notes-section")).toHaveCount(3);
  await expect(dialog.locator(".update-notes-section").first()).toHaveText("✨ 新功能");
  const notes = dialog.locator(".update-notes");
  const metrics = await notes.evaluate((node) => ({ clientHeight: node.clientHeight, scrollHeight: node.scrollHeight }));
  expect(metrics.scrollHeight).toBeGreaterThan(metrics.clientHeight);
  await notes.evaluate((node) => {
    node.scrollTop = node.scrollHeight;
  });
  await expect.poll(() => notes.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
});

for (const theme of ["apple", "light", "dark"] as const) {
  test(`update notes fit the dialog in ${theme}`, async ({ page }) => {
    await openScene(page, updateScene(), `&theme=${theme}`);
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    const notes = page.locator('[data-dialog="update"] .update-notes');
    const box = await notes.boundingBox();
    expect(box?.height ?? 0).toBeGreaterThan(80);
    expect(box?.height ?? 0).toBeLessThanOrEqual(300);
    const item = await page.locator('[data-dialog="update"] .update-notes-item').first().boundingBox();
    expect(item?.width ?? 0).toBeLessThan(box?.width ?? 0);
    await expect(page.locator('[data-dialog="update"] .update-actions')).toBeInViewport();
  });
}
