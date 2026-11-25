import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "@playwright/test";
import { pinLabSettings } from "./helpers";

for (const theme of ["apple", "light", "dark"]) {
  test(`checkout clears the branch spinner after the reference snapshot arrives (${theme})`, async ({ page }) => {
    let status: Record<string, unknown>, refs: Record<string, any>;
    let task: { id: string; path: string; action: string; label: string; targets: string[]; state: "completed"; dependsOn: string[]; revision: number } | undefined;
    let refreshing = false, deliveries = 0;
    let releaseResult!: () => void, releaseRefs!: () => void, receivedTask!: () => void;
    const resultGate = new Promise<void>((done) => { releaseResult = done; });
    const refsGate = new Promise<void>((done) => { releaseRefs = done; });
    const taskResponse = new Promise<void>((done) => { receivedTask = done; });

    await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, statusInterval: 0 });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd, args } = route.request().postDataJSON();
      if (cmd === "operation_tasks") {
        const tasks = task && deliveries === 0 ? [task] : [];
        await route.fulfill({ json: { ok: true, value: tasks } });
        if (tasks.length) { deliveries += 1; receivedTask(); }
        return;
      }
      if (cmd === "workspace_status" && !task) {
        const response = await route.fetch();
        status = (await response.json()).value;
        return route.fulfill({ response });
      }
      if (cmd === "repository_refs") {
        if (!task) {
          const response = await route.fetch();
          refs = (await response.json()).value;
          return route.fulfill({ response });
        }
        refreshing = true;
        await refsGate;
        const branches = (refs.branches ?? []).map((branch: { name: string }) => ({ ...branch, current: branch.name === "topic" }));
        return route.fulfill({ json: { ok: true, value: { ...refs, branches, revision: task.revision } } });
      }
      if (cmd === "mutate" && args.request?.action === "checkout") {
        task = { id: args.__task, path: args.path, action: "checkout", label: "检出 topic", targets: ["topic"], state: "completed", dependsOn: [], revision: Number(refs.revision ?? 0) + 1 };
        await resultGate;
        return route.fulfill({ json: { ok: true, value: {
          ...status, branch: "topic", revision: task.revision, outcome: "completed", error: "", log: "$ git checkout topic",
          refreshAreas: ["worktree", "index", "refs", "history", "flow"],
        } } });
      }
      return route.fallback();
    });
    await page.goto("/?scene=checkout-dirty&theme=" + theme + "&locale=zh");
    await expect(page.locator('[data-lab-scene="checkout-dirty"]')).toBeVisible();
    const spinner = page.locator('.sidebar .file-select[title="topic"] .reference-pending');
    await expect(spinner).toHaveCount(0);
    await page.locator('.sidebar .file-select[title="topic"]').dblclick();
    try {
      await taskResponse;
      await expect(spinner).toBeVisible();
      releaseResult();
      await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
      await expect.poll(() => refreshing).toBe(true);
      await expect(spinner).toBeVisible();
      releaseRefs();
      await expect(spinner).toHaveCount(0, { timeout: 3000 });
    } finally {
      releaseResult();
      releaseRefs();
    }
  });

  for (const kind of ["local", "remote", "index", "conflict", "failed"]) {
    test(`double-click checkout preserves local work ${kind} (${theme})`, async ({ page, request }) => {
      test.setTimeout(90_000);
      const scene = kind === "index" || kind === "conflict" ? `checkout-${kind}` : "checkout-dirty";
      const restored = await request.post("http://127.0.0.1:1429/invoke", {
        data: { cmd: "lab_restore", args: { name: scene } },
      });
      expect((await restored.json()).ok).toBe(true);
      const repo = resolve(".lab", "repos", scene);
      const git = (...args: string[]) => execFileSync("git", args, { cwd: repo, encoding: "utf8" });
      const beforeIndex = git("diff", "--cached");
      const beforeStatus = git("status", "--porcelain=v1");
      const older = git("rev-parse", "refs/stash");
      await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, fileLayout: "list", flowAutoOpen: false });
      await page.goto(`/?scene=${scene}&theme=${theme}&locale=zh`);
      await expect(page.locator(`[data-lab-scene="${scene}"]`)).toBeVisible();

      const actions: string[] = [];
      let locateReads = 0;
      page.on("request", (item) => {
        if (!item.url().includes("/__lab/invoke") || item.method() !== "POST") return;
        const body = item.postDataJSON();
        if (body.cmd === "commit_log" && actions.length === 0) locateReads += 1;
        if (body.cmd === "mutate") actions.push(body.args.request.action);
      });
      if (kind === "failed") {
        await page.route("**/__lab/invoke", async (route) => {
          const body = route.request().postDataJSON();
          if (body.cmd !== "mutate") return route.fallback();
          body.args.request.name = "missing-branch";
          const response = await route.fetch({ postData: JSON.stringify(body) });
          return route.fulfill({ response });
        });
      }
      const result = page.waitForResponse((item) => {
        if (!item.url().includes("/__lab/invoke") || item.request().method() !== "POST") return false;
        return item.request().postDataJSON().cmd === "mutate";
      });
      const target = page.locator(`.sidebar .file-select[title="${kind === "remote" ? "origin/topic" : "topic"}"]`);
      await target.dblclick();
      // A second double-click during the same write must not start another stash/pop chain.
      await target.dispatchEvent("dblclick");
      const payload = await (await result).json();
      expect(payload.ok).toBe(true);
      expect(payload.value.outcome).toBe(kind === "failed" ? "failed" : kind === "conflict" ? "conflict" : "completed");
      await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
      expect(actions).toEqual([kind === "remote" ? "checkoutRemote" : "checkout"]);
      expect(locateReads).toBe(0);
      expect(git("branch", "--show-current")).toBe(kind === "failed" ? "main\n" : "topic\n");
      expect(readFileSync(resolve(repo, "keep.txt"), "utf8")).toBe("unstaged\n");
      expect(readFileSync(resolve(repo, "loose.txt"), "utf8")).toBe("untracked\n");
      expect(readFileSync(resolve(repo, ".fakeignore"), "utf8")).toBe("local-only\n");
      if (kind === "index" || kind === "conflict") {
        expect(git("rev-parse", "stash@{1}")).toBe(older);
        expect(git("show", "stash@{0}^2:keep.txt")).toBe("staged\n");
        if (kind === "conflict") {
          await expect(page.locator('[data-dialog="mergeConflict"]')).toBeVisible();
          expect(readFileSync(resolve(repo, "note.txt"), "utf8")).toContain("<<<<<<<");
          await page.locator('[data-dialog="mergeConflict"] .dialog-close').click();
        } else {
          expect(readFileSync(resolve(repo, "note.txt"), "utf8")).toBe("one\nremote two\nthree\nfour\nlocal five\n");
        }
      } else {
        expect(git("diff", "--cached")).toBe(beforeIndex);
        expect(git("status", "--porcelain=v1")).toBe(beforeStatus);
        expect(git("rev-parse", "refs/stash")).toBe(older);
        expect(git("stash", "list", "--format=%H").trim().split("\n")).toHaveLength(1);
      }

      await page.locator(".history-btn").click();
      await page.locator(".command-filters").getByRole("button", { name: "用户", exact: true }).click();
      const detail = page.locator(".command-detail");
      await expect(detail).toContainText("[AweGit] backend=");
      await expect(detail).toContainText("refresh=");
      await expect(detail).toContainText("total=");
      const checkout = detail.locator("strong.git-command").filter({ hasText: /\$ git checkout/ });
      await expect(checkout).toHaveCount(1);
      await expect(checkout).toContainText(kind === "failed" ? "missing-branch" : "topic");
      await expect(checkout).toContainText(/\[\d+ ms, exit \d+\]/);
      await expect(checkout).toHaveCSS("font-weight", "700");
      await expect(detail.locator("strong.git-command").filter({ hasText: /\$ git stash push/ })).toHaveCount(["local", "remote"].includes(kind) ? 0 : 1);
      await expect(detail.locator("strong.git-command").filter({ hasText: /\$ git stash pop/ })).toHaveCount(0);
      await page.screenshot({ path: `.lab/checkout-${kind}-${theme}.png` });
    });
  }

  test(`single-click branch locates history without checking out (${theme})`, async ({ page, request }) => {
    const restored = await request.post("http://127.0.0.1:1429/invoke", {
      data: { cmd: "lab_restore", args: { name: "checkout-dirty" } },
    });
    expect((await restored.json()).ok).toBe(true);
    await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false });
    await page.goto(`/?scene=checkout-dirty&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="checkout-dirty"]')).toBeVisible();
    await page.locator('.sidebar .file-select[title="topic"]').click();
    await expect(page.locator(".commit-row.selected")).toContainText("topic file");
    await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
    const repo = resolve(".lab", "repos", "checkout-dirty");
    expect(execFileSync("git", ["branch", "--show-current"], { cwd: repo, encoding: "utf8" })).toBe("main\n");
  });
}
