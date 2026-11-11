import { expect, test } from "@playwright/test";
import { buildFlowPipeline, type FlowSnapshot, type MutationResult } from "../src/lib/operation-flow";
import { flowFixture } from "../src/lib/lab/flow-fixture";
import type { Settings } from "../src/lib/status";

async function lab<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const response = await fetch("http://127.0.0.1:1429/invoke", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ cmd, args }) });
  const payload = await response.json() as { ok: boolean; value: T; error?: string };
  if (!payload.ok) throw new Error(payload.error || cmd); return payload.value;
}

test("pipeline combines the operation, commands and predictions into ordered steps", async ({ page }) => {
  const snapshot = flowFixture("operation-flow-pipeline");
  const pipeline = buildFlowPipeline(snapshot.summary, snapshot.nodes);
  expect(pipeline.steps.map((step) => step.key)).toEqual(["saved", "fetch", "first", "clean", "second", "later", "restore", "pipeline-finished"]);
  expect(pipeline.activeKey).toBe("second");
  expect(pipeline.steps.flatMap((step) => step.evidence).filter((entry) => snapshot.nodes.some((node) => node.key === entry.key))).toHaveLength(snapshot.nodes.length);
  expect(pipeline.steps.find((step) => step.key === "second")!.evidence.map((node) => node.key)).toContain("preview-second");
  expect(snapshot.nodes.find((node) => node.key === "preview-second")!.kind).toBe("replay");

  const commands: string[] = [];
  page.on("request", (request) => { if (request.url().includes("/__lab/invoke")) commands.push(request.postDataJSON().cmd); });
  await page.goto("/?scene=operation-flow-pipeline&view=flow&file=live");
  const active = page.locator(".flow-row.active");
  await expect(active.locator("h2")).toHaveText("第 2 轮冲突");
  await expect(active.locator(".flow-node")).toBeInViewport();
  await expect(active).toHaveAttribute("aria-current", "step");
  await expect(active.locator(".flow-current")).toHaveText("当前步骤");
  await expect(active.locator(".flow-step-number")).toHaveText("5 / 8");
  await expect(page.locator(".flow-follow")).toHaveText("当前步骤 · 5 / 8 · 第 2 轮冲突");
  await expect(page.locator(".flow-overview li")).toHaveCount(5);
  await expect(page.locator(".flow-overview li strong")).toHaveText(["保存本地修改", "获取远程提交", "整合提交", "恢复本地修改", "结束"]);
  await expect(page.locator(".flow-overview li.active")).toHaveAttribute("data-phase", "integration");
  expect(await page.locator(".flow-overview li").evaluateAll((items) => items.map((item) => item.getAttribute("data-state")))).toEqual(["completed", "completed", "conflict", "planned", "planned"]);
  await expect(page.locator('[data-node="action-0"], [data-node="pull"], [data-node="command-rebase"], [data-node="preview-second"]')).toHaveCount(0);
  await expect(active.locator(".flow-left")).toContainText("test: cover workspace navigation");
  await expect(active.locator(".flow-right")).toContainText("refactor: reorganize navigation");
  await expect(page.locator(".flow-observer")).toHaveText("只读观察");
  await expect(page.locator(".flow-window button, .flow-window [role=button]")).toHaveCount(0);
  await active.locator(".flow-evidence > summary").click();
  await expect(active.locator(".flow-file").first()).toContainText("src/navigation.ts");
  await expect(active.locator(".flow-repeat")).toContainText("前一轮：1");
  await page.screenshot({ path: ".lab/operation-flow-pipeline.png" });
  await page.locator('.flow-overview [data-phase="integration"] a').click();
  await expect(active.locator(".flow-node")).toBeInViewport();
  await page.locator(".flow-scroll").evaluate((element) => { element.scrollTop = 0; });
  const prior = page.locator('[data-node="first"]');
  await prior.locator(".flow-evidence > summary").click();
  await expect(prior).toContainText("已解决并暂存");
  expect(commands.every((command) => ["load_settings", "log_client"].includes(command))).toBe(true);
});

test("a conflict replaces its preview and stash restoration stays at the end", () => {
  const snapshot = flowFixture("operation-flow");
  const conflict = snapshot.nodes.find((node) => node.key === "second")!;
  const preview = { ...conflict, key: "preview", kind: "replay", state: "conditional", round: 0, files: [], createdAt: snapshot.summary.startedAt - 1000 };
  snapshot.nodes.unshift(preview);
  const pipeline = buildFlowPipeline(snapshot.summary, snapshot.nodes);
  expect(pipeline.steps.map((step) => step.key)).toEqual(["saved", "fetch", "first", "clean", "second", "later", "restore", "pipeline-finished"]);
  conflict.state = "waiting";
  expect(buildFlowPipeline(snapshot.summary, snapshot.nodes).steps.find((step) => step.key === "second")!.state).toBe("waiting");
  conflict.state = "completed";
  const restore = snapshot.nodes.find((node) => node.key === "restore")!;
  restore.state = "conflict";
  snapshot.nodes.push({ ...conflict, key: "restore-conflict", state: "conflict", stash: restore.stash, source: restore.source, round: 3 });
  snapshot.summary.activeStep = "restore-conflict";
  const restoring = buildFlowPipeline(snapshot.summary, snapshot.nodes);
  expect(restoring.steps.filter((step) => step.phase === "restore").map((step) => step.key)).toEqual(["restore-conflict"]);
  expect(restoring.activeKey).toBe("restore-conflict");
});

test("ended flows have no current step and failed fetches leave integration unexecuted", () => {
  const snapshot = flowFixture("operation-flow");
  snapshot.summary.state = "aborted";
  snapshot.summary.endedAt = Date.now();
  const aborted = buildFlowPipeline(snapshot.summary, snapshot.nodes);
  expect(aborted.activeKey).toBe("");
  expect(aborted.steps.filter((step) => ["later", "restore"].includes(step.key)).every((step) => step.state === "not-run")).toBe(true);
  expect(aborted.steps.at(-1)!.state).toBe("aborted");
  const fetch = snapshot.nodes.find((node) => node.key === "fetch")!;
  fetch.state = "failed";
  snapshot.summary.state = "failed";
  snapshot.summary.activeStep = fetch.key;
  const failed = buildFlowPipeline(snapshot.summary, [snapshot.nodes[0], fetch]);
  expect(failed.phases.map(({ phase, state }) => [phase, state])).toEqual([["fetch", "failed"], ["integration", "not-run"], ["finished", "failed"]]);
  const history = flowFixture("operation-flow-history");
  expect(buildFlowPipeline(history.summary, history.nodes).activeKey).toBe("");
});

for (const viewport of [{ width: 1440, height: 900 }, { width: 640, height: 800 }]) {
  test(`expanded pipeline details grow without overlapping following steps (${viewport.width}px)`, async ({ page }) => {
    await page.setViewportSize(viewport);
    await page.goto("/?scene=operation-flow-details&view=flow&file=live");
    const row = page.locator('[data-node="second"]');
    const node = row.locator(".flow-node");
    const next = page.locator('[data-node="later"]');
    await expect(node).toBeInViewport();
    const originalNextTop = await next.evaluate((element) => element.offsetTop);
    await row.locator("summary").click();
    await expect(row.locator("details")).toHaveAttribute("open", "");
    await expect.poll(() => node.evaluate((element) => element.clientHeight)).toBeGreaterThan(300);
    expect(await next.evaluate((element) => element.offsetTop)).toBeGreaterThan(originalNextTop);
    const gap = await page.locator(".flow-track").evaluate((track) => {
      const bottom = track.querySelector('[data-node="second"] .flow-node')!.getBoundingClientRect().bottom;
      return track.querySelector('[data-node="later"]')!.getBoundingClientRect().top - bottom;
    });
    expect(gap).toBeGreaterThanOrEqual(15);
    await expect(node).toContainText("Could not apply 63eee33... # chore: test1");
    await page.screenshot({ path: `.lab/operation-flow-details-${viewport.width}.png` });
    await node.evaluate((element) => { element.scrollTop = element.scrollHeight; });
    expect(await node.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
    await page.locator(".flow-scroll").evaluate((element) => { element.scrollTop = 0; });
    await page.locator('.flow-overview [data-phase="integration"] a').click();
    await expect(row.locator("details")).toHaveAttribute("open", "");
    await row.locator("summary").click();
    await expect.poll(() => next.evaluate((element) => element.offsetTop)).toBe(originalNextTop);
    expect(await node.evaluate((element) => element.clientHeight)).toBeLessThanOrEqual(132);
    expect(await page.locator(".flow-window").evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
    const lane = await row.evaluate((element) => {
      const label = element.querySelector(".flow-stash-label")!.getBoundingClientRect();
      const axis = element.querySelector(".flow-junction")!.getBoundingClientRect();
      const node = element.querySelector(".flow-node")!.getBoundingClientRect();
      return { labelRight: label.right, axis: axis.x + axis.width / 2, nodeLeft: node.left };
    });
    expect(lane.labelRight).toBeLessThan(lane.axis);
    expect(lane.axis).toBeLessThan(lane.nodeLeft);
    await expect(page.locator(".flow-follow")).toBeVisible();
  });
}

test("the complete overview stays visible while local changes branch out and merge back", async ({ page }) => {
  await page.goto("/?scene=operation-flow-stash&view=flow&file=live");
  await page.locator('.flow-overview [data-phase="saved"] a').click();
  const saved = page.locator('[data-node="saved"]');
  await expect(saved).toHaveAttribute("data-stash-route", "out");
  await expect(saved.locator(".flow-stash-label")).toHaveText("分出 · 保存本地修改");
  const stash = await saved.getAttribute("data-stash");
  await expect(page.locator('[data-node="fetch"]')).toHaveAttribute("data-stash", stash!);
  await expect(page.locator('[data-node="fetch"]')).toHaveAttribute("data-stash-route", "held");
  await page.screenshot({ path: ".lab/operation-flow-stash-save.png" });
  await page.locator('.flow-overview [data-phase="restore"] a').click();
  const restore = page.locator('[data-node="restore"]');
  await expect(restore.locator(".flow-node")).toBeInViewport();
  await expect(restore).toHaveAttribute("data-stash-route", "in");
  await expect(restore).toHaveAttribute("data-stash", stash!);
  await expect(restore.locator(".flow-stash-road")).toHaveClass(/predicted/);
  await expect(restore.locator(".flow-stash-label")).toHaveText("汇入 · 恢复本地修改");
  const route = await restore.evaluate((element) => {
    const branch = element.querySelector(".flow-stash-road")!.getBoundingClientRect();
    const axis = element.querySelector(".flow-junction")!.getBoundingClientRect();
    const label = element.querySelector(".flow-stash-label")!.getBoundingClientRect();
    return { left: branch.left, right: branch.right, axis: axis.x + axis.width / 2, labelRight: label.right };
  });
  expect(route.left).toBeLessThan(route.axis);
  expect(route.right).toBeLessThanOrEqual(route.axis + 1);
  expect(route.labelRight).toBeLessThan(route.axis);
  await expect(page.locator(".flow-overview")).toBeInViewport();
  await page.screenshot({ path: ".lab/operation-flow-stash-restore.png" });
  await page.goto("/?scene=operation-flow-history&view=flow&file=history");
  await expect(page.locator('[data-node="restore"]')).toHaveAttribute("data-stash-route", "in");
  await expect(page.locator('[data-node="restore"] .flow-stash-road')).not.toHaveClass(/predicted/);
  await expect(page.locator('[data-node="finished"]')).not.toHaveAttribute("data-stash-route");
});

test("long pipelines virtualize rows and retain the complete phase overview", async ({ page }) => {
  await page.goto("/?scene=operation-flow-long&view=flow&file=live");
  await expect(page.locator(".flow-node.active")).toBeInViewport();
  expect(await page.locator(".flow-row").count()).toBeLessThan(20);
  expect(await page.locator(".flow-scroll").evaluate((element) => element.scrollTop)).toBeGreaterThan(180_000);
  await expect(page.locator(".flow-overview li")).toHaveCount(5);
  await expect(page.locator(".flow-overview")).toBeInViewport();
  await page.locator('.flow-overview [data-phase="saved"] a').click();
  await expect(page.locator('[data-node="saved"] .flow-node')).toBeInViewport();
  await page.locator('.flow-overview [data-phase="integration"] a').click();
  await expect(page.locator(".flow-node.active")).toBeInViewport();
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
