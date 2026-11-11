import { expect, test } from "@playwright/test";
import { mergeBlocks, MergeLayout } from "../src/lib/shell/mergeLayout";
import type { MergePiece } from "../src/lib/shell/types";

test("merge blocks preserve order, blank lines and trailing newline across a huge hunk", () => {
  const text = Array.from({ length: 10_000 }, (_, i) => i % 7 ? `line ${i}` : "").join("\n") + "\n";
  const pieces: MergePiece[] = [{ kind: "hunk", ours: "ours", theirs: text, pick: "theirs" }];
  const blocks = mergeBlocks(pieces, "result", "empty", "pending");
  expect(blocks.length).toBeGreaterThan(300);
  expect(blocks.every((block) => block.text.split("\n").length <= 32)).toBe(true);
  expect(blocks.map((block) => block.text).join("\n")).toBe(text);
  pieces[0].pick = "both";
  expect(mergeBlocks(pieces, "result", "empty", "pending").map((block) => block.text).join("\n")).toBe(`${text}\nours`);
  expect(mergeBlocks(pieces, "result", "empty", "pending", "manual\n\n").map((block) => block.text).join("\n")).toBe("manual\n\n");
});

test("merge height index follows measured heights and exact viewport boundaries", () => {
  const blocks = mergeBlocks([{ kind: "text", text: Array.from({ length: 1000 }, (_, i) => String(i)).join("\n") }], "ours", "empty", "pending");
  const layout = new MergeLayout(blocks, 600);
  let total = 0;
  blocks.forEach((_, i) => { layout.measure(i, (i + 1) * 10); });
  blocks.forEach((_, i) => {
    expect(layout.offset(i)).toBeCloseTo(total, 6);
    expect(layout.at(total + 0.001)).toBe(i);
    total += (i + 1) * 10;
    expect(layout.at(total - 0.1)).toBe(i);
  });
  expect(layout.height).toBeCloseTo(total, 6);
  expect(layout.at(total + 100)).toBe(blocks.length - 1);
  expect(layout.at(-10)).toBe(0);
  expect(layout.measure(0, NaN)).toBe(false);
  expect(new MergeLayout([], 480).height).toBe(0);
});
