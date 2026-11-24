import { repositoryKey } from "./operation-policy";

/** Bounded object cache and single-flight reads. Writes and cancellation never wait here. */
const commands = new Set(["workspace_status", "repository_refs", "commit_log", "commit_detail", "commit_files", "show_commit_file", "file_diff", "file_history", "blame_file", "compare_files", "compare_file", "commit_tree", "blob_view", "revision_preview", "file_preview", "conflict_view", "conflict_sides", "reflog", "repo_facts", "repository_summary", "repository_inspection", "in_progress", "merge_state", "merged_branches", "operation_history", "operation_flow", "git_runtime", "app_info", "external_tools", "editor_tools", "program_exists"]);
const immutableCommands = new Set(["commit_detail", "commit_files", "show_commit_file", "commit_tree", "blob_view", "revision_preview", "compare_files", "compare_file"]);
commands.add("stash_patch"); immutableCommands.add("stash_patch");
commands.add("blame_source");
for (const command of ["workspace_status_paths", "search_commits", "search_log", "pick_history", "cherry_preview", "merge_preview", "rebase_conflicts"]) commands.add(command);

const flights = new Map<string, Promise<unknown>>();
const cache = new Map<string, { value: unknown; bytes: number }>();
const activeJobs = new Set<Job>();
let bytes = 0, active = 0, background = 0, mutableActive = 0;
type Read = (signal: AbortSignal) => Promise<unknown>;
type Job = {
  key: string;
  command: string;
  repo: string;
  path: string;
  lane: string;
  start: () => void;
  prefetch: boolean;
  mutable: boolean;
  controller: AbortController;
  resolve: (value: unknown) => void;
  reject: (reason?: unknown) => void;
  promise?: Promise<unknown>;
  started: boolean;
  released: boolean;
  settled: boolean;
  countedBackground?: boolean;
  cancel: () => void;
};
const waiting: Job[] = [];
const mutableRepositories = new Set<string>();
const versions = new Map<string, number>();
const repositoryCancels = new Map<string, Promise<void>>();
let cancelRepository: (path: string) => void | Promise<void> = () => {};
const MAX_BYTES = 24 * 1024 * 1024;

/** Cancellation of a stale read is an internal scheduling result, not a user-facing error. */
export function isReadSuperseded(error: unknown) {
  return (error instanceof Error ? error.message : String(error)).trim() === "Preview superseded";
}

/** Connect repository cancellation to the desktop backend or the browser lab. */
export function setReadCanceller(cancel: (path: string) => void | Promise<void>) {
  cancelRepository = cancel;
}

function requestRepositoryCancel(path: string) {
  const repo = repositoryKey(path);
  if (repositoryCancels.has(repo)) return;
  const pending = Promise.resolve()
    .then(() => cancelRepository(path))
    .catch(() => {})
    .then(() => {
      if (repositoryCancels.get(repo) === pending) {
        repositoryCancels.delete(repo);
        pump();
      }
    });
  repositoryCancels.set(repo, pending);
}

function release(job: Job) {
  if (job.released) return;
  job.released = true;
  activeJobs.delete(job);
  active--;
  if (job.mutable) {
    mutableActive--;
    mutableRepositories.delete(job.repo);
  }
  if (job.countedBackground) background--;
  if (flights.get(job.key) === job.promise) flights.delete(job.key);
  pump();
}

function cancelJob(job: Job) {
  if (job.settled) return;
  job.settled = true;
  job.controller.abort();
  if (flights.get(job.key) === job.promise) flights.delete(job.key);
  job.reject(new Error("Preview superseded"));
  if (job.started) release(job);
}

function cancelRepositoryJobs(path: string, keep = new Set<string>()) {
  const repo = repositoryKey(path);
  const spared = (job: Job) => keep.has(job.command);
  const queued = waiting.filter((job) => job.repo === repo && job.mutable && !spared(job));
  for (const job of queued) {
    const index = waiting.indexOf(job);
    if (index >= 0) waiting.splice(index, 1);
    cancelJob(job);
  }
  const active = [...activeJobs].filter((job) => job.repo === repo && job.mutable && !spared(job));
  // A spared read is the merge panel body. Killing every Git process for the
  // repository would abort it even though this scheduler kept the promise.
  const sparedLive = [...activeJobs].some((job) => job.repo === repo && spared(job) && job.started && !job.released);
  const stableActive = [...activeJobs].some((job) => job.repo === repo && job.started && !job.released && !job.mutable);
  if (active.length > 0 && !stableActive && !sparedLive) requestRepositoryCancel(path);
  for (const job of active) cancelJob(job);
}

function pump() {
  while (active < 4) {
    const eligible = (job: Job) => !repositoryCancels.has(job.repo)
      && (!job.mutable || mutableActive < 2 && !mutableRepositories.has(job.repo))
      && (!job.prefetch || background < 1);
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

export function invalidateReads(path: string, keep: readonly string[] = []) {
  const key = repositoryKey(path);
  versions.set(key, (versions.get(key) ?? 0) + 1);
  cancelRepositoryJobs(path, new Set(keep));
}

/** Cancel stale mutable reads before a newer view starts using the repository. */
export function cancelReads(path: string) {
  cancelRepositoryJobs(path);
}

export function scheduleRead<T>(command: string, args: Record<string, unknown>, read: Read, prefetch = false): Promise<T> {
  if (!commands.has(command)) return read(new AbortController().signal) as Promise<T>;
  // Only request parameters, never status snapshots or history rows, form a cache key.
  const { __perf, __task, __prefetch, ...parameters } = args;
  prefetch ||= __prefetch === true;
  const stable = immutable(command, args);
  const path = String(args.path ?? args.repo ?? "");
  const repo = repositoryKey(path);
  const key = `${command}:${stable ? "object" : versions.get(repo) ?? 0}:${JSON.stringify(parameters)}`;
  const cached = cache.get(key);
  if (cached) { cache.delete(key); cache.set(key, cached); return Promise.resolve(cached.value as T); }
  const flight = flights.get(key);
  if (flight) {
    if (!prefetch) { const queued = waiting.find((job) => job.key === key); if (queued) queued.prefetch = false; pump(); }
    return flight as Promise<T>;
  }
  const lane = ["file_diff", "conflict_view", "blob_view", "search_log", "search_commits", "cherry_preview", "merge_preview", "rebase_conflicts"].includes(command) ? `${command}:${repo}` : "";
  if (lane) {
    const superseded = [...waiting, ...activeJobs].filter((job) => job.lane === lane && job.mutable);
    const hasActive = superseded.some((job) => job.started && !job.released);
    const stableActive = [...activeJobs].some((job) => job.repo === repo && job.started && !job.released && !job.mutable);
    if (hasActive && !stableActive) requestRepositoryCancel(path);
    for (const job of superseded) {
      const index = waiting.indexOf(job);
      if (index >= 0) waiting.splice(index, 1);
      cancelJob(job);
    }
  }
  let job!: Job;
  const promise = new Promise<T>((resolve, reject) => {
    const controller = new AbortController();
    job = {
      key, command, repo, path, lane, prefetch, mutable: !stable, controller,
      resolve: resolve as (value: unknown) => void,
      reject,
      started: false,
      released: false,
      settled: false,
      cancel: () => cancelJob(job),
      start: () => {
        job.started = true;
        activeJobs.add(job);
        const execute = async () => read(controller.signal);
        void execute().then((value) => {
          if (job.settled) return;
          job.settled = true;
          if (stable) {
            const size = weight(value);
            if (size < MAX_BYTES / 2) {
              cache.set(key, { value, bytes: size }); bytes += size;
              while (cache.size > 100 || bytes > MAX_BYTES) {
                const oldest = cache.keys().next().value!; bytes -= cache.get(oldest)!.bytes; cache.delete(oldest);
              }
            }
          }
          resolve(value as T);
        }, (error) => {
          if (job.settled) return;
          job.settled = true;
          reject(error);
        }).finally(() => release(job));
      },
    };
    waiting.push(job);
  });
  job.promise = promise;
  flights.set(key, promise); pump(); return promise;
}
