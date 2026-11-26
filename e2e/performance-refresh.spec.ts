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
    expect(logs.map(({ path }) => path).sort()).toEqual(switchRepo ? ["D:/perf/one", "D:/perf/two"] : ["D:/perf/one"]);
    for (const log of logs) expect(log.trace).toMatch(log.path.endsWith("two") ? /^operation-new-/ : /^operation-test-/);
  });
}

test("a status snapshot does not discard history for the open repository", async ({ page }) => {
  await page.route("**/__lab/invoke", async (route) => {
    const { cmd, args } = route.request().postDataJSON();
    if (!args.path?.startsWith("D:/perf/")) return route.continue();
    if (cmd === "repository_refs") await new Promise((resolve) => setTimeout(resolve, 80));
    const row = { id: "c1", shortId: "c1", summary: "kept", author: "A", email: "a@e", at: 1, when: "now", parents: [], refs: [], lane: 0, unpushed: false, remoteOnly: false };
    const value = cmd === "repository_refs" ? {
      revision: 10, head: "c1", branches: [], remotes: [], tags: [], stashes: [], submodules: [], worktrees: [], hiddenRefs: [], expandedGroups: [],
    } : cmd === "commit_log" ? [row] : null;
    await route.fulfill({ json: { ok: true, value } });
  });
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const modulePath = "/src/lib/shell/state.svelte.ts";
    const historyPath = "/src/lib/shell/history.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ modulePath);
    const { loadContext } = await import(/* @vite-ignore */ historyPath);
    const shell: any = {
      snapshot: null, repos: [], selectedPath: null, statusGen: 0, contextGen: 0, openGen: 0,
      settings: { commitSort: "date", signOff: false }, commitsLive: [], mergeState: null,
      sideView: "changes", commitSearch: "", historyFilter: "", logLimit: 500, allBranches: true,
      historyFillPending: false, historyLoading: false, actionError: null, contextRevision: 0, mergeProgressAutoSummary: "",
      section: "changes", standalone: "", mode: "live", busy: false, reflogOn: false,
      exactRefs: () => [], tr: String, message: String,
    };
    for (const method of ["applySnapshot", "repoPath", "jobKey", "viewing", "same", "loadCommits"] as const) shell[method] = ShellState.prototype[method].bind(shell);
    shell.focusPath = "D:/perf/history-keep";
    shell.applySnapshot({ path: shell.focusPath, staged: [], unstaged: [], revision: 10 });
    const pending = loadContext(shell, "history-keep");
    await new Promise((resolve) => setTimeout(resolve, 20));
    shell.applySnapshot({ path: shell.focusPath, staged: [{ path: "dirty.txt" }], unstaged: [], revision: 11 });
    await pending;
    return { ids: shell.commitsLive.map((row: { id: string }) => row.id), statusGen: shell.statusGen };
  });
  expect(result).toEqual({ ids: ["c1"], statusGen: 2 });
});

test("a repository switch paints one history page before older pages stream in", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const historyPath = "/src/lib/shell/history.svelte.ts";
    const statePath = "/src/lib/shell/state.svelte.ts";
    const { loadContext } = await import(/* @vite-ignore */ historyPath);
    const { ShellState } = await import(/* @vite-ignore */ statePath);
    const total = 2500;
    const row = (index: number) => ({ id: `c${index}`, shortId: `c${index}`, summary: `commit ${index}`, author: "A", email: "a@e",
      at: index, when: "now", parents: index + 1 < total ? [`c${index + 1}`] : [], refs: [], lane: 0, unpushed: false, remoteOnly: false });
    const calls: { cmd: string; limit: number; offset: number; prefetch: boolean }[] = [];
    const original = window.fetch;
    window.fetch = async (input, init) => {
      const body = typeof init?.body === "string" ? JSON.parse(init.body) : {};
      const cmd = body.cmd ?? "";
      if (!["repository_refs", "commit_log", "in_progress", "merge_state"].includes(cmd)) return original(input, init);
      let value: unknown = null;
      if (cmd === "repository_refs") value = { revision: 7, head: "c0", branches: [], remotes: [], tags: [], stashes: [], submodules: [], worktrees: [], hiddenRefs: [], expandedGroups: [] };
      if (cmd === "commit_log") {
        const limit = Number(body.args?.limit ?? 0), offset = Number(body.args?.offset ?? 0);
        calls.push({ cmd, limit, offset, prefetch: body.args?.__prefetch === true });
        value = Array.from({ length: Math.max(0, Math.min(limit, total - offset)) }, (_, index) => row(offset + index));
      }
      return new Response(JSON.stringify({ ok: true, value }), { headers: { "content-type": "application/json" } });
    };
    const shell: any = {
      snapshot: { path: "D:/perf/history", staged: [], unstaged: [], revision: 10 }, repos: [], selectedPath: null,
      statusGen: 0, contextGen: 0, openGen: 0, settings: { commitSort: "date", signOff: false },
      commitsLive: [], mergeState: null, progress: null, sideView: "changes", commitSearch: "",
      commitSearchField: "message", commitSearchScope: "repository", historyFilter: "", logLimit: total,
      allBranches: true, section: "history", standalone: "", mode: "live", busy: false, reflogOn: false,
      historyFillPending: false, actionError: null, contextRevision: 0, mergeProgressAutoSummary: "",
      exactRefs: () => [], tr: String, message: String,
    };
    for (const method of ["repoPath", "jobKey", "viewing", "same", "loadCommits"] as const) shell[method] = ShellState.prototype[method].bind(shell);
    try {
      await loadContext(shell, "history-page");
      const deadline = performance.now() + 3000;
      while ((shell.commitsLive.length < total || calls.length < 2) && performance.now() < deadline) await new Promise((resolve) => setTimeout(resolve, 20));
      return { calls, length: shell.commitsLive.length, pending: shell.historyFillPending };
    } finally {
      window.fetch = original;
    }
  });
  expect(result.calls).toEqual([
    { cmd: "commit_log", limit: 500, offset: 0, prefetch: false },
    { cmd: "commit_log", limit: 2000, offset: 500, prefetch: true },
  ]);
  expect(result.length).toBe(2500);
  expect(result.pending).toBe(false);
});

test("hidden history waits for the view before streaming older pages", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const historyPath = "/src/lib/shell/history.svelte.ts";
    const statePath = "/src/lib/shell/state.svelte.ts";
    const { loadContext, queueHistoryFill } = await import(/* @vite-ignore */ historyPath);
    const { ShellState } = await import(/* @vite-ignore */ statePath);
    const total = 2500;
    const row = (index: number) => ({ id: `c${index}`, shortId: `c${index}`, summary: `commit ${index}`, author: "A", email: "a@e",
      at: index, when: "now", parents: index + 1 < total ? [`c${index + 1}`] : [], refs: [], lane: 0, unpushed: false, remoteOnly: false });
    const calls: { limit: number; offset: number }[] = [];
    const original = window.fetch;
    window.fetch = async (input, init) => {
      const body = typeof init?.body === "string" ? JSON.parse(init.body) : {};
      const cmd = body.cmd ?? "";
      if (!["repository_refs", "commit_log", "in_progress", "merge_state"].includes(cmd)) return original(input, init);
      let value: unknown = null;
      if (cmd === "repository_refs") value = { revision: 7, head: "c0", branches: [], remotes: [], tags: [], stashes: [], submodules: [], worktrees: [], hiddenRefs: [], expandedGroups: [] };
      if (cmd === "commit_log") {
        const limit = Number(body.args?.limit ?? 0), offset = Number(body.args?.offset ?? 0);
        calls.push({ limit, offset });
        value = Array.from({ length: Math.max(0, Math.min(limit, total - offset)) }, (_, index) => row(offset + index));
      }
      return new Response(JSON.stringify({ ok: true, value }), { headers: { "content-type": "application/json" } });
    };
    const shell: any = {
      snapshot: { path: "D:/perf/history", staged: [], unstaged: [], revision: 10 }, repos: [], selectedPath: null,
      statusGen: 0, contextGen: 0, openGen: 0, settings: { commitSort: "date", signOff: false },
      commitsLive: [], mergeState: null, progress: null, sideView: "changes", commitSearch: "",
      commitSearchField: "message", commitSearchScope: "repository", historyFilter: "", logLimit: total,
      allBranches: true, section: "changes", standalone: "", mode: "live", busy: false, reflogOn: false,
      historyFillPending: false, actionError: null, contextRevision: 0, mergeProgressAutoSummary: "",
      exactRefs: () => [], tr: String, message: String,
    };
    for (const method of ["repoPath", "jobKey", "viewing", "same", "loadCommits"] as const) shell[method] = ShellState.prototype[method].bind(shell);
    try {
      await loadContext(shell, "history-hidden");
      await new Promise((resolve) => setTimeout(resolve, 400));
      const hidden = { calls: [...calls], length: shell.commitsLive.length, pending: shell.historyFillPending };
      shell.section = "history";
      queueHistoryFill(shell);
      const deadline = performance.now() + 3000;
      while (shell.commitsLive.length < total && performance.now() < deadline) await new Promise((resolve) => setTimeout(resolve, 20));
      return { hidden, calls, length: shell.commitsLive.length, pending: shell.historyFillPending };
    } finally {
      window.fetch = original;
    }
  });
  expect(result.hidden).toEqual({ calls: [{ limit: 500, offset: 0 }], length: 500, pending: true });
  expect(result.calls).toEqual([{ limit: 500, offset: 0 }, { limit: 2000, offset: 500 }]);
  expect(result.length).toBe(2500);
  expect(result.pending).toBe(false);
});
