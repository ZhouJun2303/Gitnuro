/** Measured row heights with O(log n) updates and offset lookup. */
export class RowHeights {
  private readonly values: Float64Array;
  private readonly sums: Float64Array;

  constructor(prefix: Uint32Array, rowHeight: number) {
    this.values = new Float64Array(Math.max(0, prefix.length - 1));
    this.sums = new Float64Array(prefix.length);
    for (let index = 1; index < prefix.length; index++) {
      const height = Math.max(1, (prefix[index] - prefix[index - 1]) * rowHeight);
      this.values[index - 1] = height;
      this.sums[index] += height;
      const parent = index + (index & -index);
      if (parent < prefix.length) this.sums[parent] += this.sums[index];
    }
  }

  get length() { return this.values.length; }
  get total() { return this.top(this.length); }

  top(end: number) {
    let total = 0;
    for (let index = Math.min(this.length, Math.max(0, end)); index > 0; index -= index & -index) total += this.sums[index];
    return total;
  }

  set(index: number, height: number) {
    if (index < 0 || index >= this.length || !Number.isFinite(height) || height <= 0) return false;
    const delta = height - this.values[index];
    if (Math.abs(delta) < 0.25) return false;
    this.values[index] = height;
    for (let at = index + 1; at <= this.length; at += at & -at) this.sums[at] += delta;
    return true;
  }

  at(offset: number) {
    let index = 0, sum = 0;
    for (let step = 2 ** Math.floor(Math.log2(Math.max(1, this.length))); step >= 1; step /= 2) {
      const next = index + step;
      if (next <= this.length && sum + this.sums[next] <= offset) {
        index = next;
        sum += this.sums[next];
      }
    }
    return Math.min(index, Math.max(0, this.length - 1));
  }
}
