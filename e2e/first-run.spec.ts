import { expect, test } from "@playwright/test";

test("a fresh install marks the first-run setup once", async ({ page }) => {
  await page.goto("/?scene=clean");
  const result = await page.evaluate(async () => {
    const { ShellState } = await import("/src/lib/shell/state.svelte.ts");
    const shell: any = {
      inApp: () => true,
      labPage: false,
      standalone: false,
      settings: { onboarding: "pending", featuresIntro: "pending" },
    };
    const first = ShellState.prototype.takeFirstRunSetup.call(shell);
    const second = ShellState.prototype.takeFirstRunSetup.call(shell);
    return { first, second, settings: { ...shell.settings } };
  });
  expect(result.first).toBe(true);
  expect(result.second).toBe(false);
  expect(result.settings).toMatchObject({ onboarding: "seen", featuresIntro: "seen" });
});

test("first-run setup imports quietly, then enables the endpoint and killer push", async ({ page }) => {
  await page.goto("/?scene=clean");
  const result = await page.evaluate(async () => {
    const { ShellState } = await import("/src/lib/shell/state.svelte.ts");
    const calls: string[] = [];
    const shell: any = {
      settings: { aiBaseUrl: "", aiEnabled: false, killerPush: false },
      forkImportNote: "missing",
      syncAccounts: async () => { calls.push("accounts"); },
      importFork: async (_path: string | null, select: boolean) => { calls.push(`fork:${select}`); },
      savePrefs: async () => { calls.push("save"); },
    };
    await ShellState.prototype.runFirstRunSetup.call(shell);
    return { calls, note: shell.forkImportNote, settings: { ...shell.settings } };
  });
  expect(result.calls).toEqual(["accounts", "fork:false", "save"]);
  expect(result.note).toBe("");
  expect(result.settings).toMatchObject({
    killerPush: true,
    aiEnabled: true,
    aiBaseUrl: "https://api.x.ai/v1",
  });
});
