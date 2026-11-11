import { expect, test, type BrowserContext, type Page } from "@playwright/test";
import { openLabScene } from "./helpers";
import type { Settings } from "../src/lib/status";

type Call = { cmd: string; args: any };
async function isolatedPreferences(context: BrowserContext, overrides: Partial<Settings> = {}) {
  let settings: Settings;
  let initial: Promise<void> | undefined;
  const calls: Call[] = [];
  const control = { failNext: false, pauseNext: null as Promise<void> | null, values: () => settings, calls };
  await context.route("**/__lab/invoke", async (route) => {
    const body = route.request().postDataJSON() as Call;
    calls.push(body);
    if (body.cmd === "load_settings") {
      initial ??= (async () => {
        const payload = await (await route.fetch()).json();
        settings = { ...payload.value, locale: "zh", diffStyle: "split", linesHeight: "compact", diffFontSize: 13,
          diffWordWrap: false, showEntireFile: false, diffContext: 3, ignoreSpace: false, showWhitespace: false,
          fileLayout: "tree", treeFiles: true, pushOnCommit: false, signCommits: false, flowAutoClose: false,
          verticalTabs: false, ...overrides };
      })();
      await initial;
      return route.fulfill({ json: { ok: true, value: settings! } });
    }
    if (body.cmd === "save_settings") {
      await initial;
      if (body.args.patch && control.pauseNext) {
        const pause = control.pauseNext;
        control.pauseNext = null;
        await pause;
      }
      if (control.failNext && body.args.patch) {
        control.failNext = false;
        return route.fulfill({ json: { ok: false, error: "preference-save-test" } });
      }
      settings = body.args.patch ? { ...settings!, ...body.args.patch } : body.args.values;
      return route.fulfill({ json: { ok: true, value: settings } });
    }
    return route.continue();
  });
  return control;
}

const pref = (page: Page, key: string, scope = "section.diff") => page.locator(`${scope} [data-preference="${key}"]`);
function reading(page: Page, cmd: string, matches: (args: any) => boolean) {
  return page.waitForResponse((response) => {
    if (!response.url().endsWith("/__lab/invoke") || response.request().method() !== "POST") return false;
    const body = response.request().postDataJSON();
    return body.cmd === cmd && matches(body.args);
  });
}

test("diff controls apply immediately and preferences keep the same saved values", async ({ page, context }) => {
  const state = await isolatedPreferences(context);
  await openLabScene(page, "contextual-diff", ".diff-body.split");
  const reads = () => state.calls.filter((call) => call.cmd === "file_diff").length;
  const before = reads();
  await pref(page, "diffStyle").selectOption("unified");
  await expect(page.locator("section.diff .diff-body")).not.toHaveClass(/split/);
  await pref(page, "diffWordWrap").check();
  await expect(page.locator(".shell")).toHaveClass(/diff-wrap/);
  await pref(page, "linesHeight").selectOption("spaced");
  await expect.poll(() => page.locator(".shell").evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--diff-row").trim())).toBe("23px");
  expect(reads()).toBe(before);
  await page.getByRole("button", { name: "偏好设置", exact: true }).click();
  const dialog = page.locator('[data-dialog="prefs"]');
  await dialog.getByRole("button", { name: "差异", exact: true }).click();
  await expect(pref(page, "diffWordWrap", '[data-dialog="prefs"]')).toBeChecked();
  await expect(pref(page, "diffStyle", '[data-dialog="prefs"]')).toHaveValue("unified");
  await pref(page, "diffWordWrap", '[data-dialog="prefs"]').uncheck();
  await dialog.locator(".dialog-close").click();
  await expect(pref(page, "diffWordWrap")).not.toBeChecked();
  await expect.poll(() => state.values().diffWordWrap).toBe(false);
  await page.reload();
  await expect(pref(page, "diffStyle")).toHaveValue("unified");
  await expect(pref(page, "linesHeight")).toHaveValue("spaced");
});

test("diff content controls reload context and whitespace without changing the selected file", async ({ page, context }) => {
  await isolatedPreferences(context);
  await openLabScene(page, "contextual-diff", ".diff-body.split");
  const whole = reading(page, "file_diff", (args) => args.unified === 100000);
  await pref(page, "showEntireFile").check();
  const wholeData = await (await whole).json();
  expect(wholeData.value.lines.length).toBeGreaterThan(240);
  await expect(pref(page, "diffContext")).toBeDisabled();
  await expect(page.locator(".diff-head").first()).toContainText("sample.ts");
  const partial = reading(page, "file_diff", (args) => args.unified === 3);
  await pref(page, "showEntireFile").uncheck();
  expect((await (await partial).json()).value.lines.length).toBeLessThan(40);
  const ignoring = reading(page, "file_diff", (args) => args.ignoreSpace === true);
  await pref(page, "ignoreSpace").check();
  const lines = (await (await ignoring).json()).value.lines;
  expect(lines.filter((line: any) => line.kind === "hunk")).toHaveLength(1);
  await expect(page.locator("section.diff .split-code.add")).toContainText(["line200"]);
});

test("history diff invalidates cached content and keeps the selected commit and file", async ({ page, context }) => {
  await isolatedPreferences(context);
  await openLabScene(page, "contextual-history", ".detail .diff-body");
  const sha = await page.locator(".strip-sha").textContent();
  const whole = reading(page, "show_commit_file", (args) => args.unified === 100000);
  await pref(page, "showEntireFile", ".detail").check();
  expect((await (await whole).json()).value.lines.length).toBeGreaterThan(240);
  await expect(page.locator(".strip-sha")).toHaveText(sha!);
  const ignoring = reading(page, "show_commit_file", (args) => args.ignoreSpace === true);
  await pref(page, "ignoreSpace", ".detail").check();
  const lines = (await (await ignoring).json()).value.lines;
  expect(lines.filter((line: any) => line.kind === "add")).toHaveLength(1);
  await expect(page.locator(".detail .tree-row.selected")).toContainText("sample.ts");
});

test("concurrent windows synchronize field patches and retain the composer draft", async ({ page, context }) => {
  const state = await isolatedPreferences(context);
  await openLabScene(page, "contextual-diff", ".diff-body");
  await page.locator(".summary-input").fill("Keep my draft");
  const second = await context.newPage();
  await openLabScene(second, "contextual-history", ".detail .diff-body");
  await Promise.all([pref(page, "diffWordWrap").check(), pref(second, "linesHeight", ".detail").selectOption("spaced")]);
  await expect(pref(page, "linesHeight")).toHaveValue("spaced");
  await expect(pref(second, "diffWordWrap", ".detail")).toBeChecked();
  await expect(page.locator(".summary-input")).toHaveValue("Keep my draft");
  expect(state.values().diffWordWrap).toBe(true);
  expect(state.values().linesHeight).toBe("spaced");
  const patches = state.calls.filter((call) => call.cmd === "save_settings" && call.args.patch).map((call) => call.args.patch);
  expect(patches).toContainEqual({ diffWordWrap: true });
  expect(patches).toContainEqual({ linesHeight: "spaced" });
});

test("save failure restores the control and reports the error without losing text", async ({ page, context }) => {
  const state = await isolatedPreferences(context);
  await openLabScene(page, "contextual-diff", ".diff-body");
  await page.locator(".summary-input").fill("Retain this text");
  state.failNext = true;
  await pref(page, "diffWordWrap").check();
  await expect(pref(page, "diffWordWrap")).not.toBeChecked();
  await expect(page.locator(".summary-input")).toHaveValue("Retain this text");
  await expect(page.getByText(/preference-save-test/).first()).toBeVisible();
  await pref(page, "diffWordWrap").check();
  await expect.poll(() => state.values().diffWordWrap).toBe(true);
});

test("file layout controls and composer helpers operate from their own regions", async ({ page, context }) => {
  const state = await isolatedPreferences(context);
  await openLabScene(page, "contextual-diff", ".diff-body");
  await pref(page, "fileLayout", ".changes-column").selectOption("combined");
  await expect(pref(page, "swapPanes", ".changes-column")).toBeDisabled();
  await expect.poll(() => state.values().treeFiles).toBe(false);
  await pref(page, "fileLayout", ".changes-column").selectOption("tree");
  await pref(page, "swapPanes", ".changes-column").check();
  await expect(page.locator(".changes-column .status-pane").first()).toHaveClass(/staged-pane/);
  await pref(page, "pushOnCommit", ".composer").check();
  await expect(page.locator(".composer button.commit")).toHaveText("提交并推送");
  await pref(page, "aiPromptStyle", ".composer").selectOption("long");
  await pref(page, "aiLanguage", ".composer").selectOption("en");
  await expect(page.locator(".suggest-message")).toHaveClass(/long/);
  await expect.poll(() => state.values().aiLanguage).toBe("en");
});

test("flow preferences synchronize with the main preferences page", async ({ page, context }) => {
  await isolatedPreferences(context);
  await openLabScene(page, "operation-flow", ".flow-window");
  await expect(pref(page, "flowAutoClose", ".flow-window")).toBeVisible();
  const main = await context.newPage();
  await openLabScene(main, "prefs-flow", '[data-dialog="prefs"]');
  await pref(page, "flowAutoClose", ".flow-window").check();
  await expect(pref(main, "flowAutoClose", '[data-dialog="prefs"]')).toBeChecked();
  await pref(main, "flowAutoClose", '[data-dialog="prefs"]').uncheck();
  await expect(pref(page, "flowAutoClose", ".flow-window")).not.toBeChecked();
});

test("tag push is a single-operation choice seeded from preferences", async ({ page, context }) => {
  const state = await isolatedPreferences(context, { pushTagOnCreate: true });
  await openLabScene(page, "dialog-tag", '[data-dialog="tag"]');
  const checkbox = page.locator("[data-tag-push]");
  await expect(checkbox).toBeChecked();
  await checkbox.uncheck();
  expect(state.values().pushTagOnCreate).toBe(true);
  await page.reload();
  await expect(checkbox).toBeChecked();
});

test("backend field patches persist concurrent edits and preserve unrelated settings", async ({ request }) => {
  const call = async (cmd: string, args = {}) => (await (await request.post("/__lab/invoke", { data: { cmd, args } })).json());
  const original = (await call("load_settings")).value;
  try {
    const replies = await Promise.all([
      call("save_settings", { patch: { diffContext: 17 } }),
      call("save_settings", { patch: { diffFontSize: 16 } }),
    ]);
    expect(replies.every((item) => item.ok)).toBe(true);
    const saved = (await call("load_settings")).value;
    expect(saved.diffContext).toBe(17);
    expect(saved.diffFontSize).toBe(16);
    expect(saved.recent).toEqual(original.recent);
    expect(saved.theme).toBe(original.theme);
  } finally { expect((await call("save_settings", { values: original })).ok).toBe(true); }
});

test("diff layout changes preserve the source line and virtual scrolling reaches the tail", async ({ page, context }) => {
  await isolatedPreferences(context, { showEntireFile: true });
  await openLabScene(page, "contextual-diff", ".diff-body.split");
  const body = page.locator("section.diff .diff-body");
  const topLine = () => body.evaluate((node) => {
    const bounds = node.getBoundingClientRect();
    return [...node.querySelectorAll<HTMLElement>(".split-code, .virtual-row")]
      .find((row) => row.getBoundingClientRect().bottom > bounds.top + 2)?.textContent?.match(/line(\d+)/)?.[1];
  });
  await body.evaluate((node) => { node.scrollTop = 2000; });
  await expect.poll(topLine).toBeDefined();
  const before = Number(await topLine());
  await pref(page, "diffStyle").selectOption("unified");
  await expect.poll(async () => Math.abs(Number(await topLine()) - before)).toBeLessThanOrEqual(1);
  await pref(page, "linesHeight").selectOption("spaced");
  await pref(page, "diffWordWrap").check();
  await expect.poll(async () => Math.abs(Number(await topLine()) - before)).toBeLessThanOrEqual(1);
  await pref(page, "diffFontSize").fill("17");
  await pref(page, "diffFontSize").press("Tab");
  await expect.poll(async () => Math.abs(Number(await topLine()) - before)).toBeLessThanOrEqual(1);
  await body.evaluate((node) => { node.scrollTop = node.scrollHeight; });
  await expect(body).toContainText("line240");
  expect(await body.locator(".virtual-row").count()).toBeLessThan(200);
});

test("a late diff response cannot replace newer whitespace options", async ({ page, context }) => {
  await isolatedPreferences(context);
  await openLabScene(page, "contextual-diff", ".diff-body.split");
  let release!: () => void;
  const gate = new Promise<void>((resolve) => { release = resolve; });
  let started!: () => void;
  const pending = new Promise<void>((resolve) => { started = resolve; });
  await page.route("**/__lab/invoke", async (route) => {
    const { cmd, args } = route.request().postDataJSON();
    if (cmd !== "file_diff" || args.unified !== 0) return route.fallback();
    const response = await route.fetch();
    started();
    await gate;
    return route.fulfill({ response });
  });
  await pref(page, "diffContext").fill("0");
  await pref(page, "diffContext").press("Tab");
  await pending;
  await pref(page, "diffContext").fill("3");
  await pref(page, "diffContext").press("Tab");
  const latest = reading(page, "file_diff", (args) => args.ignoreSpace === true);
  await pref(page, "ignoreSpace").check();
  await latest;
  release();
  await expect(page.locator("section.diff .split-code.add")).toHaveCount(1);
  await expect(page.locator("section.diff .split-code.add")).toContainText("line200");
  await pref(page, "diffStyle").selectOption("unified");
  await pref(page, "showDiffMarks").uncheck();
  await expect(page.locator("section.diff .virtual-row.add")).not.toContainText("+");
  await pref(page, "showDiffMarks").check();
  await expect(page.locator("section.diff .virtual-row.add")).toContainText("+");
});

test("history and repository controls retain selection and the draft", async ({ page, context }) => {
  const state = await isolatedPreferences(context);
  await openLabScene(page, "contextual-history", ".detail .diff-body");
  const sha = await page.locator(".strip-sha").textContent();
  await pref(page, "commitSort", ".history-stage").selectOption("date");
  await expect.poll(() => state.values().commitSort).toBe("date");
  await expect(page.locator(".strip-sha")).toHaveText(sha!);
  await pref(page, "verticalTabs", ".shell").check();
  await expect(page.locator(".shell")).toHaveClass(/vertical-tabs/);
  await pref(page, "showWorktrees", ".sidebar").uncheck();
  await expect.poll(() => state.values().showWorktrees).toBe(false);
});

test("compact windows keep diff content and submit controls reachable", async ({ page, context }) => {
  await isolatedPreferences(context, { locale: "en" });
  await page.setViewportSize({ width: 960, height: 640 });
  await openLabScene(page, "contextual-diff", ".diff-body");
  await expect(page.locator(".composer .commit")).toBeInViewport();
  const size = await page.locator("section.diff .diff-body").boundingBox();
  expect(size!.height).toBeGreaterThan(90);
  const clipped = await page.locator(".composer").evaluate((node) => {
    const bounds = node.getBoundingClientRect();
    return [...node.querySelectorAll<HTMLElement>("input, select, textarea, button.commit")]
      .filter((item) => item.getBoundingClientRect().bottom > bounds.bottom + 1).map((item) => item.className);
  });
  expect(clipped).toEqual([]);
  await page.setViewportSize({ width: 480, height: 360 });
  await page.goto("/?scene=operation-flow&view=flow&file=live&locale=en");
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(pref(page, "flowHistoryDays", ".flow-window")).toBeInViewport();
  expect(await page.locator(".flow-scroll").evaluate((node) => node.clientHeight)).toBeGreaterThan(60);
  await page.screenshot({ path: ".lab/contextual-flow-compact.png" });
});

test("expanded commit patches grow with context and keep their tail reachable", async ({ page, context }) => {
  await isolatedPreferences(context);
  await openLabScene(page, "contextual-history", ".detail .diff-body");
  await page.locator(".detail").getByRole("tab", { name: "提交", exact: true }).click();
  await page.locator(".patch-toggle").first().click();
  const patch = page.locator(".commit-patch");
  await expect(patch.locator(".split-code").first()).toBeVisible();
  const initial = (await patch.boundingBox())!.height;
  const loaded = reading(page, "show_commit_file", (args) => args.unified === 100000);
  await pref(page, "showEntireFile", ".detail").check();
  await loaded;
  await expect.poll(async () => (await patch.boundingBox())!.height).toBeGreaterThan(initial * 4);
  await pref(page, "diffStyle", ".detail").selectOption("unified");
  await pref(page, "linesHeight", ".detail").selectOption("spaced");
  await page.locator(".detail-scroll").evaluate((node) => { node.scrollTop = node.scrollHeight; });
  await expect(patch).toContainText("line240");
  expect(await patch.locator(".virtual-row").count()).toBeLessThan(200);
});

test("commit waits for signing preferences and performs the selected follow-up push", async ({ page, context }) => {
  const state = await isolatedPreferences(context);
  const actions: string[] = [];
  await page.route("**/__lab/invoke", async (route) => {
    const { cmd, args } = route.request().postDataJSON();
    if (cmd !== "commit_changes" && cmd !== "mutate") return route.fallback();
    if (cmd === "commit_changes") expect(state.values().signCommits).toBe(true);
    actions.push(cmd === "mutate" ? args.request.action : cmd);
    const status = await page.request.post("/__lab/invoke", { data: { cmd: "workspace_status", args: { path: args.path } } });
    return route.fulfill({ json: await status.json() });
  });
  await openLabScene(page, "contextual-diff", ".composer");
  await page.locator(".summary-input").fill("Verify contextual settings");
  await pref(page, "pushOnCommit", ".composer").check();
  let release!: () => void;
  state.pauseNext = new Promise<void>((resolve) => { release = resolve; });
  await pref(page, "signCommits", ".composer").check();
  await page.locator(".composer .commit").click();
  expect(actions).toEqual([]);
  release();
  await expect.poll(() => actions).toEqual(["commit_changes", "push"]);
});

for (const push of [false, true]) test(`tag creation honors the single-operation push choice ${push}`, async ({ page, context }) => {
  await isolatedPreferences(context, { pushTagOnCreate: !push });
  const actions: string[] = [];
  await page.route("**/__lab/invoke", async (route) => {
    const { cmd, args } = route.request().postDataJSON();
    if (cmd !== "mutate") return route.fallback();
    actions.push(args.request.action);
    const status = await page.request.post("/__lab/invoke", { data: { cmd: "workspace_status", args: { path: args.path } } });
    return route.fulfill({ json: await status.json() });
  });
  await openLabScene(page, "dialog-tag", '[data-dialog="tag"]');
  await page.locator("[data-tag-push]").setChecked(push);
  await page.locator('[data-dialog="tag"] input:not([type="checkbox"])').first().fill("test-contextual-tag");
  await page.locator('[data-dialog="tag"] button[type="submit"]').click();
  await expect(page.locator('[data-dialog="tag"]')).toHaveCount(0);
  await expect.poll(() => actions).toEqual(push ? ["tag", "push"] : ["tag"]);
});

test("command retention only removes expired errors after settings are saved", async ({ page, context }) => {
  const state = await isolatedPreferences(context, { errorLogDays: 7 });
  await page.addInitScript(() => localStorage.setItem("awegit-errors", JSON.stringify([
    { id: 900, at: Date.now() - 3 * 86400000, title: "Older error", text: "keep until saved", command: "" },
    { id: 901, at: Date.now(), title: "Recent error", text: "retain", command: "" },
  ])));
  await openLabScene(page, "contextual-diff", ".diff-body");
  await page.getByRole("button", { name: "命令记录", exact: true }).click();
  const retention = pref(page, "errorLogDays", ".command-log");
  const stored = () => page.evaluate(() => JSON.parse(localStorage.getItem("awegit-errors") || "[]").map((row: { id: number }) => row.id));
  state.failNext = true;
  await retention.fill("1");
  await retention.press("Tab");
  await expect(retention).toHaveValue("7");
  expect(await stored()).toContain(900);
  await retention.fill("1");
  await retention.press("Tab");
  await expect.poll(stored).not.toContain(900);
  expect(await stored()).toContain(901);
});
