// Generate release notes for the next publish with a user-selected AI CLI.
// publish-menu.ps1 calls `list` for the menu and `generate` for the notes.
// Chinese text crosses process boundaries through files, not stdout.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const maxCommits = 300;
const maxCommitChars = 120_000;
const maxBullets = 40;
const defaultTimeoutSeconds = 240;

const tools = [
  {
    id: "claude",
    label: "Claude Code",
    command: "claude",
    build: ({ prompt }) => ({ args: ["-p", "--output-format", "text"], stdin: prompt }),
  },
  {
    id: "codex",
    label: "Codex",
    command: "codex",
    build: ({ prompt, outFile }) => ({
      args: ["exec", "-s", "read-only", "--ephemeral", "--skip-git-repo-check", "-o", outFile],
      stdin: prompt,
      outFile,
    }),
  },
  {
    id: "grok",
    label: "Grok",
    command: "grok",
    build: ({ promptFile }) => ({ args: ["--prompt-file", promptFile, "--output-format", "plain"], stdin: null }),
  },
  {
    id: "cursor-agent",
    label: "Cursor Agent",
    command: "cursor-agent",
    build: ({ prompt }) => ({ args: ["-p", "--output-format", "text", "--mode", "ask"], stdin: prompt }),
  },
];

const sections = [
  { title: "### ✨ 新功能", test: /新功能|新增|特性|功能/ },
  { title: "### ⚡ 体验优化", test: /优化|改进|提升|性能|体验|界面|交互/ },
  { title: "### 🐛 问题修复", test: /修复|修正|问题|缺陷/ },
];

function git(args, options = {}) {
  return spawnSync("git", ["-C", root, ...args], {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    windowsHide: true,
    ...options,
  });
}

function writeUtf8(path, text) {
  writeFileSync(path, text, "utf8");
}

function findCommand(name) {
  const finder = process.platform === "win32" ? "where.exe" : "which";
  const result = spawnSync(finder, [name], { encoding: "utf8", windowsHide: true });
  if (result.status !== 0) return "";
  const paths = result.stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  if (process.platform === "win32") {
    return paths.find((path) => /\.exe$/i.test(path)) || paths.find((path) => /\.(cmd|bat)$/i.test(path)) || "";
  }
  return paths[0] || "";
}

export function detectTools() {
  return tools
    .map((tool) => ({ id: tool.id, label: tool.label, path: findCommand(tool.command) }))
    .filter((tool) => tool.path);
}

function resolveBase() {
  git(["fetch", "--quiet", "origin", "release"], { timeout: 20_000 });
  if (git(["rev-parse", "-q", "--verify", "origin/release"]).status === 0) return "origin/release";
  const tag = git(["describe", "--tags", "--abbrev=0", "--match", "v*"]);
  if (tag.status === 0 && tag.stdout.trim()) return tag.stdout.trim();
  return "";
}

function collectCommits(base) {
  const range = base ? [`${base}..HEAD`] : ["-n", "50"];
  const log = git(["log", "--no-merges", "--date=short", "--pretty=format:@@ %h %s%n%b", "--stat=140", ...range]);
  if (log.status !== 0) return "";
  const entries = log.stdout
    .replace(/\r\n/g, "\n")
    .trim()
    .split(/\n(?=@@ )/)
    .map((entry) => entry.trim())
    .filter(Boolean)
    .filter((entry) => {
      const subject = entry.split("\n")[0].replace(/^@@\s*\S+\s*/, "");
      return !/^chore:\s*(版本|发布|合并)(\s|$)/.test(subject);
    });
  const parts = [];
  let used = 0;
  for (const entry of entries) {
    if (parts.length >= maxCommits || used + entry.length > maxCommitChars) break;
    parts.push(entry);
    used += entry.length;
  }
  if (parts.length === 0) return "";
  let text = parts.join("\n\n");
  if (parts.length < entries.length) {
    text += `\n\n（还有 ${entries.length - parts.length} 条较早的提交没有列出）`;
  }
  return text;
}

export function buildPrompt(commits, version) {
  const versionLine = version ? `AweGit 是一个面向中文用户的桌面 Git 客户端，本次要发布 ${version}。` : "AweGit 是一个面向中文用户的桌面 Git 客户端。";
  return [
    "你是 AweGit 的发布编辑。",
    versionLine,
    "",
    "下面是本次发布包含的提交（自上次发布以来，格式为提交摘要、正文和改动文件统计）：",
    "",
    "<commits>",
    commits,
    "</commits>",
    "",
    "请把提交整理成用户能看懂的更新说明。只输出 Markdown，不要代码块围栏，不要开场白，不要结尾总结。",
    "",
    "格式（只保留有内容的分类，顺序固定）：",
    "### ✨ 新功能",
    "- 一句话说明一个新增功能",
    "",
    "### ⚡ 体验优化",
    "- 一句话说明一个体验、性能或界面上的改进",
    "",
    "### 🐛 问题修复",
    "- 一句话说明一个修复",
    "",
    "要求：",
    "- 每个用户能感知的小改动单独写一条，一条只讲一件事。",
    "- 同一个功能分在多个提交里完成时合并成一条；不同功能不要合并。",
    "- 每条不超过 40 个汉字，使用简体中文，从用户视角描述，不写提交类型前缀、文件名、函数名或实现细节。",
    "- 只写提交里确实包含的内容，不要推测或编造。",
    "- 纯内部改动（测试、构建、文档、无用户感知的重构）不要列出。",
    "- 不要写版本号标题，不要写“本次更新共 N 项”这类统计。",
  ].join("\n");
}

function cleanInline(text) {
  return String(text)
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/`([^`]*)`/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/__([^_]+)__/g, "$1")
    .replace(/\*([^*]+)\*/g, "$1")
    .replace(/~~([^~]+)~~/g, "$1")
    .replace(/<[^>]+>/g, "")
    .trim();
}

function canonicalHeading(text) {
  const plain = cleanInline(text).replace(/^#+\s*/, "").trim();
  for (const section of sections) {
    if (section.test.test(plain)) return section.title;
  }
  return "";
}

// Keep only headings, bullets and blank lines, then rebuild the sections in a
// fixed order so every release reads the same way.
export function sanitizeMarkdown(raw) {
  const text = String(raw)
    .replace(/\u001b\[[0-9;?]*[a-zA-Z]/g, "")
    .replace(/^﻿/, "")
    .replace(/\r\n?/g, "\n")
    .trim()
    .replace(/^```[a-zA-Z]*\s*\n/, "")
    .replace(/\n```\s*$/, "")
    .trim();
  const buckets = new Map(sections.map((section) => [section.title, []]));
  const loose = [];
  const seen = new Set();
  let current = "";

  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.startsWith("```")) continue;
    if (!trimmed) continue;
    const heading = trimmed.match(/^#{1,6}\s+(.+)$/) || trimmed.match(/^\*\*(.+?)\*\*:?$/);
    if (heading) {
      current = canonicalHeading(heading[1]);
      continue;
    }
    const bullet = trimmed.match(/^(?:[-*+•·]|\d+[.)])\s+(.+)$/);
    if (!bullet) continue;
    const item = cleanInline(bullet[1]).replace(/\s+/g, " ").trim();
    if (!item) continue;
    const key = item.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    if (current) buckets.get(current).push(item);
    else loose.push(item);
  }

  let remaining = maxBullets;
  const lines = [];
  for (const item of loose) {
    if (remaining <= 0) break;
    lines.push(`- ${item}`);
    remaining--;
  }
  for (const section of sections) {
    if (remaining <= 0) break;
    const items = buckets.get(section.title).slice(0, remaining);
    if (items.length === 0) continue;
    if (lines.length) lines.push("");
    lines.push(section.title, "");
    for (const item of items) lines.push(`- ${item}`);
    remaining -= items.length;
  }
  if (!lines.some((line) => line.startsWith("- "))) {
    throw new Error("AI 没有返回可用的更新说明");
  }
  return lines.join("\n");
}

export function toPlainText(markdown) {
  return String(markdown)
    .split("\n")
    .map((line) => {
      const trimmed = line.trimEnd();
      if (trimmed.startsWith("### ")) return cleanInline(trimmed.slice(4));
      if (trimmed.startsWith("- ")) return `· ${cleanInline(trimmed.slice(2))}`;
      return cleanInline(trimmed);
    })
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function quoteCmd(value) {
  const text = String(value);
  if (text && !/[\s"&|<>^]/.test(text)) return text;
  return `"${text.replace(/"/g, '""')}"`;
}

function buildCmdLine(exe, args) {
  const line = [quoteCmd(exe), ...args.map(quoteCmd)].join(" ");
  return line.startsWith('"') ? `"${line}"` : line;
}

function killTree(pid) {
  if (!pid) return;
  if (process.platform === "win32") {
    spawnSync("taskkill", ["/pid", String(pid), "/t", "/f"], { windowsHide: true });
    return;
  }
  try {
    process.kill(pid, "SIGKILL");
  } catch {
    // already gone
  }
}

function runTool(exe, invocation, { cwd, timeoutMs }) {
  return new Promise((done, fail) => {
    const isShim = /\.(cmd|bat)$/i.test(exe);
    const command = isShim ? process.env.ComSpec || "cmd.exe" : exe;
    const args = isShim ? ["/d", "/s", "/c", buildCmdLine(exe, invocation.args)] : invocation.args;
    const child = spawn(command, args, {
      cwd,
      windowsHide: true,
      env: { ...process.env, NO_COLOR: "1", FORCE_COLOR: "0" },
    });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      killTree(child.pid);
    }, timeoutMs);

    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      if (stdout.length < 2_000_000) stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      if (stderr.length < 200_000) stderr += chunk;
    });
    child.stdin.on("error", () => {});
    child.on("error", (error) => {
      clearTimeout(timer);
      fail(new Error(`${exe} 启动失败：${error.message}`));
    });
    child.on("close", (code) => {
      clearTimeout(timer);
      if (timedOut) {
        fail(new Error(`${exe} 超过 ${Math.round(timeoutMs / 1000)} 秒没有返回`));
        return;
      }
      if (code !== 0) {
        const tail = stderr.trim().split("\n").slice(-8).join("\n");
        fail(new Error(`${exe} 退出码 ${code}${tail ? `：\n${tail}` : ""}`));
        return;
      }
      let answer = "";
      if (invocation.outFile && existsSync(invocation.outFile)) {
        answer = readFileSync(invocation.outFile, "utf8");
      }
      if (!answer.trim()) answer = stdout;
      done(answer);
    });
    if (invocation.stdin != null) child.stdin.end(invocation.stdin);
    else child.stdin.end();
  });
}

function parseArgs(argv) {
  const values = {};
  for (let i = 0; i < argv.length; i++) {
    const key = argv[i];
    if (!key.startsWith("--")) continue;
    const value = argv[i + 1];
    if (value === undefined || value.startsWith("--")) {
      values[key.slice(2)] = "";
      continue;
    }
    values[key.slice(2)] = value;
    i++;
  }
  return values;
}

async function generate(options) {
  const toolId = options.tool || "";
  const tool = tools.find((item) => item.id === toolId);
  if (!tool) throw new Error(`未知的 AI 工具：${toolId || "（空）"}`);
  const exe = findCommand(tool.command);
  if (!exe) throw new Error(`没有找到 ${tool.command} 命令`);
  if (!options.text) throw new Error("缺少 --text 输出文件");

  const base = resolveBase();
  const commits = collectCommits(base);
  if (!commits) throw new Error("上次发布之后没有找到可以整理的提交");
  const prompt = buildPrompt(commits, options.version || "");

  const workDir = mkdtempSync(join(tmpdir(), "awegit-notes-"));
  try {
    const promptFile = join(workDir, "prompt.md");
    const outFile = join(workDir, "answer.md");
    writeUtf8(promptFile, prompt);
    if (options.prompt) writeUtf8(options.prompt, prompt);
    const invocation = tool.build({ prompt, promptFile, outFile });
    const timeoutSeconds = Number(options.timeout) || defaultTimeoutSeconds;
    // Run from the repo: the user's CLI logins already trust it, and a fresh
    // temp directory makes some of them stop at a trust prompt.
    const raw = await runTool(exe, invocation, { cwd: root, timeoutMs: timeoutSeconds * 1000 });
    const markdown = sanitizeMarkdown(raw);
    writeUtf8(options.text, toPlainText(markdown) + "\n");
    if (options.md) writeUtf8(options.md, markdown + "\n");
  } finally {
    rmSync(workDir, { recursive: true, force: true });
  }
}

async function main() {
  const [command = "", ...rest] = process.argv.slice(2);
  const options = parseArgs(rest);
  if (command === "list") {
    for (const tool of detectTools()) process.stdout.write(`${tool.id}|${tool.label}\n`);
    return;
  }
  if (command !== "generate") {
    process.stderr.write(
      "用法：release-notes.mjs list | generate --tool <id> --text <path> [--md <path>] [--version <ver>] [--prompt <path>] [--error <path>]\n",
    );
    process.exitCode = 2;
    return;
  }
  try {
    await generate(options);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (options.error) writeUtf8(options.error, `${message}\n`);
    process.stderr.write(`${message}\n`);
    process.exitCode = 1;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main();
}
