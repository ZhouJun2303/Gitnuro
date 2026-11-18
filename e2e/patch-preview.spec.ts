import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve, sep } from "node:path";
import { expect, test } from "@playwright/test";
import { patchFiles } from "../src/lib/patch-preview";
import { scenes } from "../src/lib/lab/scenes";
import { openScene, pinLabSettings } from "./helpers";

test("patch preview preserves paths emitted by Git for Unicode renames and deletions", () => {
  const lab = resolve(".lab");
  mkdirSync(lab, { recursive: true });
  const repo = mkdtempSync(join(lab, "patch-preview-"));
  const git = (...args: string[]) => {
    const result = spawnSync("git", ["-c", "user.name=Patch Preview", "-c", "user.email=patch@example.invalid", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=.git/no-hooks", "-c", "core.quotePath=true", ...args], {
      cwd: repo, encoding: "utf8", windowsHide: true,
    });
    expect(result.status, result.stderr).toBe(0);
    return result.stdout;
  };
  try {
    git("init", "-q", "--template=", "-b", "main");
    mkdirSync(join(repo, "folder b"));
    writeFileSync(join(repo, "旧 名.txt"), "one\ntwo\nthree\n");
    writeFileSync(join(repo, "folder b/deleted.txt"), "deleted\n");
    git("add", "."); git("commit", "-qm", "fixture");
    mkdirSync(join(repo, "a")); mkdirSync(join(repo, "b"));
    renameSync(join(repo, "旧 名.txt"), join(repo, "b/新 文件.txt"));
    rmSync(join(repo, "folder b/deleted.txt"));
    writeFileSync(join(repo, "a/added.txt"), "added\n");
    mkdirSync(join(repo, "a/binary b"));
    writeFileSync(join(repo, "a/binary b/data.bin"), Buffer.from([0, 1, 2, 3]));
    writeFileSync(join(repo, "b/二进制.bin"), Buffer.from([0, 4, 5, 6]));
    git("add", "-A");
    const patch = git("diff", "--cached", "--binary", "--find-renames");
    expect(patch).toContain("rename to");
    expect(patch).toContain("GIT binary patch");
    expect(patchFiles(patch).sort()).toEqual(["a/added.txt", "a/binary b/data.bin", "b/二进制.bin", "b/新 文件.txt", "folder b/deleted.txt"].sort());
  } finally {
    if (!realpathSync(repo).startsWith(realpathSync(lab) + sep)) throw new Error("Patch fixture escaped .lab");
    rmSync(repo, { recursive: true, force: true });
  }
});

test("patch preview decodes Git escapes and keeps real a and b directory prefixes", () => {
  const patch = String.raw`diff --git "a/old.txt" "b/b/new\tname.txt"
similarity index 100%
rename from old.txt
rename to "b/new\tname.txt"
diff --git a/old.bin b/a/copied.bin
similarity index 100%
copy from old.bin
copy to a/copied.bin
`;
  expect(patchFiles(patch)).toEqual(["b/new\tname.txt", "a/copied.bin"]);
});

test("large patch preview ignores file headers inside changed content", () => {
  const lines = Array.from({ length: 250_000 }, (_, index) => `+line ${index}`);
  lines[200_000] = "+++ b/this-is-content.txt";
  const patch = `diff --git a/real.txt b/real.txt\n--- /dev/null\n+++ b/real.txt\n@@ -0,0 +1,250000 @@\n${lines.join("\n")}\n`;
  const samples: number[] = [];
  let files: string[] = [];
  for (let index = 0; index < 6; index++) {
    const started = performance.now();
    files = patchFiles(patch);
    if (index) samples.push(performance.now() - started);
  }
  const directory = ".lab/ui-audit/patch-preview";
  mkdirSync(directory, { recursive: true });
  writeFileSync(join(directory, "large-patch.json"), JSON.stringify({ bytes: Buffer.byteLength(patch), lines: lines.length, samples, files }, null, 2));
  expect(files).toEqual(["real.txt"]);
});

test("plain unified patch previews respect hunk boundaries and deleted paths", () => {
  expect(patchFiles("--- a/first.txt\n+++ b/first.txt\n@@ -1 +1 @@\n-old\n+++ b/content.txt\n--- a/removed.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-deleted\n"))
    .toEqual(["first.txt", "removed.txt"]);
});

for (const theme of ["apple", "light", "dark"] as const) {
  test(`patch confirmation shows decoded targets without duplicates (${theme})`, async ({ page }, info) => {
    await pinLabSettings(page, { theme, locale: "en" });
    await openScene(page, scenes.find((scene) => scene.id === "dialog-patch")!, `&theme=${theme}&locale=en`);
    const dialog = page.locator('[data-dialog="patch"]');
    await dialog.locator("textarea").fill(String.raw`diff --git "a/old.txt" "b/b/\346\226\260\346\226\207\344\273\266.txt"
similarity index 100%
rename from old.txt
rename to "b/\346\226\260\346\226\207\344\273\266.txt"
diff --git a/removed.txt b/removed.txt
--- a/removed.txt
+++ /dev/null
@@ -1 +0,0 @@
-removed
`);
    await expect(dialog.locator(".patch-targets > div")).toHaveText(["b/新文件.txt", "removed.txt"]);
    await expect(dialog.locator("button.commit")).toBeEnabled();
    mkdirSync(".lab/ui-audit/patch-preview", { recursive: true });
    const screenshot = `.lab/ui-audit/patch-preview/${theme}.png`;
    await page.evaluate(() => Promise.allSettled(document.getAnimations()
      .filter((animation) => animation.effect?.getTiming().iterations !== Infinity).map((animation) => animation.finished)));
    await page.screenshot({ path: screenshot });
    await info.attach("patch-targets", { path: screenshot, contentType: "image/png" });
    await dialog.locator(".dialog-close").click();
    await expect(dialog).toHaveCount(0);
  });
}
