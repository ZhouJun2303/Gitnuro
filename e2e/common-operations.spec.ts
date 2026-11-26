import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { pinLabSettings } from "./helpers";

function resultOf(page: Page, command: string) {
  return page.waitForResponse((response) => {
    if (!response.url().includes("/__lab/invoke") || response.request().method() !== "POST") return false;
    return response.request().postDataJSON().cmd === command;
  });
}

function rowOpacity(row: Locator) {
  return row.evaluate((element) => {
    const canvas = document.createElement("canvas");
    const context = canvas.getContext("2d")!;
    context.fillStyle = getComputedStyle(element).backgroundColor;
    context.fillRect(0, 0, 1, 1);
    return context.getImageData(0, 0, 1, 1).data[3] / 255;
  });
}

for (const theme of ["apple", "light", "dark"]) {
  for (const scene of ["unstage-initial", "stage-conflict-paths"]) {
    test(`common operations ${scene} (${theme})`, async ({ page, request }) => {
      test.setTimeout(90_000);
      const restored = await request.post("http://127.0.0.1:1429/invoke", {
        data: { cmd: "lab_restore", args: { name: scene } },
      });
      expect((await restored.json()).ok).toBe(true);
      const repo = resolve(".lab", "repos", scene);
      const git = (...args: string[]) => execFileSync("git", args, { cwd: repo, encoding: "utf8" });
      const unmerged = git("ls-files", "-u", "-z");
      await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false, fileLayout: "list" });
      await page.goto(`/?scene=${scene}&theme=${theme}&locale=zh`);
      await expect(page.locator(`[data-lab-scene="${scene}"]`)).toBeVisible();

      if (scene === "unstage-initial") {
        await expect(page.locator(".staged-pane")).toContainText("new.txt");
        const response = resultOf(page, "unstage_all");
        await page.getByRole("button", { name: "全部取消暂存", exact: true }).click();
        expect((await (await response).json()).ok).toBe(true);
        await expect(page.locator(".staged-pane .file-list")).toHaveCount(0);
        await expect(page.locator(".unstaged-pane")).toContainText("new.txt");
        expect(git("ls-files", "-z")).toBe("");
        expect(readFileSync(resolve(repo, "new.txt"), "utf8")).toBe("edited after staging\n");
      } else {
        // The second click has only conflicts left; it must not resolve them.
        for (let attempt = 0; attempt < 2; attempt++) {
          const response = resultOf(page, "stage_all");
          await page.getByRole("button", { name: "全部暂存", exact: true }).click();
          expect((await (await response).json()).ok).toBe(true);
          await expect(page.locator(".staged-pane")).toContainText("loose.txt");
          await expect(page.locator(".unstaged-pane")).toContainText("conflict.txt");
          expect(git("ls-files", "-u", "-z")).toBe(unmerged);
        }
        expect(unmerged).not.toBe("");
        expect(git("show", ":renamed.txt")).toBe("edited after rename\n");
      }
      await expect(page.locator('.identity-floor[data-layer="error"]')).toHaveCount(0);
      await page.screenshot({ path: `.lab/${scene}-${theme}.png` });
    });
  }

  test(`discard unstaged restores the index and highlights selections (${theme})`, async ({ page, request }) => {
    test.setTimeout(90_000);
    const restored = await request.post("http://127.0.0.1:1429/invoke", {
      data: { cmd: "lab_restore", args: { name: "discard-unstaged" } },
    });
    const fixture = await restored.json();
    expect(fixture.ok, fixture.error).toBe(true);
    const repo = resolve(".lab", "repos", "discard-unstaged");
    const git = (...args: string[]) => execFileSync("git", args, { cwd: repo, encoding: "utf8" });
    const index = git("diff", "--cached", "--binary");
    const worktree = (name: string) => readFileSync(resolve(repo, "Assets", `${name}.txt`), "utf8").replaceAll("\r\n", "\n");
    await pinLabSettings(page, { theme, locale: "zh", fetchAutomatically: false });
    await page.goto(`/?scene=discard-unstaged&theme=${theme}&locale=zh`);
    await expect(page.locator('[data-lab-scene="discard-unstaged"]')).toBeVisible();

    const staged = page.locator(".staged-pane");
    const unstaged = page.locator(".unstaged-pane");
    const row = (pane: Locator, name: string) => pane.locator(`.file:has(.file-select[title="Assets/${name}.txt"])`);
    const select = (pane: Locator, name: string) => row(pane, name).locator(".file-select");
    for (const pane of [staged, unstaged]) {
      await select(pane, "first").click();
      await select(pane, "second").click({ modifiers: ["Control"] });
      await pane.locator(".pane-head").hover();
      await expect(row(pane, "first")).toHaveClass(/\bpicked\b/);
      await expect(row(pane, "second")).toHaveClass(/\bselected\b/);
      const picked = await rowOpacity(row(pane, "first"));
      expect(picked).toBeGreaterThan(0.12);
      expect(await rowOpacity(row(pane, "second"))).toBeGreaterThan(picked + 0.05);
      expect(await rowOpacity(row(pane, "third"))).toBe(0);

      await pane.locator('.dir-row .file-select[title="Assets"]').click();
      const folder = pane.locator(".dir-row.picked");
      await expect(folder).toHaveCount(1);
      expect(await rowOpacity(folder)).toBeGreaterThan(0.12);
    }
    await select(unstaged, "first").click();
    await select(unstaged, "second").click({ modifiers: ["Control"] });
    await unstaged.locator(".pane-head").hover();
    await page.screenshot({ path: `.lab/discard-unstaged-${theme}.png` });

    const confirm = page.locator('[data-layer="confirm"]');
    const applyDiscard = async () => {
      const response = resultOf(page, "mutate");
      await confirm.locator(".commit").click();
      expect((await (await response).json()).ok).toBe(true);
      expect(git("diff", "--cached", "--binary")).toBe(index);
    };

    await select(unstaged, "first").click();
    await select(unstaged, "first").click({ button: "right" });
    await page.getByRole("menuitem", { name: /^丢弃修改/ }).click();
    await applyDiscard();
    await expect(row(unstaged, "first")).toHaveCount(0);
    await expect(row(staged, "first")).toBeVisible();
    expect(worktree("first")).toBe("staged first\n");
    expect(worktree("second")).toBe("edited after staging second\n");

    await select(unstaged, "second").click();
    await select(unstaged, "third").click({ modifiers: ["Control"] });
    await unstaged.getByRole("button", { name: "丢弃修改", exact: true }).click();
    await applyDiscard();
    await expect(unstaged.locator(".file")).toHaveCount(1);
    await expect(unstaged.locator('.file-select[title="other.txt"]')).toBeVisible();
    await expect(staged.locator(".file")).toHaveCount(3);
    expect(worktree("second")).toBe("staged second\n");
    expect(worktree("third")).toBe("staged third\n");
    await select(staged, "first").click();
    await expect(page.locator("section.diff")).toContainText("staged first");
  });
}
