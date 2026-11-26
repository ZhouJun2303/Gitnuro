import { expect, test } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"] as const) {
  test(`${theme} reads and selects history while the initial file status and refs are pending`, async ({ page }) => {
    let finishStatus!: () => void;
    const gate = new Promise<void>((resolve) => { finishStatus = resolve; });
    const commands: string[] = [];
    await openLabScene(page, "welcome", ".recent-card", { theme });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd } = route.request().postDataJSON();
      commands.push(cmd);
      if (cmd === "workspace_status" || cmd === "repository_refs") await gate;
      await route.fallback();
    });
    try {
      await page.locator(".recent-card").first().click();
      const pending = page.locator(".body > section[aria-busy='true']");
      await expect(pending).toBeVisible();
      await page.locator('.sidebar [data-section="history"]').click();
      const commit = page.locator(".commit-row[data-commit-id]").first();
      await expect(commit).toBeVisible();
      await commit.click();
      await expect(commit).toHaveAttribute("aria-pressed", "true");
      expect(commands).not.toContain("merge_state");
      finishStatus();
      await expect(page.locator(".sidebar [data-section='branches']")).toBeVisible();
      await expect(commit).toHaveAttribute("aria-pressed", "true");
      await page.locator('.sidebar [data-section="changes"]').click();
      await expect(pending).toHaveCount(0);
      await expect(page.locator(".unstaged-pane")).toBeVisible();
    } finally {
      finishStatus();
    }
  });

  test(`${theme} submits an amend with the repository sign-off choice while refs are pending`, async ({ page }) => {
    let releaseRefs!: () => void;
    const refsPending = new Promise<void>((resolve) => { releaseRefs = resolve; });
    const commits: Record<string, unknown>[] = [];
    await pinLabSettings(page, { pushOnCommit: false });
    await openLabScene(page, "welcome", ".recent-card", { theme });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd, args } = route.request().postDataJSON();
      if (cmd === "repository_refs") await refsPending;
      if (cmd === "commit_changes") {
        commits.push(args);
        await route.fulfill({ json: { ok: false, error: "fixture: leave the index unchanged" } });
        return;
      }
      await route.fallback();
    });
    try {
      await page.locator(".recent-card").first().click();
      await page.locator('.composer .check input[type="checkbox"]').check();
      await page.locator(".composer .summary-input").fill("commit while loading refs");
      await page.locator(".composer button.commit").click();
      await expect.poll(() => commits.length).toBe(1);
      expect(commits[0]).not.toHaveProperty("signOff");
    } finally {
      releaseRefs();
    }
  });

  test(`${theme} opens and switches to a repository with a Windows-incompatible submodule path`, async ({ page }) => {
    const regularChange = ".file .file-select[title='regular.txt']";
    const submoduleChange = ".file .file-select[title='third_party/child']";
    await openLabScene(page, "submodule-foreign-path", regularChange, { theme });
    await expect(page.locator(submoduleChange)).toBeVisible();

    const picker = "[data-layer='repos']";
    await openLabScene(page, "picker", picker, { theme });
    await page.locator(`${picker} .repo-row[title$='submodule-foreign-path']`).click();
    await expect(page.locator(picker)).toBeHidden();
    await expect(page.locator(regularChange)).toBeVisible();
    await expect(page.locator(submoduleChange)).toBeVisible();
    await expect(page.getByText("Could not determine status for submodule", { exact: false })).toHaveCount(0);
  });
}

test("a quiet watcher keeps the painted status and a named edit refreshes only those paths", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const planPath = "/src/lib/shell/status-plan.ts";
    const statePath = "/src/lib/shell/state.svelte.ts";
    const { planStatusRefresh, mergeWatchNote } = await import(/* @vite-ignore */ planPath);
    const { ShellState } = await import(/* @vite-ignore */ statePath);
    const fingerprint = { head: "ref: refs/heads/main", refId: "abc", indexLen: 10, indexSum: "ff", submoduleSum: "child-one", worktreeSum: "checkout-one" };
    const probe = { fingerprint, revision: 4, watcherAlive: true };
    const trust = planStatusRefresh({ fingerprint, seen: 4, unverified: false, probe, note: null });
    const staleIndex = planStatusRefresh({
      fingerprint, seen: 4, unverified: false,
      probe: { ...probe, fingerprint: { ...fingerprint, indexSum: "ee" } }, note: null,
    });
    const staleSubmodule = planStatusRefresh({
      fingerprint, seen: 4, unverified: false,
      probe: { ...probe, fingerprint: { ...fingerprint, submoduleSum: "child-two" } }, note: null,
    });
    const staleWorktree = planStatusRefresh({
      fingerprint, seen: 4, unverified: false,
      probe: { ...probe, fingerprint: { ...fingerprint, worktreeSum: "checkout-two" } }, note: null,
    });
    const cold = planStatusRefresh({ fingerprint, seen: 0, unverified: true, probe: { ...probe, watcherAlive: false, revision: 0 }, note: null });
    const noted = mergeWatchNote(null, "status", 5, ["src/a.ts"]);
    const paths = planStatusRefresh({ fingerprint, seen: 4, unverified: false, probe: { ...probe, revision: 5 }, note: noted });
    const overflow = planStatusRefresh({
      fingerprint, seen: 4, unverified: false, probe: { ...probe, revision: 9 },
      note: mergeWatchNote(noted, "full", 9, null),
    });
    const missed = planStatusRefresh({ fingerprint, seen: 4, unverified: false, probe: { ...probe, revision: 8 }, note: null });
    const shell: any = {
      repoWatchNotes: new Map(),
      jobKey: (path: string) => path.replaceAll("\\", "/").toLowerCase(),
    };
    shell.noteRepoWatch = ShellState.prototype.noteRepoWatch.bind(shell);
    shell.noteRepoWatch("D:/One", "status", 2, ["a.ts"]);
    shell.noteRepoWatch("D:/One", "status", 3, ["b.ts"]);
    shell.noteRepoWatch("D:/Two", "full", 4, null);
    const one = shell.repoWatchNotes.get("d:/one");
    const two = shell.repoWatchNotes.get("d:/two");
    return { trust, staleIndex, staleSubmodule, staleWorktree, cold, paths, overflow, missed, one, two };
  });
  expect(result.trust).toEqual({ kind: "trust" });
  expect(result.staleIndex).toEqual({ kind: "full" });
  expect(result.staleSubmodule).toEqual({ kind: "full" });
  expect(result.staleWorktree).toEqual({ kind: "full" });
  expect(result.cold).toEqual({ kind: "full" });
  expect(result.paths).toEqual({ kind: "paths", paths: ["src/a.ts"] });
  expect(result.overflow).toEqual({ kind: "full" });
  expect(result.missed).toEqual({ kind: "full" });
  expect(result.one).toEqual({ revision: 3, scope: "status", paths: ["a.ts", "b.ts"] });
  expect(result.two).toEqual({ revision: 4, scope: "full", paths: null });
});

test("a stored snapshot paints the file list and stays unverified until a live read", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const planPath = "/src/lib/shell/status-plan.ts";
    const statePath = "/src/lib/shell/state.svelte.ts";
    const { planStatusRefresh } = await import(/* @vite-ignore */ planPath);
    const { ShellState } = await import(/* @vite-ignore */ statePath);
    const fingerprint = { head: "ref: refs/heads/main", refId: "abc", indexLen: 10, indexSum: "ff" };
    const shell: any = {
      mode: "loading", stageStatusUnverified: false, snapshot: null, statusRevision: 0,
      applySnapshot(value: { path: string; staged: { path: string }[]; revision?: number; fingerprint?: typeof fingerprint }) {
        shell.snapshot = value;
        shell.mode = "live";
        shell.statusRevision = value.revision ?? 0;
        shell.stageStatusUnverified = false;
      },
    };
    shell.showStoredStatus = ShellState.prototype.showStoredStatus.bind(shell);
    const painted = shell.showStoredStatus({
      path: "D:/quiet", branch: "main", fingerprint,
      staged: [{ path: "kept.ts", kind: "modified", previousPath: null }], unstaged: [],
    });
    const plan = planStatusRefresh({
      fingerprint: shell.snapshot.fingerprint, seen: shell.snapshot.revision,
      unverified: shell.stageStatusUnverified,
      probe: { fingerprint, revision: 0, watcherAlive: false }, note: null,
    });
    const overflow = shell.showStoredStatus({
      path: "D:/huge", branch: "main", fingerprint, overflow: true, staged: [], unstaged: [],
    });
    return {
      painted, plan, overflow,
      path: shell.snapshot.path,
      file: shell.snapshot.staged[0].path,
      revision: shell.snapshot.revision,
      unverified: shell.stageStatusUnverified,
    };
  });
  expect(result.painted).toBe(true);
  expect(result.plan).toEqual({ kind: "full" });
  expect(result.overflow).toBe(false);
  expect(result.path).toBe("D:/quiet");
  expect(result.file).toBe("kept.ts");
  expect(result.revision).toBe(0);
  expect(result.unverified).toBe(true);
});

test("status refresh keeps the next or previous unstaged file selected after removal", async ({ page }) => {
  await page.goto("/?scene=clean");
  const result = await page.evaluate(async () => {
    const modulePath = "/src/lib/shell/state.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ modulePath);
    const shell: any = {
      snapshot: null,
      repos: [],
      selectedPath: null,
      selectedSide: "unstaged",
      fileQuery: "",
      conflictsOnly: false,
      statusGen: 0,
      statusRevision: 0,
      statusObservedRevision: 0,
      stageStatusUnverified: false,
      contextRevision: 0,
      watchQueued: null,
      watchRevision: 0,
      repoWatchNotes: new Map(),
      mode: "loading",
      loadError: "",
      viewing: () => true,
      jobKey: (path: string) => path.toLowerCase(),
      scheduleStageRefresh: () => {},
    };
    shell.applySnapshot = ShellState.prototype.applySnapshot.bind(shell);
    const status = (unstaged: string[], revision: number) => ({
      path: "D:/perf/one",
      branch: "main",
      staged: [],
      unstaged: unstaged.map((path) => ({ path, kind: "modified", previousPath: null })),
      revision,
    });
    shell.applySnapshot(status(["a.txt", "b.txt", "c.txt"], 1));
    shell.selectedPath = "b.txt";
    shell.applySnapshot(status(["a.txt", "c.txt"], 2));
    const next = shell.selectedPath;
    shell.applySnapshot(status(["a.txt"], 3));
    return { next, previous: shell.selectedPath };
  });
  expect(result).toEqual({ next: "c.txt", previous: "a.txt" });
});
