import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { operationAreas, repositoryKey } from "../src/lib/operation-policy";
import { scheduleRead, invalidateReads, setReadCanceller, isReadSuperseded } from "../src/lib/read-scheduler";
import { mergeFields, mergeRecords, mergeResponse } from "../src/lib/reconcile";
import { acceptTask, projectRefs, runTask, subscribeTasks, type OperationTask } from "../src/lib/tasks";
import { projectStages, stageActions } from "../src/lib/shell/staging";
import type { RefSnapshot } from "../src/lib/status";
import { patchFiles } from "../src/lib/patch-preview";

const deferred = <T>() => { let resolve!: (value: T) => void; const promise = new Promise<T>((done) => { resolve = done; }); return { promise, resolve }; };
const tick = () => new Promise<void>((done) => setTimeout(done, 0));

test("queued inputs and patch preview retain the original paths and options", async () => {
  const request = { action: "stagePaths", files: ["original.txt"] };
  await runTask("mutate", { path: "captured", request }, "captured-write", async (args) => {
    request.files[0] = "later.txt";
    expect((args.request as typeof request).files).toEqual(["original.txt"]);
  });
  expect(patchFiles('diff --git a/deleted.txt b/deleted.txt\n--- a/deleted.txt\n+++ /dev/null\ndiff --git "a/with space.txt" "b/with space.txt"\n+++ "b/with space.txt"')).toEqual(["deleted.txt", "with space.txt"]);
});

test("every Rust mutation declares its refresh domains", () => {
  const source = readFileSync("crates/git/src/ops.rs", "utf8").split("pub enum Mutation {")[1].split("#[derive")[0];
  const variants = [...source.matchAll(/^    ([A-Z][A-Za-z]+)(?: \{|,)/gm)].map((match) => match[1][0].toLowerCase() + match[1].slice(1));
  expect(variants).toHaveLength(77);
  expect(Object.keys(operationAreas).filter((key) => key !== "commit").sort()).toEqual(variants.sort());
  expect(operationAreas.createBranch).toEqual(["refs"]);
  expect(operationAreas.stageLines).toEqual(["index"]);
});

test("immutable object reads are single flight and reusable; mutable reads expire with writes", async () => {
  const gate = deferred<string>(), path = "scheduler-cache", id = "a".repeat(40);
  let calls = 0;
  const read = () => { calls++; return gate.promise; };
  const first = scheduleRead("commit_detail", { path, id }, read);
  const duplicate = scheduleRead("commit_detail", { path, id }, read);
  expect(calls).toBe(1); gate.resolve("detail");
  expect(await duplicate).toBe(await first);
  await tick(); invalidateReads(path);
  expect(await scheduleRead("commit_detail", { path, id }, read)).toBe("detail");
  expect(calls).toBe(1);
  let reads = 0;
  expect(await scheduleRead("workspace_status", { path }, async () => ++reads)).toBe(1);
  await tick(); invalidateReads(path);
  expect(await scheduleRead("workspace_status", { path }, async () => ++reads)).toBe(2);
});

test("cancelling a mutable read keeps an active immutable object read alive", async () => {
  const path = "scheduler-stable-cancellation", id = "d".repeat(40);
  const stableGate = deferred<number>();
  const mutableGate = deferred<number>();
  let cancelled = 0;
  setReadCanceller(() => { cancelled += 1; });
  try {
    const stable = scheduleRead("commit_detail", { path, id }, () => stableGate.promise);
    const mutable = scheduleRead("file_diff", { path, file: "old.txt" }, () => mutableGate.promise).catch((error) => error.message);
    invalidateReads(path);
    expect(cancelled).toBe(0);
    stableGate.resolve(7);
    expect(await stable).toBe(7);
    expect(await mutable).toBe("Preview superseded");
  } finally {
    setReadCanceller(() => {});
  }
});

test("invalidateReads can keep the open conflict read", async () => {
  const path = "scheduler-keep-conflict";
  const gate = deferred<number>();
  let cancelled = 0;
  setReadCanceller(() => { cancelled += 1; });
  try {
    const kept = scheduleRead("conflict_view", { path, file: "GameLauncher.cs" }, () => gate.promise);
    const dropped = scheduleRead("file_diff", { path, file: "other.cs" }, () => gate.promise).catch((error) => error.message);
    await tick();
    invalidateReads(path, ["conflict_view", "conflict_sides"]);
    expect(cancelled).toBe(0);
    gate.resolve(4);
    expect(await kept).toBe(4);
    expect(await dropped).toBe("Preview superseded");
  } finally {
    setReadCanceller(() => {});
  }
});

test("superseded reads are recognized as internal cancellations", () => {
  expect(isReadSuperseded(new Error("Preview superseded"))).toBe(true);
  expect(isReadSuperseded("Preview superseded")).toBe(true);
  expect(isReadSuperseded(new Error("read failed"))).toBe(false);
});

test("a new repository read waits for backend cancellation to finish", async () => {
  const path = "scheduler-cancellation-order";
  const oldGate = deferred<number>(), cancelGate = deferred<void>();
  const started: string[] = [];
  setReadCanceller(() => cancelGate.promise);
  try {
    const old = scheduleRead("file_diff", { path, file: "old.txt" }, () => {
      started.push("old");
      return oldGate.promise;
    }).catch((error) => error.message);
    await tick();
    invalidateReads(path);
    const next = scheduleRead("file_diff", { path, file: "new.txt" }, async () => {
      started.push("new");
      return 2;
    });
    await tick();
    expect(started).toEqual(["old"]);
    cancelGate.resolve();
    expect(await next).toBe(2);
    expect(await old).toBe("Preview superseded");
  } finally {
    setReadCanceller(() => {});
  }
});

test("reads waiting for repository writes leave capacity to browse immutable commits", async () => {
  const gate = deferred<number>(); let mutable = 0, objects = 0;
  const jobs = [0, 1, 2, 3].map((index) => scheduleRead("workspace_status", { path: `waiting-${index}` }, () => { mutable++; return gate.promise; }));
  expect(mutable).toBe(2);
  const detail = scheduleRead("commit_detail", { path: "objects", id: "b".repeat(40) }, async () => ++objects);
  expect(await detail).toBe(1);
  gate.resolve(7); expect(await Promise.all(jobs)).toEqual([7, 7, 7, 7]); await tick();
});

test("mutable reads waiting in one repository leave a slot for another", async () => {
  const gate = deferred<number>(), started: string[] = [];
  const first = scheduleRead("workspace_status", { path: "D:/repo-a" }, () => { started.push("A status"); return gate.promise; });
  const next = scheduleRead("repository_refs", { path: "d:\\repo-a" }, async () => { started.push("A refs"); return 2; });
  expect(await scheduleRead("workspace_status", { path: "D:/repo-b" }, async () => { started.push("B status"); return 3; })).toBe(3);
  expect(started).toEqual(["A status", "B status"]);
  gate.resolve(1);
  expect(await Promise.all([first, next])).toEqual([1, 2]);
});

test("foreground reads pass queued prefetch and only the latest queued preview runs", async () => {
  const gate = deferred<number>(), started: string[] = [];
  const active = [0, 1, 2, 3].map((index) => scheduleRead("commit_files", { path: "priority", id: String(index).repeat(40) }, () => gate.promise));
  const prefetch = scheduleRead("commit_detail", { path: "priority", id: "c".repeat(40) }, async () => { started.push("prefetch"); return 1; }, true);
  const old = scheduleRead("file_diff", { path: "priority", file: "old" }, async () => { started.push("old"); return 1; }).catch((error) => error.message);
  const latest = scheduleRead("file_diff", { path: "priority", file: "latest" }, async () => { started.push("latest"); return 2; });
  expect(await old).toBe("Preview superseded");
  gate.resolve(0); await Promise.all([...active, prefetch, latest]); await tick();
  expect(started).toEqual(["latest", "prefetch"]);
});

test("settings and account discovery preserve in-flight edits, additions and deletions", () => {
  const same = (left: unknown, right: unknown) => JSON.stringify(left) === JSON.stringify(right);
  const before = [{ id: "a", label: "old", token: "one" }, { id: "removed", label: "old", token: "two" }];
  const current = [{ id: "a", label: "edited", token: "one" }, { id: "new", label: "new", token: "three" }];
  const incoming = [{ id: "a", label: "server", token: "refreshed" }, before[1], { id: "discovered", label: "found", token: "four" }];
  expect(mergeRecords(current, before, incoming, same)).toEqual([{ id: "a", label: "edited", token: "refreshed" }, current[1], incoming[2]]);
  expect(mergeFields({ theme: "dark", locale: "zh" }, { theme: "light", locale: "zh" }, { theme: "light", locale: "en" }, same)).toEqual({ theme: "dark", locale: "en" });
});

test("reference intentions never change the authoritative snapshot or HEAD", () => {
  const actual = { head: "head", branches: [{ name: "main", current: true }], tags: [], remotes: [], stashes: [] } as unknown as RefSnapshot;
  const task = { id: "branch", path: "D:/repo", action: "createBranch", preview: { name: "pending" }, state: "queued", targets: ["pending"], dependsOn: [], revision: 1 } as OperationTask;
  const view = projectRefs(actual, [task], "d:\\repo", 0)!;
  expect(view.branches.map((row) => row.name)).toEqual(["main", "pending"]);
  expect(actual.branches).toHaveLength(1); expect(view.head).toBe(actual.head);
  expect(projectRefs(actual, [{ ...task, state: "failed" }], "D:/repo", 0)).toBe(actual);
  expect(repositoryKey("/case/A")).not.toBe(repositoryKey("/case/a"));
});

test("settings responses merge nested edits without resurrecting removed records", () => {
  const same = (left: unknown, right: unknown) => JSON.stringify(left) === JSON.stringify(right);
  const before = { theme: "light", shortcuts: { commit: "old", refresh: "old" }, accounts: [{ id: "a", label: "old", token: "one" }, { id: "gone", label: "old", token: "two" }] };
  const current = { theme: "dark", shortcuts: { commit: "edited", refresh: "old" }, accounts: [{ id: "a", label: "edited", token: "one" }, { id: "new", label: "local", token: "three" }] };
  const incoming = { theme: "light", shortcuts: { commit: "old", refresh: "server" }, accounts: [{ id: "a", label: "old", token: "refreshed" }, before.accounts[1], { id: "found", label: "server", token: "four" }] };
  expect(mergeResponse(current, before, incoming, same)).toEqual({ theme: "dark", shortcuts: { commit: "edited", refresh: "server" }, accounts: [{ id: "a", label: "edited", token: "refreshed" }, current.accounts[1], incoming.accounts[2]] });
});

test("write order is independent from success dependencies", async () => {
  const gate = deferred<number>();
  const first = runTask("mutate", { path: "chain", request: { action: "fetch" } }, "chain-first", () => gate.promise);
  await runTask("mutate", { path: "chain", request: { action: "push" } }, "chain-second", async (args) => {
    expect(args.__after).toBe("chain-first");
    expect(args.__dependsOn).toEqual([]);
  });
  await runTask("mutate", { path: "chain", request: { action: "push" }, __dependsOn: ["chain-first"] }, "chain-dependent", async (args) => {
    expect(args.__dependsOn).toEqual(["chain-first"]);
  });
  await runTask("suggest_commit_message", { path: "chain" }, "chain-ai", async (args) => { expect(args.__after).toBeUndefined(); });
  gate.resolve(1); await first;
});

test("stale task events cannot move a task backwards", () => {
  const id = "late-event", task = { id, path: "D:/repo", action: "stagePath", label: "", targets: [],
    dependsOn: [], state: "running", revision: 2, queuedMs: 1, executeMs: 0, verifyMs: 0, percent: 61, sequence: 10 } as const;
  const seen: OperationTask[] = [];
  const stop = subscribeTasks((tasks) => { const entry = tasks.find((task) => task.id === id); if (entry) seen.push(entry); });
  try {
    expect(acceptTask({ ...task, targets: [], dependsOn: [] })).toBe(true);
    expect(acceptTask({ ...task, targets: [], dependsOn: [], state: "queued", percent: 0, sequence: 9 })).toBe(false);
    expect(seen.map((item) => [item.state, item.percent])).toEqual([["running", 61]]);
  } finally { stop(); }
});

test("stage projection preserves sorted paths and collapses repeated intents", () => {
  const file = (path: string, kind: "added" | "untracked" | "modified" | "deleted" | "conflict") => ({ path, kind, previousPath: null });
  const snapshot = { path: "/repo", branch: "main", staged: [file("a", "added"), file("c", "modified")],
    unstaged: [file("a", "deleted"), file("b", "untracked"), file("c", "modified"), file("folder/d", "conflict")] };
  const files = ["a", "b", "c", "folder/d"];
  const changes = Array.from({ length: 24 }, (_, id) => ({ id, files, repo: "/repo", unstage: id % 2 === 1 }));
  expect(projectStages(snapshot, changes)).toEqual({ staged: [], unstaged: [snapshot.unstaged[1], snapshot.unstaged[2], snapshot.unstaged[3]] });
  expect(stageActions(changes, "/repo").get("folder")).toBe(true);
  expect(projectStages(snapshot, [{ ...changes[0], repo: "/Repo" }])).toBe(snapshot);
});
