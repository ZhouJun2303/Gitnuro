import { execFileSync } from "node:child_process";
import { expect, test, type Page } from "@playwright/test";
import { scenes, type LabScene } from "../src/lib/lab/scenes";

async function lab(cmd: string, args: Record<string, unknown> = {}) {
  const response = await fetch("http://127.0.0.1:1429/invoke", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ cmd, args }),
  });
  const payload = (await response.json()) as { ok?: boolean; value?: unknown; error?: string };
  if (!payload.ok) throw new Error(payload.error || cmd);
  return payload.value;
}

async function openScene(page: Page, scene: LabScene) {
  const errors: string[] = [];
  const onError = (error: Error) => errors.push(error.message);
  page.on("pageerror", onError);
  try {
    console.log(`scene ${scene.id}`);
    await page.goto(scene.href, { waitUntil: "domcontentloaded" });
    const ready = page.locator(`[data-lab-scene="${scene.id}"]`);
    const failed = page.locator("[data-lab-error]");
    await expect(ready.or(failed)).toBeVisible({ timeout: 90_000 });
    if (!(await ready.isVisible())) throw new Error(`${scene.id}: ${await failed.getAttribute("data-lab-error")}`);
    if (scene.ready.startsWith("text=")) await expect(page.getByText(scene.ready.slice(5)).first()).toBeVisible();
    else await expect(page.locator(scene.ready).first()).toBeVisible();
    expect(errors, scene.id).toEqual([]);
  } finally {
    page.off("pageerror", onError);
  }
}

test("home lists a scene", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("link", { name: "变更" })).toBeVisible();
});

test("every scene opens", async ({ page }) => {
  test.setTimeout(scenes.length * 90_000 + 30_000);
  for (const scene of scenes) await openScene(page, scene);
});

test("pushbox commits and pushes", async () => {
  const layout = (await lab("lab_layout")) as { pushbox: string; remotePush: string };
  await lab("stage_path", { path: layout.pushbox, file: "note.txt" });
  await lab("commit_changes", {
    path: layout.pushbox,
    summary: "lab: record the push note",
    description: "",
    amend: false,
    signOff: false,
    skipHooks: true,
  });
  await lab("mutate", {
    path: layout.pushbox,
    request: { action: "push", remote: "origin", setUpstream: true, tags: false, forceWithLease: false },
  });
  const subject = execFileSync("git", ["--git-dir", layout.remotePush, "log", "-1", "--format=%s"], { encoding: "utf8" }).trim();
  expect(subject).toBe("lab: record the push note");
});
