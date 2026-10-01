import { invoke as rawInvoke } from "@tauri-apps/api/core";

type Args = Parameters<typeof rawInvoke>[1];
type Options = Parameters<typeof rawInvoke>[2];

type Entry = { level: "info" | "slow" | "warn" | "error"; kind: string; name: string; ms?: number; detail?: string };

export type ClientStat = { name: string; count: number; totalMs: number; maxMs: number; lastMs: number; failed: number };

const SLOW_INVOKE_MS = 150;
const LONG_TASK_MS = 100;

const live = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const queue: Entry[] = [];
const stats = new Map<string, ClientStat>();
export const frame = { longTasks: 0, longTaskMs: 0, longTaskMax: 0 };

function push(entry: Entry) {
  if (!live) return;
  queue.push(entry);
  if (queue.length > 500) queue.splice(0, queue.length - 500);
}

function record(name: string, ms: number, ok: boolean) {
  const stat = stats.get(name) ?? { name, count: 0, totalMs: 0, maxMs: 0, lastMs: 0, failed: 0 };
  stat.count += 1;
  stat.totalMs += ms;
  stat.maxMs = Math.max(stat.maxMs, ms);
  stat.lastMs = ms;
  if (!ok) stat.failed += 1;
  stats.set(name, stat);
}

export function clientStats() {
  return [...stats.values()].sort((a, b) => b.totalMs - a.totalMs);
}

export function logClient(level: Entry["level"], kind: string, name: string, detail?: string, ms?: number) {
  push({ level, kind, name, detail, ms });
}

/** Same as `invoke` from `@tauri-apps/api/core`, with round-trip time and failures logged. */
export async function invoke<T>(cmd: string, args?: Args, options?: Options): Promise<T> {
  const start = performance.now();
  try {
    const value = await rawInvoke<T>(cmd, args, options);
    const ms = performance.now() - start;
    record(cmd, ms, true);
    if (ms >= SLOW_INVOKE_MS) push({ level: "slow", kind: "invoke", name: cmd, ms });
    return value;
  } catch (error) {
    const ms = performance.now() - start;
    record(cmd, ms, false);
    push({ level: "error", kind: "invoke", name: cmd, ms, detail: String(error) });
    throw error;
  }
}

function flush() {
  if (!queue.length) return;
  const entries = queue.splice(0, queue.length);
  void rawInvoke("log_client", { entries }).catch(() => {});
}

function observe(type: string, handle: (entry: PerformanceEntry) => void) {
  try {
    new PerformanceObserver((list) => list.getEntries().forEach(handle)).observe({ type, buffered: true });
  } catch {
    /* WKWebView lacks some entry types */
  }
}

if (live) {
  observe("longtask", (entry) => {
    frame.longTasks += 1;
    frame.longTaskMs += entry.duration;
    frame.longTaskMax = Math.max(frame.longTaskMax, entry.duration);
    if (entry.duration >= LONG_TASK_MS) push({ level: "slow", kind: "longtask", name: entry.name, ms: entry.duration });
  });
  observe("paint", (entry) => push({ level: "info", kind: "paint", name: entry.name, ms: entry.startTime }));
  window.addEventListener("load", () => {
    const nav = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming | undefined;
    if (!nav) return;
    push({ level: "info", kind: "page", name: "domContentLoaded", ms: nav.domContentLoadedEventEnd });
    push({ level: "info", kind: "page", name: "load", ms: nav.loadEventEnd || performance.now() });
  });
  window.addEventListener("error", (event) => push({ level: "error", kind: "error", name: event.message, detail: `${event.filename}:${event.lineno}` }));
  window.addEventListener("unhandledrejection", (event) => push({ level: "error", kind: "rejection", name: String(event.reason) }));
  window.setInterval(flush, 2000);
  window.addEventListener("beforeunload", flush);
}
