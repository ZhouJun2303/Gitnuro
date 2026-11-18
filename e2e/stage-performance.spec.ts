import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "@playwright/test";
import { pinLabSettings } from "./helpers";

const repo = resolve(".lab/repos/stage-many");
const baseline = process.env.AWEGIT_UI_AUDIT_BASELINE === "1";

test.beforeAll(() => {
  execFileSync(process.execPath, ["scripts/lab-seed.mjs", "stage-many"], { stdio: "pipe" });
});

for (const theme of ["apple", "light", "dark"]) {
  test(`many changes stage and unstage files and folders without blocking the list (${theme})`, async ({ page }) => {
    test.setTimeout(90_000);
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, statusInterval: 0, fileLayout: "tree" });
    // Also allows the same scenario to measure an older backend binary.
    await page.route("**/__lab/invoke", async (route) => {
      if (route.request().postDataJSON().cmd !== "lab_layout") return route.fallback();
      const response = await route.fetch();
      const payload = await response.json();
      payload.value["stage-many"] = repo;
      return route.fulfill({ json: payload });
    });
    await page.goto(`/?scene=stage-many&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="stage-many"]')).toBeVisible();
    await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
    await page.evaluate(() => {
      (window as any).__stageLongTasks = [];
      new PerformanceObserver((list) => {
        (window as any).__stageLongTasks.push(...list.getEntries().map((entry) => entry.duration));
      }).observe({ entryTypes: ["longtask"] });
    });
    const requests: string[] = [];
    page.on("request", (request) => {
      if (request.url().includes("/__lab/invoke") && request.method() === "POST") requests.push(request.postDataJSON().cmd);
    });
    const unstaged = page.locator(".unstaged-pane");
    const staged = page.locator(".staged-pane");
    const file = "loose/file-00000.txt";
    const samples: { action: string; ms: number; calls: string[]; longTasks: number[] }[] = [];
    for (const [action, source, destination, path] of [
      ["stage file", unstaged, staged, file],
      ["unstage file", staged, unstaged, file],
      ["stage folder", unstaged, staged, "loose"],
      ["unstage folder", staged, unstaged, "loose"],
    ] as const) {
      requests.length = 0;
      await page.evaluate(() => { (window as any).__stageLongTasks = []; });
      const start = performance.now();
      await source.locator(`.file-select[title="${path}"]`).dblclick();
      await expect(destination.locator(`.file-select[title="${path}"]`)).toBeVisible();
      await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
      samples.push({ action, ms: performance.now() - start, calls: [...requests], longTasks: await page.evaluate(() => (window as any).__stageLongTasks) });
      expect(await unstaged.locator(".file").count()).toBeLessThan(200);
      expect(await staged.locator(".file").count()).toBeLessThan(200);
    }
    if (!baseline) {
      await unstaged.locator('.file-select[title="loose/file-00001.txt"]').click();
      await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
      let release!: () => void;
      let reading!: () => void;
      let blocked = false;
      const gate = new Promise<void>((done) => { release = done; });
      const started = new Promise<void>((done) => { reading = done; });
      await page.route("**/__lab/invoke", async (route) => {
        const body = route.request().postDataJSON();
        if (body.cmd !== "file_diff" || body.args.file !== file || body.args.staged || blocked) return route.fallback();
        blocked = true;
        const response = await route.fetch();
        reading();
        await gate;
        return route.fulfill({ response });
      });
      await unstaged.locator(`.file-select[title="${file}"]`).click();
      await started;
      try {
        await unstaged.locator('.file-select[title="loose"]').dblclick();
        await expect(staged.locator('.file-select[title="loose"]')).toBeVisible();
        const list = unstaged.locator(".file-list");
        await list.evaluate((element) => { element.scrollTop = 3000; });
        await expect.poll(() => list.evaluate((element) => element.scrollTop)).toBeGreaterThan(1000);
        await expect(list.locator(".file").first()).toBeVisible();
        expect(await list.locator(".file").count()).toBeLessThan(200);
      } finally { release(); }
      await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
      await staged.locator('.file-select[title="loose"]').dblclick();
      await expect(staged.locator('.file-select[title="loose"]')).toHaveCount(0);
      await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
      await unstaged.locator(".file-list").evaluate((element) => { element.scrollTop = 0; });
    }
    expect(execFileSync("git", ["diff", "--cached", "--name-only"], { cwd: repo, encoding: "utf8" })).toBe("");
    expect(execFileSync("git", ["ls-files", "--others", "--exclude-standard", "-z"], { cwd: repo, encoding: "utf8" }).split("\0").filter(Boolean)).toHaveLength(5000);
    expect(execFileSync("git", ["diff", "--name-only"], { cwd: repo, encoding: "utf8" }).trim().split("\n")).toHaveLength(5000);
    expect(errors).toEqual([]);
    const directory = `.lab/ui-audit/${process.env.AWEGIT_UI_AUDIT_RUN || "current"}`;
    mkdirSync(directory, { recursive: true });
    writeFileSync(`${directory}/stage-many-${theme}.json`, JSON.stringify(samples, null, 2));
    await page.screenshot({ path: `${directory}/stage-many-${theme}.png` });
    console.log(theme, JSON.stringify(samples));
  });
}

test("folder staging starts while an obsolete diff is still reading", async ({ page }) => {
  test.setTimeout(60_000);
  await pinLabSettings(page, { fetchAutomatically: false, statusInterval: 0 });
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const path = "/src/lib/shell/state.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ path);
    let writes = 0;
    let release!: () => void;
    const gate = new Promise<void>((done) => { release = done; });
    const shell: any = {
      mode: "live", path: "D:/stage/perf", snapshot: { path: "D:/stage/perf", staged: [], unstaged: [] },
      busy: true, switching: false, userWrites: 0, currentJob: { label: "chrome.readingFile", depth: 1 },
      backend: () => true, repoPath() { return this.path; }, tr: String, viewing: () => true,
      jobKey: () => "repo", diffReads: { repo: 1 },
      waitForWriteSlot: () => gate, applySnapshot() {}, loadDiff: () => gate, needsContext: () => false,
      describeRequest: () => ({ title: "stage" }), startWork() {}, stopWork() {}, settings: {},
      noteActivity() {}, recordCommand() {}, replayWatch() {}, message: String,
      contextActions: new Set(), quietGit: new Set(), cancelAutomaticFetch() {},
      patchExpectation: () => undefined, statusGen: 0,
    };
    shell.onlyDiffReads = ShellState.prototype.onlyDiffReads.bind(shell);
    const original = window.fetch;
    window.fetch = async (input, init) => {
      const body = typeof init?.body === "string" ? JSON.parse(init.body) : null;
      if (body?.args?.path !== shell.path) return original(input, init);
      writes++;
      return new Response(JSON.stringify({ ok: true, value: { path: shell.path, staged: [], unstaged: [], outcome: "completed", output: "", log: "" } }));
    };
    try {
      const pending = ShellState.prototype.mutate.call(shell, { action: "stagePaths", files: ["folder/a.txt"], unstage: false });
      await new Promise((resolve) => setTimeout(resolve, 30));
      const immediate = writes;
      release();
      await pending;
      return immediate;
    } finally { window.fetch = original; }
  });
  if (!baseline) expect(result).toBe(1);
  console.log("folder writes before diff settles:", result);
});

test("stage write waiters ignore previews but keep waiting for index writes", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const path = "/src/lib/shell/state.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ path);
    const shell: any = { jobs: {}, diffReads: {}, writeWaiters: [], switching: false,
      repoPath: () => "repo", jobKey: () => "repo", tr: String,
      get currentJob() { return this.jobs.repo; },
      get busy() { return (this.currentJob?.depth ?? 0) > 0; },
    };
    for (const method of ["startWork", "stopWork", "withWork", "waitForWriteSlot", "releaseWriteWaiters", "onlyDiffReads"]) shell[method] = ShellState.prototype[method].bind(shell);
    let release!: () => void;
    const gate = new Promise<void>((done) => { release = done; });
    shell.startWork("write");
    const preview = shell.withWork("chrome.readingFile", () => gate);
    let stages = 0, exclusive = 0;
    const stage = shell.waitForWriteSlot(true).then(() => { stages++; });
    const other = shell.waitForWriteSlot().then(() => { exclusive++; });
    await new Promise((done) => setTimeout(done, 0));
    const whileWriting = { stages, exclusive };
    shell.stopWork();
    await new Promise((done) => setTimeout(done, 0));
    const whileReading = { stages, exclusive, label: shell.currentJob.label, onlyReads: shell.onlyDiffReads() };
    release();
    await Promise.all([preview, stage, other]);
    return { whileWriting, whileReading, finished: { stages, exclusive }, reads: shell.diffReads };
  });
  expect(result).toEqual({ whileWriting: { stages: 0, exclusive: 0 },
    whileReading: { stages: 1, exclusive: 0, label: "write", onlyReads: true },
    finished: { stages: 1, exclusive: 1 }, reads: {} });
});

test("status refresh retains unchanged panes and selection lookup reuses the file index", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const statePath = "/src/lib/shell/state.svelte.ts";
    const diffPath = "/src/lib/shell/diff.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ statePath);
    const { pickedIn } = await import(/* @vite-ignore */ diffPath);
    const unstaged = Array.from({ length: 20_000 }, (_, i) => ({ path: `loose/${i}.txt`, kind: "untracked" }));
    const shell: any = { snapshot: null, repos: [], statusGen: 0, selectedPath: null };
    for (const method of ["viewing", "jobKey"]) shell[method] = ShellState.prototype[method].bind(shell);
    ShellState.prototype.applySnapshot.call(shell, { path: "repo", staged: [], unstaged, revision: 1 });
    ShellState.prototype.applySnapshot.call(shell, { path: "repo", staged: [{ path: "tracked.txt", kind: "modified" }], unstaged: structuredClone(unstaged), revision: 2 });
    const retained = shell.snapshot.unstaged === unstaged;
    let reads = 0;
    const rows = unstaged.map((file) => ({ get path() { reads++; return file.path; } }));
    const selection = { picked: ["loose/0.txt"] };
    pickedIn(selection, rows);
    const initialReads = reads;
    for (let i = 0; i < 20; i++) { selection.picked = [`loose/${i}.txt`]; pickedIn(selection, rows); }
    return { retained, revision: shell.statusRevision, initialReads, repeatedReads: reads - initialReads };
  });
  expect(result).toEqual({ retained: true, revision: 2, initialReads: 20_000, repeatedReads: 0 });
});

test("large stage projections retain the last intent across a burst", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const samples = await page.evaluate(async () => {
    const path = "/src/lib/shell/staging.ts";
    const { projectStages } = await import(/* @vite-ignore */ path);
    const files = Array.from({ length: 10_000 }, (_, index) => `file-${String(index).padStart(5, "0")}.txt`);
    const snapshot = { path: "/repo", branch: "main", staged: [], unstaged: files.map((path) => ({ path, kind: "untracked", previousPath: null })) };
    return [1, 4, 12, 24].map((pending) => {
      const changes = Array.from({ length: pending }, (_, id) => ({ id, repo: "/repo", files: files.slice(), unstage: id % 2 === 1 }));
      let staged = -1, count = -1;
      const times = Array.from({ length: 20 }, () => {
        const start = performance.now();
        const result = projectStages(snapshot, changes);
        staged = result.staged.length;
        count = staged + result.unstaged.length;
        return performance.now() - start;
      }).sort((a, b) => a - b);
      return { pending, p95Ms: times[18], staged, count };
    });
  });
  expect(samples.map(({ staged, count }) => [staged, count])).toEqual([[10_000, 10_000], [0, 10_000], [0, 10_000], [0, 10_000]]);
  console.log("stage projection 10k:", JSON.stringify(samples));
});

test("partial status merges directory and rename boundaries without covering external events", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const path = "/src/lib/shell/state.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ path);
    let scheduled = 0;
    const outside = { path: "outside.txt", kind: "modified" };
    const shell: any = { snapshot: null, repos: [], statusGen: 0, selectedPath: "dir/a.txt", selectedSide: "unstaged",
      scheduleStageRefresh: () => { scheduled++; }, contextRevision: 0,
    };
    for (const method of ["viewing", "jobKey"]) shell[method] = ShellState.prototype[method].bind(shell);
    const apply = ShellState.prototype.applySnapshot.bind(shell);
    apply({ path: "repo", revision: 7,
      staged: [{ path: "kept.txt", kind: "modified" }, { path: "renamed.txt", kind: "renamed", previousPath: "old.txt" }],
      unstaged: [{ path: "dir/a.txt", kind: "modified" }, { path: "dir/sub/b.txt", kind: "untracked" }, { path: "dir2/c.txt", kind: "modified" }, outside],
    });
    apply({ path: "repo", revision: 20, statusPaths: ["dir"], staged: [{ path: "dir/a.txt", kind: "modified" }], unstaged: [{ path: "dir/sub/b.txt", kind: "untracked" }] });
    const directory = { staged: shell.snapshot.staged.map((file: any) => file.path), unstaged: shell.snapshot.unstaged.map((file: any) => file.path),
      outsideRetained: shell.snapshot.unstaged.at(-1) === outside, side: shell.selectedSide,
    };
    apply({ path: "repo", revision: 21, statusPaths: ["old.txt"], staged: [], unstaged: [{ path: "old.txt", kind: "deleted" }] });
    const renameRemoved = !shell.snapshot.staged.some((file: any) => file.path === "renamed.txt");
    const coveredExternal = ShellState.prototype.watchCovered.call(shell, "status", 8);
    const partialRevision = shell.statusRevision;
    apply({ path: "repo", revision: 22, staged: [], unstaged: [{ ...outside, kind: "deleted" }] });
    apply({ path: "repo", revision: 20, statusPaths: ["dir/a.txt"], staged: [{ path: "dir/a.txt", kind: "modified" }], unstaged: [] });
    return { directory, renameRemoved, coveredExternal, partialRevision, scheduled, fullRevision: shell.statusRevision, externalKind: shell.snapshot.unstaged[0].kind };
  });
  expect(result).toEqual({ directory: { staged: ["dir/a.txt", "kept.txt", "renamed.txt"], unstaged: ["dir/sub/b.txt", "dir2/c.txt", "outside.txt"], outsideRetained: true, side: "staged" },
    renameRemoved: true, coveredExternal: false, partialRevision: 7, scheduled: 3, fullRevision: 22, externalKind: "deleted" });
});

test("stage validation coalesces bursts and cancels after a repository switch or full snapshot", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const path = "/src/lib/shell/state.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ path);
    const queued: unknown[] = [];
    const shell: any = { stageRefreshTimer: null, openGen: 0, repoPath: () => "repo", viewing: () => true,
      queueWatch: (...args: unknown[]) => { queued.push(args); }, replayWatch() {}, snapshot: null, repos: [], statusGen: 0, selectedPath: null,
    };
    const schedule = ShellState.prototype.scheduleStageRefresh.bind(shell);
    schedule(); schedule(); schedule();
    await new Promise((done) => setTimeout(done, 350));
    schedule(); shell.openGen++;
    await new Promise((done) => setTimeout(done, 350));
    schedule();
    ShellState.prototype.applySnapshot.call(shell, { path: "repo", revision: 30, staged: [], unstaged: [] });
    await new Promise((done) => setTimeout(done, 350));
    return { queued, timer: shell.stageRefreshTimer };
  });
  expect(result).toEqual({ queued: [["status", 0]], timer: null });
});

test("partial status keeps native Git ordering for supplementary Unicode paths", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const path = "/src/lib/shell/state.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ path);
    const shell: any = { snapshot: null, repos: [], statusGen: 0, selectedPath: null, scheduleStageRefresh() {} };
    for (const method of ["viewing", "jobKey"]) shell[method] = ShellState.prototype[method].bind(shell);
    const file = (path: string) => ({ path, kind: "added" });
    const apply = ShellState.prototype.applySnapshot.bind(shell);
    apply({ path: "repo", staged: [file("\uE000.txt"), file("😀.txt")], unstaged: [] });
    apply({ path: "repo", statusPaths: ["\uE001.txt"], staged: [file("\uE001.txt")], unstaged: [] });
    apply({ path: "repo", statusPaths: ["😁.txt"], staged: [file("😁.txt")], unstaged: [] });
    return shell.snapshot.staged.map((item: any) => item.path);
  });
  expect(result).toEqual(["\uE000.txt", "\uE001.txt", "😀.txt", "😁.txt"]);
});

test("clicking the selected file reuses its loaded preview but changing sides reloads", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const path = "/src/lib/shell/diff.svelte.ts";
    const { selectFile } = await import(/* @vite-ignore */ path);
    let reads = 0;
    const shell: any = { mode: "live", selectedPath: "large.ts", selectedSide: "unstaged", blameGen: 0,
      diff: { path: "large.ts", staged: false }, shownStaged: [{ path: "large.ts" }], shownUnstaged: [{ path: "large.ts" }],
      clearTextSelection() {}, loadDiff() { reads++; }, picked: ["large.ts", "other.ts"],
    };
    selectFile(shell, "large.ts", "unstaged", { detail: 1 } as MouseEvent);
    selectFile(shell, "large.ts", "unstaged", { detail: 2 } as MouseEvent);
    const loaded = { reads, picked: shell.picked };
    shell.diff = null;
    selectFile(shell, "large.ts", "unstaged");
    const missing = reads;
    shell.diff = { path: "large.ts", staged: false };
    selectFile(shell, "large.ts", "staged");
    return { loaded, missing, changedSide: reads };
  });
  expect(result).toEqual({ loaded: { reads: 0, picked: ["large.ts"] }, missing: 1, changedSide: 2 });
});
