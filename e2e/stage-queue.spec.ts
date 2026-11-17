import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, renameSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { pinLabSettings } from "./helpers";

const repo = resolve(".lab/repos/stage-many");
const git = (...args: string[]) => execFileSync("git", args, { cwd: repo, encoding: "utf8", stdio: "pipe" });
const audit = ".lab/ui-audit/stage-queue";
const a = "loose/file-00000.txt", b = "loose/file-00001.txt", c = "loose/file-00002.txt";
const writes = new Set(["stage_path", "unstage_path", "stage_all", "unstage_all", "mutate"]);
type Layout = "tree" | "list" | "combined";

test.beforeAll(() => {
  execFileSync(process.execPath, ["scripts/lab-seed.mjs", "stage-many"], { stdio: "pipe" });
  mkdirSync(audit, { recursive: true });
});

test.beforeEach(() => {
  // Only the generated fixture's index is reset. Working files remain intact.
  git("reset", "--quiet", "--no-refresh", "--", ".");
});

async function open(page: Page, theme: string, fileLayout: Layout) {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await pinLabSettings(page, { theme, locale: "zh", fileLayout, fetchAutomatically: false, statusInterval: 0 });
  await page.goto(`/?scene=stage-queue&theme=${theme}&locale=zh`);
  await expect(page.locator('[data-lab-scene="stage-queue"]')).toBeVisible();
  await idle(page);
  // Lab startup deliberately defaults to list layout, so exercise the real view menu.
  if (fileLayout !== "list") {
    await page.locator(".unstaged-pane .pane-more").click();
    await page.locator('[data-layer="menu"]').getByRole("menuitemradio", {
      name: fileLayout === "tree" ? "按树查看" : "按合并列表查看", exact: true,
    }).click();
  }
  return errors;
}

async function idle(page: Page) {
  await expect(page.locator(".stage-pending")).toHaveCount(0);
  await expect(page.locator(".identity-floor.tone-busy")).toHaveCount(0);
}

function row(page: Page, layout: Layout, side: "staged" | "unstaged", file: string) {
  if (layout !== "combined") return page.locator(`.${side}-pane .file-select[title="${file}"]`);
  return page.locator(".file-list button.side")
    .filter({ has: page.getByText(file, { exact: true }) })
    .filter({ has: page.getByText(side === "staged" ? "已暂存" : "未暂存", { exact: true }) });
}

async function holdWrites(page: Page, failFile?: string) {
  let release!: () => void;
  const gate = new Promise<void>((done) => { release = done; });
  const calls: { cmd: string; file?: string; unstage?: boolean }[] = [];
  await page.route("**/__lab/invoke", async (route) => {
    const body = route.request().postDataJSON();
    if (!writes.has(body.cmd)) return route.fallback();
    calls.push({ cmd: body.cmd, file: body.args.file, unstage: body.args.request?.unstage });
    if (calls.length === 1) await gate;
    if (failFile && body.args.file === failFile) {
      return route.fulfill({ json: { ok: false, error: "stage queue test: index.lock is held" } });
    }
    return route.fallback();
  });
  return { release, calls };
}

for (const theme of ["apple", "light", "dark"]) for (const layout of ["tree", "list", "combined"] as const) {
  test(`stage queue moves rows before Git and preserves rapid inverse intents (${theme}, ${layout})`, async ({ page }) => {
    test.setTimeout(60_000);
    const errors = await open(page, theme, layout);
    const held = await holdWrites(page);
    try {
      await row(page, layout, "unstaged", a).dblclick();
      await expect(row(page, layout, "staged", a)).toBeVisible();
      await expect(page.locator("[data-stage-pending]")).toHaveText("正在暂存…");
      await expect.poll(() => held.calls.length).toBe(1);
      expect(git("diff", "--cached", "--name-only")).toBe("");

      await row(page, layout, "staged", a).dblclick();
      await expect(row(page, layout, "unstaged", a)).toBeVisible();
      await expect(page.locator("[data-stage-pending]")).toHaveText("正在取消暂存…");
      await row(page, layout, "unstaged", a).dblclick();
      await row(page, layout, "unstaged", b).dblclick();
      await expect(row(page, layout, "staged", a)).toBeVisible();
      await expect(row(page, layout, "staged", b)).toBeVisible();
      await expect.poll(() => held.calls.length).toBe(4);
      expect(git("diff", "--cached", "--name-only")).toBe("");

      await page.locator(".summary-input").fill("still responsive");
      await expect(page.locator(".summary-input")).toHaveValue("still responsive");
      await expect(page.locator('.composer button[type="submit"]')).toBeDisabled();
      const list = page.locator(".unstaged-pane .file-list");
      await list.evaluate((node) => { node.scrollTop = 3000; });
      await expect.poll(() => list.evaluate((node) => node.scrollTop)).toBeGreaterThan(1000);
      expect(await list.locator(".file, button.side").count()).toBeLessThan(200);
      await list.evaluate((node) => { node.scrollTop = 0; });
      await expect(row(page, layout, "staged", b).locator(".stage-pending")).toBeVisible();
      await page.screenshot({ path: `${audit}/pending-${theme}-${layout}.png` });
    } finally { held.release(); }
    await idle(page);
    expect(held.calls.map((call) => call.cmd)).toEqual(["stage_path", "unstage_path", "stage_path", "stage_path"]);
    expect(git("diff", "--cached", "--name-only").trim().split("\n")).toEqual([a, b]);
    expect(git("show", `:${a}`)).toBe(readFileSync(resolve(repo, a), "utf8"));
    expect(git("show", `:${b}`)).toBe(readFileSync(resolve(repo, b), "utf8"));
    expect(errors).toEqual([]);
  });
}

for (const theme of ["apple", "light", "dark"]) {
  test(`folder, all and selected staging share immediate feedback and ordering (${theme})`, async ({ page }) => {
    test.setTimeout(90_000);
    const errors = await open(page, theme, "tree");
    const held = await holdWrites(page);
    try {
      await row(page, "tree", "unstaged", "loose").dblclick();
      await expect(row(page, "tree", "staged", "loose")).toBeVisible();
      await expect(page.locator(".staged-pane .pane-head .count")).toHaveText("5000");
      await expect.poll(() => held.calls.length).toBe(1);
      expect(git("diff", "--cached", "--name-only")).toBe("");
      await row(page, "tree", "staged", "loose").dblclick();
      await expect(page.locator(".staged-pane .pane-head .count")).toHaveText("0");
      await page.getByRole("button", { name: "全部暂存", exact: true }).click();
      await expect(page.locator(".staged-pane .pane-head .count")).toHaveText("10000");
      await page.getByRole("button", { name: "全部取消暂存", exact: true }).click();
      await expect(page.locator(".staged-pane .pane-head .count")).toHaveText("0");
      await expect.poll(() => held.calls.length).toBe(4);
      expect(git("diff", "--cached", "--name-only")).toBe("");
    } finally { held.release(); }
    await idle(page);
    expect(held.calls.map((call) => call.cmd)).toEqual(["stage_path", "unstage_path", "stage_all", "unstage_all"]);
    expect(git("diff", "--cached", "--name-only")).toBe("");

    await page.locator(".unstaged-pane .file-list").evaluate((node) => { node.scrollTop = 0; });
    const batch = await holdWrites(page);
    try {
      await row(page, "tree", "unstaged", a).click();
      await row(page, "tree", "unstaged", b).click({ modifiers: ["ControlOrMeta"] });
      await page.getByRole("button", { name: "暂存所选", exact: true }).click();
      await expect(row(page, "tree", "staged", a)).toBeVisible();
      await expect(row(page, "tree", "staged", b)).toBeVisible();
      await row(page, "tree", "staged", a).click();
      await row(page, "tree", "staged", b).click({ modifiers: ["ControlOrMeta"] });
      await page.getByRole("button", { name: "取消暂存所选", exact: true }).click();
      await expect(page.locator(".staged-pane .pane-head .count")).toHaveText("0");
      await expect.poll(() => batch.calls.length).toBe(2);
    } finally { batch.release(); }
    await idle(page);
    expect(batch.calls).toEqual([{ cmd: "mutate", unstage: false }, { cmd: "mutate", unstage: true }]);
    expect(git("diff", "--cached", "--name-only")).toBe("");
    expect(git("diff", "--name-only").trim().split("\n")).toHaveLength(5000);
    expect(git("ls-files", "--others", "--exclude-standard", "-z").split("\0").filter(Boolean)).toHaveLength(5000);
    expect(errors).toEqual([]);
  });
}

test("a failed write restores confirmed state, cancels dependent intents and allows retry", async ({ page }) => {
  test.setTimeout(60_000);
  const errors = await open(page, "apple", "list");
  await row(page, "list", "unstaged", a).dblclick();
  await idle(page);
  const held = await holdWrites(page, b);
  try {
    await row(page, "list", "unstaged", b).dblclick();
    await row(page, "list", "staged", a).dblclick();
    await row(page, "list", "unstaged", c).dblclick();
    await expect(row(page, "list", "staged", c)).toBeVisible();
    expect(git("diff", "--cached", "--name-only").trim()).toBe(a);
  } finally { held.release(); }
  await idle(page);
  expect(held.calls).toEqual([{ cmd: "stage_path", file: b }, { cmd: "unstage_path", file: a }, { cmd: "stage_path", file: c }]);
  expect(git("diff", "--cached", "--name-only").trim()).toBe(a);
  await expect(row(page, "list", "staged", a)).toBeVisible();
  await expect(row(page, "list", "unstaged", b)).toBeVisible();
  await expect(row(page, "list", "unstaged", c)).toBeVisible();
  await expect(page.locator(".identity-floor")).toContainText("index.lock");
  await row(page, "list", "unstaged", c).dblclick();
  await idle(page);
  expect(git("diff", "--cached", "--name-only").trim().split("\n")).toEqual([a, c]);
  expect(errors).toEqual([]);
});

test("partial staging keeps working contents and shows the final side after queued inverses", async ({ page }) => {
  test.setTimeout(60_000);
  const file = "tracked/file-00000.txt";
  writeFileSync(resolve(repo, file), "index version\n");
  git("add", "--", file);
  writeFileSync(resolve(repo, file), "working\n");
  const errors = await open(page, "dark", "list");
  const held = await holdWrites(page);
  try {
    await row(page, "list", "staged", file).dblclick();
    await expect(row(page, "list", "staged", file)).toHaveCount(0);
    await expect(page.locator("[data-stage-pending]")).toHaveText("正在取消暂存…");
    expect(git("show", `:${file}`)).toBe("index version\n");
    // The tracked folder is outside the initial virtual window on the unstaged side.
    await page.locator(".unstaged-pane .file-list").evaluate((node) => {
      node.scrollTop = (node.querySelector<HTMLElement>(".commit-window")?.offsetHeight ?? 0) / 2;
    });
    await expect(row(page, "list", "unstaged", file)).toBeVisible();
    await row(page, "list", "unstaged", file).dblclick();
    await expect(row(page, "list", "staged", file)).toBeVisible();
    await expect.poll(() => held.calls.length).toBe(2);
  } finally { held.release(); }
  await idle(page);
  await expect(row(page, "list", "staged", file)).toBeVisible();
  expect(git("show", `:${file}`)).toBe("working\n");
  expect(readFileSync(resolve(repo, file), "utf8")).toBe("working\n");
  expect(errors).toEqual([]);
});

test("selecting a pending file invalidates a late preview of another file", async ({ page }) => {
  test.setTimeout(60_000);
  const errors = await open(page, "light", "list");
  const held = await holdWrites(page);
  let releaseOld!: () => void, releaseNew!: () => void;
  const oldGate = new Promise<void>((done) => { releaseOld = done; });
  const newGate = new Promise<void>((done) => { releaseNew = done; });
  let oldReading = false, newReading = false;
  await page.route("**/__lab/invoke", async (route) => {
    const body = route.request().postDataJSON();
    if (body.cmd !== "file_diff") return route.fallback();
    if (body.args.file === c && !body.args.staged) {
      const response = await route.fetch();
      oldReading = true;
      await oldGate;
      return route.fulfill({ response });
    }
    if (body.args.file === a && body.args.staged) {
      newReading = true;
      await newGate;
    }
    return route.fallback();
  });
  try {
    await row(page, "list", "unstaged", a).dblclick();
    await row(page, "list", "unstaged", c).click();
    await expect.poll(() => oldReading).toBe(true);
    await row(page, "list", "staged", a).click();
    const oldDone = page.waitForResponse((response) => response.url().includes("/__lab/invoke")
      && response.request().postDataJSON()?.cmd === "file_diff"
      && response.request().postDataJSON()?.args.file === c);
    releaseOld();
    await oldDone;
    await expect(page.locator("[data-stage-pending]")).toBeVisible();
    held.release();
    await expect.poll(() => newReading).toBe(true);
    await expect(page.locator(".stage-pending")).toHaveCount(0);
    // Until the new preview arrives, the other file's old diff must stay hidden.
    await expect(page.locator("section.diff .diff-body")).toHaveCount(0);
  } finally { releaseOld(); held.release(); releaseNew(); }
  await idle(page);
  await expect(page.locator("section.diff .diff-head > span")).toHaveText(a);
  await expect(page.locator("section.diff .diff-body")).toBeVisible();
  expect(errors).toEqual([]);
});

test("batch stage failures preserve stale lock removal and retry", async ({ page }) => {
  test.setTimeout(60_000);
  const errors = await open(page, "dark", "list");
  const lock = resolve(repo, ".git/index.lock");
  writeFileSync(lock, "");
  const earlier = new Date(Date.now() - 3600_000);
  utimesSync(lock, earlier, earlier);
  try {
    await row(page, "list", "unstaged", a).click();
    await row(page, "list", "unstaged", b).click({ modifiers: ["ControlOrMeta"] });
    await page.getByRole("button", { name: "暂存所选", exact: true }).click();
    const confirm = page.locator('[data-layer="confirm"]');
    await expect(confirm).toContainText("移除残留的 index.lock 并重试？");
    await expect(confirm).toContainText("锁文件已存在");
    expect(git("diff", "--cached", "--name-only")).toBe("");
    await confirm.getByRole("button", { name: "移除并重试", exact: true }).click();
    await expect(row(page, "list", "staged", a)).toBeVisible();
    await idle(page);
    expect(git("diff", "--cached", "--name-only").trim().split("\n")).toEqual([a, b]);
    expect(errors).toEqual([]);
  } finally { rmSync(lock, { force: true }); }
});

test("renamed files keep their confirmed sides until the index result arrives", async ({ page }) => {
  test.setTimeout(60_000);
  const old = "tracked/file-00000.txt", renamed = "renamed.txt";
  git("mv", "--", old, renamed);
  try {
    const errors = await open(page, "apple", "list");
    const held = await holdWrites(page);
    try {
      await row(page, "list", "staged", renamed).dblclick();
      await expect(row(page, "list", "staged", renamed).locator(".stage-pending")).toBeVisible();
      await expect(page.locator("[data-stage-pending]")).toHaveText("正在取消暂存…");
      expect(git("diff", "--cached", "--name-status")).toContain("R100");
    } finally { held.release(); }
    await idle(page);
    await expect(row(page, "list", "staged", renamed)).toHaveCount(0);
    await page.locator(".unstaged-pane .file-list").evaluate((node) => {
      node.scrollTop = (node.querySelector<HTMLElement>(".commit-window")?.offsetHeight ?? 0) / 2;
    });
    await expect(row(page, "list", "unstaged", renamed)).toBeVisible();
    // Native path-limited reset may leave the old path's deletion staged.
    // The UI must show the actual result instead of guessing both boundaries.
    const cached = git("diff", "--cached", "--name-only").trim().split("\n").filter(Boolean);
    expect(cached).not.toContain(renamed);
    await expect(page.locator(".staged-pane .pane-head .count")).toHaveText(String(cached.length));
    expect(readFileSync(resolve(repo, renamed), "utf8")).toBe("working\n");
    expect(errors).toEqual([]);
  } finally {
    git("reset", "--quiet", "--no-refresh", "--", old, renamed);
    renameSync(resolve(repo, renamed), resolve(repo, old));
  }
});

for (const unstage of [false, true]) {
  test(`an index-only addition disappears correctly after ${unstage ? "unstaging" : "staging its deletion"}`, async ({ page }) => {
    const contents = readFileSync(resolve(repo, a));
    git("add", "--", a);
    rmSync(resolve(repo, a));
    try {
      const errors = await open(page, "light", "list");
      const held = await holdWrites(page);
      try {
        await row(page, "list", unstage ? "staged" : "unstaged", a).dblclick();
        await expect(row(page, "list", "staged", a)).toHaveCount(0);
        await expect(row(page, "list", "unstaged", a)).toHaveCount(0);
        await expect.poll(() => held.calls.length).toBe(1);
        expect(git("diff", "--cached", "--name-only").trim()).toBe(a);
      } finally { held.release(); }
      await idle(page);
      expect(git("diff", "--cached", "--name-only")).toBe("");
      expect(errors).toEqual([]);
    } finally { writeFileSync(resolve(repo, a), contents); }
  });
}

test("tracked deletions can stage and unstage before the first write completes", async ({ page }) => {
  const file = "tracked/file-00000.txt";
  const contents = readFileSync(resolve(repo, file));
  rmSync(resolve(repo, file));
  try {
    const errors = await open(page, "dark", "list");
    const held = await holdWrites(page);
    try {
      await page.locator(".unstaged-pane .file-list").evaluate((node) => {
        node.scrollTop = (node.querySelector<HTMLElement>(".commit-window")?.offsetHeight ?? 0) / 2;
      });
      await row(page, "list", "unstaged", file).dblclick();
      await expect(row(page, "list", "staged", file)).toBeVisible();
      await row(page, "list", "staged", file).dblclick();
      await expect(row(page, "list", "staged", file)).toHaveCount(0);
      await expect.poll(() => held.calls.length).toBe(2);
    } finally { held.release(); }
    await idle(page);
    expect(git("diff", "--cached", "--name-only")).toBe("");
    expect(git("diff", "--name-status")).toContain(`D\t${file}`);
    expect(errors).toEqual([]);
  } finally { writeFileSync(resolve(repo, file), contents); }
});
