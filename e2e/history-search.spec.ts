import { expect, test } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";

test("commit search scans a long history without dropping rows", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const { commitHaystacks, commitHitIndexes, hitContains } = await import(/* @vite-ignore */ "/src/lib/view.ts");
    const rows = Array.from({ length: 20_000 }, (_, index) => ({
      id: index.toString(16).padStart(8, "0"),
      shortId: index.toString(16).padStart(7, "0"),
      summary: index % 17 === 0 ? "glow target" : "plain note",
      author: index % 19 === 0 ? "Glow Author" : "Ada",
      email: "",
      refs: index === 3 ? ["release"] : [],
    }));
    const needles = commitHaystacks(rows);
    const started = performance.now();
    const hits = commitHitIndexes(rows, "glow", needles);
    const elapsed = performance.now() - started;
    const again = performance.now();
    const authorHits = commitHitIndexes(rows, "glow author", needles);
    const authorElapsed = performance.now() - again;
    const bodyRows = [{ id: "bcca32716ce15043e4486929b3a354a4df41eac8", summary: "art: hero atlas", author: "周俊", email: "", refs: [] }];
    const bodyNeedles = commitHaystacks(bodyRows);
    const bodyHits = commitHitIndexes(bodyRows, "ID: 1002778", bodyNeedles, new Set([bodyRows[0].id]));
    return {
      hits: hits.length,
      first: hits[0] ?? -1,
      hasSecond: hitContains(hits, 17),
      missesPlain: hitContains(hits, 1),
      authorHits: authorHits.length,
      bodyHit: bodyHits.length === 1 && bodyHits[0] === 0,
      content: commitHitIndexes(rows, "S:glow", needles).length,
      elapsed,
      authorElapsed,
    };
  });
  let expected = 0;
  for (let index = 0; index < 20_000; index += 1) {
    if (index % 17 === 0 || index % 19 === 0) expected += 1;
  }
  expect(result.hits).toBe(expected);
  expect(result.first).toBe(0);
  expect(result.hasSecond).toBe(true);
  expect(result.missesPlain).toBe(false);
  expect(result.authorHits).toBeGreaterThan(0);
  expect(result.bodyHit).toBe(true);
  expect(result.content).toBe(0);
  expect(result.elapsed).toBeLessThan(80);
  expect(result.authorElapsed).toBeLessThan(80);
});

test("reveal scrolls to a loaded commit and grows history when it is below the page", async ({ page }) => {
  await page.goto("/?scene=clean");
  await expect(page.locator('[data-lab-scene="clean"]')).toBeVisible();
  const result = await page.evaluate(async () => {
    const history = await import(/* @vite-ignore */ "/src/lib/shell/history.svelte.ts");
    const original = window.fetch;
    const calls: { cmd?: string; limit?: number }[] = [];
    const row = (index: number) => ({
      id: `abc${String(index).padStart(4, "0")}`,
      shortId: String(index),
      summary: `commit ${index}`,
      author: "Ada",
      email: "",
      when: "",
      at: index,
      parents: [],
      refs: [],
      lane: 0,
    });
    window.fetch = async (input, init) => {
      const body = typeof init?.body === "string" ? JSON.parse(init.body) : null;
      if (body?.cmd === "commit_log" || body?.cmd === "locate_ref") {
        calls.push({ cmd: body.cmd, limit: body.args?.limit });
        const limit = Number(body.args?.limit ?? 0);
        if (body.cmd === "locate_ref") {
          const buried = row(4000);
          return new Response(JSON.stringify({ ok: true, value: { id: buried.id, commits: [buried, row(0)] } }));
        }
        if (limit >= 2000) {
          const rows = Array.from({ length: 1200 }, (_, index) => row(index));
          return new Response(JSON.stringify({ ok: true, value: rows }));
        }
        const rows = Array.from({ length: 80 }, (_, index) => row(index));
        return new Response(JSON.stringify({ ok: true, value: rows }));
      }
      return original(input, init);
    };
    const host = document.createElement("section");
    host.style.cssText = "height:200px;overflow:auto";
    const spacer = document.createElement("div");
    host.append(spacer);
    document.body.append(host);
    const shell: any = {
      mode: "live",
      locateSeq: 0,
      openGen: 0,
      contextGen: 0,
      reflogOn: false,
      treeOn: false,
      historyFilter: "",
      commitQuery: "",
      contentHits: null,
      actionError: null,
      commitsLive: [],
      historyTop: 0,
      historyHeight: 200,
      historyEl: host,
      rowCommit: 28,
      logLimit: 500,
      allBranches: false,
      graphSolo: "main",
      graphBranches: ["main"],
      expandedMerges: [],
      section: "changes",
      selectedCommit: null,
      settings: { commitSort: "date" },
      repoPath: () => "D:/reveal",
      viewing: () => true,
      openHistory() { this.section = "history"; this.reflogOn = false; this.treeOn = false; },
      exactRefs: () => ["refs/heads/main"],
      tr: (_key: string, vars?: { name?: string }) => vars?.name ?? "",
      message: (error: unknown) => String(error),
      withWork: (_label: string, body: () => Promise<void>) => body(),
      rememberHistoryFilter() { this.remembered = true; },
      selectCommitSoon(id: string) { this.selectedCommit = id; },
      get historyItems() { return this.commitsLive; },
    };
    const resize = (count: number) => { spacer.style.height = `${count * 28}px`; };
    try {
      const loaded = Array.from({ length: 100 }, (_, index) => row(index));
      shell.commitsLive = loaded;
      resize(loaded.length);
      calls.length = 0;
      await history.revealCommit(shell, loaded[80].id);
      const loadedCase = {
        calls: calls.length,
        section: shell.section,
        selected: shell.selectedCommit,
        top: host.scrollTop,
        filter: shell.historyFilter,
        kept: shell.commitsLive.length,
      };

      shell.historyFilter = "note.txt";
      shell.commitsLive = [row(80)];
      shell.logLimit = 500;
      shell.allBranches = false;
      resize(80);
      calls.length = 0;
      await history.revealCommit(shell, loaded[60].id);
      const filteredCase = {
        calls: calls.map((call) => call.cmd),
        filter: shell.historyFilter,
        selected: shell.selectedCommit,
        top: host.scrollTop,
        length: shell.commitsLive.length,
        allBranches: shell.allBranches,
      };

      shell.historyFilter = "";
      shell.commitsLive = Array.from({ length: 500 }, (_, index) => row(index));
      shell.logLimit = 500;
      shell.allBranches = false;
      shell.graphSolo = "main";
      resize(1200);
      calls.length = 0;
      await history.revealCommit(shell, row(900).id);
      const grownCase = {
        limits: calls.filter((call) => call.cmd === "commit_log").map((call) => call.limit),
        located: calls.some((call) => call.cmd === "locate_ref"),
        selected: shell.selectedCommit,
        top: host.scrollTop,
        length: shell.commitsLive.length,
        allBranches: shell.allBranches,
      };

      shell.commitsLive = [row(0)];
      shell.logLimit = 500;
      shell.allBranches = false;
      shell.remembered = false;
      resize(2);
      calls.length = 0;
      await history.revealCommit(shell, row(4000).id);
      const widenedCase = {
        located: calls.some((call) => call.cmd === "locate_ref"),
        selected: shell.selectedCommit,
        allBranches: shell.allBranches,
        solo: shell.graphSolo,
        remembered: shell.remembered === true,
        length: shell.commitsLive.length,
      };
      return { loadedCase, filteredCase, grownCase, widenedCase };
    } finally {
      window.fetch = original;
      host.remove();
    }
  });

  expect(result.loadedCase).toMatchObject({ calls: 0, section: "history", selected: "abc0080", filter: "", kept: 100 });
  expect(result.loadedCase.top).toBeGreaterThan(1800);
  expect(result.filteredCase.filter).toBe("");
  expect(result.filteredCase.selected).toBe("abc0060");
  expect(result.filteredCase.top).toBeGreaterThan(1000);
  expect(result.filteredCase.length).toBe(80);
  expect(result.filteredCase.allBranches).toBe(false);
  expect(result.filteredCase.calls).toContain("commit_log");
  expect(result.grownCase.limits).toEqual([2000]);
  expect(result.grownCase.located).toBe(false);
  expect(result.grownCase.selected).toBe("abc0900");
  expect(result.grownCase.length).toBe(1200);
  expect(result.grownCase.allBranches).toBe(false);
  expect(result.grownCase.top).toBeGreaterThan(1000);
  expect(result.widenedCase).toMatchObject({
    located: true,
    selected: "abc4000",
    allBranches: true,
    solo: null,
    remembered: true,
    length: 2,
  });
});

for (const theme of ["apple", "light", "dark"] as const) {
  test(`all commits search rings matches and steps to them (${theme})`, async ({ page }) => {
    await pinLabSettings(page, { theme, locale: "en" });
    await openLabScene(page, "history", ".commit-row", { locale: "en" });
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    const list = page.locator("section.history");
    const full = list.locator(":scope > .commit-window");
    const before = await full.evaluate((node) => node.getBoundingClientRect().height);
    await page.locator(".history-head .search-toggle").click();
    const input = page.locator(".history-head input.search");
    await input.fill("readme");
    const count = page.locator(".history-head .match-count");
    await expect(count).toBeVisible();
    const total = Number(await count.getAttribute("data-match-count"));
    expect(total).toBeGreaterThan(1);
    await expect(count).toHaveText(`1/${total}`);
    expect(await full.evaluate((node) => node.getBoundingClientRect().height)).toBe(before);
    const inputBox = await input.boundingBox();
    expect(inputBox?.width ?? 0).toBeGreaterThan(120);
    const painted = await list.locator(".commit-row").count();
    const marked = await list.locator(".commit-row.match").count();
    expect(painted).toBeGreaterThan(marked);
    expect(marked).toBeGreaterThan(0);
    const current = list.locator(".commit-row.current-match");
    await expect(current).toBeVisible();
    const shadow = await current.evaluate((node) => getComputedStyle(node).boxShadow);
    expect(shadow).not.toBe("none");
    const first = await current.getAttribute("data-commit-id");
    await page.getByRole("button", { name: "Next match" }).click();
    await expect(count).toHaveText(`2/${total}`);
    const second = await list.locator(".commit-row.current-match").getAttribute("data-commit-id");
    expect(second).not.toBe(first);
    const row = await list.locator(".commit-row.current-match").boundingBox();
    const view = await list.boundingBox();
    expect(row).not.toBeNull();
    expect(view).not.toBeNull();
    expect(row!.y).toBeGreaterThanOrEqual(view!.y - 2);
    expect(row!.y + row!.height).toBeLessThanOrEqual(view!.y + view!.height + 2);
    await page.getByRole("button", { name: "Previous match" }).click();
    await expect(list.locator(".commit-row.current-match")).toHaveAttribute("data-commit-id", first ?? "");
    await expect(count).toHaveText(`1/${total}`);
  });
}
