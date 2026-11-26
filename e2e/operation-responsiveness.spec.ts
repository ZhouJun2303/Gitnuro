import { execFileSync } from "node:child_process";
import { once } from "node:events";
import { mkdirSync, mkdtempSync } from "node:fs";
import { createServer } from "node:http";
import { resolve } from "node:path";
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";
import { pinLabSettings } from "./helpers";

const repo = resolve(".lab/repos/pull-noop");
const git = (...args: string[]) => execFileSync("git", args, { cwd: repo, encoding: "utf8", stdio: "pipe" }).trim();

async function restore(request: APIRequestContext) {
  await expect.poll(async () => {
    const response = await request.post("http://127.0.0.1:1429/invoke", { data: { cmd: "lab_restore", args: { name: "pull-noop" } } });
    return (await response.json()).ok;
  }, { timeout: 30_000, intervals: [200, 500, 1000] }).toBe(true);
}

async function open(page: Page, theme: string) {
  await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, statusInterval: 0, fileLayout: "list", pushOnCommit: true, useCommitType: true });
  await page.goto(`/?scene=pull-noop&theme=${theme}&locale=zh`);
  await expect(page.locator('[data-lab-scene="pull-noop"]')).toBeVisible();
  await expect(page.locator(".staged-pane")).toContainText("note.txt");
}

for (const theme of ["apple", "light", "dark"]) {
  test(`file diffs update while fetch waits for the remote (${theme})`, async ({ page, request }) => {
    test.setTimeout(60_000);
    await restore(request);
    const originalRemote = git("remote", "get-url", "origin");
    const head = git("rev-parse", "HEAD");
    let requested = false;
    let releaseFetch!: () => void;
    let releaseDiff = () => {};
    const fetchGate = new Promise<void>((resolve) => { releaseFetch = resolve; });
    const remote = createServer(async (req, res) => {
      if (req.url === "/repo.git/HEAD") return res.end("ref: refs/heads/main\n");
      if (!req.url?.startsWith("/repo.git/info/refs")) return res.writeHead(404).end();
      requested = true;
      await fetchGate;
      res.writeHead(200, { "content-type": "text/plain" }).end(`${head}\trefs/heads/main\n`);
    });
    try {
      remote.listen(0, "127.0.0.1");
      await once(remote, "listening");
      const address = remote.address();
      if (!address || typeof address === "string") throw new Error("Missing test remote address");
      git("remote", "set-url", "origin", `http://127.0.0.1:${address.port}/repo.git`);
      await pinLabSettings(page, { fetchAllRemotes: false, diffStyle: "unified" });
      await open(page, theme);

      await page.locator('.chrome-left [title="获取"]').click();
      await page.locator('[data-dialog="fetch"] button.commit').click();
      await expect.poll(() => requested).toBe(true);
      const diff = page.locator("section.diff .diff-body");
      const staged = page.locator('.staged-pane .file-select[title="note.txt"]');
      await staged.click();
      await expect(diff).toContainText("staged");
      await page.locator('.unstaged-pane .file-select[title="loose/file-0.txt"]').click();
      await expect(diff).toContainText("untracked");
      await expect(page.getByRole("progressbar")).toBeVisible();

      // Finish Fetch while the next diff response is still on its way to the pane.
      let diffReads = 0;
      const diffGate = new Promise<void>((resolve) => { releaseDiff = resolve; });
      await page.route("**/__lab/invoke", async (route) => {
        const body = route.request().postDataJSON();
        if (body.cmd !== "file_diff" || body.args.file !== "note.txt" || !body.args.staged) return route.fallback();
        const response = await route.fetch();
        diffReads++;
        await diffGate;
        return route.fulfill({ response });
      });
      await staged.click();
      await expect.poll(() => diffReads).toBe(1);
      const finished = page.waitForResponse((response) => response.url().endsWith("/__lab/invoke")
        && response.request().postDataJSON()?.args?.request?.action === "fetch");
      releaseFetch();
      const payload = await (await finished).json();
      expect(payload.ok && payload.value.outcome, payload.value?.error).toBe("completed");
      await page.waitForFunction(async (modulePath) => {
        const { clientStats } = await import(/* @vite-ignore */ modulePath);
        return clientStats().some((entry: { name: string }) => entry.name === "mutate");
      }, "/src/lib/perf.ts");
      releaseDiff();
      await expect(diff).toContainText("staged");
      expect(diffReads).toBe(1);
    } finally {
      releaseFetch();
      releaseDiff();
      remote.close();
      git("remote", "set-url", "origin", originalRemote);
    }
  });

  test(`account testing preserves a new token entered while the response is pending (${theme})`, async ({ page }) => {
    await pinLabSettings(page, { locale: "zh", theme, fetchAutomatically: false, statusInterval: 0 });
    await page.goto(`/?scene=prefs-accounts&theme=${theme}&locale=zh`);
    const dialog = page.locator('[data-dialog="prefs"]');
    await expect(dialog).toBeVisible();
    const token = dialog.getByRole("textbox", { name: "令牌", exact: true });
    await token.fill("fixture-token-before");
    let submitted = false, release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    await page.route("**/__lab/invoke", async (route) => {
      const body = route.request().postDataJSON();
      if (body.cmd !== "test_account") return route.fallback();
      submitted = true; await gate;
      return route.fulfill({ json: { ok: true, value: { code: "ok", login: "old-login", name: "old profile", email: "", avatarUrl: "", message: "" } } });
    });
    await dialog.getByRole("button", { name: "保存账号", exact: true }).click();
    try {
      await expect.poll(() => submitted).toBe(true);
      await token.fill("fixture-token-after");
      const finished = page.waitForResponse((response) => response.url().endsWith("/__lab/invoke") && response.request().postDataJSON()?.cmd === "test_account");
      release(); await finished;
      await expect(token).toHaveValue("fixture-token-after");
      await expect(dialog.locator(".account-name").filter({ hasText: "old profile" })).toHaveCount(0);
    } finally { release(); }
  });

  test(`commit and push finish independently of auxiliary history and preserve a new draft (${theme})`, async ({ page, request }) => {
    test.setTimeout(90_000);
    await restore(request);
    mkdirSync(".lab/responsiveness-remotes", { recursive: true });
    const remote = resolve(mkdtempSync(".lab/responsiveness-remotes/push-"), "remote.git");
    execFileSync("git", ["clone", "--bare", resolve(".lab/repos/pull-noop-upstream"), remote], { stdio: "pipe" });
    git("remote", "set-url", "origin", remote);
    const errors: string[] = []; page.on("pageerror", (error) => errors.push(error.message));
    await open(page, theme);
    let releaseCommit!: () => void, releaseHistory!: () => void;
    const commitGate = new Promise<void>((done) => { releaseCommit = done; });
    const historyGate = new Promise<void>((done) => { releaseHistory = done; });
    let committed = false, pushStarted = false, historyStarted = false;
    await page.route("**/__lab/invoke", async (route) => {
      const body = route.request().postDataJSON();
      if (body.cmd === "commit_changes") {
        const response = await route.fetch(); committed = true;
        await commitGate; return route.fulfill({ response });
      }
      if (body.cmd === "mutate" && ["push", "pushRef"].includes(body.args.request?.action)) pushStarted = true;
      if (body.cmd === "commit_log") { historyStarted = true; await historyGate; }
      return route.fallback();
    });
    const subject = `responsive ${theme} ${Date.now()}`;
    await page.locator(".summary-input").fill(subject);
    await page.locator('.composer button[type="submit"]').click();
    try {
      await expect.poll(() => committed).toBe(true);
      await expect.poll(() => pushStarted).toBe(true);
      await page.locator(".summary-input").fill("next draft while committing");
      await expect.poll(() => git("rev-parse", "HEAD") === git("rev-parse", "origin/main")).toBe(true);
      await expect(page.locator(".chrome-right .task-queue")).toHaveCount(0);
      await page.locator(".history-btn").click();
      await page.locator(".command-filters .task-queue summary").click();
      await expect(page.locator(".task-popover")).toBeVisible();
      await expect(page.locator('.task-row[data-task-state="completed"]').first()).toBeVisible();
      releaseCommit();
      await expect.poll(() => historyStarted).toBe(true);
      await expect(page.locator(".summary-input")).toHaveValue("next draft while committing");
      await expect(page.locator('.chrome-left [title="获取"]')).toBeEnabled();
      mkdirSync(".lab/ui-audit/responsiveness", { recursive: true });
      await page.screenshot({ path: `.lab/ui-audit/responsiveness/tasks-${theme}.png` });
    } finally { releaseCommit(); releaseHistory(); }
    await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
    expect(git("log", "-1", "--format=%s")).toContain(subject);
    expect(errors).toEqual([]);
  });

  test(`queued reference intentions show a pending marker and can be cancelled before delivery (${theme})`, async ({ page, request }) => {
    test.setTimeout(60_000);
    await restore(request);
    await open(page, theme);
    const name = `pending-reference-${theme}-${Date.now()}`;
    let release!: () => void; const gate = new Promise<void>((done) => { release = done; });
    let submitted = false;
    await page.route("**/__lab/invoke", async (route) => {
      const body = route.request().postDataJSON();
      if (body.cmd === "mutate" && body.args.request?.action === "createBranch") { submitted = true; await gate; }
      return route.fallback();
    });
    // Open the normal branch form through the local branch section.
    await page.locator('.sidebar .side.section').first().hover();
    await page.locator('.sidebar .side-add').first().click();
    const dialog = page.locator('[data-dialog="branch"]');
    await dialog.locator("input").first().fill(name);
    const checkout = dialog.locator('input[type="checkbox"]'); if (await checkout.isChecked()) await checkout.uncheck();
    await dialog.locator("button.commit").click();
    try {
      await expect.poll(() => submitted).toBe(true);
      await expect(dialog).toHaveCount(0);
      const pending = page.locator(`.sidebar .file-select[title="${name}"]`);
      await expect(pending.locator(".reference-pending")).toBeVisible();
      expect(git("branch", "--list", name)).toBe("");
      await page.locator(".history-btn").click();
      await page.locator(".command-filters .task-queue summary").click();
      await page.locator('.task-row[data-task-state="queued"]').filter({ hasText: name }).getByRole("button", { name: "取消", exact: true }).click();
      await expect(page.locator('.task-row[data-task-state="cancelled"]').filter({ hasText: name })).toBeVisible();
      await expect(pending).toHaveCount(0);
    } finally { release(); }
    await expect(page.locator('.task-row[data-task-state="cancelled"]').filter({ hasText: name })).toBeVisible();
    await expect(page.locator(`.sidebar .file-select[title="${name}"]`)).toHaveCount(0);
    expect(git("branch", "--list", name)).toBe("");
  });
}
