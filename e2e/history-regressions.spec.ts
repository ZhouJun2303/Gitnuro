import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
});

for (const kind of ["tree", "blob", "history"] as const) {
  for (const { fail, change } of [
    { fail: false, change: "repository" },
    { fail: false, change: "selection" },
    { fail: true, change: "repository" },
    { fail: true, change: "selection" },
  ] as const) {
    test(`history rejects late ${kind} ${fail ? "errors" : "results"} after ${change} changes`, async ({ page }) => {
      const result = await page.evaluate(async ({ kind, fail, change }) => {
        const modulePath = "/src/lib/shell/history.svelte.ts";
        const history = await import(/* @vite-ignore */ modulePath);
        const original = window.fetch;
        let release!: () => void;
        const pending = new Promise<Response>((resolve) => {
          release = () => resolve(new Response(JSON.stringify(fail
            ? { ok: false, error: "old error" }
            : { ok: true, value: kind === "tree" ? ["old.txt"] : kind === "blob"
              ? { text: "old content", binary: false, preview: null }
              : [{ id: "old-commit" }] }), { headers: { "content-type": "application/json" } }));
        });
        window.fetch = (input, init) => {
          const body = typeof init?.body === "string" ? JSON.parse(init.body) : null;
          return body?.args?.path === "D:/history-review/old" ? pending : original(input, init);
        };
        const shell: any = {
          mode: "live", selectedCommit: "old-commit", historyFile: "same.txt",
          treePaths: [], treeText: "", preview: null, historyFilter: "old.txt",
          commitsLive: [{ id: "current-commit" }], actionError: null,
          patchGen: 0, openGen: 0, contextGen: 0,
          path: "D:/history-review/old", repoPath() { return this.path; },
          viewing(path: string) { return path === this.path; }, message: String,
          selectCommit: async () => { throw new Error("must not select an old commit"); },
        };
        try {
          const task = kind === "tree" ? history.openHistoryTab(shell, "tree")
            : kind === "blob" ? history.openTreeFile(shell, "same.txt")
            : history.showFileHistory(shell, "old.txt");
          if (change === "repository") {
            // Switching repositories can retain the same commit id and file name.
            shell.path = "D:/history-review/new";
            shell.openGen += 1;
          } else if (kind === "history") {
            shell.historyFilter = "new.txt";
            shell.contextGen += 1;
          } else {
            shell.selectedCommit = "new-commit";
            shell.patchGen += 1;
          }
          release();
          await task;
          return { paths: shell.treePaths, text: shell.treeText, commits: shell.commitsLive, error: shell.actionError };
        } finally {
          window.fetch = original;
        }
      }, { kind, fail, change });
      expect(result).toEqual({ paths: [], text: "", commits: [{ id: "current-commit" }], error: null });
    });
  }
}

test("history graph frames match a complete build across merge boundaries", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const modulePath = "/src/lib/view.ts";
    const { commitGraph } = await import(/* @vite-ignore */ modulePath);
    const commits = Array.from({ length: 12000 }, (_, i) => ({ id: `c${i}`, parents: i < 11999 ? [`c${i + 1}`] : [] }));
    commits[1999].parents.push("c7001");
    commits[6999].parents.push("c9000");
    const complete = commitGraph(commits);
    let frame = commitGraph(commits, undefined, 2000);
    frame = commitGraph(commits, { ...frame, count: 2000 }, 7000);
    frame = commitGraph(commits, { ...frame, count: 7000 });
    return { complete, frame };
  });
  expect(result.frame).toEqual(result.complete);
});

test("history fold skips linear ancestors and reuses merge analysis when toggled", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const modulePath = "/src/lib/view.ts";
    const { foldedMerges } = await import(/* @vite-ignore */ modulePath);
    let walks = 0;
    const parents = (ids: string[]) => new Proxy(ids, {
      get(target, key, receiver) {
        if (key === Symbol.iterator) walks += 1;
        return Reflect.get(target, key, receiver);
      },
    });
    const linear = Array.from({ length: 25000 }, (_, i) => ({ id: `c${i}`, parents: parents(i < 24999 ? [`c${i + 1}`] : []) }));
    foldedMerges(linear, []);
    const linearWalks = walks;
    const commits = [
      { id: "merge", parents: parents(["main", "side"]) },
      { id: "side", parents: parents(["root"]) },
      { id: "main", parents: parents(["root"]) },
      { id: "root", parents: parents([]) },
    ];
    const closed = foldedMerges(commits, []);
    const before = walks;
    const open = foldedMerges(commits, ["merge"]);
    return { linearWalks, reused: walks === before, closed: [...closed.hidden], open: [...open.hidden], owner: open.owner.get("side") };
  });
  expect(result).toEqual({ linearWalks: 0, reused: true, closed: ["side"], open: [], owner: "merge" });
});

for (const kind of ["blame", "content"] as const) {
  for (const change of ["repository", "reopened", "selection", "newer", "current"] as const) {
    for (const fail of [false, true]) {
      test(`async reads ${kind} ${fail ? "errors" : "results"} respect ${change} context`, async ({ page }) => {
        const result = await page.evaluate(async ({ kind, change, fail }) => {
          const path = kind === "blame" ? "/src/lib/shell/diff.svelte.ts" : "/src/lib/shell/history.svelte.ts";
          const module = await import(/* @vite-ignore */ path);
          const original = window.fetch;
          const response = (value: unknown, error = false) => new Response(JSON.stringify(error ? { ok: false, error: "old error" } : { ok: true, value }));
          let release!: () => void;
          const pending = new Promise<Response>((resolve) => { release = () => resolve(response([{ id: "old" }], fail)); });
          let first = true;
          window.fetch = (input, init) => {
            const body = typeof init?.body === "string" ? JSON.parse(init.body) : null;
            if (!body?.args?.path?.startsWith("D:/async-review/")) return original(input, init);
            if (first) { first = false; return pending; }
            return Promise.resolve(response([{ id: "new" }]));
          };
          const shell: any = {
            path: "D:/async-review/old", repoPath() { return this.path; },
            openGen: 0, blameGen: 0, contentSearchGen: 0, patchGen: 0,
            selectedPath: "old.txt", selectedSide: "unstaged", section: "changes", selectedCommit: null, historyFile: null,
            commitQuery: "S:old", blameLines: null, contentHits: null, actionError: null,
            tr: String, message: (error: unknown) => error instanceof Error ? error.message : String(error),
            withWork: (_label: string, body: () => Promise<void>) => body(),
          };
          const read = () => kind === "blame" ? module.showBlame(shell, "old.txt") : module.searchContent(shell, "S", "old");
          try {
            const task = read();
            const field = kind === "blame" ? "blameLines" : "contentHits";
            if (change !== "current") shell[field] = [{ id: "new" }];
            if (change === "repository") { shell.path = "D:/async-review/new"; shell.openGen++; }
            if (change === "reopened") shell.openGen++;
            if (change === "selection") { shell.selectedPath = "new.txt"; shell.commitQuery = "S:new"; }
            if (change === "newer") await read();
            release();
            await task;
            return { value: shell[field], error: shell.actionError };
          } finally { window.fetch = original; }
        }, { kind, change, fail });
        expect(result).toEqual(change !== "current" ? { value: [{ id: "new" }], error: null }
          : fail ? { value: kind === "blame" ? null : [], error: "old error" }
          : { value: [{ id: "old" }], error: null });
      });
    }
  }
}

for (const stage of ["file_history", "commit_tree"] as const) {
  for (const change of ["repository", "selection", "newer", "current"] as const) {
    for (const fail of [false, true]) {
      test(`navigation ${stage} ${fail ? "errors" : "results"} respect ${change} context`, async ({ page }) => {
        const result = await page.evaluate(async ({ stage, change, fail }) => {
          const path = "/src/lib/shell/state.svelte.ts";
          const { ShellState } = await import(/* @vite-ignore */ path);
          const original = window.fetch;
          let release!: () => void;
          const pending = new Promise<Response>((resolve) => {
            release = () => resolve(new Response(JSON.stringify(fail ? { ok: false, error: "old error" }
              : { ok: true, value: stage === "file_history" ? [{ id: "old" }] : ["old.txt"] })));
          });
          window.fetch = (input, init) => {
            const body = typeof init?.body === "string" ? JSON.parse(init.body) : null;
            return body?.cmd === stage && body?.args?.path === "D:/navigation-review/old" ? pending : original(input, init);
          };
          const reads: string[] = [];
          const shell: any = {
            mode: "live", path: "D:/navigation-review/old", repoPath() { return this.path; },
            navLock: 0, navGen: 0, openGen: 0, contextGen: 0,
            section: "changes", selectedPath: null, selectedSide: "unstaged", selectedCommit: null,
            historyFilter: "", treeOn: false, blameLines: null, actionError: null,
            message: (error: unknown) => error instanceof Error ? error.message : String(error),
            commitsLive: [{ id: "new" }], treePaths: ["new.txt"], lastPlace: null,
            loadContext: async () => {},
            selectCommit: async (id: string) => { reads.push(`${shell.path}:${id}`); },
          };
          shell.snapshotPlace = ShellState.prototype.snapshotPlace.bind(shell);
          shell.restorePlace = ShellState.prototype.restorePlace.bind(shell);
          const place = { section: "history", path: "old.txt", side: "unstaged", commit: "old-commit", blame: false,
            tree: stage === "commit_tree", historyFilter: stage === "file_history" ? "old.txt" : "" };
          try {
            const task = shell.restorePlace(place);
            if (change === "repository") { shell.path = "D:/navigation-review/new"; shell.openGen++; }
            if (change === "selection") shell.selectedCommit = "new-commit";
            if (change === "newer") await shell.restorePlace({ ...place, path: "new.txt", commit: "new-commit", tree: false, historyFilter: "" });
            const readsBeforeRelease = reads.length;
            release();
            await task;
            return { commits: shell.commitsLive, tree: shell.treePaths, error: shell.actionError, navLock: shell.navLock,
              readsAfterRelease: reads.slice(readsBeforeRelease) };
          } finally { window.fetch = original; }
        }, { stage, change, fail });
        expect(result).toEqual({
          commits: change === "current" && !fail && stage === "file_history" ? [{ id: "old" }] : [{ id: "new" }],
          tree: change === "current" && !fail && stage === "commit_tree" ? ["old.txt"] : ["new.txt"],
          error: change === "current" && fail ? "old error" : null,
          navLock: 0,
          readsAfterRelease: change === "current" && !fail && stage === "file_history" ? ["D:/navigation-review/old:old-commit"] : [],
        });
      });
    }
  }
}
