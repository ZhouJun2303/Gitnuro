import { pixelDimensions, renderPixelDiff } from "./pixel-render";
export { PixelDiffLimitError } from "./pixel-render";

function loadImage(src: string, signal: AbortSignal): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    signal.throwIfAborted();
    const image = new Image();
    const clean = () => {
      signal.removeEventListener("abort", abort);
      image.onload = null;
      image.onerror = null;
    };
    const abort = () => { clean(); image.src = ""; reject(signal.reason); };
    image.onload = () => { clean(); resolve(image); };
    image.onerror = () => { clean(); reject(new Error("image")); };
    signal.addEventListener("abort", abort, { once: true });
    image.src = src;
  });
}

function compareInWorker(before: string, after: string, signal: AbortSignal): Promise<Blob> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL("./pixel-diff.worker.ts", import.meta.url), { type: "module" });
    let settled = false;
    const finish = (error?: unknown, blob?: Blob) => {
      if (settled) return;
      settled = true;
      worker.terminate();
      signal.removeEventListener("abort", abort);
      if (blob) resolve(blob);
      else reject(error);
    };
    const abort = () => finish(signal.reason);
    worker.onmessage = ({ data }: MessageEvent<{ blob?: Blob; error?: string }>) => finish(data.error, data.blob);
    worker.onerror = (event) => { event.preventDefault(); finish(new Error("worker")); };
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) abort();
    else {
      try { worker.postMessage({ before, after }); }
      catch (error) { finish(error); }
    }
  });
}

export async function comparePixels(left: string, right: string, signal: AbortSignal): Promise<Blob> {
  signal.throwIfAborted();
  const loading = new AbortController();
  const abort = () => loading.abort(signal.reason);
  signal.addEventListener("abort", abort, { once: true });
  try {
    const [before, after] = await Promise.all([loadImage(left, loading.signal), loadImage(right, loading.signal)]);
    return await compareLoadedPixels(before, after, left, right, signal);
  } finally {
    loading.abort();
    signal.removeEventListener("abort", abort);
  }
}

async function compareLoadedPixels(before: HTMLImageElement, after: HTMLImageElement, left: string, right: string, signal: AbortSignal): Promise<Blob> {
  const first = { image: before, width: before.naturalWidth, height: before.naturalHeight };
  const second = { image: after, width: after.naturalWidth, height: after.naturalHeight };
  pixelDimensions(first, second);
  signal.throwIfAborted();
  if (typeof Worker !== "undefined" && typeof OffscreenCanvas !== "undefined" && typeof createImageBitmap === "function") {
    try {
      return await compareInWorker(left, right, signal);
    } catch (error) {
      if (signal.aborted) throw error;
      // Some older webviews expose these APIs but cannot use them in a worker.
    }
  }
  const output = document.createElement("canvas");
  const scratch = document.createElement("canvas");
  const context = output.getContext("2d");
  const tile = scratch.getContext("2d", { willReadFrequently: true });
  if (!context || !tile) throw new Error("canvas");
  await renderPixelDiff(first, second, context, tile, signal, true);
  const blob = await new Promise<Blob>((resolve, reject) => output.toBlob((blob) => blob ? resolve(blob) : reject(new Error("image")), "image/png"));
  signal.throwIfAborted();
  return blob;
}
