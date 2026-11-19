import { expect, test } from "@playwright/test";

async function openClean(page: import("@playwright/test").Page) {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
}

test("commit and push fetches after a remote update rejection", async ({ page }) => {
  await openClean(page);
  const pushes: unknown[] = [];
  await page.route("**/__lab/invoke", async (route) => {
    const body = route.request().postDataJSON();
    if (body.cmd === "mutate" && body.args?.request?.action === "push") {
      pushes.push(body);
      await route.fulfill({ json: { ok: true, value: {
        outcome: "failed",
        error: "! [rejected] main -> main (fetch first)\nerror: failed to push some refs",
        log: "",
      } } });
      return;
    }
    await route.fallback();
  });

  const fetches = await page.evaluate(async () => {
    const { submitCommit } = await import(/* @vite-ignore */ "/src/lib/shell/dialogs.svelte.ts");
    const fetched: string[] = [];
    const shell = {
      progress: null,
      canCommit: true,
      fullSummary: "commit subject",
      summary: "commit subject",
      description: "",
      commitType: "",
      amend: false,
      skipHooks: false,
      signOff: false,
      settings: { pushOnCommit: false, forceWithLease: false },
      openGen: 0,
      actionError: null,
      conflictCount: 0,
      drafts: new Map(),
      repoPath: () => "D:/push-fetch",
      viewing: () => true,
      tr: (key: string) => key,
      gitLine: (args: string[]) => args.join(" "),
      runChange: async () => ({ log: "commit complete" }),
      recordCommand: () => {},
      scheduleAuxiliary: () => {},
      fillTemplate: () => {},
      doFetch: async (source: string) => { fetched.push(source); return "completed"; },
    } as any;
    await submitCommit(shell, true);
    return fetched;
  });

  expect(pushes).toHaveLength(1);
  expect(fetches).toEqual(["background"]);
});

test("commit and push does not fetch for unrelated push failures", async ({ page }) => {
  await openClean(page);
  await page.route("**/__lab/invoke", async (route) => {
    const body = route.request().postDataJSON();
    if (body.cmd === "mutate" && body.args?.request?.action === "push") {
      await route.fulfill({ json: { ok: true, value: {
        outcome: "failed",
        error: "awegit:auth:authentication failed",
        log: "",
      } } });
      return;
    }
    await route.fallback();
  });

  const fetches = await page.evaluate(async () => {
    const { submitCommit } = await import(/* @vite-ignore */ "/src/lib/shell/dialogs.svelte.ts");
    const fetched: string[] = [];
    const shell = {
      progress: null,
      canCommit: true,
      fullSummary: "commit subject",
      summary: "commit subject",
      description: "",
      commitType: "",
      amend: false,
      skipHooks: false,
      signOff: false,
      settings: { pushOnCommit: false, forceWithLease: false },
      openGen: 0,
      actionError: null,
      conflictCount: 0,
      drafts: new Map(),
      repoPath: () => "D:/push-fetch",
      viewing: () => true,
      tr: (key: string) => key,
      gitLine: (args: string[]) => args.join(" "),
      runChange: async () => ({ log: "commit complete" }),
      recordCommand: () => {},
      scheduleAuxiliary: () => {},
      fillTemplate: () => {},
      doFetch: async (source: string) => { fetched.push(source); return "completed"; },
    } as any;
    await submitCommit(shell, true);
    return fetched;
  });

  expect(fetches).toEqual([]);
});
