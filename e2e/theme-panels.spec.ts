import { expect, test, type Page } from "@playwright/test";
import { scenes } from "../src/lib/lab/scenes";
import { openScene } from "./helpers";

type ThemeName = "apple" | "light" | "dark";

const themes: { name: ThemeName; locale: "zh" | "en" }[] = [
  { name: "apple", locale: "zh" },
  { name: "light", locale: "zh" },
  { name: "dark", locale: "en" },
];

type PanelPage = {
  name: string;
  scene: string;
  panels: string[];
  tab?: string;
  tool?: string;
  history?: boolean;
  remote?: boolean;
  merge?: boolean;
  sidebar?: boolean;
  typeMenu?: boolean;
};

// One entry per panel surface. Grep the panel name so Apple, light, and dark run together.
const pages: PanelPage[] = [
  { name: "welcome", scene: "welcome", panels: [".welcome-view", ".welcome-titles", ".welcome-actions"] },
  { name: "changes", scene: "changes", sidebar: true, typeMenu: true, panels: [".chrome", ".chrome-tabs", ".sidebar", ".changes-column", ".unstaged-pane", ".staged-pane", ".composer"] },
  { name: "diff text", scene: "diff-text", panels: ["section.diff", ".diff-body"] },
  { name: "diff image", scene: "diff-image", panels: [".image-stage"] },
  { name: "diff binary", scene: "diff-binary", panels: [".binary-diff"] },
  { name: "history", scene: "history", history: true, panels: [".history-stage", ".history", ".commit-row"] },
  { name: "history remote", scene: "history-remote", remote: true, panels: [".history", ".commit-row.remote-only"] },
  { name: "commit detail", scene: "commit", panels: [".detail", ".detail-tabs", ".detail-message"] },
  { name: "file tree", scene: "tree", panels: [".detail", ".detail-tree", ".tree-row"] },
  { name: "conflict", scene: "conflict", panels: [".conflict-pane", ".conflict-title", ".conflict-file"] },
  { name: "merge text", scene: "text-merge-window", merge: true, panels: [".merge-panel.standalone", ".merge-panel-title", ".merge-parties", ".merge-top", ".merge-result-pane", ".merge-panel-foot"] },
  { name: "merge binary", scene: "file-merge", merge: true, panels: [".merge-panel.standalone", ".merge-panel-title", ".merge-parties", ".merge-stage.files", ".merge-panel-foot"] },
  { name: "merge hunks", scene: "text-merge-many", merge: true, panels: [".merge-panel.standalone", ".merge-count", ".merge-top", ".merge-result-pane", ".merge-panel-foot"] },
  { name: "file history", scene: "file-history", panels: [".file-history", ".history-file", ".history-list", ".history-diff"] },
  { name: "blob", scene: "blob", panels: [".blob-pane", ".blob-pane > header", ".blob-scroll"] },
  { name: "operation flow", scene: "operation-flow", panels: [".flow-window", ".flow-header", ".flow-overview", ".flow-graph-pane", ".flow-node.active", ".flow-footer"] },
  { name: "search", scene: "search", panels: ["[data-layer=\"search\"]"] },
  { name: "menu", scene: "menu", panels: ["[data-layer=\"menu\"]"] },
  { name: "confirm", scene: "confirm", panels: ["[data-layer=\"confirm\"]"] },
  { name: "command log", scene: "commands", panels: ["[data-layer=\"commands\"]"] },
  { name: "repository picker", scene: "picker", panels: ["[data-layer=\"repos\"]"] },
  { name: "update", scene: "update-available", panels: [".update-panel"] },
  { name: "preferences general", scene: "prefs-appearance", tab: "general", panels: [".pref-nav", ".pref-page", ".theme-styles"] },
  { name: "preferences workspace", scene: "prefs-workspace", tab: "workspace", panels: [".pref-nav", ".pref-page", ".pref-source-row"] },
  { name: "preferences diff", scene: "prefs-diff", tab: "diff", panels: [".pref-page", "[data-pref=\"diff-style\"]"] },
  { name: "preferences commit", scene: "prefs-commit", tab: "commit", panels: [".pref-page", ".commit-type-list"] },
  { name: "preferences ai", scene: "prefs-ai", tab: "ai", panels: [".pref-page", "[data-ai-field=\"enabled\"]"] },
  { name: "preferences git", scene: "prefs-git", tab: "git", panels: [".pref-page", ".pref-card"] },
  { name: "preferences integration", scene: "prefs-integration", tab: "integration", tool: "diff", panels: [".pref-page", ".tool-tabs", ".tool-fields"] },
  { name: "preferences integration merge", scene: "prefs-integration-merge", tab: "integration", tool: "merge", panels: [".pref-page", ".tool-tabs", ".tool-fields"] },
  { name: "preferences integration editor", scene: "prefs-integration-editor", tab: "integration", tool: "editor", panels: [".pref-page", ".tool-tabs", ".tool-fields"] },
  { name: "preferences commands", scene: "prefs-commands", tab: "commands", panels: [".pref-page", ".pref-card"] },
  { name: "preferences network", scene: "prefs-network", tab: "network", panels: [".pref-page", ".pref-card"] },
  { name: "preferences accounts", scene: "prefs-accounts", tab: "accounts", panels: [".pref-page", ".account-card", "[data-account-test]", "[data-remote-match]"] },
  { name: "preferences advanced", scene: "prefs-advanced", tab: "advanced", panels: [".pref-page", ".pref-card"] },
];

let accountSyncs = 0;

test.beforeEach(async ({ context }) => {
  accountSyncs = 0;
  let settings: Record<string, unknown> | undefined;
  await context.route("**/__lab/invoke", async (route) => {
    const body = route.request().postDataJSON() as { cmd?: string; args?: { values?: Record<string, unknown> } };
    if (body.cmd === "sync_accounts") accountSyncs += 1;
    if (body.cmd === "load_settings") {
      if (!settings) {
        const response = await route.fetch();
        const payload = await response.json();
        settings = payload.value;
      }
      return route.fulfill({ json: { ok: true, value: settings } });
    }
    if (body.cmd === "save_settings") {
      settings = body.args?.values;
      return route.fulfill({ json: { ok: true, value: settings } });
    }
    return route.continue();
  });
});

for (const theme of themes) {
  test.describe(theme.name, () => {
    for (const item of pages) {
      test(`theme panel ${item.name}`, async ({ page }) => {
        test.setTimeout(60_000);
        const scene = scenes.find((entry) => entry.id === item.scene);
        if (!scene) throw new Error(`Unknown lab scene: ${item.scene}`);
        await openScene(page, scene, `&locale=${theme.locale}&theme=${theme.name}`);
        await expect(page.locator("html")).toHaveAttribute("data-theme", theme.name);
        await expect(page.locator("html")).toHaveAttribute("lang", theme.locale === "zh" ? "zh-CN" : "en");
        expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
        const viewport = page.viewportSize();
        expect(viewport).not.toBeNull();
        for (const selector of item.panels) {
          const node = page.locator(selector).first();
          await expect(node, selector).toBeVisible();
          const box = await node.boundingBox();
          expect(box, selector).not.toBeNull();
          expect(box!.width, selector).toBeGreaterThan(8);
          expect(box!.height, selector).toBeGreaterThan(8);
          expect(box!.x, selector).toBeGreaterThanOrEqual(-1);
          expect(box!.x, selector).toBeLessThan(viewport!.width);
        }
        if (item.sidebar) {
          const radius = await page.locator(".sidebar").evaluate((node) => parseFloat(getComputedStyle(node).borderRadius));
          expect(radius).toBe(theme.name === "apple" ? 20 : 0);
        }
        if (item.tab) {
          await expect(page.locator(".pref-page")).toHaveAttribute("data-pref-tab", item.tab);
          const radius = await page.locator(".dialog").first().evaluate((node) => parseFloat(getComputedStyle(node).borderRadius));
          expect(radius).toBe(theme.name === "apple" ? 24 : theme.name === "light" ? 14 : 8);
        }
        if (item.tool) await expect(page.locator(".tool-tabs")).toHaveAttribute("data-tool-kind", item.tool);
        if (item.typeMenu) await expectComposerTypeMenu(page);
        if (item.history) await expectHistoryWeights(page, theme.name);
        if (item.remote) await expectRemoteSubject(page);
        if (item.merge) await expectMergeFrame(page, theme.name);
        if (item.scene === "prefs-accounts") await expectAccountActions(page, theme.locale);
      });
    }
  });
}

async function expectAccountActions(page: Page, locale: "zh" | "en") {
  const connected = locale === "zh" ? "已连通" : "Connected";
  await expect(page.locator(".account-sync-line")).not.toContainText(locale === "zh" ? "正在同步" : "Syncing");
  await expect(page.getByRole("button", { name: locale === "zh" ? "同步本机已登录的账号" : "Sync accounts already signed in on this machine" })).toBeEnabled();
  expect(accountSyncs).toBe(0);
  const paste = locale === "zh"
    ? "这台电脑没有可用的浏览器登录。请创建令牌后粘贴到下面。"
    : "This machine has no browser sign-in for this forge. Create a token and paste it below.";
  const card = page.locator('[data-account-card="lab-github"]');
  await card.locator("[data-account-test]").click();
  await expect(card.locator(".account-status")).toHaveText(connected);
  await expect(page.locator('[data-remote-match="origin"] select')).toHaveValue("lab-github");
  const browser = page.getByRole("button", { name: locale === "zh" ? "用浏览器登录" : "Sign in with the browser" });
  await expect(browser).toBeEnabled({ timeout: 15_000 });
  await browser.click();
  await expect(page.locator(".account-sync-line")).toContainText(paste);
  const add = page.locator("section.pref-card").filter({ hasText: locale === "zh" ? "添加账号" : "Add an account" });
  await add.getByRole("button", { name: locale === "zh" ? "测试连通" : "Test connection" }).click();
  await expect(add.locator(".account-note").filter({ hasText: connected })).toBeVisible();
}

async function weight(node: ReturnType<Page["locator"]>) {
  return node.evaluate((element) => getComputedStyle(element).fontWeight);
}

async function expectHistoryWeights(page: Page, theme: ThemeName) {
  const selected = page.locator(".history .commit-row.selected .subject").first();
  await expect(selected).toBeVisible();
  expect(await weight(selected)).toBe(theme === "apple" ? "500" : "600");
  await page.locator(".history").evaluate((node) => {
    node.scrollTop = node.scrollHeight;
    node.dispatchEvent(new Event("scroll"));
  });
  await page.waitForTimeout(400);
  const pushed = page.locator(".history .commit-row.pushed:not(.selected)").first();
  await expect(pushed).toBeVisible();
  expect(await weight(pushed.locator(".subject"))).toBe("600");
  await pushed.click();
  await page.locator(".history").evaluate((node) => {
    node.scrollTop = 0;
    node.dispatchEvent(new Event("scroll"));
  });
  await page.waitForTimeout(400);
  const local = page.locator(".history .commit-row.unpushed").first();
  await expect(local).not.toHaveClass(/selected/);
  expect(await weight(local.locator(".subject"))).toBe("400");
  expect(await local.locator(".subject").evaluate((node) => getComputedStyle(node).opacity)).toBe("1");
}

async function expectRemoteSubject(page: Page) {
  const subject = page.locator(".history .commit-row.remote-only:not(.selected) .subject").first();
  await expect(subject).toBeVisible();
  expect(await weight(subject)).toBe("400");
  expect(await subject.evaluate((node) => getComputedStyle(node).opacity)).toBe("0.55");
}

async function expectMergeFrame(page: Page, theme: ThemeName) {
  const panel = page.locator(".merge-panel.standalone");
  const box = await panel.boundingBox();
  const viewport = page.viewportSize();
  expect(box).not.toBeNull();
  expect(viewport).not.toBeNull();
  const inset = theme === "apple" ? 20 : 0;
  expect(Math.round(box!.width)).toBe(viewport!.width - inset);
  expect(Math.round(box!.height)).toBe(viewport!.height - inset);
  const radius = await panel.evaluate((node) => parseFloat(getComputedStyle(node).borderRadius));
  expect(radius).toBe(theme === "apple" ? 16 : 0);
  await expect(page.locator(".merge-scrim")).toHaveCount(0);
}

// The menu is fixed-positioned outside the right pane, whose glass backdrop-filter
// would otherwise become its containing block and push it to the window corner.
async function expectComposerTypeMenu(page: Page) {
  const chip = page.locator(".composer .type-chip");
  const menu = page.locator(".type-menu");
  await chip.click();
  await expect(menu).toBeVisible();
  const chipBox = await chip.boundingBox();
  const menuBox = await menu.boundingBox();
  const viewport = page.viewportSize();
  expect(chipBox).not.toBeNull();
  expect(menuBox).not.toBeNull();
  expect(viewport).not.toBeNull();
  expect(menuBox!.y + menuBox!.height).toBeLessThanOrEqual(chipBox!.y);
  expect(Math.abs(menuBox!.x - chipBox!.x)).toBeLessThanOrEqual(1);
  expect(menuBox!.y).toBeGreaterThanOrEqual(0);
  expect(menuBox!.x + menuBox!.width).toBeLessThanOrEqual(viewport!.width);
  await expect(menu.locator(".type-option.selected")).toHaveCount(1);
  await menu.locator(".type-option.selected").click();
  await expect(menu).toHaveCount(0);
}
