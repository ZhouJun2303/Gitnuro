import { invoke } from "../perf";

const resolved = new Map<string, string>();
const pending = new Map<string, Promise<string | null>>();

/** Data URL loaded this session, if any. */
export function cachedAvatar(url: string) {
  return resolved.get(url) ?? null;
}

/** Load an avatar through the backend disk cache; a failure leaves the initial in place. */
export function loadAvatar(url: string): Promise<string | null> {
  const hit = resolved.get(url);
  if (hit) return Promise.resolve(hit);
  const running = pending.get(url);
  if (running) return running;
  const task = invoke<string>("avatar_image", { url })
    .then((src) => {
      if (!src.startsWith("data:image/")) return null;
      resolved.set(url, src);
      return src;
    })
    .catch(() => null)
    .finally(() => {
      pending.delete(url);
    });
  pending.set(url, task);
  return task;
}
