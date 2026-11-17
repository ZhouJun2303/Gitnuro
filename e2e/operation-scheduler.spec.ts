import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { operationAreas, repositoryKey } from "../src/lib/operation-policy";
import { scheduleRead, invalidateReads } from "../src/lib/read-scheduler";
import { mergeFields, mergeRecords, mergeResponse } from "../src/lib/reconcile";
import { projectRefs, runTask, type OperationTask } from "../src/lib/tasks";
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

test("reads waiting for repository writes leave capacity to browse immutable commits", async () => {
  const gate = deferred<number>(); let mutable = 0, objects = 0;
  const jobs = [0, 1, 2, 3].map((index) => scheduleRead("workspace_status", { path: `waiting-${index}` }, () => { mutable++; return gate.promise; }));
  expect(mutable).toBe(2);
  const detail = scheduleRead("commit_detail", { path: "objects", id: "b".repeat(40) }, async () => ++objects);
  expect(await detail).toBe(1);
  gate.resolve(7); expect(await Promise.all(jobs)).toEqual([7, 7, 7, 7]); await tick();
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

test("writes capture dependency identity; recovery and services remain independent", async () => {
  const gate = deferred<number>(); let dependency: unknown;
  const first = runTask("mutate", { path: "chain", request: { action: "fetch" } }, "chain-first", () => gate.promise);
  const second = runTask("mutate", { path: "chain", request: { action: "push" } }, "chain-second", async (args) => { dependency = args.__dependsOn; return 1; });
  await second; expect(dependency).toEqual(["chain-first"]);
  await runTask("mutate", { path: "chain", request: { action: "abort" } }, "chain-recovery", async (args) => { expect(args.__dependsOn).toEqual([]); });
  await runTask("suggest_commit_message", { path: "chain" }, "chain-ai", async (args) => { expect(args.__dependsOn).toEqual([]); });
  gate.resolve(1); await first;
});
