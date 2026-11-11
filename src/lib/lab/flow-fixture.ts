import type { FlowCommit, FlowNode, FlowSnapshot } from "../operation-flow";

/** Deterministic display fixtures; lifecycle tests use actual lab repositories. */
export function flowFixture(scene: string): FlowSnapshot {
  const at = Date.now() - 120_000;
  const commit = (id: string, summary: string): FlowCommit => ({ id: id.repeat(40).slice(0, 40), summary, author: "Lin Chen" });
  const before = commit("a1", "feat: update workspace navigation");
  const remote = commit("b2", "fix: preserve remote navigation state");
  const saved = commit("c3", "AweGit autostash: pull --rebase");
  const node = (key: string, kind: string, state: string, extra: Partial<FlowNode> = {}): FlowNode => ({
    key, kind, state, at, createdAt: at, round: 0, repeatOf: "", command: "", detail: "", source: null, target: null, base: null, stash: "", files: [], ...extra,
  });
  const nodes = [
    node("pull", "pull", "waiting", { target: before, command: "git pull --rebase --no-autostash" }),
    node("saved", "local-saved", "completed", { stash: saved.id, source: saved, base: before, files: [{ path: "src/navigation.ts", base: "", ours: "", theirs: "", resolved: false }] }),
    node("fetch", "fetch", "completed", { source: remote, detail: "git-child" }),
    node("first", "conflict", "completed", { round: 1, source: before, target: remote, files: [{ path: "src/navigation.ts", base: "01".repeat(20), ours: "02".repeat(20), theirs: "03".repeat(20), resolved: true }] }),
    node("clean", "replay", "completed", { source: commit("d4", "test: cover workspace navigation"), target: remote }),
    node("second", "conflict", "conflict", { round: 2, repeatOf: "first", source: commit("e5", "refactor: reorganize navigation"), target: commit("f6", "test: cover workspace navigation"), base: commit("d4", "test: cover workspace navigation"), detail: "replay-vs-accumulated", files: [
      { path: "src/navigation.ts", base: "04".repeat(20), ours: "05".repeat(20), theirs: "06".repeat(20), resolved: false },
      { path: "src/workspace.ts", base: "07".repeat(20), ours: "08".repeat(20), theirs: "09".repeat(20), resolved: true },
    ] }),
    node("restore", "local-restore", "planned", { source: saved, stash: saved.id, detail: "restore-after-integration" }),
    node("later", "replay", "conditional", { source: commit("77", "feat: remember recent workspace"), detail: "preview-conditional" }),
  ];
  const details = scene === "operation-flow-details";
  if (details) nodes[5].detail = [
    "From https://github.com/example/workspace",
    " * branch            main       -> FETCH_HEAD",
    ...Array.from({ length: 12 }, (_, index) => `Auto-merging src/section-${index + 1}.ts`),
    "error: could not apply 63eee33... chore: test1",
    'hint: Resolve all conflicts manually, mark them as resolved with',
    'hint: "git add/rm <conflicted_files>", then run "git rebase --continue".',
    'hint: You can instead skip this commit: run "git rebase --skip".',
    'hint: To abort and get back to the state before "git rebase",',
    'hint: run "git rebase --abort".',
    'hint: Disable this message with "git config set advice.mergeConflict false"',
    "Could not apply 63eee33... # chore: test1",
  ].join("\n");
  if (scene === "operation-flow-pipeline") {
    nodes.unshift(node("action-0", "pullRef", "waiting", { detail: "requested", target: before }));
    nodes.splice(2, 0, node("command-stash", "stash-push", "completed", { command: "git stash push --include-untracked" }));
    nodes.splice(5, 0, node("command-rebase", "rebase", "failed", { command: "git rebase", detail: "git-child" }));
    const current = nodes.find((item) => item.key === "second")!;
    nodes.splice(5, 0, node("preview-second", "replay", "conflict", { source: current.source, detail: "preview-conflict" }));
    nodes.push(node("action-resolve", "resolve", "completed", { detail: "requested" }),
      node("command-add", "add", "completed", { command: "git add src/workspace.ts" }),
      node("git-message", "error", "paused", { detail: "Resolve all conflicts, then continue." }));
  }
  nodes.forEach((item, index) => { item.createdAt = at + index * 1000; item.at = item.createdAt; });
  if (scene === "operation-flow-long") {
    const completed = Array.from({ length: 1200 }, (_, index) => node(`step-${index}`, "replay", "completed", { source: commit(String(index.toString(16)), `feat: workspace change ${index + 1}`), createdAt: at + index }));
    nodes.splice(3, 0, ...completed);
    nodes.forEach((node, index) => { node.createdAt = at + index; node.at = at + index; });
  }
  const history = scene === "operation-flow-history" || scene === "operation-flow-completed";
  if (history) {
    for (const item of nodes) { item.state = "completed"; for (const file of item.files) file.resolved = true; }
    nodes.push(node("finished", "finished", "completed", { target: remote }));
  }
  return { summary: {
    id: "1700000000000-1-1", repo: "D:/Projects/workspace", kind: "pull", state: history ? "completed" : "conflict", sequence: 20,
    startedAt: at, endedAt: history ? Date.now() : null, activeStep: history ? "finished" : "second", canAutoClose: history,
    branch: "feat/workspace", before, after: remote, progress: history ? null : "rebase", pendingStash: history ? "" : saved.id, recordingError: "", previous: "",
  }, nodes };
}
