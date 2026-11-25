/**
 * Shared scheduler for recurring background work.
 *
 * The browser has no Unity-style component Update hook. A single timeout
 * dispatcher is enough for background polling and avoids one native timer per
 * feature. Visual work should continue to use requestAnimationFrame directly.
 */

export type UpdateInterval = number | (() => number);
export type UpdateCallback = () => void | Promise<void>;

type Job = {
  callback: UpdateCallback;
  interval: UpdateInterval;
  nextAt: number;
  running: boolean;
  active: boolean;
};

const jobs = new Set<Job>();
let wakeTimer: number | undefined;

function intervalMs(job: Job) {
  try {
    const value = typeof job.interval === "function" ? job.interval() : job.interval;
    return Number.isFinite(value) && value > 0 ? value : 1000;
  } catch {
    return 1000;
  }
}

function arm() {
  if (typeof window === "undefined") return;
  if (wakeTimer !== undefined) {
    window.clearTimeout(wakeTimer);
    wakeTimer = undefined;
  }
  let nextAt = Number.POSITIVE_INFINITY;
  for (const job of jobs) {
    if (job.active && !job.running) nextAt = Math.min(nextAt, job.nextAt);
  }
  if (!Number.isFinite(nextAt)) return;
  wakeTimer = window.setTimeout(runDue, Math.max(0, nextAt - performance.now()));
}

function runJob(job: Job) {
  if (!job.active || job.running) return;
  job.running = true;
  job.nextAt = Number.POSITIVE_INFINITY;
  void Promise.resolve()
    .then(() => {
      if (job.active) return job.callback();
    })
    .catch(() => {
      /* Individual background jobs own their user-visible error handling. */
    })
    .finally(() => {
      job.running = false;
      if (!job.active) return;
      job.nextAt = performance.now() + intervalMs(job);
      arm();
    });
}

function runDue() {
  wakeTimer = undefined;
  const now = performance.now();
  for (const job of [...jobs]) {
    if (job.active && !job.running && job.nextAt <= now) runJob(job);
  }
  arm();
}

/**
 * Register recurring work and return an idempotent unsubscribe function.
 * Async callbacks never overlap with themselves. A callback starts its next
 * interval after it settles, so a slow backend call cannot build a queue.
 */
export function subscribeUpdate(callback: UpdateCallback, interval: UpdateInterval, immediate = false) {
  if (typeof window === "undefined") return () => {};
  const job: Job = {
    callback,
    interval,
    nextAt: 0,
    running: false,
    active: true,
  };
  job.nextAt = performance.now() + intervalMs(job);
  jobs.add(job);
  if (immediate) runJob(job);
  arm();
  return () => {
    if (!job.active) return;
    job.active = false;
    jobs.delete(job);
    arm();
  };
}
