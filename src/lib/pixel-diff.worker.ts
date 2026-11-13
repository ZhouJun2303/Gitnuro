import { renderPixelDiff } from "./pixel-render";

function imageBlob(src: string): Blob {
  const comma = src.indexOf(",");
  if (!src.startsWith("data:") || comma < 0) throw new Error("image data");
  const header = src.slice(5, comma);
  const content = src.slice(comma + 1);
  if (!header.endsWith(";base64")) {
    return new Blob([decodeURIComponent(content)], { type: header.split(";")[0] });
  }
  const binary = atob(content);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index++) bytes[index] = binary.charCodeAt(index);
  return new Blob([bytes], { type: header.split(";")[0] });
}

self.onmessage = async ({ data }: MessageEvent<{ before: string; after: string }>) => {
  let before: ImageBitmap | undefined;
  let after: ImageBitmap | undefined;
  try {
    // Decode here as well: creating a full bitmap on the UI thread can itself stall.
    before = await createImageBitmap(imageBlob(data.before));
    after = await createImageBitmap(imageBlob(data.after));
    const output = new OffscreenCanvas(1, 1);
    const scratch = new OffscreenCanvas(1, 1);
    const context = output.getContext("2d");
    const tile = scratch.getContext("2d", { willReadFrequently: true });
    if (!context || !tile) throw new Error("canvas");
    await renderPixelDiff({ image: before, width: before.width, height: before.height }, { image: after, width: after.width, height: after.height }, context, tile);
    const blob = await output.convertToBlob({ type: "image/png" });
    self.postMessage({ blob });
  } catch {
    self.postMessage({ error: "pixel comparison failed" });
  } finally {
    before?.close();
    after?.close();
  }
};
