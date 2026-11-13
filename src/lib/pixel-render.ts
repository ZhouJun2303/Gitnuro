export const MAX_PIXEL_DIFF_PIXELS = 4096 * 4096;
const MAX_SIDE = 16384;
const TILE = 256;

export class PixelDiffLimitError extends Error {}

export type PixelSource = { image: CanvasImageSource; width: number; height: number };
type PixelContext = CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D;

export function pixelDimensions(before: Pick<PixelSource, "width" | "height">, after: Pick<PixelSource, "width" | "height">) {
  const width = Math.max(before.width, after.width);
  const height = Math.max(before.height, after.height);
  if (width < 1 || height < 1 || width > MAX_SIDE || height > MAX_SIDE || width * height > MAX_PIXEL_DIFF_PIXELS) {
    throw new PixelDiffLimitError("pixel limit");
  }
  return { width, height };
}

export async function renderPixelDiff(before: PixelSource, after: PixelSource, output: PixelContext, scratch: PixelContext, signal?: AbortSignal, yieldToUi = false) {
  const { width, height } = pixelDimensions(before, after);
  output.canvas.width = width;
  output.canvas.height = height;
  scratch.canvas.width = TILE;
  scratch.canvas.height = TILE;
  const tile = (source: PixelSource, x: number, y: number, columns: number, rows: number) => {
    scratch.clearRect(0, 0, TILE, TILE);
    const w = Math.min(columns, source.width - x);
    const h = Math.min(rows, source.height - y);
    if (w > 0 && h > 0) scratch.drawImage(source.image, x, y, w, h, 0, 0, w, h);
    return scratch.getImageData(0, 0, columns, rows);
  };
  let frame = performance.now();
  for (let y = 0; y < height; y += TILE) {
    for (let x = 0; x < width; x += TILE) {
      signal?.throwIfAborted();
      const columns = Math.min(TILE, width - x);
      const rows = Math.min(TILE, height - y);
      // Two 256x256 RGBA buffers use at most 512 KiB, independent of image size.
      const left = tile(before, x, y, columns, rows);
      const right = tile(after, x, y, columns, rows);
      for (let index = 0; index < left.data.length; index += 4) {
        left.data[index] = Math.abs(left.data[index] - right.data[index]);
        left.data[index + 1] = Math.abs(left.data[index + 1] - right.data[index + 1]);
        left.data[index + 2] = Math.abs(left.data[index + 2] - right.data[index + 2]);
        left.data[index + 3] = 255;
      }
      output.putImageData(left, x, y);
      // Older webviews without OffscreenCanvas still return control during comparison.
      if (yieldToUi && performance.now() - frame >= 4) {
        await new Promise<void>((resolve) => setTimeout(resolve, 0));
        frame = performance.now();
      }
    }
  }
  signal?.throwIfAborted();
}
