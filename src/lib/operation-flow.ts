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

export function stateSymbol(state: string) {
  return ({ completed: "✓", processed: "✓", running: "◉", conflict: "!", failed: "×", waiting: "Ⅱ", paused: "Ⅱ", skipped: "↷", aborted: "■", interrupted: "×", superseded: "—" } as Record<string, string>)[state] ?? "○";
}
