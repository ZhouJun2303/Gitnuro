import { expect, test } from "@playwright/test";
import { openLabScene } from "./helpers";

test("merge choices work from the keyboard and dirty close defaults to keeping work", async ({ page }) => {
  await openLabScene(page, "text-merge", ".merge-panel");
  const pick = page.locator(".merge-side.left .merge-hover-btn").first();
  await pick.focus();
  await page.keyboard.press("Enter");
  await expect(pick).toHaveAttribute("aria-pressed", "true");
  await expect(page).toHaveTitle(/\* src\/both-sides\.ts/);
  await page.keyboard.press("Escape");
  const confirm = page.locator("[data-confirm]");
  await expect(confirm).toBeVisible();
  await expect(confirm.locator(".text-button")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(confirm).toHaveCount(0);
  await expect(pick).toHaveAttribute("aria-pressed", "true");
  await page.locator(".merge-close").click();
  await confirm.locator(".commit").click();
  await expect(page.locator(".merge-panel")).toHaveCount(0);
  await expect(page.locator(".conflict-pane")).toBeVisible();
});

test("merge preview preserves the manual draft and guards replacing it with side choices", async ({ page }) => {
  await openLabScene(page, "text-merge", ".merge-panel");
  await page.locator(".merge-party input").first().check();
  await page.locator(".merge-edit-entry").click();
  const draft = "hand edited result\n\nwith a final newline\n";
  await page.locator(".merge-result-edit").fill(draft);
  await page.locator(".merge-edit-entry").click();
  await expect(page.locator(".merge-manual-preview")).toHaveText(draft);
  await page.locator(".merge-party input").last().click();
  const confirm = page.locator("[data-confirm]");
  await expect(confirm).toBeVisible();
  await confirm.locator(".text-button").click();
  await expect(page.locator(".merge-manual-preview")).toHaveText(draft);
  await expect(page.locator(".merge-party input").last()).not.toBeChecked();
  await page.locator(".merge-edit-entry").click();
  await expect(page.locator(".merge-result-edit")).toHaveValue(draft);
});

for (const kind of ["text", "file"]) test(`failed ${kind} merge keeps the draft or selection and can retry`, async ({ page }) => {
  let attempt = 0;
  const requests: Record<string, unknown>[] = [];
  await page.route("**/__lab/invoke", async (route) => {
    const { cmd, args } = route.request().postDataJSON();
    if (cmd !== "mutate") return route.continue();
    // Never run a real Git write for this test, even on a fixture repository.
    requests.push(args.request);
    if (++attempt === 1) return route.fulfill({ json: { ok: false, error: "merge test: file is locked" } });
    const response = await page.request.post("/__lab/invoke", { data: { cmd: "workspace_status", args: { path: args.path } } });
    const snapshot = await response.json();
    return route.fulfill({ json: { ok: true, value: { ...snapshot.value, outcome: "completed" } } });
  });
  await openLabScene(page, kind === "text" ? "text-merge" : "file-merge", ".merge-panel");
  await page.locator(".merge-party input").first().check();
  if (kind === "text") {
    await page.locator(".merge-edit-entry").click();
    await page.locator(".merge-result-edit").fill("preserve this manual draft\n");
  }
  await page.locator(".merge-panel .commit").click();
  await expect(page.locator(".merge-error")).toContainText("file is locked");
  await expect(page.locator(".merge-panel .commit")).toBeEnabled();
  await expect(page.locator(".merge-party input").first()).toBeChecked();
  if (kind === "text") await expect(page.locator(".merge-result-edit")).toHaveValue("preserve this manual draft\n");
  await page.locator(".merge-panel .commit").click();
  await expect(page.locator(".merge-panel")).toHaveCount(0);
  expect(requests).toHaveLength(2);
  expect(requests[1]).toEqual(requests[0]);
  expect(requests[0].side).toBe(kind === "text" ? "text" : "theirs");
  if (kind === "text") expect(requests[0].text).toBe("preserve this manual draft\n");
});

test("resolving one file while other conflicts remain closes the panel", async ({ page }) => {
  await page.route("**/__lab/invoke", async (route) => {
    const { cmd, args } = route.request().postDataJSON();
    if (cmd !== "mutate") return route.continue();
    // The backend reports "conflict" whenever the worktree still has unmerged paths,
    // even though resolving this file succeeded.
    const response = await page.request.post("/__lab/invoke", { data: { cmd: "workspace_status", args: { path: args.path } } });
    const snapshot = await response.json();
    return route.fulfill({ json: { ok: true, value: { ...snapshot.value, outcome: "conflict", error: "" } } });
  });
  await openLabScene(page, "text-merge", ".merge-panel");
  await page.locator(".merge-party input").first().check();
  await page.locator(".merge-panel .commit").click();
  await expect(page.locator(".merge-panel")).toHaveCount(0);
  await expect(page.locator(".merge-error")).toHaveCount(0);
});

test("merge navigation reuses the same file and guards changing files or repositories", async ({ page }) => {
  await openLabScene(page, "text-merge", ".merge-panel");
  const result = await page.evaluate(async () => {
    const modulePath = "/src/lib/shell/state.svelte.ts";
    const { ShellState } = await import(/* @vite-ignore */ modulePath);
    const opened: string[] = [];
    // Exercise the production navigation/guard methods without mounting a second application.
    const shell = {
      inApp: () => false,
      pathsMatch: (a: string, b: string) => a === b,
      tr: (key: string) => key,
      busy: false,
      mergeApplying: false,
      confirmAsk: null as any,
      ask(title: string, body: string, run: () => void) { this.confirmAsk = { title, body, run }; },
    } as any;
    shell.guardMerge = ShellState.prototype.guardMerge.bind(shell);
    shell.applyWindowNav = ShellState.prototype.applyWindowNav.bind(shell);
    shell.repoPath = () => "D:/lab/one";
    shell.openRepo = async (repo: string) => { opened.push(repo); shell.repoPath = () => repo; };
    shell.openTextMerge = async (file: string) => { opened.push(file); shell.mergeFile = file; shell.textMerge = true; };
    shell.textMerge = true;
    shell.mergeFile = "first.ts";
    shell.mergeDirty = shell.mergeEdited = true;
    shell.mergeResult = "keep my draft";
    await shell.applyWindowNav({ view: "merge", repo: "D:/lab/one", file: "first.ts", rev: "" });
    const same = opened.length === 0 && shell.mergeResult === "keep my draft" && !shell.confirmAsk;
    await shell.applyWindowNav({ view: "merge", repo: "D:/lab/one", file: "second.ts", rev: "" });
    const guardedFile = !!shell.confirmAsk && opened.length === 0;
    shell.confirmAsk = null; // Cancel.
    await shell.applyWindowNav({ view: "merge", repo: "D:/lab/two", file: "first.ts", rev: "" });
    const guardedRepo = !!shell.confirmAsk && opened.length === 0;
    const run = shell.confirmAsk.run;
    shell.confirmAsk = null;
    run();
    await new Promise((resolve) => setTimeout(resolve, 0));
    return { same, guardedFile, guardedRepo, opened };
  });
  expect(result).toEqual({ same: true, guardedFile: true, guardedRepo: true, opened: ["D:/lab/two", "first.ts"] });
});

for (const scene of ["text-merge-many", "file-merge"]) test(`merge controls fit the minimum window size: ${scene}`, async ({ page }) => {
  await page.setViewportSize({ width: 480, height: 360 });
  await openLabScene(page, scene, ".merge-panel");
  for (const selector of [".merge-close", ".merge-party input", ".merge-panel-foot button"]) {
    for (const control of await page.locator(selector).all()) await expect(control).toBeInViewport({ ratio: 1 });
  }
  expect(await page.locator(".merge-panel").evaluate((node) => node.scrollWidth - node.clientWidth)).toBeLessThanOrEqual(1);
  await page.screenshot({ path: `.lab/ui-audit/${scene}-minimum.png` });
});

test("one huge merge hunk stays virtual while scrolling, selecting and previewing", async ({ page }) => {
  test.setTimeout(60_000);
  const ours = Array.from({ length: 25_000 }, (_, i) => `main line ${i}`).join("\n");
  const theirs = Array.from({ length: 25_000 }, (_, i) => `topic line ${i}`).join("\n");
  await page.route("**/__lab/invoke", (route) => {
    const { cmd } = route.request().postDataJSON();
    return cmd === "conflict_sides"
      ? route.fulfill({ json: { ok: true, value: { ours, theirs, working: `<<<<<<< HEAD\n${ours}\n=======\n${theirs}\n>>>>>>> topic` } } })
      : route.continue();
  });
  await openLabScene(page, "conflict-large-asset", ".merge-large");
  await page.locator(".merge-large button").click();
  const left = page.locator(".merge-side.left"), right = page.locator(".merge-side.right");
  await expect(left).toContainText("topic line 0");
  await left.evaluate((node) => { node.scrollTop = node.scrollHeight; });
  await expect(left).toContainText("topic line 24999");
  await expect.poll(() => right.evaluate((node) => node.scrollTop)).toBeGreaterThan(1000);
  expect(await page.locator(".merge-block").count()).toBeLessThan(100);
  await page.locator(".merge-party input").first().check();
  await expect(page.locator(".merge-panel .commit")).toBeEnabled();
  const result = page.locator(".merge-result-scroll");
  await result.evaluate((node) => { node.scrollTop = node.scrollHeight; });
  await expect(result).toContainText("topic line 24999");
  expect(await page.locator(".merge-block").count()).toBeLessThan(150);
  await page.locator(".merge-edit-entry").click();
  await expect(page.locator(".merge-result-edit")).toHaveValue(theirs);
  await page.locator(".merge-edit-entry").click();
  expect(await page.locator(".merge-block").count()).toBeLessThan(150);
});

test("unequal merge sides can scroll in both directions without a feedback loop", async ({ page }) => {
  await page.route("**/__lab/invoke", (route) => {
    const { cmd } = route.request().postDataJSON();
    const ours = Array.from({ length: 200 }, (_, i) => `short ${i}`).join("\n");
    const theirs = Array.from({ length: 600 }, (_, i) => `long ${i}`).join("\n");
    return cmd === "conflict_sides"
      ? route.fulfill({ json: { ok: true, value: { ours, theirs, working: `<<<<<<< HEAD\n${ours}\n=======\n${theirs}\n>>>>>>> topic` } } })
      : route.continue();
  });
  await openLabScene(page, "text-merge", ".merge-panel");
  const left = page.locator(".merge-side.left"), right = page.locator(".merge-side.right");
  await left.evaluate((node) => { node.scrollTop = node.scrollHeight; });
  await expect.poll(() => right.evaluate((node) => node.scrollTop)).toBeGreaterThan(100);
  await right.evaluate((node) => { node.scrollTop = 80; });
  await expect.poll(() => left.evaluate((node) => node.scrollTop)).toBe(80);
  const positions = await page.evaluate(async () => {
    const panes = [...document.querySelectorAll(".merge-side")];
    const before = panes.map((node) => node.scrollTop);
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    return { before, after: panes.map((node) => node.scrollTop) };
  });
  expect(positions.after).toEqual(positions.before);
});

test("merge conflict navigation reaches virtual hunks across large context", async ({ page }, info) => {
  test.setTimeout(60_000);
  const working = Array.from({ length: 1000 }, (_, i) =>
    `${Array.from({ length: 64 }, (_, line) => `context ${i}:${line}`).join("\n")}\n<<<<<<< HEAD\nmain ${i}\n=======\ntopic ${i}\n>>>>>>> topic`,
  ).join("\n");
  await page.route("**/__lab/invoke", (route) => {
    const { cmd } = route.request().postDataJSON();
    return cmd === "conflict_sides"
      ? route.fulfill({ json: { ok: true, value: { ours: "main", theirs: "topic", working } } })
      : route.continue();
  });
  await openLabScene(page, "conflict-large-asset", ".merge-large");
  await page.locator(".merge-large button").click();
  await expect(page.locator(".merge-count")).toHaveText("0/1000");
  await page.locator(".merge-nav-btn").first().click();
  const target = page.locator(".merge-side.left .merge-cell.hunk").filter({ hasText: "topic 999" });
  await expect(target).toBeInViewport();
  await expect(page.locator(".merge-side.right")).toContainText("main 999");
  await target.locator(".merge-hover-btn").click();
  await expect(page.locator(".merge-result-hunk")).toContainText("topic 999");
  expect(await page.locator(".merge-block").count()).toBeLessThan(100);
  await page.screenshot({ path: ".lab/ui-audit/merge-large-context.png" });
  const metrics = await page.evaluate(() => ({ renderedBlocks: document.querySelectorAll(".merge-block").length,
    nodes: document.querySelectorAll("*").length }));
  await info.attach("virtual-context-metrics", { body: JSON.stringify(metrics), contentType: "application/json" });
  await page.locator(".merge-nav-btn").last().click();
  await expect(page.locator(".merge-side.left")).toContainText("topic 0");
});

test("closing a merge while a large file loads prevents the late response reopening it", async ({ page }) => {
  await openLabScene(page, "conflict-large-asset", ".merge-large");
  let release: () => void = () => {};
  const pending = new Promise<void>((resolve) => { release = resolve; });
  let started: () => void = () => {};
  const loading = new Promise<void>((resolve) => { started = resolve; });
  await page.route("**/__lab/invoke", async (route) => {
    const { cmd } = route.request().postDataJSON();
    if (cmd !== "conflict_sides") return route.continue();
    started();
    await pending;
    return route.fulfill({ json: { ok: true, value: { ours: "main", theirs: "topic", working: "<<<<<<< HEAD\nmain\n=======\ntopic\n>>>>>>> topic" } } });
  });
  await page.locator(".merge-large button").click();
  await loading;
  await page.locator(".merge-close").click();
  await expect(page.locator(".merge-panel")).toHaveCount(0);
  const response = page.waitForResponse((value) => value.url().endsWith("/__lab/invoke") && value.request().postDataJSON().cmd === "conflict_sides");
  release();
  await response;
  await expect(page.locator(".merge-panel")).toHaveCount(0);
});

for (const blankLine of [false, true]) test(`empty merge side preserves its actual line count: ${blankLine ? "blank line" : "deletion"}`, async ({ page }) => {
  let text: string | undefined;
  const expected = blankLine ? "before\n\nafter\n" : "before\nafter\n";
  await page.route("**/__lab/invoke", (route) => {
    const { cmd, args } = route.request().postDataJSON();
    if (cmd === "conflict_sides") return route.fulfill({ json: { ok: true, value: {
      ours: expected, theirs: "before\ntopic\nafter\n",
      working: `before\n<<<<<<< HEAD\n${blankLine ? "\n" : ""}=======\ntopic\n>>>>>>> topic\nafter\n`,
    } } });
    if (cmd === "mutate") {
      text = args.request.text;
      return route.fulfill({ json: { ok: false, error: "merge test: inspect request" } });
    }
    return route.continue();
  });
  await openLabScene(page, "text-merge", ".merge-panel");
  await page.locator(".merge-party input").last().check();
  await expect(page.locator(".merge-panel .commit")).toBeEnabled();
  await page.locator(".merge-edit-entry").click();
  await expect(page.locator(".merge-result-edit")).toHaveValue(expected);
  await page.locator(".merge-panel .commit").click();
  await expect(page.locator(".merge-error")).toContainText("inspect request");
  expect(text).toBe(expected);
});
