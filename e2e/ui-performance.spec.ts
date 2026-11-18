import { expect, test } from "@playwright/test";
import { pinLabSettings } from "./helpers";
import { mkdirSync, writeFileSync } from "node:fs";

test.beforeEach(async ({ page }) => {
  await pinLabSettings(page, { fetchAutomatically: false, statusInterval: 0 });
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
});

test("large selections clear completed files and preserve other selections without quadratic work", async ({ page }) => {
  test.setTimeout(90_000);
  const result = await page.evaluate(async () => {
    const modulePath = "/src/lib/shell/diff.svelte.ts";
    const actions = await import(/* @vite-ignore */ modulePath);
    const files = Array.from({ length: 20_000 }, (_, index) => `Assets/Generated/file-${index}.asset`);
    const samples: { action: string; ms: number; remaining: string[]; sent: number }[] = [];
    for (const action of ["stagePicked", "unstagePicked", "discardPicked", "deletePicked", "resolvePicked"]) {
      let sent = 0;
      const shell: any = { mode: "live", busy: false, writing: false, actionError: null, picked: [...files, "retained.txt"],
        mutate: async (request: { files: string[] }) => { sent = request.files.length; },
      };
      const start = performance.now();
      if (action === "resolvePicked") await actions[action](shell, "ours", files);
      else await actions[action](shell, files);
      samples.push({ action, ms: performance.now() - start, remaining: shell.picked, sent });
    }
    const rows = files.map((path) => ({ path, letter: "M", tone: "modified" }));
    const shell: any = { mode: "sample", picked: [...files, "retained.txt"], settings: {}, staged: [], unstaged: rows,
      shownUnstaged: rows, filesInDir: () => files, clearTextSelection() {}, selectedIndex: 0,
    };
    const stashStart = performance.now();
    actions.openStash(shell, files);
    const stashMs = performance.now() - stashStart;
    const stash = { included: shell.includeUntracked, count: shell.stashFiles.length };
    const folderStart = performance.now();
    actions.selectFolder(shell, "Assets", "unstaged", { ctrlKey: true });
    const folderMs = performance.now() - folderStart;
    shell.staged = [{ path: "staged-new.txt", letter: "A" }];
    shell.unstaged = [...rows, { path: "other-new.txt", letter: "?" }];
    const scopes: boolean[] = [];
    for (const selected of [[files[0]], ["other-new.txt"], ["staged-new.txt"]]) {
      actions.openStash(shell, selected);
      scopes.push(shell.includeUntracked);
    }
    shell.settings.stashIncludeUntracked = true;
    actions.openStash(shell, []);
    scopes.push(shell.includeUntracked);
    return { samples, stashMs, stash, folderMs, folderRemaining: shell.picked, scopes };
  });
  for (const sample of result.samples) expect(sample).toMatchObject({ sent: 20_000, remaining: ["retained.txt"] });
  expect(result.stash).toEqual({ included: false, count: 20_000 });
  expect(result.folderRemaining).toEqual(["retained.txt"]);
  expect(result.scopes).toEqual([false, true, true, true]);
  const directory = `.lab/ui-audit/${process.env.AWEGIT_UI_AUDIT_RUN || "current"}`;
  mkdirSync(directory, { recursive: true });
  writeFileSync(`${directory}/selections.json`, JSON.stringify(result, null, 2));
});

test("virtual lists keep their last rows when a deep list shrinks", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const path = "/src/lib/view.ts";
    const { windowSlice, virtualLines, onVirtualScroll } = await import(/* @vite-ignore */ path);
    const rows = Array.from({ length: 160 }, (_, index) => ({ text: `row-${index}` }));
    const list = windowSlice(rows, 500_000, 24);
    const diff = virtualLines(rows, 500_000, 20, 80, false);
    const writes: number[] = [];
    const scroll = onVirtualScroll((top: number) => writes.push(top));
    const old = document.createElement("div"), next = document.createElement("div");
    document.body.append(old, next);
    Object.defineProperty(old, "scrollTop", { value: 100 });
    Object.defineProperty(next, "scrollTop", { value: 200 });
    old.addEventListener("scroll", scroll); next.addEventListener("scroll", scroll);
    old.dispatchEvent(new Event("scroll")); old.remove(); next.dispatchEvent(new Event("scroll"));
    await new Promise(requestAnimationFrame);
    next.dispatchEvent(new Event("scroll")); next.remove();
    await new Promise(requestAnimationFrame);
    return { list: list.rows.at(-1)?.text, diff: diff.rows.at(-1)?.text, count: list.rows.length, writes };
  });
  expect(result).toEqual({ list: "row-159", diff: "row-159", count: 72, writes: [200] });
});

test("history refresh compares all displayed metadata and ref boundaries", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const path = "/src/lib/view.ts";
    const { sameHistory } = await import(/* @vite-ignore */ path);
    const row = { id: "abc123", shortId: "abc", summary: "subject", author: "Author", email: "a@example.com",
      at: 100, when: "now", parents: ["parent"], refs: ["a", "bc"], lane: 0, unpushed: false, remoteOnly: false };
    const patches = { shortId: "abc1", summary: "other", author: "New author", email: "b@example.com", at: 200,
      when: "yesterday", parents: ["replacement"], refs: ["ab", "c"], lane: 1, unpushed: true, remoteOnly: true };
    const changed = Object.entries(patches).filter(([field, value]) => !sameHistory([row], [{ ...row, [field]: value }])).map(([field]) => field);
    let reads = 0;
    const tail = new Proxy(row, { get(target, key, receiver) { reads++; return Reflect.get(target, key, receiver); } });
    sameHistory([row, tail], [{ ...row, id: "different" }, { ...row }]);
    return { changed, equal: sameHistory([row], [structuredClone(row)]), reads };
  });
  expect(result.equal).toBe(true);
  expect(result.changed).toEqual(["shortId", "summary", "author", "email", "at", "when", "parents", "refs", "lane", "unpushed", "remoteOnly"]);
  expect(result.reads).toBe(0);
});

for (const kind of ["compare", "reflog", "branch", "locate", "commit", "patch"] as const) {
  test(`history ${kind} reads discard obsolete results and errors`, async ({ page }) => {
    const results = await page.evaluate(async (kind) => {
      const path = "/src/lib/shell/history.svelte.ts";
      const history = await import(/* @vite-ignore */ path);
      const original = window.fetch;
      const results: { change: string; fail: boolean; stale: boolean; error: string | null }[] = [];
      try {
        for (const change of ["repository", "reopened", "selection", "current"]) for (const fail of [false, true]) {
          const row = { id: "old", shortId: "old", summary: "old", author: "a", email: "", at: 1, when: "now", parents: [], refs: ["main"], lane: 0 };
          let release!: () => void;
          const gate = new Promise<void>((resolve) => { release = resolve; });
          window.fetch = async (input, init) => {
            const body = typeof init?.body === "string" ? JSON.parse(init.body) : null;
            if (body?.args?.path !== "D:/ui-audit/old") return original(input, init);
            await gate;
            const value = body.cmd === "locate_ref" ? { id: "old", commits: [row] }
              : body.cmd === "commit_detail" ? { ...row, body: "old" }
              : body.cmd === "show_commit_file" ? { path: "old.txt", lines: [], binary: false }
              : body.cmd === "compare_files" || body.cmd === "commit_files" ? [{ path: "old.txt" }] : [row];
            return new Response(JSON.stringify(fail ? { ok: false, error: "read failed" } : { ok: true, value }));
          };
          const shell: any = {
            mode: "live", path: "D:/ui-audit/old", repoPath() { return this.path; }, viewing(path: string) { return path === this.path; },
            openGen: 0, patchGen: 0, reflogGen: 0, locateSeq: 0, reflogOn: false, reflogRows: [], actionError: null,
            drops: ["base", "old"], comparePair: null, commitFiles: [], selectedCommit: "old", historyFile: "old.txt",
            historyItems: [], commitsLive: [], settings: {}, patchCache: {}, patchVersion: 0, patchHeights: {}, openPatchSet: new Set(),
            commitSelection: { select: (id: string) => id },
            allBranches: true, refs: { branches: kind === "branch" ? [{ name: "main" }] : [], remotes: [], tags: [] },
            commitQuery: "", historyFilter: "", logLimit: 500, tr: String, message: (error: unknown) => error instanceof Error ? error.message : String(error),
            withWork: (_label: string, body: () => Promise<void>) => body(),
            resetCommitView() { this.patchGen++; }, openHistory() {}, exactRefs: () => [],
            async selectCommit(id: string) { this.selectedCommit = id; }, scrollToCommit() {}, async pickCommitFile() {},
          };
          const task = kind === "compare" ? history.compareDrops(shell) : kind === "reflog" ? history.showReflog(shell)
            : kind === "commit" ? history.selectCommit(shell, "old") : kind === "patch" ? history.pickCommitFile(shell, "old.txt")
            : history.locateRef(shell, kind === "branch" ? "main" : "old");
          if (change !== "current") {
            shell.commitFiles = [{ path: "new.txt" }]; shell.commitsLive = [{ id: "new" }]; shell.reflogRows = [{ id: "new" }];
            shell.commitDetail = { id: "new" }; shell.historyDiff = { path: "new.txt" };
            if (change === "repository") shell.path = "D:/ui-audit/new";
            if (change === "reopened") shell.openGen++;
            if (change === "selection") {
              shell.patchGen++;
              if (kind === "reflog") await history.showReflog(shell);
            }
          }
          release(); await task;
          const stale = kind === "compare" ? shell.commitFiles[0]?.path === "old.txt" : kind === "reflog" ? shell.reflogRows[0]?.id === "old"
            : kind === "commit" ? shell.commitDetail?.id === "old" || shell.commitFiles[0]?.path === "old.txt"
            : kind === "patch" ? shell.historyDiff?.path === "old.txt" : shell.commitsLive[0]?.id === "old";
          results.push({ change, fail, stale, error: shell.actionError });
        }
      } finally { window.fetch = original; }
      return results;
    }, kind);
    for (const row of results) {
      if (row.change !== "current") expect(row, `${kind} ${row.change} ${row.fail}`).toMatchObject({ stale: false, error: null });
      else if (row.fail) expect(row.error).toBe("read failed");
      else expect(row.stale).toBe(true);
    }
  });
}

test("commit patch loading stays bounded across switches without changing the selected file", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const statePath = "/src/lib/shell/state.svelte.ts", historyPath = "/src/lib/shell/history.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ statePath);
    const { ensurePatch } = await import(/* @vite-ignore */ historyPath);
    const pending: (() => void)[] = [], started: string[] = [];
    let active = 0, peak = 0;
    const shell: any = { patchQueue: [], patchLoading: new Set(), patchCache: {}, patchGen: 0,
      ensurePatch: (file: string) => new Promise<void>((resolve) => {
        started.push(file); peak = Math.max(peak, ++active);
        pending.push(() => { active--; resolve(); });
      }),
    };
    for (const name of ["requestPatch", "pumpPatches", "clearPatchQueue"]) shell[name] = ShellState.prototype[name].bind(shell);
    for (let i = 0; i < 8; i++) shell.requestPatch(`old-${i}`);
    shell.patchGen++; shell.clearPatchQueue();
    for (let i = 0; i < 6; i++) shell.requestPatch(`new-${i}`);
    while (pending.length) { pending.shift()!(); await Promise.resolve(); }
    const original = window.fetch;
    const selected = { path: "selected.txt", lines: [] };
    const state: any = { mode: "live", selectedCommit: "commit", historyFile: "selected.txt", historyDiff: selected,
      patchCache: {}, patchGen: 0, patchVersion: 0, openGen: 0, patchHeights: { "background.txt": 30 }, settings: {},
      repoPath: () => "D:/ui-audit/background", viewing: () => true, tr: String, withWork: (_: string, body: () => Promise<void>) => body(),
    };
    try {
      window.fetch = async (input, init) => {
        const body = typeof init?.body === "string" ? JSON.parse(init.body) : null;
        if (body?.args?.path !== "D:/ui-audit/background") return original(input, init);
        return new Response(JSON.stringify({ ok: true, value: { path: body.args.file, lines: [{ text: "loaded" }] } }));
      };
      await ensurePatch(state, "background.txt");
      return { peak, started, selection: state.historyFile, retained: state.historyDiff === selected,
        cached: state.patchCache["background.txt"]?.lines[0].text, height: state.patchHeights["background.txt"] ?? null };
    } finally { window.fetch = original; }
  });
  expect(result).toEqual({ peak: 4, started: ["old-0", "old-1", "old-2", "old-3", "new-0", "new-1", "new-2", "new-3", "new-4", "new-5"],
    selection: "selected.txt", retained: true, cached: "loaded", height: null });
});

test("cached split diffs update tab widths and reuse paired rows when display options change", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const path = "/src/lib/shell/diff.svelte.ts";
    const { cachedSplit } = await import(/* @vite-ignore */ path);
    const shell: any = { selectedCommit: "id", patchSplits: new Map(), settings: { showWhitespace: false, showDiffMarks: false } };
    const lines = [{ text: "-\tbefore", kind: "delete" }, { text: "+\tafter", kind: "add" }];
    const before = cachedSplit(shell, "file.ts", lines);
    shell.settings.showWhitespace = true;
    const whitespace = cachedSplit(shell, "file.ts", lines);
    shell.settings.showDiffMarks = true;
    const marked = cachedSplit(shell, "file.ts", lines);
    return { widths: [before.columns.left, whitespace.columns.left, marked.columns.left],
      reused: before.rows === whitespace.rows && whitespace.rows === marked.rows, cached: marked === cachedSplit(shell, "file.ts", lines) };
  });
  expect(result).toEqual({ widths: [14, 7, 8], reused: true, cached: true });
});

for (const theme of ["apple", "light", "dark"]) {
  test(`${theme} split handle batches moves, flushes release and cleans up on unmount`, async ({ page }) => {
    await page.goto(`/?scene=history&theme=${theme}`);
    await expect(page.locator('[data-lab-scene="history"]')).toBeVisible();
    const handle = page.locator('.history-stage > [role="separator"]');
    const originalSize = Number(await handle.getAttribute("aria-valuenow"));
    await handle.focus(); await page.keyboard.press("ArrowUp");
    await expect(handle).toHaveAttribute("aria-valuenow", String(originalSize + 8));
    const result = await page.evaluate(async () => {
      const handle = document.querySelector<HTMLElement>('.history-stage > [role="separator"]')!;
      const initial = Number(handle.getAttribute("aria-valuenow"));
      let writes = 0;
      const observer = new MutationObserver((records) => { writes += records.length; });
      observer.observe(handle, { attributes: true, attributeFilter: ["aria-valuenow"] });
      const event = (name: string, y = 300) => new PointerEvent(name, { pointerId: 4, button: 0, clientY: y, bubbles: true });
      document.body.style.cursor = "crosshair";
      handle.dispatchEvent(event("pointerdown"));
      for (let i = 1; i <= 100; i++) { window.dispatchEvent(event("pointermove", 300 - i)); await Promise.resolve(); }
      const beforeFrame = writes;
      window.dispatchEvent(event("pointerup", 200));
      await new Promise(requestAnimationFrame);
      const flushed = Number(handle.getAttribute("aria-valuenow")) - initial;
      observer.disconnect();
      handle.dispatchEvent(event("pointerdown"));
      window.dispatchEvent(event("pointermove", 220));
      document.querySelector<HTMLButtonElement>('.sidebar [data-section="changes"]')!.click();
      await new Promise(requestAnimationFrame);
      window.dispatchEvent(event("pointerup", 220));
      const cursor = document.body.style.cursor;
      document.body.style.removeProperty("cursor");
      return { beforeFrame, flushed, removed: !handle.isConnected, cursor };
    });
    expect(result).toEqual({ beforeFrame: 0, flushed: 100, removed: true, cursor: "crosshair" });
  });
}
