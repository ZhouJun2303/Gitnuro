import type { InProgress } from "./status";

export type FlowCommit = { id: string; summary: string; author: string };
export type FlowFile = { path: string; base: string; ours: string; theirs: string; resolved: boolean };
export type FlowNode = {
  key: string; kind: string; state: string; at: number; createdAt: number; round: number; repeatOf: string;
  command: string; detail: string; source: FlowCommit | null; target: FlowCommit | null;
  base: FlowCommit | null; stash: string; files: FlowFile[];
};
export type FlowSummary = {
  id: string; repo: string; kind: string; state: string; sequence: number; startedAt: number;
  endedAt: number | null; activeStep: string; canAutoClose: boolean; branch: string;
  before: FlowCommit | null; after: FlowCommit | null; progress: InProgress | null;
  pendingStash: string; recordingError: string; previous: string;
};
export type FlowSnapshot = { summary: FlowSummary; nodes: FlowNode[] };
export type FlowDelta = { summary: FlowSummary; node: FlowNode | null };
export type MutationResult = { outcome: "completed" | "conflict" | "failed"; error: string; output?: string; log?: string; flow?: FlowSummary | null };

export type FlowPhase = "saved" | "fetch" | "integration" | "operation" | "restore" | "finished";
export type PipelineStep = FlowNode & { phase: FlowPhase; evidence: FlowNode[] };
export const isFutureStep = (node: FlowNode) => ["planned", "predicted", "conditional", "not-run"].includes(node.state);

/** Commands, previews and conflict observations describe the same pipeline steps.
 * Keep the journal intact and combine those observations only for display. */
export function buildFlowPipeline(summary: FlowSummary | null, nodes: FlowNode[]) {
  if (!summary) return { steps: [] as PipelineStep[], phases: [], activeKey: "" };
  const buckets = new Map<FlowPhase, PipelineStep[]>();
  const owners = new Map<string, PipelineStep>();
  const commits = new Map<string, PipelineStep>();
  const stashes = new Map<string, PipelineStep>();
  const notes: FlowNode[] = [];
  const positions = new Map<PipelineStep, number>();
  const indices = new Map(nodes.map((node, index) => [node.key, index]));
  const add = (node: FlowNode, phase: FlowPhase) => {
    const step: PipelineStep = { ...node, phase, evidence: [node] };
    positions.set(step, indices.get(node.key) ?? nodes.length);
    const bucket = buckets.get(phase) ?? [];
    bucket.push(step); buckets.set(phase, bucket); owners.set(node.key, step);
    return step;
  };
  const combine = (step: PipelineStep, node: FlowNode, primary = false) => {
    step.evidence.push(node); owners.set(node.key, step);
    if (primary) Object.assign(step, node);
  };
  const observations = nodes.slice().sort((a, b) => a.createdAt - b.createdAt);
  // Preview ids and stash ids let later conflict rounds replace their planned
  // step in place instead of appearing again at a different point in the graph.
  for (const node of observations) {
    if (node.kind === "local-saved") add(node, "saved");
    else if (node.kind === "local-restore") stashes.set(node.stash, add(node, "restore"));
    else if (["replay", "integration"].includes(node.kind)) {
      const step = add(node, "integration");
      if (node.source) commits.set(node.source.id, step);
    } else if (node.kind === "finished") add(node, "finished");
  }
  for (const node of observations) {
    if (owners.has(node.key)) continue;
    if (node.kind === "conflict") {
      const step = (node.stash ? stashes.get(node.stash) : undefined)
        ?? (node.source ? commits.get(node.source.id) : undefined);
      if (step && step.kind !== "conflict") combine(step, node, true);
      else add(node, node.stash ? "restore" : "integration");
    } else if (node.kind === "fetch" && !node.key.startsWith("action-")) add(node, "fetch");
    else notes.push(node);
  }
  const integrationKinds = ["pull", "pullRef", "merge", "rebase", "rebaseInteractive", "cherryPick", "cherry-pick", "revert"];
  if (!buckets.has("integration") && integrationKinds.includes(summary.kind)) {
    const command = notes.find((node) => !node.key.startsWith("action-") && ["merge", "rebase", "cherry-pick", "revert"].includes(node.kind));
    const request = notes.find((node) => node.key === summary.activeStep) ?? notes[0];
    if (["pull", "pullRef"].includes(summary.kind) && !command && summary.state !== "completed") {
      add(placeholder("pipeline-integration", "integration"), "integration");
    } else if (command ?? request) add(command ?? request!, "integration");
  }
  if (!buckets.has("fetch") && ["pull", "pullRef", "fetch"].includes(summary.kind)) {
    const request = notes.find((node) => node.kind === "fetch");
    if (request) add(request, "fetch");
    else {
      const fetch = placeholder("pipeline-fetch", "fetch");
      if (buckets.get("integration")?.some((step) => !isFutureStep(step) && !step.key.startsWith("action-"))) {
        fetch.state = "processed"; fetch.detail = "fetch-inferred";
      }
      add(fetch, "fetch");
    }
  }
  if (![...buckets.keys()].some((phase) => phase !== "finished")) {
    const request = notes.find((node) => node.key === summary.activeStep) ?? notes[0];
    if (request) add(request, "operation");
  }
  if (!buckets.has("finished")) {
    const finish = placeholder("pipeline-finished", "finished");
    if (summary.endedAt !== null) Object.assign(finish, { state: summary.state, at: summary.endedAt, target: summary.after });
    add(finish, "finished");
  }
  if (summary.endedAt !== null && summary.state !== "completed") {
    for (const bucket of buckets.values()) for (const step of bucket) if (isFutureStep(step)) step.state = "not-run";
  }

  const order: FlowPhase[] = ["saved", "fetch", "integration", "operation", "restore", "finished"];
  const steps = order.flatMap((phase) => {
    const bucket = (buckets.get(phase) ?? []).sort((a, b) => positions.get(a)! - positions.get(b)!);
    // Keep commit order within the executed and upcoming portions. Preview
    // updates must never move an executed step behind the current conflict.
    const ongoing = (step: PipelineStep) => ["running", "conflict", "waiting", "paused", "failed", "interrupted"].includes(step.state);
    return [...bucket.filter((step) => !isFutureStep(step) && !ongoing(step)), ...bucket.filter(ongoing), ...bucket.filter(isFutureStep)];
  });
  const current = owners.get(summary.activeStep)
    ?? steps.findLast((step) => ["conflict", "waiting", "running", "failed", "interrupted"].includes(step.state))
    ?? steps.find((step) => step.phase === "integration" || step.phase === "operation")
    ?? steps[0];
  const chronological = new Map(order.map((phase) => [phase, (buckets.get(phase) ?? []).slice().sort((a, b) => a.createdAt - b.createdAt)]));
  for (const node of notes) {
    if (owners.has(node.key)) continue;
    const phase: FlowPhase = node.kind === "preview" ? "integration"
      : node.kind.startsWith("stash-") ? node.kind === "stash-push" ? "saved" : "restore"
      : node.kind === "fetch" ? "fetch" : "integration";
    const candidates = chronological.get(phase) ?? [];
    // Resolution commands belong to the conflict round they act on, including
    // rounds whose final observation was updated after the command completed.
    let low = 0, high = candidates.length;
    while (low < high) {
      const mid = (low + high) >>> 1;
      if (candidates[mid].createdAt <= node.createdAt) low = mid + 1;
      else high = mid;
    }
    const step = candidates[Math.max(0, low - 1)] ?? current;
    if (step) combine(step, node);
  }
  for (const step of steps) step.evidence.sort((a, b) => a.createdAt - b.createdAt);
  const activeKey = summary.endedAt !== null ? "" : current?.key ?? "";
  const phases = order.flatMap((phase) => {
    const bucket = steps.filter((step) => step.phase === phase);
    if (!bucket.length) return [];
    const active = bucket.find((step) => step.key === activeKey);
    const pending = bucket.find(isFutureStep);
    const state = active?.state ?? (pending ? pending.state === "not-run" ? "not-run" : "planned" : bucket.every((step) => ["completed", "processed", "skipped", "superseded"].includes(step.state)) ? "completed" : bucket.at(-1)!.state);
    return [{ phase, state, key: active?.key ?? bucket[0].key, active: !!active }];
  });
  return { steps, phases, activeKey };

  function placeholder(key: string, kind: string): FlowNode {
    return { key, kind, state: "planned", at: summary!.startedAt, createdAt: summary!.startedAt, round: 0, repeatOf: "", command: "", detail: "", source: null, target: null, base: null, stash: "", files: [] };
  }
}

export function stateSymbol(state: string) {
  return ({ completed: "✓", processed: "✓", running: "◉", conflict: "!", failed: "×", waiting: "Ⅱ", paused: "Ⅱ", skipped: "↷", aborted: "■", interrupted: "×", superseded: "—" } as Record<string, string>)[state] ?? "○";
}
