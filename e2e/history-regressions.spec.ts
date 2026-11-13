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
