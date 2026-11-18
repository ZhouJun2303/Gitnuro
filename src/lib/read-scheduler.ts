import { repositoryKey } from "./operation-policy";
/** Bounded object cache and single-flight reads. Writes and cancellation never wait here. */
const commands = new Set(["workspace_status", "repository_refs", "commit_log", "commit_detail", "commit_files", "show_commit_file", "file_diff", "file_history", "blame_file", "compare_files", "compare_file", "commit_tree", "blob_view", "revision_preview", "file_preview", "conflict_view", "conflict_sides", "reflog", "repo_facts", "repository_summary", "repository_inspection", "in_progress", "merge_state", "merged_branches", "operation_history", "operation_flow", "git_runtime", "app_info", "external_tools", "editor_tools", "program_exists"]);
const immutableCommands = new Set(["commit_detail", "commit_files", "show_commit_file", "commit_tree", "blob_view", "revision_preview", "compare_files", "compare_file"]);
commands.add("stash_patch"); immutableCommands.add("stash_patch");
commands.add("blame_source");
for (const command of ["workspace_status_paths", "search_commits", "search_log", "pick_history", "cherry_preview", "merge_preview", "rebase_conflicts"]) commands.add(command);
const flights = new Map<string, Promise<unknown>>();
const cache = new Map<string, { value: unknown; bytes: number }>();
let bytes = 0, active = 0, background = 0, mutableActive = 0;
type Job = { key: string; repo: string; lane: string; start: () => void; prefetch: boolean; mutable: boolean; countedBackground?: boolean; cancel: () => void };
const waiting: Job[] = [];
const mutableRepositories = new Set<string>();
const versions = new Map<string, number>();
export function invalidateReads(path: string) { const key = repositoryKey(path); versions.set(key, (versions.get(key) ?? 0) + 1); }
const MAX_BYTES = 24 * 1024 * 1024;

function pump() {
  while (active < 4) {
    const eligible = (job: Job) => (!job.mutable || mutableActive < 2 && !mutableRepositories.has(job.repo)) && (!job.prefetch || background < 1);
    let index = waiting.findIndex((job) => !job.prefetch && eligible(job));
    if (index < 0) index = waiting.findIndex(eligible);
    const next = index >= 0 ? waiting.splice(index, 1)[0] : undefined;
    if (!next) return;
    active++; if (next.mutable) { mutableActive++; mutableRepositories.add(next.repo); }
    next.countedBackground = next.prefetch; if (next.countedBackground) background++;
    next.start();
  }
}

function immutable(command: string, args: Record<string, unknown>) {
  const revisions = [args.id, args.rev, args.from, args.to, args.left, args.right].filter((item) => typeof item === "string");
  return immutableCommands.has(command) && revisions.length > 0 && revisions.every((rev) => /^[0-9a-f]{40,64}$/i.test(String(rev)));
}

function weight(value: unknown, limit = MAX_BYTES): number {
  if (typeof value === "string") return value.length * 2;
  if (!value || typeof value !== "object") return 16;
  let size = 64;
  for (const child of Object.values(value)) { size += weight(child, limit - size); if (size > limit) break; }
  return size;
}

export function scheduleRead<T>(command: string, args: Record<string, unknown>, read: () => Promise<T>, prefetch = false): Promise<T> {
  if (!commands.has(command)) return read();
  // Only request parameters, never status snapshots or history rows, form a cache key.
  const { __perf, __task, __prefetch, ...parameters } = args;
  prefetch ||= __prefetch === true;
  const stable = immutable(command, args);
  const repo = repositoryKey(String(args.path ?? args.repo ?? ""));
  const key = `${command}:${stable ? "object" : versions.get(repo) ?? 0}:${JSON.stringify(parameters)}`;
  const cached = cache.get(key);
  if (cached) { cache.delete(key); cache.set(key, cached); return Promise.resolve(cached.value as T); }
  const flight = flights.get(key);
  if (flight) {
    if (!prefetch) { const queued = waiting.find((job) => job.key === key); if (queued) queued.prefetch = false; pump(); }
    return flight as Promise<T>;
  }
  const lane = ["file_diff", "conflict_view", "blob_view", "search_log", "search_commits", "cherry_preview", "merge_preview", "rebase_conflicts"].includes(command) ? `${command}:${repo}` : "";
  if (lane) for (let index = waiting.length - 1; index >= 0; index--) {
    if (waiting[index].lane === lane) waiting.splice(index, 1)[0].cancel();
  }
  const promise = new Promise<T>((resolve, reject) => {
    const job: Job = { key, repo, lane, prefetch, mutable: !stable, cancel: () => { flights.delete(key); reject(new Error("Preview superseded")); }, start: () => {
      const execute = async () => read();
      void execute().then((value) => {
        if (stable) {
          const size = weight(value);
          if (size < MAX_BYTES / 2) {
            cache.set(key, { value, bytes: size }); bytes += size;
            while (cache.size > 100 || bytes > MAX_BYTES) {
              const oldest = cache.keys().next().value!; bytes -= cache.get(oldest)!.bytes; cache.delete(oldest);
            }
          }
        }
        resolve(value);
      }, reject).finally(() => { active--; if (job.mutable) { mutableActive--; mutableRepositories.delete(job.repo); } if (job.countedBackground) background--; flights.delete(key); pump(); });
    } };
    waiting.push(job);
  });
  flights.set(key, promise); pump(); return promise;
}
