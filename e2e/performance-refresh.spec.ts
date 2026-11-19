import { expect, test } from "@playwright/test";

test("automatic fetch waits for idle and a foreground sync cancels the pending timer", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const modulePath = "/src/lib/shell/state.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ modulePath);
    const shell: any = {
      automaticFetchTimer: null, openGen: 1, settings: { fetchAutomatically: true },
      repoPath: () => "D:/perf/one", viewing: () => true,
    };
    for (const method of ["scheduleAutomaticFetch", "cancelAutomaticFetch"] as const) shell[method] = ShellState.prototype[method].bind(shell);
    let calls = 0;
    shell.doFetch = () => { calls++; };
    shell.scheduleAutomaticFetch();
    const immediate = calls;
    shell.cancelAutomaticFetch();
    await new Promise((resolve) => setTimeout(resolve, 1100));
    const cancelled = calls;
    shell.scheduleAutomaticFetch();
    shell.openGen++;
    await new Promise((resolve) => setTimeout(resolve, 1100));
    const switched = calls;
    shell.scheduleAutomaticFetch();
    await new Promise((resolve) => setTimeout(resolve, 1100));
    return { immediate, cancelled, switched, idle: calls };
  });
  expect(result).toEqual({ immediate: 0, cancelled: 0, switched: 0, idle: 1 });
});

test("automatic fetch waits for refs, coalesces triggers, and uses the tracking remote", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const modulePath = "/src/lib/shell/state.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ modulePath);
    const shell: any = {
      automaticFetchTimer: null, automaticFetchStates: new Map(), openGen: 1,
      settings: { fetchAutomatically: true, fetchAllRemotes: false, fetchPrune: true, fetchTags: false },
      snapshot: { path: "D:/perf/one" }, mode: "live", refs: null,
      busy: true, refreshing: false, userWrites: 0, dialog: null, backgroundFetch: new Set(),
      repoPath: () => "D:/perf/one", viewing: (path: string) => path === "D:/perf/one",
      replayWatch: () => {},
      mutate: async (request: Record<string, unknown>) => { requests.push(request); return "completed"; },
    };
    const requests: Record<string, unknown>[] = [];
    for (const method of ["automaticFetchState", "automaticFetchRemote", "automaticFetchBackoff", "requestAutomaticFetch", "scheduleAutomaticFetch", "cancelAutomaticFetch", "doFetch", "noteAutomaticFetchAttempt", "noteAutomaticFetchResult", "jobKey"] as const) {
      shell[method] = ShellState.prototype[method].bind(shell);
    }
    shell.scheduleAutomaticFetch = () => ShellState.prototype.scheduleAutomaticFetch.call(shell, 5);
    shell.requestAutomaticFetch("ready");
    const pendingWithoutRefs = shell.automaticFetchStates.get("d:/perf/one")?.pending === true;
    shell.refs = {
      branches: [{ name: "main", current: true, upstream: "origin/main", ahead: 0, behind: 0 }],
      remotes: [{ name: "origin" }, { name: "mirror" }], tags: [], stashes: [], submodules: [], worktrees: [],
    };
    shell.requestAutomaticFetch("ready");
    const pendingWhileBusy = shell.automaticFetchStates.get("d:/perf/one")?.pending === true;
    shell.busy = false;
    shell.requestAutomaticFetch("idle");
    shell.requestAutomaticFetch("focus");
    await new Promise((resolve) => setTimeout(resolve, 30));
    const completedState = { ...shell.automaticFetchStates.get("d:/perf/one") };
    const retryDelays: number[] = [];
    shell.scheduleAutomaticFetch = (delay: number) => {
      retryDelays.push(delay);
      shell.automaticFetchTimer = 1;
    };
    shell.noteAutomaticFetchAttempt("d:/perf/one");
    shell.noteAutomaticFetchResult("d:/perf/one", "failed");
    return { pendingWithoutRefs, pendingWhileBusy, requests, completedState, retryDelays,
      failedState: shell.automaticFetchStates.get("d:/perf/one") };
  });
  expect(result.pendingWithoutRefs).toBe(true);
  expect(result.pendingWhileBusy).toBe(true);
  expect(result.requests).toEqual([{ action: "fetch", remote: "origin", prune: true, tags: false }]);
  expect(result.completedState.failures).toBe(0);
  expect(result.retryDelays).toEqual([60_000]);
  expect(result.failedState.failures).toBe(1);
  expect(result.failedState.pending).toBe(true);
});

test("background fetch defers hidden history and refreshes it when the history view is active", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const modulePath = "/src/lib/shell/state.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ modulePath);
    const calls: unknown[] = [];
    const shell: any = {
      auxiliaryTimer: null, auxiliaryAreas: new Set(), auxiliaryDeferHistory: false,
      historyRefreshPending: false, section: "changes", standalone: "", allBranches: false, graphBranches: [],
      openGen: 1, mode: "live", busy: false, refreshing: false, backgroundFetch: new Set(),
      repoPath: () => "D:/perf/one", viewing: () => true, jobKey: (path: string) => path.toLowerCase(),
      loadContext: async (_operation: string | undefined, parts: unknown) => { calls.push(parts); },
      message: String, actionError: null,
    };
    shell.scheduleAuxiliary = ShellState.prototype.scheduleAuxiliary.bind(shell);
    shell.refreshPendingHistory = ShellState.prototype.refreshPendingHistory.bind(shell);
    shell.loadContext = async (_operation: string | undefined, parts: unknown) => { calls.push(parts); };
    shell.scheduleAuxiliary(["refs"], "automatic", true);
    await new Promise((resolve) => setTimeout(resolve, 280));
    const hidden = { calls: [...calls], pending: shell.historyRefreshPending };
    shell.section = "history";
    shell.refreshPendingHistory();
    await Promise.resolve();
    return { hidden, calls, pending: shell.historyRefreshPending };
  });
  expect(result.hidden.calls).toEqual([{ history: false, flow: false, deferHistory: true }]);
  expect(result.hidden.pending).toBe(true);
  expect(result.calls).toHaveLength(2);
  expect(result.calls[1]).toEqual({ history: true, flow: false });
});

test("covered write events reuse snapshots while external and unversioned changes refresh", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const modulePath = "/src/lib/shell/state.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ modulePath);
    const shell: any = {
      snapshot: null, repos: [], selectedPath: null, statusGen: 0, contextRevision: 0,
      watchQueued: null, watchRevision: 0, refreshing: false, busy: false,
      backgroundFetch: new Set(),
    };
    for (const method of ["applySnapshot", "queueWatch", "replayWatch", "watchCovered", "jobKey", "repoPath", "viewing"] as const) shell[method] = ShellState.prototype[method].bind(shell);
    const value = { path: "D:/perf/one", branch: "main", staged: [], unstaged: [], revision: 10 };
    shell.applySnapshot(value);
    const retained = shell.snapshot;
    shell.applySnapshot({ ...value, revision: 12 });
    shell.contextRevision = 12;
    const scopes: string[] = [];
    shell.refresh = async (scope = "full") => { scopes.push(scope); };
    shell.queueWatch("full", 11);
    shell.replayWatch();
    const covered = scopes.length === 0 && retained === shell.snapshot && shell.statusRevision === 12;
    shell.queueWatch("status", 13);
    shell.queueWatch("full", 14);
    shell.replayWatch();
    shell.queueWatch("status", 0);
    shell.queueWatch("status", 11);
    shell.replayWatch();
    shell.queueWatch("full", 11);
    shell.queueWatch("status", 0);
    shell.replayWatch();
    const key = shell.jobKey(shell.repoPath());
    shell.backgroundFetch.add(key);
    shell.queueWatch("full", 15);
    shell.replayWatch();
    const queuedDuringFetch = shell.watchQueued === "full" && scopes.length === 3;
    shell.backgroundFetch.delete(key);
    shell.replayWatch();
    shell.applySnapshot({ ...value, path: "D:/perf/two", revision: 20 });
    return { covered, scopes, queuedDuringFetch, reset: shell.contextRevision === 0 && shell.watchQueued === null };
  });
  expect(result).toEqual({ covered: true, scopes: ["full", "status", "full", "full"], queuedDuringFetch: true, reset: true });
});

for (const switchRepo of [false, true]) {
  test(`context reads ${switchRepo ? "reject late results after switching repositories" : "share matching requests and keep trace attribution"}`, async ({ page }) => {
    const calls: { cmd: string; path: string; trace: string }[] = [];
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd, args } = route.request().postDataJSON();
      if (!args.path?.startsWith("D:/perf/")) return route.continue();
      calls.push({ cmd, path: args.path, trace: args.__perf });
      if (cmd === "repository_refs" && args.path.endsWith("one")) await new Promise((resolve) => setTimeout(resolve, 150));
      const revision = args.path.endsWith("two") ? 20 : 10;
      const value = cmd === "repository_refs" ? {
        revision, head: `head-${revision}`, branches: [], remotes: [], tags: [], stashes: [], submodules: [], worktrees: [], hiddenRefs: [], expandedGroups: [],
      } : cmd === "commit_log" ? [] : null;
      await route.fulfill({ json: { ok: true, value } });
    });
    await page.goto("/?scene=clean");
    await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
    const result = await page.evaluate(async (switchRepo) => {
      const modulePath = "/src/lib/shell/state.svelte.ts";
      const historyPath = "/src/lib/shell/history.svelte.ts";
      const { ShellState } = await import(/* @vite-ignore */ modulePath);
      const { loadContext } = await import(/* @vite-ignore */ historyPath);
      const shell: any = {
        snapshot: null, repos: [], selectedPath: null, statusGen: 0, contextGen: 0, openGen: 0,
        settings: { commitSort: "date", signOff: false }, commitsLive: [], mergeState: null,
        sideView: "changes", commitSearch: "", historyFilter: "", logLimit: 500, allBranches: true,
        mergeProgressAutoSummary: "", exactRefs: () => [],
      };
      for (const method of ["applySnapshot", "repoPath", "jobKey", "viewing", "same", "loadCommits"] as const) shell[method] = ShellState.prototype[method].bind(shell);
      shell.focusPath = "D:/perf/one";
      shell.applySnapshot({ path: shell.focusPath, staged: [], unstaged: [], revision: 10 });
      const first = loadContext(shell, "operation-test");
      const shared = loadContext(shell, "operation-test");
      const same = first === shared;
      if (switchRepo) {
        shell.focusPath = "D:/perf/two";
        shell.applySnapshot({ path: shell.focusPath, staged: [], unstaged: [], revision: 20 });
        await loadContext(shell, "operation-new");
      }
      await Promise.all([first, shared]);
      return { same, revision: shell.contextRevision, head: shell.refs?.head };
    }, switchRepo);
    expect(result).toEqual({ same: true, revision: switchRepo ? 20 : 10, head: switchRepo ? "head-20" : "head-10" });
    expect(calls.filter(({ cmd }) => cmd === "repository_refs")).toHaveLength(switchRepo ? 2 : 1);
    const logs = calls.filter(({ cmd }) => cmd === "commit_log");
    expect(logs).toHaveLength(1);
    expect(logs[0].path).toBe(switchRepo ? "D:/perf/two" : "D:/perf/one");
    expect(logs[0].trace).toMatch(switchRepo ? /^operation-new-/ : /^operation-test-/);
  });
}
