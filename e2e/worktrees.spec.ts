import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, renameSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { openLabScene } from "./helpers";

const main = resolve(".lab/repos/worktree");
const checkouts = resolve(".lab/repos/worktree-checkouts");
const git = (path: string, ...args: string[]) => execFileSync("git", args, { cwd: path, encoding: "utf8" }).trim();
const row = (page: Page, name: string) => page.locator(`[data-worktree-path$="/${name}"]`);
const dialog = (page: Page) => page.locator('[data-dialog="worktree"]');
const button = (page: Page, name: string) => dialog(page).getByRole("button", { name, exact: true });

async function worktreeAction(page: Page, name: string, action: string) {
  await row(page, name).click({ button: "right" });
  await page.getByRole("menuitem", { name: action, exact: true }).click();
}

test.beforeEach(async ({ request }) => {
  // A background Git read from the previous page can briefly hold a Windows directory handle.
  await expect.poll(async () => {
    const response = await request.post("http://127.0.0.1:1429/invoke", {
      data: { cmd: "lab_restore", args: { name: "worktree" } },
    });
    return response.json();
  }).toMatchObject({ ok: true });
});

for (const theme of ["apple", "light", "dark"] as const) {
  test(`worktree tab badges (${theme})`, async ({ page }) => {
    const locale = theme === "dark" ? "en" : "zh";
    const badge = locale === "zh" ? "工作树" : "Worktree";
    await openLabScene(page, "worktree-tabs", ".tab-worktree", { theme, locale });
    const mainTab = page.locator('.tab[data-repo$="worktree"]');
    const topicTab = page.locator('.tab[data-repo$="/topic"]');
    const detachedTab = page.locator('.tab[data-repo$="/detached"]');

    await expect(mainTab).toHaveClass(/\bactive\b/);
    await expect(mainTab.locator(".tab-worktree")).toHaveCount(0);
    await expect(topicTab.locator(".tab-worktree-full")).toHaveText(badge);
    await expect(topicTab.locator(".tab-worktree-full")).toBeVisible();
    await expect(detachedTab.getByRole("img", { name: badge, exact: true })).toBeVisible();

    await topicTab.locator(".file-select").click();
    await expect(topicTab).toHaveClass(/\bactive\b/);
    await expect(topicTab.locator(".tab-worktree-full")).toBeVisible();
    await expect(detachedTab.locator(".tab-worktree-full")).toBeVisible();

    await page.setViewportSize({ width: 360, height: 800 });
    await expect(topicTab.locator(".tab-worktree-short")).toBeVisible();
    await expect(topicTab.getByRole("img", { name: badge, exact: true })).toBeVisible();
    await expect(topicTab.locator(".tab-name")).toBeVisible();
  });

  test(`worktree creation, branch switching and removal (${theme})`, async ({ page }) => {
    test.setTimeout(90_000);
    await openLabScene(page, "worktree", '[data-worktree-path$="/topic"]', { theme, locale: "zh" });
    await page.locator(".sidebar").getByRole("button", { name: "新增工作树", exact: true }).click();
    const branch = await dialog(page).getByLabel("分支", { exact: true }).inputValue();
    expect(branch).toMatch(/^wt-/);
    await dialog(page).getByLabel("工作树目录", { exact: true }).fill("../worktree-checkouts/created");
    await button(page, "创建").click();
    await expect(row(page, "created")).toHaveAttribute("aria-current", "true");
    await expect(page.locator('.tab[data-repo$="created"] .tab-worktree')).toBeVisible();
    expect(git(resolve(checkouts, "created"), "branch", "--show-current")).toBe(branch);

    await worktreeAction(page, "created", "删除工作树");
    await button(page, "删除工作树").click();
    await expect.poll(() => existsSync(resolve(checkouts, "created"))).toBe(false);
    await expect(row(page, "worktree")).toHaveAttribute("aria-current", "true");
    await expect(row(page, "created")).toHaveCount(0);
    await expect(page.locator('.tab[data-repo$="created"]')).toHaveCount(0);
    expect(git(main, "show-ref", "--verify", `refs/heads/${branch}`)).toContain(`refs/heads/${branch}`);

    const topicBranch = page.locator('.sidebar .file-select[title^="topic"]');
    await topicBranch.click({ button: "right" });
    await page.getByRole("menuitem", { name: "打开工作树", exact: true }).click();
    await expect(row(page, "topic")).toHaveAttribute("aria-current", "true");
    await expect(page.locator(".unstaged-pane")).toContainText("local.txt");

    await row(page, "worktree").dblclick();
    await expect(row(page, "worktree")).toHaveAttribute("aria-current", "true");
    const remotes = page.locator('[data-section="remotes"] .file-select');
    if (await remotes.getAttribute("aria-expanded") === "false") await remotes.click();
    const origin = page.locator(".sidebar").getByRole("button", { name: "origin", exact: true });
    if (await origin.getAttribute("aria-expanded") === "false") await origin.click();
    await page.locator('.sidebar .file-select[title="origin/topic"]').click({ button: "right" });
    await page.getByRole("menuitem", { name: "打开工作树", exact: true }).click();
    await expect(row(page, "topic")).toHaveAttribute("aria-current", "true");
    expect(git(main, "branch", "--show-current")).toBe("main");
  });

  test(`worktree history and detached commit protection (${theme})`, async ({ page }) => {
    test.setTimeout(90_000);
    await openLabScene(page, "worktree", '[data-worktree-path$="/detached"]', { theme, locale: "zh" });
    await worktreeAction(page, "detached", "查看工作树提交");
    await expect(page.locator(".commit-row").filter({ hasText: "worktree detached commit" })).toBeVisible();
    await expect(page.locator(".commit-row").filter({ hasText: "worktree main commit" })).toHaveCount(0);
    await expect(row(page, "worktree")).toHaveAttribute("aria-current", "true");
    const history = page.locator(".worktree-history-bar");
    await history.getByRole("button", { name: "比较", exact: true }).click();
    await expect(history).toContainText("领先 1 · 落后 1");
    await expect(page.locator(".commit-row")).toHaveCount(1);
    await history.getByRole("button", { name: "比较文件", exact: true }).click();
    await expect(page.locator(".detail")).toContainText("detached.txt");
    await history.getByRole("button", { name: "清除过滤", exact: true }).click();
    await expect(page.locator(".commit-row").filter({ hasText: "worktree detached commit" })).toBeVisible();
    await expect(page.locator(".commit-row").filter({ hasText: "worktree main commit" })).toBeVisible();

    await worktreeAction(page, "detached", "删除工作树");
    await expect(button(page, "保存为分支")).toBeVisible();
    await expect(button(page, "删除工作树")).toBeDisabled();
    const saved = "saved-detached";
    await dialog(page).getByLabel("分支", { exact: true }).fill(saved);
    const tip = git(resolve(checkouts, "detached"), "rev-parse", "HEAD");
    await button(page, "保存为分支").click();
    await expect(button(page, "删除工作树")).toBeEnabled();
    await button(page, "删除工作树").click();
    await expect(row(page, "detached")).toHaveCount(0);
    expect(git(main, "rev-parse", saved)).toBe(tip);
  });
}

test("worktree unlock, stale removal review, move, repair and prune", async ({ page }) => {
  test.setTimeout(90_000);
  await openLabScene(page, "worktree-manage", ".checkout-list-row.active", { theme: "dark", locale: "zh" });
  await expect(dialog(page)).toContainText("external disk");
  await button(page, "打开工作树").click();
  await expect(row(page, "topic")).toHaveAttribute("aria-current", "true");
  await row(page, "worktree").dblclick();
  await expect(row(page, "worktree")).toHaveAttribute("aria-current", "true");
  await worktreeAction(page, "topic", "管理工作树");
  await expect(button(page, "删除工作树")).toBeDisabled();
  await button(page, "解锁工作树").click();
  await button(page, "删除工作树").click();
  await expect(dialog(page).getByText("ignored.txt", { exact: true })).toBeVisible();
  const force = dialog(page).getByRole("checkbox", { name: "强制删除并丢弃本地内容" });
  await force.check();
  writeFileSync(resolve(checkouts, "topic/ignored.txt"), "changed since preview\n");
  await button(page, "强制删除并丢弃本地内容").click();
  await expect(dialog(page).getByRole("alert")).toContainText("请刷新并核对工作树状态后删除");
  await expect(force).not.toBeChecked();
  expect(existsSync(resolve(checkouts, "topic/ignored.txt"))).toBe(true);

  await button(page, "取消").click();
  await button(page, "移动工作树").click();
  const parent = resolve(checkouts, "new-parent");
  mkdirSync(parent);
  await dialog(page).getByLabel("工作树目录", { exact: true }).fill(parent);
  await button(page, "移动工作树").click();
  await expect(page.locator('[data-worktree-path$="/new-parent/topic"]')).toBeVisible();
  await expect.poll(async () => (await page.locator(".tab[data-repo]").evaluateAll((tabs) =>
    tabs.map((tab) => tab.getAttribute("data-repo")?.replaceAll("\\", "/"))))
    .find((path) => path?.endsWith("/topic"))).toContain("/new-parent/topic");
  expect(existsSync(resolve(parent, "topic/local.txt"))).toBe(true);

  const relocated = resolve(checkouts, "relocated");
  renameSync(resolve(parent, "topic"), relocated);
  await button(page, "刷新").click();
  await expect(row(page, "topic")).toContainText("可清理");
  await button(page, "关闭").click();
  await worktreeAction(page, "topic", "修复工作树");
  await dialog(page).getByLabel("搬迁后的工作树目录", { exact: true }).fill(relocated);
  await button(page, "修复工作树").click();
  await expect(row(page, "relocated")).toBeVisible();
  await button(page, "清理失效记录").click();
  await expect(dialog(page).locator(".checkout-prune")).toContainText("worktrees/missing");
  await button(page, "清理记录").click();
  await expect(row(page, "missing")).toHaveCount(0);
  expect(git(main, "show-ref", "--verify", "refs/heads/missing")).toContain("refs/heads/missing");
});

test("worktree creation opens a checkout from a bare repository", async ({ page }) => {
  test.setTimeout(90_000);
  await openLabScene(page, "worktree-bare", "[data-worktree-path]", { theme: "apple", locale: "zh" });
  await expect(row(page, "worktree-bare.git")).toContainText("裸仓库");
  await page.locator('.sidebar .file-select[title^="main"]').click({ button: "right" });
  await page.getByRole("menuitem", { name: "检出", exact: true }).click();
  await expect(dialog(page).getByLabel("检出方式")).toHaveValue("existing");
  await expect(dialog(page).getByLabel("分支", { exact: true })).toHaveValue("main");
  await dialog(page).getByLabel("工作树目录", { exact: true }).fill("../worktree-checkouts/from-bare");
  await button(page, "创建").click();
  await expect(row(page, "from-bare")).toHaveAttribute("aria-current", "true");
  expect(git(resolve(checkouts, "from-bare"), "branch", "--show-current")).toBe("main");
});

test("checking out a commit in a bare repository creates a detached worktree", async ({ page }) => {
  test.setTimeout(90_000);
  const bare = resolve(".lab/repos/worktree-bare.git");
  const head = git(bare, "rev-parse", "HEAD");
  await openLabScene(page, "worktree-bare", "[data-worktree-path]", { theme: "light", locale: "zh" });
  await page.locator(".sidebar").getByRole("button", { name: "全部提交", exact: true }).click();
  await page.locator(".commit-row").filter({ hasText: "worktree main commit" }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "检出", exact: true }).click();
  await expect(dialog(page).getByLabel("检出方式")).toHaveValue("detached");
  await expect(dialog(page).getByLabel("起点", { exact: true })).toHaveValue(head);
  await dialog(page).getByLabel("工作树目录", { exact: true }).fill("../worktree-checkouts/detached-from-bare");
  await button(page, "创建").click();
  await expect(row(page, "detached-from-bare")).toHaveAttribute("aria-current", "true");
  const checkout = resolve(checkouts, "detached-from-bare");
  expect(git(checkout, "branch", "--show-current")).toBe("");
  expect(git(checkout, "rev-parse", "HEAD")).toBe(head);
});
