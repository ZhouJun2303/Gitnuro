import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { pinLabSettings } from "./helpers";

function resultOf(page: Page, command: string) {
  return page.waitForResponse((response) => {
    if (!response.url().includes("/__lab/invoke") || response.request().method() !== "POST") return false;
    return response.request().postDataJSON().cmd === command;
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
}
