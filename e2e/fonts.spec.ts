import { existsSync, readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { openLabScene, pinLabSettings } from "./helpers";
import catalog from "../src/lib/font-catalog.json" with { type: "json" };

// Use an installed test font so UI tests never need the Google network or ship font binaries.
const fixture = ["C:/Windows/Fonts/segoeui.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/System/Library/Fonts/Supplemental/Arial.ttf"].find(existsSync);

test.beforeEach(async ({ page }) => {
  // Register settings first: its final continue() must run after our font mocks.
  await pinLabSettings(page, {});
  // All availability and progress checks stay local, including debounced requests.
  await page.route("**/__lab/invoke", async (route) => {
    const { cmd, args } = route.request().postDataJSON();
    if (cmd === "font_download_progress") return route.fulfill({ json: { ok: true, value: null } });
    if (cmd === "recognize_google_font") return route.fulfill({ json: { ok: true, value: { id: `google-${args.name.replace(/[^a-z0-9]/gi, "").toLowerCase()}`, name: args.name, bytes: null, cached: false } } });
    return route.fallback();
  });
});

for (const theme of ["apple", "light", "dark"] as const) {
  test(`${theme}: optional fonts load on request and work from cache while offline`, async ({ page }) => {
    test.skip(!fixture, "No system test font installed");
    const source = "awegit-font:inter";
    await page.route("**/__lab/font/inter", (route) => route.fulfill({ body: readFileSync(fixture!), contentType: "font/ttf" }));
    await pinLabSettings(page, { theme, uiFont: "", codeFont: "" });
    let cached = false;
    let downloads = 0;
    let requestId = "";
    let foldersOpened = 0;
    const saves: Record<string, unknown>[] = [];
    let release!: () => void;
    const pending = new Promise<void>((resolve) => { release = resolve; });
    await page.route("**/__lab/invoke", async (route) => {
      const request = route.request().postDataJSON();
      if (request.cmd === "save_settings") saves.push(request.args.values);
      if (request.cmd === "font_download_progress") return route.fulfill({ json: { ok: true, value: request.args.requestId === requestId ? {
        requestId, id: "inter", phase: cached ? "ready" : "downloading", received: cached ? readFileSync(fixture!).length : 1024, total: readFileSync(fixture!).length, percent: cached ? 100 : 0,
      } : null } });
      if (request.cmd === "open_font_folder") { foldersOpened++; return route.fulfill({ json: { ok: true, value: null } }); }
      if (request.cmd === "font_cache_info") return route.fulfill({ json: { ok: true, value: {
        directory: "D:\\Lab\\AweGit\\fonts",
        fonts: catalog.map(({ id }) => ({ id, path: `D:\\Lab\\AweGit\\fonts\\${id}.ttf`, bytes: cached && id === "inter" ? readFileSync(fixture!).length : null })),
        customFonts: [],
      } } });
      if (request.cmd === "font_data") return route.fulfill({ json: { ok: true, value: cached ? source : null } });
      if (request.cmd !== "download_font") return route.fallback();
      downloads++;
      requestId = request.args.requestId;
      await pending;
      cached = true;
      return route.fulfill({ json: { ok: true, value: source } });
    });
    await openLabScene(page, "prefs-fonts", '[data-pref-tab="fonts"]', { locale: "en" });
    const picker = page.locator('[data-font-kind="ui"]');
    await expect(picker.getByRole("button", { name: "Download and apply", exact: true })).toBeDisabled();
    await page.locator("#font-ui").selectOption("noto-sans-sc");
    await expect(picker).toContainText("16.9 MB");
    await expect(picker).toContainText("preview uses system font");
    await expect(picker).toContainText("D:\\Lab\\AweGit\\fonts\\noto-sans-sc.ttf");
    await page.locator("#font-ui").selectOption("inter");
    const button = picker.getByRole("button", { name: "Download and apply", exact: true });
    await expect(button).toBeEnabled();
    expect(downloads).toBe(0);
    await button.click();
    await expect(picker.getByRole("button", { name: "Downloading…", exact: true })).toBeDisabled();
    // Preferences remain interactive while the network request is held open.
    await page.getByRole("spinbutton", { name: "Interface font size" }).fill("15");
    await page.locator("#font-weight-ui").selectOption("500");
    await page.getByRole("button", { name: "Diff", exact: true }).click();
    await page.getByRole("button", { name: "Fonts", exact: true }).click();
    release();
    await expect(picker.getByRole("button", { name: "Available offline", exact: true })).toBeDisabled();
    await expect.poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue("--font-user"))).toContain("AweGit inter");
    expect(await page.locator(".pref-label").first().evaluate((node) => getComputedStyle(node).fontFamily)).toContain("AweGit inter");
    expect(await page.locator(".pref-label").first().evaluate((node) => getComputedStyle(node).fontFamily)).not.toContain("Times New Roman");
    const preview = page.locator('[data-font-preview="ui"]');
    await expect(preview).toHaveCSS("font-size", "15px");
    await expect(preview).toHaveCSS("font-weight", "500");
    await expect(preview).toHaveCSS("font-family", /AweGit inter/);
    await page.getByRole("spinbutton", { name: "Diff font size" }).fill("18");
    await page.locator("#font-weight-code").selectOption("600");
    await expect(page.locator('[data-font-preview="code"]')).toHaveCSS("font-size", "18px");
    await expect(page.locator('[data-font-preview="code"]')).toHaveCSS("font-weight", "600");
    expect(downloads).toBe(1);
    await picker.scrollIntoViewIfNeeded();
    await page.screenshot({ path: `.lab/screenshots/fonts-${theme}.png`, fullPage: true });
    await page.getByRole("button", { name: "Open font folder", exact: true }).click();
    expect(foldersOpened).toBe(1);
    await expect(page.locator(".font-cache-path")).toHaveText("D:\\Lab\\AweGit\\fonts");
    await page.screenshot({ path: `.lab/screenshots/fonts-storage-${theme}.png`, fullPage: true });
    await expect.poll(() => saves.some((value) => value.uiFont === "inter" && value.uiScale === 15 && value.codeFontWeight === 600 && value.diffFontSize === 18)).toBe(true);
    await page.reload();
    await expect(page.locator("#font-ui")).toHaveValue("inter");
    await expect(picker.getByRole("button", { name: "Available offline", exact: true })).toBeDisabled();
    expect(downloads).toBe(1);
    await expect(preview).toHaveCSS("font-size", "15px");
    await expect(preview).toHaveCSS("font-weight", "500");
    await page.getByRole("button", { name: "Restore font defaults", exact: true }).click();
    await expect(page.locator("#font-ui")).toHaveValue("");
    await expect(page.locator("#font-code")).toHaveValue("");
    await expect(preview).toHaveCSS("font-size", "14px");
    await expect(preview).toHaveCSS("font-weight", "400");
    await expect(page.locator('[data-font-preview="code"]')).toHaveCSS("font-size", "14px");
    await expect(page.locator('[data-font-preview="code"]')).toHaveCSS("font-weight", "400");
    await expect.poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue("--font-user"))).toBe("");
    await page.locator("#font-ui").selectOption("inter");
    await expect(picker.getByRole("button", { name: "Available offline", exact: true })).toBeDisabled();
    expect(downloads).toBe(1);
  });
}

for (const theme of ["apple", "light", "dark"] as const) {
  test(`${theme}: download a family copied from Google Fonts and reuse it after restart`, async ({ page }) => {
    test.skip(!fixture, "No system test font installed");
    const bytes = readFileSync(fixture!);
    const source = "awegit-font:google-outfit";
    await page.route("**/__lab/font/google-outfit", (route) => route.fulfill({ body: bytes, contentType: "font/ttf" }));
    const font = { id: "google-outfit", name: "Outfit", bytes: bytes.length, variable: true };
    await pinLabSettings(page, { theme, uiFont: "", codeFont: "" });
    let cached = false;
    const names: string[] = [];
    const saves: Record<string, unknown>[] = [];
    let requestId = "";
    let release!: () => void;
    const pending = new Promise<void>((resolve) => { release = resolve; });
    await page.route("**/__lab/invoke", async (route) => {
      const { cmd, args } = route.request().postDataJSON();
      if (cmd === "save_settings") saves.push(args.values);
      if (cmd === "font_download_progress") return route.fulfill({ json: { ok: true, value: args.requestId === requestId ? {
        requestId, id: font.id, phase: cached ? "ready" : "downloading", received: cached ? bytes.length : Math.floor(bytes.length / 2), total: bytes.length, percent: cached ? 100 : 50,
      } : null } });
      if (cmd === "font_cache_info") return route.fulfill({ json: { ok: true, value: {
        directory: "D:\\Lab\\AweGit\\fonts", customFonts: cached ? [font] : [],
        fonts: cached ? [{ id: font.id, path: "D:\\Lab\\AweGit\\fonts\\google-outfit.ttf", bytes: bytes.length }] : [],
      } } });
      if (cmd === "font_data") return route.fulfill({ json: { ok: true, value: cached && args.id === font.id ? source : null } });
      if (cmd !== "download_google_font") return route.fallback();
      names.push(args.name);
      requestId = args.requestId;
      await pending;
      cached = true;
      return route.fulfill({ json: { ok: true, value: { font, dataUrl: source } } });
    });
    await openLabScene(page, "prefs-fonts", '[data-pref-tab="fonts"]', { locale: "en" });
    await expect(page.getByRole("link", { name: "Browse Google Fonts" })).toHaveAttribute("href", "https://fonts.google.com/");
    const input = page.locator("#google-font-family");
    const download = page.getByRole("button", { name: "Download and add", exact: true });
    await input.fill("https://example.com/specimen/Outfit");
    await expect(download).toBeDisabled();
    await expect(page.locator(".font-recognition.failed")).toContainText("Paste a font family name");
    expect(names).toEqual([]);
    await input.fill("https://fonts.google.com/specimen/Outfit?preview.text=AweGit");
    await expect(page.locator(".font-recognition.recognized")).toContainText("Recognized · available to download · Outfit");
    await expect(download).toBeEnabled();
    expect(names).toEqual([]);
    await page.screenshot({ path: `.lab/screenshots/fonts-recognized-${theme}.png`, fullPage: true });
    await download.click();
    await expect(page.locator(".google-font-import").getByRole("button", { name: "Downloading…", exact: true })).toBeDisabled();
    const progress = page.getByRole("progressbar", { name: "Font download progress" });
    await expect(progress).toHaveAttribute("value", "50");
    await expect(page.locator(".font-download-progress")).toContainText("50%");
    await expect(page.locator(".font-download-progress")).toContainText(`${Math.ceil(Math.floor(bytes.length / 2) / 1024)} KB / ${Math.ceil(bytes.length / 1024)} KB`);
    await page.screenshot({ path: `.lab/screenshots/fonts-progress-${theme}.png`, fullPage: true });
    await page.locator("#font-weight-ui").selectOption("500");
    release();
    await expect(page.locator("#font-ui")).toHaveValue(font.id);
    await expect(progress).toHaveAttribute("value", "100");
    await expect(page.locator(".font-download-progress")).toContainText("Downloaded · ready");
    await expect(page.locator('[data-font-preview="ui"]')).toHaveCSS("font-family", /AweGit google-outfit/);
    await expect(page.locator('[data-font-preview="ui"]')).toHaveCSS("font-weight", "500");
    await expect(page.locator('[data-font-kind="ui"]')).toContainText("google-outfit.ttf");
    await page.locator("#font-code").selectOption(font.id);
    await expect(page.locator('[data-font-preview="code"]')).toHaveCSS("font-family", /AweGit google-outfit/);
    await expect.poll(() => saves.some((value) => value.uiFont === font.id && value.codeFont === font.id)).toBe(true);
    await page.locator(".google-font-import").scrollIntoViewIfNeeded();
    await page.screenshot({ path: `.lab/screenshots/fonts-google-${theme}.png`, fullPage: true });
    await page.reload();
    await expect(page.locator("#font-ui")).toHaveValue(font.id);
    await expect(page.locator("#font-code")).toHaveValue(font.id);
    await expect(page.locator('[data-font-preview="ui"]')).toHaveCSS("font-family", /AweGit google-outfit/);
    await expect(page.locator('[data-font-preview="code"]')).toHaveCSS("font-family", /AweGit google-outfit/);
    expect(names).toEqual(["Outfit"]);
  });
}

test("restoring defaults during a custom download keeps the later font choice", async ({ page }) => {
  test.skip(!fixture, "No system test font installed");
  const bytes = readFileSync(fixture!);
  const font = { id: "google-outfit", name: "Outfit", bytes: bytes.length, variable: true };
  await pinLabSettings(page, { uiFont: "", codeFont: "" });
  let downloaded = false;
  let release!: () => void;
  const pending = new Promise<void>((resolve) => { release = resolve; });
  await page.route("**/__lab/invoke", async (route) => {
    const { cmd } = route.request().postDataJSON();
    if (cmd === "font_cache_info") return route.fulfill({ json: { ok: true, value: { directory: "D:\\Lab\\fonts", fonts: [], customFonts: downloaded ? [font] : [] } } });
    if (cmd !== "download_google_font") return route.fallback();
    await pending;
    downloaded = true;
    return route.fulfill({ json: { ok: true, value: { font, dataUrl: `data:font/ttf;base64,${bytes.toString("base64")}` } } });
  });
  await openLabScene(page, "prefs-fonts", '[data-pref-tab="fonts"]', { locale: "en" });
  await page.locator("#google-font-family").fill("Outfit");
  await page.getByRole("button", { name: "Download and add", exact: true }).click();
  await expect(page.locator(".google-font-import").getByRole("button", { name: "Downloading…", exact: true })).toBeDisabled();
  await page.getByRole("button", { name: "Restore font defaults", exact: true }).click();
  release();
  await expect(page.locator('#font-ui option[value="google-outfit"]')).toHaveCount(1);
  await expect(page.locator("#font-ui")).toHaveValue("");
  await expect.poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue("--font-user"))).toBe("");
});

test("late availability results cannot enable downloading a different or missing family", async ({ page }) => {
  await pinLabSettings(page, { uiFont: "", codeFont: "" });
  let release!: () => void;
  const pending = new Promise<void>((resolve) => { release = resolve; });
  let checked = false;
  await page.route("**/__lab/invoke", async (route) => {
    const { cmd, args } = route.request().postDataJSON();
    if (cmd !== "recognize_google_font") return route.fallback();
    if (args.name === "Old Family") {
      checked = true;
      await pending;
      return route.fulfill({ json: { ok: true, value: { id: "google-oldfamily", name: "Old Family", cached: false, bytes: null } } });
    }
    return route.fulfill({ json: { ok: false, error: "Font family not found" } });
  });
  await openLabScene(page, "prefs-fonts", '[data-pref-tab="fonts"]', { locale: "en" });
  const input = page.locator("#google-font-family");
  const download = page.getByRole("button", { name: "Download and add", exact: true });
  await input.fill("Old Family");
  await expect.poll(() => checked).toBe(true);
  await expect(page.locator(".font-recognition")).toContainText("Checking the official font catalog");
  await expect(download).toBeDisabled();
  await input.fill("Unavailable");
  await expect(page.locator(".font-recognition.failed")).toContainText("Font family not found");
  const oldResponse = page.waitForResponse((response) => response.url().endsWith("/__lab/invoke") && response.request().postDataJSON()?.cmd === "recognize_google_font" && response.request().postDataJSON().args.name === "Old Family");
  release();
  await oldResponse;
  await expect(page.locator(".font-recognition.recognized")).toHaveCount(0);
  await expect(download).toBeDisabled();
});

test("font download errors keep the system font and allow retry", async ({ page }) => {
  await pinLabSettings(page, { uiFont: "", codeFont: "" });
  let downloads = 0;
  await page.route("**/__lab/invoke", async (route) => {
    const request = route.request().postDataJSON();
    if (request.cmd === "font_data") return route.fulfill({ json: { ok: true, value: null } });
    if (request.cmd !== "download_font") return route.fallback();
    downloads++;
    return route.fulfill({ json: { ok: false, error: "awegit:network:simulated download failure" } });
  });
  await openLabScene(page, "prefs-fonts", '[data-pref-tab="fonts"]', { locale: "en" });
  await page.locator("#font-code").selectOption("jetbrains-mono");
  const button = page.locator('[data-font-kind="code"]').getByRole("button", { name: "Download and apply" });
  for (let count = 1; count <= 2; count++) {
    await expect(button).toBeEnabled();
    await button.click();
    await expect(page.locator(".font-error")).toContainText("simulated download failure");
    expect(downloads).toBe(count);
    expect(await page.evaluate(() => document.documentElement.style.getPropertyValue("--mono-user"))).toBe("");
  }
});
