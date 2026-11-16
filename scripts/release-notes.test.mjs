import assert from "node:assert/strict";
import test from "node:test";
import { buildPrompt, sanitizeMarkdown, toPlainText } from "./release-notes.mjs";

test("strips fences and preamble, maps headings to the three sections", () => {
  const raw = [
    "好的，以下是更新说明：",
    "```markdown",
    "## 新功能",
    "* 提交图支持按作者过滤",
    "- 更新面板显示下载速度",
    "",
    "## 改进",
    "* 打开仓库更快",
    "",
    "## 修复",
    "- 按 Delete 打开丢弃确认时默认选中确认",
    "- **修复** 主题切换闪烁",
    "```",
    "希望有帮助。",
  ].join("\n");
  assert.equal(
    sanitizeMarkdown(raw),
    [
      "### ✨ 新功能",
      "",
      "- 提交图支持按作者过滤",
      "- 更新面板显示下载速度",
      "",
      "### ⚡ 体验优化",
      "",
      "- 打开仓库更快",
      "",
      "### 🐛 问题修复",
      "",
      "- 按 Delete 打开丢弃确认时默认选中确认",
      "- 修复 主题切换闪烁",
    ].join("\n"),
  );
});

test("sections keep a fixed order even when the model writes fixes first", () => {
  const raw = ["### 修复", "- 修好一个问题", "", "### 新功能", "- 增加一个功能"].join("\n");
  assert.equal(
    sanitizeMarkdown(raw),
    ["### ✨ 新功能", "", "- 增加一个功能", "", "### 🐛 问题修复", "", "- 修好一个问题"].join("\n"),
  );
});

test("drops duplicate bullets and prose between them", () => {
  const raw = ["### 新功能", "- 同一个改动", "这句是废话。", "- 同一个改动", "- 另一个改动"].join("\n");
  assert.equal(sanitizeMarkdown(raw), ["### ✨ 新功能", "", "- 同一个改动", "- 另一个改动"].join("\n"));
});

test("rejects an answer without bullets", () => {
  assert.throws(() => sanitizeMarkdown("抱歉，我无法整理这些提交。"), /没有返回可用的更新说明/);
});

test("plain text keeps one line per change and removes Markdown markers", () => {
  const markdown = ["### ✨ 新功能", "", "- **提交图** 支持按作者过滤", "- 打开 `仓库` 更快[详情](https://example.com)"].join("\n");
  assert.equal(toPlainText(markdown), ["✨ 新功能", "", "· 提交图 支持按作者过滤", "· 打开 仓库 更快详情"].join("\n"));
});

test("prompt asks for one line per small change and carries the commits", () => {
  const prompt = buildPrompt("@@ abc123 feat: 支持过滤\n src/a.ts | 2 +-", "2.0.11");
  assert.match(prompt, /2\.0\.11/);
  assert.match(prompt, /@@ abc123 feat: 支持过滤/);
  assert.match(prompt, /每个用户能感知的小改动单独写一条/);
  assert.match(prompt, /### 🐛 问题修复/);
});
