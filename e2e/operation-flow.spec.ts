import { expect, test } from "@playwright/test";
import type { FlowSnapshot, MutationResult } from "../src/lib/operation-flow";
import type { Settings } from "../src/lib/status";

async function lab<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const response = await fetch("http://127.0.0.1:1429/invoke", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ cmd, args }) });
  const payload = await response.json() as { ok: boolean; value: T; error?: string };
  if (!payload.ok) throw new Error(payload.error || cmd); return payload.value;
}

test("observer joins left and right commits onto the timeline without action buttons", async ({ page }) => {
  const commands: string[] = [];
  page.on("request", (request) => { if (request.url().includes("/__lab/invoke")) commands.push(request.postDataJSON().cmd); });
  await page.goto("/?scene=operation-flow&view=flow&file=live");
  const active = page.locator(".flow-row.active");
  await expect(active.locator(".flow-node h2")).toHaveText("第 2 轮冲突");
  await expect(active.locator(".flow-node")).toBeInViewport();
  await expect(active.locator(".flow-left")).toContainText("test: cover workspace navigation");
  await expect(active.locator(".flow-right")).toContainText("refactor: reorganize navigation");
  await expect(page.locator(".flow-observer")).toHaveText("只读观察");
  await expect(page.locator(".flow-window button, .flow-window [role=button]")).toHaveCount(0);
  const sides = await active.evaluate((row) => {
    const left = row.querySelector(".flow-left")!.getBoundingClientRect();
    const right = row.querySelector(".flow-right")!.getBoundingClientRect();
    const junction = row.querySelector(".flow-junction")!.getBoundingClientRect();
    return { left: left.right, right: right.left, axis: junction.x + junction.width / 2, top: left.top - right.top };
  });
  expect(sides.left).toBeLessThan(sides.axis); expect(sides.right).toBeGreaterThan(sides.axis); expect(sides.top).toBe(0);
  await page.screenshot({ path: ".lab/operation-flow.png" });
  await active.locator(".flow-evidence > summary").click();
  await expect(active.locator(".flow-file").first()).toContainText("src/navigation.ts");
  await expect(active.locator(".flow-repeat")).toContainText("前一轮：1");
  await page.locator(".flow-scroll").evaluate((element) => { element.scrollTop = 3 * 344; });
  const prior = page.locator('[data-node="first"]');
  await prior.locator(".flow-evidence > summary").click();
  await expect(prior).toContainText("已解决并暂存");
  expect(commands.every((command) => ["load_settings", "log_client"].includes(command))).toBe(true);
  await expect(page.locator(".flow-window")).not.toHaveAttribute("data-closed", "true");
});

test("long observation flows render visible rows and manually viewed history stays open", async ({ page }) => {
  await page.goto("/?scene=operation-flow-long&view=flow&file=live");
  await expect(page.locator(".flow-node.active")).toBeInViewport();
  expect(await page.locator(".flow-row").count()).toBeLessThan(20);
  expect(await page.locator(".flow-scroll").evaluate((element) => element.scrollTop)).toBeGreaterThan(350_000);
  await page.goto("/?scene=operation-flow-history&view=flow&file=history");
  await expect(page.locator(".flow-footer")).toContainText("窗口保持打开");
  await page.waitForTimeout(1700);
  await expect(page.locator(".flow-window")).not.toHaveAttribute("data-closed", "true");
});

test("observer follows conflicts resolved elsewhere and closes only after final stash recovery", async ({ page }) => {
  test.setTimeout(90_000);
  await lab("lab_restore", { name: "operation" });
  const layout = await lab<Record<string, string>>("lab_layout"); const path = layout.operation;
  const mutate = (request: Record<string, unknown>) => lab<MutationResult>("mutate", { path, request });
  const snapshot = () => lab<FlowSnapshot>("operation_flow", { path });
  const start = await mutate({ action: "rebase", onto: "incoming", autostash: true });
  expect(start.outcome).toBe("conflict"); const id = start.flow!.id;
  await page.goto(`/?scene=operation-flow&view=flow&file=live&repo=${encodeURIComponent(path)}&rev=${id}`);
  const activeTitle = page.locator(".flow-node.active h2");
  await expect(activeTitle).toHaveText("第 1 轮冲突");
  await expect(page.locator(".flow-window button")).toHaveCount(0);
  // Writes represent the main workspace. The observer only receives updates.
  await mutate({ action: "resolve", file: "note.txt", side: "text", text: "resolved first\n" });
  expect((await snapshot()).summary.canAutoClose).toBe(false);
  await expect(page.locator('.flow-header > .flow-badge')).toContainText("等待继续");
  await mutate({ action: "continue", message: "" });
  await expect(activeTitle).toHaveText("第 2 轮冲突");
  await expect(page.locator(".flow-node.active")).toBeInViewport();
  await mutate({ action: "resolve", file: "note.txt", side: "text", text: "resolved second\n" });
  await mutate({ action: "continue", message: "" });
  await expect(activeTitle).toHaveText("第 3 轮冲突");
  const restoring = await snapshot(); expect(restoring.summary.progress).toBeNull(); expect(restoring.summary.canAutoClose).toBe(false);
  expect(restoring.nodes.filter((node) => node.kind === "conflict")).toHaveLength(3);
  await page.waitForTimeout(1700);
  await expect(page.locator(".flow-window")).not.toHaveAttribute("data-closed", "true");
  await mutate({ action: "resolve", file: "note.txt", side: "text", text: "restored local work\n" });
  await expect(page.locator(".flow-window")).toHaveAttribute("data-closed", "true", { timeout: 10_000 });
  expect((await snapshot()).summary.id).toBe(id);
  await page.goto(`/?scene=operation-flow&view=flow&file=history&repo=${encodeURIComponent(path)}&rev=${id}`);
  await expect(page.locator(".flow-footer")).toContainText("窗口保持打开");
  await page.waitForTimeout(1700);
  await expect(page.locator(".flow-window")).not.toHaveAttribute("data-closed", "true");
});

test("observer automatic opening can be disabled and saved in preferences", async ({ page }) => {
  test.setTimeout(60_000);
  const original = await lab<Settings>("load_settings");
  try {
    await lab("save_settings", { values: { ...original, flowAutoOpen: true } });
    await page.goto("/?scene=dialog-prefs");
    await expect(page.locator('[data-lab-scene="dialog-prefs"]')).toBeVisible();
    const toggle = page.locator('[data-dialog="prefs"] label').filter({ hasText: /自动打开操作与冲突观察面板|Automatically open the operation and conflict observer/ }).getByRole("checkbox");
    await expect(toggle).toBeChecked();
    await toggle.uncheck();
    await page.locator('[data-dialog="prefs"] .commit').click();
    await expect.poll(async () => (await lab<Settings>("load_settings")).flowAutoOpen).toBe(false);
    await page.goto("/?scene=dialog-prefs");
    await expect(page.locator('[data-lab-scene="dialog-prefs"]')).toBeVisible();
    await expect(toggle).not.toBeChecked();
    await toggle.check();
    await page.locator('[data-dialog="prefs"] .commit').click();
    await expect.poll(async () => (await lab<Settings>("load_settings")).flowAutoOpen).toBe(true);
  } finally { await lab("save_settings", { values: original }); }
});
