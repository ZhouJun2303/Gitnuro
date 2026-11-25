import type { StatusFingerprint } from "../status";

/** Paths named by the watcher while this repository was in the background. */
export type WatchNote = {
  revision: number;
  scope: "status" | "full";
  paths: string[] | null;
};

export type StatusProbe = {
  fingerprint: StatusFingerprint;
  revision: number;
  watcherAlive: boolean;
};

export type OpenPlan =
  | { kind: "trust" }
  | { kind: "paths"; paths: string[] }
  | { kind: "full" };

const PATH_LIMIT = 512;

export function sameFingerprint(left: StatusFingerprint | null | undefined, right: StatusFingerprint | null | undefined) {
  return !!left && !!right && left.head === right.head && left.refId === right.refId && left.indexLen === right.indexLen
    && left.indexSum === right.indexSum && (left.submoduleSum ?? "") === (right.submoduleSum ?? "");
}

/**
 * Decide whether switching back can keep the painted status.
 * A quiet watcher plus an unchanged HEAD and index skips the work tree.
 * Anything else is a full read, unless the watcher named a short path list.
 */
export function planStatusRefresh(input: {
  fingerprint: StatusFingerprint | null | undefined;
  seen: number;
  unverified: boolean;
  probe: StatusProbe | null;
  note: WatchNote | null;
}): OpenPlan {
  if (input.unverified || !input.probe?.watcherAlive || !sameFingerprint(input.fingerprint, input.probe.fingerprint)) {
    return { kind: "full" };
  }
  const seen = input.seen || 0;
  const note = input.note;
  if (note && note.revision > seen && (note.scope === "full" || !note.paths)) return { kind: "full" };
  if ((input.probe.revision || 0) !== seen) {
    if (note && note.scope === "status" && note.paths && note.paths.length > 0 && note.paths.length <= PATH_LIMIT && note.revision >= input.probe.revision) {
      return { kind: "paths", paths: note.paths };
    }
    return { kind: "full" };
  }
  return { kind: "trust" };
}

export function mergeWatchNote(previous: WatchNote | null, scope: "status" | "full", revision: number, paths: string[] | null | undefined): WatchNote {
  const incoming = scope === "full" || paths == null ? null : [...new Set(paths)];
  const nextRevision = Math.max(previous?.revision ?? 0, revision);
  if (!previous) {
    return incoming == null ? { revision: nextRevision, scope: "full", paths: null } : { revision: nextRevision, scope: "status", paths: incoming };
  }
  if (previous.scope === "full" || previous.paths == null || incoming == null) {
    return { revision: nextRevision, scope: "full", paths: null };
  }
  const merged = [...new Set([...previous.paths, ...incoming])];
  if (merged.length > PATH_LIMIT) return { revision: nextRevision, scope: "full", paths: null };
  return { revision: nextRevision, scope: "status", paths: merged };
}
