// Builds the ignored .lab git repositories the browser lab opens.
// Uses only local git config (-c and the repo's own config). Never touches the global gitconfig
// and never writes inside the AweGit repository except under .lab/.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { deflateSync } from "node:zlib";

if (process.argv.includes("--performance")) {
  await import("./perf-seed.mjs");
  process.exit(0);
}

const root = resolve(fileURLToPath(new URL(".", import.meta.url)), "..");
const lab = join(root, ".lab");
const repos = join(lab, "repos");
const shelf = join(repos, "shelf");
const remote = join(lab, "remote.git");
const remotePush = join(lab, "remote-push.git");
const subBare = join(lab, "sub.git");
const healthBare = join(lab, "sub-health.git");
const editor = join(root, "scripts", "lab-break-editor.mjs");
const FILL = 24;

const pngA = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);
const pngB = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
  "base64",
);
const pngC = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==",
  "base64",
);
const xlsx = (mark) => {
  const body = Buffer.concat([Buffer.from("PK\u0003\u0004"), Buffer.from(mark), Buffer.from([0])]);
  const size = mark.endsWith("b") ? 1_087_487 : 939_456;
  return Buffer.concat([body, Buffer.alloc(size - body.length)]);
};
const largePak = (mark) => Buffer.concat([Buffer.from(`lab-large-${mark}\n`), Buffer.alloc(3 * 1024 * 1024 + (mark === "b" ? 120_000 : 0))]);

const textExts = [
  "js", "py", "rs", "json", "css", "html", "md", "sql", "yml", "toml", "csv", "go", "java", "xml", "sh",
  "ts", "tsx", "c", "cpp", "h", "rb", "php", "swift", "kt",
];

const gitEnv = {
  ...process.env,
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_CONFIG_GLOBAL: join(lab, "gitconfig"),
  GIT_AUTHOR_NAME: "AweGit Lab",
  GIT_AUTHOR_EMAIL: "lab@example.com",
  GIT_COMMITTER_NAME: "AweGit Lab",
  GIT_COMMITTER_EMAIL: "lab@example.com",
};
delete gitEnv.GIT_DIR;
delete gitEnv.GIT_WORK_TREE;

function wipe(path) {
  if (!existsSync(path)) return;
  rmSync(path, { recursive: true, force: true, maxRetries: 8, retryDelay: 150 });
}

function git(cwd, args, { allowFail = false, extraEnv = {} } = {}) {
  const result = spawnSync(
    "git",
    ["-c", "user.name=AweGit Lab", "-c", "user.email=lab@example.com", "-c", "commit.gpgsign=false", "-c", "core.autocrlf=false", ...args],
    { cwd, env: { ...gitEnv, ...extraEnv }, encoding: "utf8" },
  );
  if (result.error) throw result.error;
  if (!allowFail && result.status !== 0) {
    throw new Error(`git ${args.join(" ")} failed in ${cwd}\n${result.stdout ?? ""}\n${result.stderr ?? ""}`);
  }
  return result;
}

function write(repo, path, body) {
  const full = join(repo, path);
  mkdirSync(dirname(full), { recursive: true });
  writeFileSync(full, body);
}

function commit(repo, path, body, message) {
  write(repo, path, body);
  git(repo, ["add", "--", path]);
  git(repo, ["commit", "-m", message, "--", path]);
}

function fileUrl(path) {
  const text = path.replaceAll("\\", "/");
  return text.startsWith("//") ? `file:${text}` : `file:///${text}`;
}

function pad(n) {
  return String(n).padStart(2, "0");
}

function lines(count, prefix) {
  return Array.from({ length: count }, (_, index) => `${prefix} ${index + 1}`).join("\n") + "\n";
}

function initBare(path) {
  mkdirSync(dirname(path), { recursive: true });
  git(lab, ["init", "--bare", "-b", "main", path]);
}

function cloneFrom(url, path) {
  mkdirSync(dirname(path), { recursive: true });
  git(lab, ["clone", url, path]);
  git(path, ["config", "user.name", "AweGit Lab"]);
  git(path, ["config", "user.email", "lab@example.com"]);
  git(path, ["config", "commit.gpgsign", "false"]);
  git(path, ["config", "protocol.file.allow", "always"]);
}

function initWork(path) {
  mkdirSync(path, { recursive: true });
  git(path, ["init", "-b", "main"]);
  git(path, ["config", "user.name", "AweGit Lab"]);
  git(path, ["config", "user.email", "lab@example.com"]);
  git(path, ["config", "commit.gpgsign", "false"]);
  git(path, ["config", "protocol.file.allow", "always"]);
}

function has(repo, rel) {
  return existsSync(join(repo, rel));
}

function pauseIndex(repo, head) {
  const unmerged = git(repo, ["diff", "--name-only", "--diff-filter=U"], { allowFail: true });
  for (const line of (unmerged.stdout ?? "").split(/\r?\n/)) {
    const file = line.trim();
    if (!file) continue;
    git(repo, ["checkout", "--ours", "--", file], { allowFail: true });
    git(repo, ["add", "--", file]);
  }
  if (!has(repo, join(".git", head))) {
    throw new Error(`${repo} did not keep ${head}`);
  }
  const left = git(repo, ["diff", "--name-only", "--diff-filter=U"]);
  if ((left.stdout ?? "").trim()) throw new Error(`${repo} still has conflicts:\n${left.stdout}`);
}

function dirtyBatch(repo, folder, count = FILL) {
  for (let i = 1; i <= count; i++) {
    write(repo, `${folder}/dirty-${pad(i)}.txt`, `${folder} dirty ${i}\n`);
  }
}

// Every entry is either shared context or one region both branches rewrite.
// Together they merge into many separate conflict hunks in one file.
const manyHunkSpec = [
  {
    base: ['export const project = "base";'],
    topic: ['export const project = "topic";'],
    main: ['export const project = "main";'],
  },
  {
    stable: ["", 'import { readFile } from "node:fs/promises";', "", "// 02 · both sides replace one line"],
    base: ['export const mode = "base";'],
    topic: ['export const mode = "topic";'],
    main: ['export const mode = "main";'],
  },
  {
    stable: ["", "const stableOne = 1;", "const stableTwo = 2;", "", "// 03 · topic deletes, main edits"],
    base: ['const legacyA = "base";', 'const legacyB = "base";'],
    topic: [],
    main: ['const legacyA = "main";', 'const legacyB = "main";'],
  },
  {
    stable: ["", "const stableThree = 3;", "const stableFour = 4;", "", "// 04 · main deletes, topic edits"],
    base: ['const retiredA = "base";', 'const retiredB = "base";'],
    topic: ['const retiredA = "topic";', 'const retiredB = "topic";'],
    main: [],
  },
  {
    stable: ["", "const stableFive = 5;", "", "// 05 · both sides insert in the same spot", 'const beforeInsert = "kept";'],
    base: [],
    topic: ['const insertedByTopic = true;', "const topicKeepsOrder = 1;"],
    main: ['const insertedByMain = true;'],
  },
  {
    stable: ["", 'const afterInsert = "kept";', "", "const stableSix = 6;", "const stableSeven = 7;", "", "// 06 · one line on topic, four on main"],
    base: ["function settle() {", '  return "base";', "}"],
    topic: ['const settle = () => "topic";'],
    main: ["function settle() {", "  const rate = 2;", "  const fee = 1;", "  return rate + fee;", "}"],
  },
  {
    stable: ["", "const stableEight = 8;", "", "// 07 · blank line inside the block"],
    base: ['const blankA = "base";', "", 'const blankB = "base";'],
    topic: ['const blankA = "topic";', "", 'const blankB = "topic";'],
    main: ['const blankA = "main";', "", 'const blankB = "main";'],
  },
  {
    stable: ["", "const stableNine = 9;", "", "// 08 · overlapping but offset edits"],
    base: ["const overlapA = 1;", "const overlapB = 2;", "const overlapC = 3;"],
    topic: ["const overlapA = 10;", "const overlapB = 20;", "const overlapC = 3;"],
    main: ["const overlapA = 1;", "const overlapB = 200;", "const overlapC = 300;"],
  },
  {
    stable: ["", "const stableTen = 10;", "", "// 09 · tabs and Chinese text"],
    base: ['const greeting = "你好";', '\tconst indented = "base";'],
    topic: ['const greeting = "你好，主题";', '\tconst indented = "topic";'],
    main: ['const greeting = "你好，主分支";', '\tconst indented = "main";'],
  },
  {
    stable: ["", "const stableEleven = 11;", "", "// 10 · whitespace-only reindent"],
    base: ['const spaced = "base";'],
    topic: ['\tconst spaced = "base";'],
    main: ['    const spaced = "base";'],
  },
  {
    stable: ["", "const stableTwelve = 12;", "", "// 11 · long line replaced on both sides"],
    base: ['const notice = "base notice — every side rewrites this long sentence so the row overflows the pane and the merger has to wrap or scroll it horizontally";'],
    topic: ['const notice = "topic notice — the topic branch rewrites the same long sentence with its own wording, keeping the line long enough to overflow the pane";'],
    main: ['const notice = "main notice — the main branch rewrites the same long sentence with different wording, also wide enough to overflow the pane";'],
  },
  {
    stable: ["", "const stableThirteen = 13;", "", "// 12 · edits one kept line apart coalesce into one block"],
    base: ['const pairA = "base";', 'const gap = "kept";', 'const pairB = "base";'],
    topic: ['const pairA = "topic";', 'const gap = "kept";', 'const pairB = "topic";'],
    main: ['const pairA = "main";', 'const gap = "kept";', 'const pairB = "main";'],
  },
  {
    stable: ["", "const stableFourteen = 14;", "", "// 13 · tall block on both sides"],
    base: Array.from({ length: 12 }, (_, index) => `const tall${index + 1} = "base";`),
    topic: Array.from({ length: 12 }, (_, index) => `const tall${index + 1} = "topic";`),
    main: Array.from({ length: 12 }, (_, index) => `const tall${index + 1} = "main";`),
  },
  {
    stable: ["", "const stableFifteen = 15;", "const stableSixteen = 16;", "", "// 14 · last line replaced on both sides"],
    base: ['const closing = "base";'],
    topic: ['const closing = "topic";'],
    main: ['const closing = "main";'],
  },
];

function manyHunkText(side) {
  return manyHunkSpec.flatMap((part) => [...(part.stable ?? []), ...(part[side] ?? [])]).join("\n") + "\n";
}

// A Unity-style YAML asset past the old 2 MB diff cutoff, with one changed header line.
function bigAsset(name) {
  const head = [
    "%YAML 1.1",
    "%TAG !u! tag:unity3d.com,2011:",
    "--- !u!114 &11400000",
    "MonoBehaviour:",
    `  m_Name: ${name}`,
    "  m_Entries:",
  ];
  const row = "0123456789abcdef".repeat(58);
  const rows = Array.from({ length: 2400 }, (_, index) => `  m_Entry_${index}: ${row}`);
  return `${[...head, ...rows].join("\n")}\n`;
}

function conflictRepo(path, pause) {
  initWork(path);
  const longBase = lines(40, "both base");
  const longTopic = lines(40, "both topic");
  const longMain = lines(40, "both main");
  commit(path, "src/both-sides.ts", longBase, "base text");
  commit(path, "src/many-hunks.ts", manyHunkText("base"), "base many hunks");
  commit(path, "docs/removed-there.md", "keep\n", "base doc");
  commit(path, "docs/removed-here.md", "keep here\n", "base kept here");
  commit(path, "art/removed-here.png", pngA, "base image");
  commit(path, "data/sheet.xlsx", xlsx("base"), "base sheet");
  commit(path, "src/shared.txt", "shared base\n", "shared name");
  commit(path, "assets/Config/Generated.asset", bigAsset("base"), "base large asset");
  for (let i = 0; i < FILL; i++) {
    const ext = textExts[i % textExts.length];
    commit(path, `conflict/file-${pad(i + 1)}.${ext}`, `base ${ext} ${i + 1}\n`, `base conflict ${i + 1}`);
  }
  git(path, ["checkout", "-b", "topic"]);
  write(path, "src/both-sides.ts", longTopic);
  git(path, ["add", "--", "src/both-sides.ts"]);
  git(path, ["commit", "-m", "topic text", "--", "src/both-sides.ts"]);
  commit(path, "src/many-hunks.ts", manyHunkText("topic"), "topic many hunks");
  commit(path, "assets/Config/Generated.asset", bigAsset("topic"), "topic large asset");
  git(path, ["rm", "docs/removed-there.md"]);
  git(path, ["commit", "-m", "topic removes the doc"]);
  commit(path, "docs/removed-here.md", "keep here\ntopic edit\n", "topic edits kept here");
  commit(path, "src/both-added.ts", "export const added = \"topic\";\n", "topic adds a file");
  commit(path, "art/both-added.png", pngB, "topic adds image");
  commit(path, "art/removed-here.png", pngB, "topic image");
  commit(path, "data/sheet.xlsx", xlsx("topic"), "topic sheet");
  git(path, ["mv", "src/shared.txt", "src/shared-topic.txt"]);
  git(path, ["commit", "-m", "topic renames shared"]);
  for (let i = 0; i < FILL; i++) {
    const ext = textExts[i % textExts.length];
    write(path, `conflict/file-${pad(i + 1)}.${ext}`, `topic ${ext} ${i + 1}\nline two\n`);
  }
  git(path, ["add", "--", "conflict"]);
  git(path, ["commit", "-m", "topic conflict files"]);
  for (let i = 1; i <= FILL; i++) {
    commit(path, `topic/extra-${pad(i)}.md`, `topic extra ${i}\n`, `topic extra ${i}`);
  }
  git(path, ["checkout", "main"]);
  write(path, "src/both-sides.ts", longMain);
  git(path, ["add", "--", "src/both-sides.ts"]);
  git(path, ["commit", "-m", "main text", "--", "src/both-sides.ts"]);
  commit(path, "src/many-hunks.ts", manyHunkText("main"), "main many hunks");
  commit(path, "assets/Config/Generated.asset", bigAsset("main"), "main large asset");
  commit(path, "docs/removed-there.md", "keep\nmain edit\n", "main edits the doc");
  git(path, ["rm", "docs/removed-here.md"]);
  git(path, ["commit", "-m", "main removes kept here"]);
  commit(path, "src/both-added.ts", "export const added = \"main\";\n", "main adds a file");
  commit(path, "art/both-added.png", pngC, "main adds image");
  commit(path, "art/removed-here.png", pngC, "main image");
  commit(path, "data/sheet.xlsx", xlsx("main"), "main sheet");
  git(path, ["mv", "src/shared.txt", "src/shared-main.txt"]);
  git(path, ["commit", "-m", "main renames shared"]);
  for (let i = 0; i < FILL; i++) {
    const ext = textExts[i % textExts.length];
    write(path, `conflict/file-${pad(i + 1)}.${ext}`, `main ${ext} ${i + 1}\nother\n`);
  }
  git(path, ["add", "--", "conflict"]);
  git(path, ["commit", "-m", "main conflict files"]);
  const merged = git(path, ["merge", "topic"], { allowFail: true });
  if (merged.status === 0) throw new Error(`${path} merge did not conflict`);
  if (!has(path, join(".git", "MERGE_HEAD"))) throw new Error(`${path} has no MERGE_HEAD`);
  if (pause) {
    pauseIndex(path, "MERGE_HEAD");
    dirtyBatch(path, "paused", FILL);
  }
}

function diverge(path, verb) {
  initWork(path);
  if (verb === "cherry") {
    commit(path, "a.txt", "one\n", "first");
    git(path, ["checkout", "-b", "side"]);
    commit(path, "a.txt", "side\n", "side");
    git(path, ["checkout", "main"]);
    commit(path, "a.txt", "main\n", "main");
    const result = git(path, ["cherry-pick", "side"], { allowFail: true });
    if (result.status === 0) throw new Error(`${path} cherry-pick did not conflict`);
    pauseIndex(path, "CHERRY_PICK_HEAD");
    dirtyBatch(path, "paused", FILL);
    return;
  }
  commit(path, "a.txt", "one\n", "first");
  commit(path, "a.txt", "two\n", "second");
  commit(path, "a.txt", "three\n", "third");
  const result = git(path, ["revert", "--no-edit", "HEAD~1"], { allowFail: true });
  if (result.status === 0) throw new Error(`${path} revert did not conflict`);
  pauseIndex(path, "REVERT_HEAD");
  dirtyBatch(path, "paused", FILL);
}

function seedConflictRepo() {
  const path = join(repos, "conflict");
  wipe(path);
  conflictRepo(path, null);
}

function seedMergeRepo() {
  const path = join(repos, "merge");
  wipe(path);
  conflictRepo(path, pauseIndex);
}

function seedRebaseRepo() {
  const path = join(repos, "rebase");
  wipe(path);
  initWork(path);
  commit(path, "README.md", "rebase base\n", "rebase base");
  git(path, ["checkout", "-b", "side"]);
  for (let i = 1; i <= FILL; i++) {
    commit(path, `side/step-${pad(i)}.txt`, `side ${i}\n`, `side work ${i}`);
  }
  git(path, ["checkout", "main"]);
  commit(path, "main.txt", "main\n", "main work");
  git(path, ["checkout", "side"]);
  const editorCmd = `node "${editor.replaceAll("\\", "/")}"`;
  const paused = git(path, ["rebase", "-i", "main"], {
    allowFail: true,
    extraEnv: { GIT_SEQUENCE_EDITOR: editorCmd },
  });
  if (!has(path, join(".git", "rebase-merge")) && !has(path, join(".git", "rebase-apply"))) {
    throw new Error(`rebase did not pause\n${paused.stdout ?? ""}\n${paused.stderr ?? ""}`);
  }
  dirtyBatch(path, "paused", FILL);
}

function seedCherryRepo() {
  const path = join(repos, "cherry");
  wipe(path);
  diverge(path, "cherry");
}

function seedOperationRepo() {
  const path = join(repos, "operation");
  wipe(path); initWork(path);
  commit(path, "note.txt", "base\n", "base"); git(path, ["branch", "incoming"]);
  commit(path, "note.txt", "local one\n", "first local");
  commit(path, "clean.txt", "clean\n", "between conflicts");
  commit(path, "note.txt", "local two\n", "second local");
  git(path, ["checkout", "incoming"]); commit(path, "note.txt", "remote\n", "incoming change");
  git(path, ["checkout", "main"]); write(path, "note.txt", "uncommitted work\n");
}

function seedUnstageInitialRepo() {
  const path = join(repos, "unstage-initial");
  wipe(path); initWork(path);
  write(path, "new.txt", "staged before first commit\n");
  git(path, ["add", "new.txt"]);
  write(path, "new.txt", "edited after staging\n");
}

function seedWorktreeRepo() {
  const main = join(repos, "worktree");
  const checkouts = join(repos, "worktree-checkouts");
  const bare = join(repos, "worktree-bare.git");
  for (const path of [main, checkouts, bare]) wipe(path);
  initWork(main);
  mkdirSync(checkouts, { recursive: true });
  commit(main, ".gitignore", "ignored.txt\n", "worktree base");
  git(main, ["branch", "available"]);
  git(main, ["tag", "base"]);
  const topic = join(checkouts, "topic");
  const detached = join(checkouts, "detached");
  const missing = join(checkouts, "missing");
  git(main, ["worktree", "add", "-b", "topic", topic]);
  commit(topic, "topic.txt", "committed on topic\n", "worktree topic commit");
  write(topic, "topic.txt", "modified in topic\n");
  write(topic, "local.txt", "untracked in topic\n");
  write(topic, "ignored.txt", "ignored in topic\n");
  git(main, ["worktree", "lock", "--reason", "external disk", topic]);
  git(main, ["worktree", "add", "--detach", detached, "base"]);
  commit(detached, "detached.txt", "detached history\n", "worktree detached commit");
  git(main, ["worktree", "add", "-b", "missing", missing]);
  wipe(missing);
  commit(main, "main.txt", "only on main\n", "worktree main commit");
  git(main, ["clone", "--bare", main, bare]);
  git(main, ["remote", "add", "origin", bare]);
  git(main, ["update-ref", "refs/remotes/origin/topic", "refs/heads/topic"]);
}

function seedStageConflictPathsRepo() {
  const path = join(repos, "stage-conflict-paths");
  wipe(path); initWork(path);
  commit(path, "conflict.txt", "base\n", "base");
  commit(path, "old.txt", "original\n", "rename source");
  commit(path, "delete.txt", "delete\n", "delete source");
  git(path, ["checkout", "-b", "topic"]);
  commit(path, "conflict.txt", "topic\n", "topic");
  git(path, ["checkout", "main"]);
  commit(path, "conflict.txt", "main\n", "main");
  const result = git(path, ["merge", "topic"], { allowFail: true });
  if (result.status !== 1) throw new Error("stage-conflict-paths did not conflict");
  git(path, ["mv", "old.txt", "renamed.txt"]);
  git(path, ["rm", "delete.txt"]);
  write(path, "renamed.txt", "edited after rename\n");
  write(path, "loose.txt", "new file\n");
}

function seedPullDeletedRepo() {
  const path = join(repos, "pull-deleted");
  const upstream = join(repos, "pull-deleted-upstream");
  wipe(path); wipe(upstream); initWork(upstream);
  commit(upstream, "deleted.txt", "base\n", "base");
  commit(upstream, "remote.txt", "base\n", "remote base");
  cloneFrom(fileUrl(upstream), path);
  commit(upstream, "remote.txt", "remote update\n", "remote update");
  git(path, ["fetch", "origin"]);
  rmSync(join(path, "deleted.txt"));
  write(path, "new.txt", "staged new file\n");
  git(path, ["add", "new.txt"]);
  rmSync(join(path, "new.txt"));
  write(path, ".fakeignore", "local-only\n");
}

function seedPullNoopRepo(name = "pull-noop") {
  const path = join(repos, name);
  const upstream = join(repos, `${name}-upstream`);
  wipe(path); wipe(upstream); initWork(upstream);
  git(upstream, ["config", "core.autocrlf", "false"]);
  commit(upstream, "note.txt", "base\n", "base");
  if (name === "pull-disjoint") {
    for (let index = 0; index < 1000; index++) write(upstream, `tracked/file-${index}.txt`, "base\n");
    git(upstream, ["add", "."]); git(upstream, ["commit", "-m", "many tracked files"]);
  }
  cloneFrom(fileUrl(upstream), path);
  git(path, ["config", "core.autocrlf", "false"]);
  if (name === "pull-disjoint") {
    commit(upstream, "incoming.txt", "incoming\n", "incoming without overlapping local work");
    for (let index = 0; index < 1000; index++) write(path, `tracked/file-${index}.txt`, "working\n");
  }
  write(path, "note.txt", "staged\n");
  git(path, ["add", "note.txt"]);
  write(path, "note.txt", "working\n");
  for (let index = 0; index < 1000; index++) write(path, `loose/file-${index}.txt`, "untracked\n");
}

function seedCheckoutRepo(name = "checkout-dirty") {
  const path = join(repos, name);
  wipe(path); initWork(path);
  git(path, ["config", "core.autocrlf", "false"]);
  commit(path, "note.txt", "one\ntwo\nthree\nfour\nfive\n", "base");
  commit(path, "keep.txt", "base\n", "keep base");
  write(path, "keep.txt", "older stash\n");
  git(path, ["stash", "push", "-m", "older user stash"]);
  git(path, ["checkout", "-b", "topic"]);
  if (name === "checkout-index") commit(path, "note.txt", "one\nremote two\nthree\nfour\nfive\n", "remote note");
  else if (name === "checkout-conflict") commit(path, "note.txt", "one\ntwo\nthree\nfour\nremote five\n", "remote note");
  else commit(path, "remote.txt", "remote\n", "topic file");
  git(path, ["checkout", "main"]);
  git(path, ["remote", "add", "origin", "."]);
  git(path, ["fetch", "origin"]);
  write(path, "note.txt", "one\ntwo\nthree\nfour\nlocal five\n");
  write(path, "keep.txt", "staged\n");
  git(path, ["add", "note.txt", "keep.txt"]);
  write(path, "keep.txt", "unstaged\n");
  write(path, "loose.txt", "untracked\n");
  write(path, ".fakeignore", "local-only\n");
}

function seedGoneUpstreamRepo() {
  const path = join(repos, "gone-upstream");
  const bare = join(lab, "gone-upstream.git");
  wipe(path); wipe(bare);
  initBare(bare);
  initWork(path);
  commit(path, "README.md", "base\n", "base");
  git(path, ["remote", "add", "origin", fileUrl(bare)]);
  git(path, ["push", "-u", "origin", "main"]);
  git(path, ["checkout", "-b", "Dev/EnterFast_2026_10_27"]);
  commit(path, "README.md", "branch\n", "local work on a deleted upstream");
  git(path, ["push", "-u", "origin", "Dev/EnterFast_2026_10_27"]);
  // Deleting the remote branch leaves branch.<name>.merge behind: %(upstream:short)
  // still names origin/Dev/EnterFast_2026_10_27 while no such ref can be resolved.
  git(path, ["push", "origin", "--delete", "Dev/EnterFast_2026_10_27"]);
}

function seedRevertRepo() {
  const path = join(repos, "revert");
  wipe(path);
  diverge(path, "revert");
}

function seedBisectRepo() {
  const path = join(repos, "bisect");
  wipe(path);
  if (!existsSync(remote)) throw new Error("lab remote missing; run a full seed first");
  cloneFrom(fileUrl(remote), path);
  for (let i = 1; i <= FILL; i++) {
    commit(path, "steps.txt", `step ${i}\n`, `bisect ${i}`);
  }
  git(path, ["bisect", "start", "HEAD", "HEAD~2"]);
}

function seedTagsRepo() {
  const path = join(repos, "tags");
  wipe(path);
  initWork(path);
  const dated = (day) => ({ extraEnv: { GIT_AUTHOR_DATE: `2024-01-${pad(day)}T12:00:00Z`, GIT_COMMITTER_DATE: `2024-01-${pad(day)}T12:00:00Z` } });
  for (const [index, name] of ["v2.0.6", "v2.0.10", "v2.0.0"].entries()) {
    git(path, ["commit", "--allow-empty", "-m", name], dated(index + 1));
    git(path, ["tag", name]);
  }
  // Annotated tags use their own date, even when pointing at an older commit.
  git(path, ["tag", "-a", "v2.0.1", "HEAD~2", "-m", "latest release"], dated(4));
  for (const [index, name] of ["archive/v9", "release/v8", "archive/v1", "release/v1"].entries()) {
    git(path, ["tag", "-a", name, "HEAD", "-m", name], dated(index + 5));
  }
}

function seedUnityRepo() {
  const path = join(repos, "unity");
  wipe(path);
  initWork(path);
  commit(path, "ProjectSettings/ProjectVersion.txt", "m_EditorVersion: 2022.3.62f1\n", "create the unity project");
  commit(path, "Assets/Scripts/Player.cs", "public class Player {}\n", "add the player script");
  commit(path, "Assets/Prefabs/UIChapter.prefab", "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!1 &1\nGameObject:\n  m_Name: UIChapter\n", "add the chapter prefab");
  commit(path, "Assets/Scripts/Player.cs", "public class Player\n{\n    public int level;\n}\n", "track the player level");
  commit(path, "Assets/Prefabs/UIChapter.prefab", "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!1 &1\nGameObject:\n  m_Name: UIChapter\n  m_IsActive: 1\n", "activate the chapter prefab");
}

function seedHistoryImagesRepo() {
  const path = join(repos, "history-images");
  wipe(path);
  initWork(path);
  const preview = (updated, width = updated ? 360 : 240, height = updated ? 216 : 144) => {
    const pixels = Buffer.alloc((width * 4 + 1) * height);
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
      const circle = (x - width * (updated ? 0.66 : 0.34)) ** 2 + (y - height / 2) ** 2 < (Math.min(width, height) * 0.24) ** 2;
      const color = circle ? (updated ? [84, 154, 255] : [250, 182, 68]) : [38, 44, 58];
      const at = y * (width * 4 + 1) + 1 + x * 4;
      pixels.set([...color, 255], at);
    }
    const chunk = (type, data) => {
      const body = Buffer.concat([Buffer.from(type), data]);
      let crc = 0xffffffff;
      for (const byte of body) {
        crc ^= byte;
        for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
      }
      const result = Buffer.alloc(body.length + 8);
      result.writeUInt32BE(data.length, 0);
      body.copy(result, 4);
      result.writeUInt32BE((crc ^ 0xffffffff) >>> 0, result.length - 4);
      return result;
    };
    const header = Buffer.alloc(13);
    header.writeUInt32BE(width, 0); header.writeUInt32BE(height, 4); header[8] = 8; header[9] = 6;
    return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", header), chunk("tEXt", Buffer.concat([Buffer.from("Comment\0"), Buffer.alloc(width * 4, 65)])), chunk("IDAT", deflateSync(pixels)), chunk("IEND", Buffer.alloc(0))]);
  };
  commit(path, "art/mark.png", preview(false), "add the image");
  commit(path, "art/mark.png", preview(true), "change the image");
  commit(path, "art/deleted.png", preview(false), "add an image to delete");
  git(path, ["rm", "--", "art/deleted.png"]);
  git(path, ["commit", "-m", "delete the image"]);
  commit(path, "art/large.png", Buffer.concat([pngA, Buffer.alloc(8_000_001)]), "add an oversized image");
  write(path, "art/mark.png", preview(true, 480, 144));
  git(path, ["add", "--", "art/mark.png"]);
  write(path, "art/mark.png", preview(false, 240, 288));
}

function seedStageManyRepo(name = "stage-many", dense = false) {
  const path = join(repos, name);
  const text = (count, changed) => Array.from({ length: count }, (_, index) =>
    `export const value${index} = "${changed ? "working" : "base"}-${index} ${"payload ".repeat(4)}";\n`).join("");
  wipe(path);
  initWork(path);
  git(path, ["config", "core.autocrlf", "false"]);
  for (let index = 0; index < 5000; index++) write(path, `tracked/file-${String(index).padStart(5, "0")}.txt`, "base\n");
  if (dense) {
    write(path, "a-many.ts", text(25_000, false));
    write(path, "b-large.asset", text(120_000, false));
  }
  git(path, ["add", "."]);
  git(path, ["commit", "-m", "test: many tracked files"]);
  for (let index = 0; index < 5000; index++) {
    write(path, `tracked/file-${String(index).padStart(5, "0")}.txt`, "working\n");
    write(path, `loose/file-${String(index).padStart(5, "0")}.txt`, "untracked\n");
  }
  if (dense) {
    write(path, "a-many.ts", text(25_000, true));
    write(path, "b-large.asset", text(120_000, true));
  }
}

function seedSubmoduleDiffRepo() {
  const path = join(repos, "submodule-diff");
  wipe(path);
  initWork(path);
  const initial = git(subBare, ["rev-list", "--max-parents=0", "main"]).stdout.trim();
  const latest = git(subBare, ["rev-parse", "main"]).stdout.trim();
  git(path, ["-c", "protocol.file.allow=always", "submodule", "add", fileUrl(subBare), "vendor"]);
  git(join(path, "vendor"), ["checkout", "--detach", initial]);
  git(path, ["add", "vendor"]);
  git(path, ["commit", "-m", "add submodule"]);
  if (latest !== initial) git(join(path, "vendor"), ["checkout", "--detach", latest]);
}

function seedSubmoduleForeignPathRepo() {
  const path = join(repos, "submodule-foreign-path");
  wipe(path);
  initWork(path);
  const child = join(path, "third_party", "child");
  git(path, ["-c", "protocol.file.allow=always", "submodule", "add", fileUrl(subBare), "third_party/child"]);
  git(child, ["config", "gitoxide.core.protectWindows", "true"]);
  // A commit created on another platform can contain a filename Windows cannot check out.
  const blob = Buffer.from(git(child, ["rev-parse", "HEAD:lib.txt"]).stdout.trim(), "hex");
  const treeFile = join(path, ".git", "foreign-tree");
  writeFileSync(treeFile, Buffer.concat([
    Buffer.from("100644 fixture?.txt\0"), blob, Buffer.from("100644 lib.txt\0"), blob,
  ]));
  const tree = git(child, ["hash-object", "-w", "-t", "tree", treeFile]).stdout.trim();
  const head = git(child, ["commit-tree", tree, "-p", "HEAD", "-m", "test: foreign filename"]).stdout.trim();
  git(child, ["update-ref", "HEAD", head]);
  write(path, "regular.txt", "base\n");
  git(path, ["add", "."]);
  git(path, ["commit", "-m", "test: add submodule and regular file"]);
  write(path, "regular.txt", "local edit\n");
}

function seedSubmoduleHealthRepo() {
  const path = join(repos, "submodule-health");
  wipe(path);
  initWork(path);
  commit(path, "README.md", "submodule health\n", "base");

  const src = join(repos, "health-src");
  wipe(src);
  initWork(src);
  commit(src, "health.txt", "shared\n", "health base");
  git(src, ["remote", "add", "origin", fileUrl(healthBare)]);
  git(src, ["push", "-u", "origin", "HEAD:main"]);
  const mainSha = git(src, ["rev-parse", "HEAD"]).stdout.trim();

  git(path, ["-c", "protocol.file.allow=always", "submodule", "add", "-q", "-b", "main", fileUrl(healthBare), "Packages/Clean"]);

  git(path, ["-c", "protocol.file.allow=always", "submodule", "add", "-q", "-b", "main", fileUrl(healthBare), "Packages/WrongBranch"]);
  const wrong = join(path, "Packages/WrongBranch");
  git(wrong, ["checkout", "-q", "-b", "topic"]);
  write(wrong, "health.txt", "local edit\n");

  git(path, ["-c", "protocol.file.allow=always", "submodule", "add", "-q", "-b", "main", fileUrl(healthBare), "Packages/Moved"]);
  const moved = join(path, "Packages/Moved");
  write(moved, "health.txt", "local commit\n");
  git(moved, ["commit", "-qam", "test: move submodule"]);

  git(path, ["-c", "protocol.file.allow=always", "submodule", "add", "-q", fileUrl(healthBare), "Packages/Detached"]);
  git(join(path, "Packages/Detached"), ["checkout", "-q", "--detach"]);

  git(path, ["config", "-f", ".gitmodules", "submodule.Packages/Uninitialized.path", "Packages/Uninitialized"]);
  git(path, ["config", "-f", ".gitmodules", "submodule.Packages/Uninitialized.url", fileUrl(healthBare)]);
  git(path, ["update-index", "--add", "--cacheinfo", `160000,${mainSha},Packages/Uninitialized`]);
  mkdirSync(join(path, "Packages/Uninitialized"), { recursive: true });
  git(path, ["add", ".gitmodules"]);
  git(path, ["commit", "-m", "test: submodules in every health state"]);
}

const restore = {
  "stage-many": seedStageManyRepo,
  "stage-dense": () => seedStageManyRepo("stage-dense", true),
  "history-images": seedHistoryImagesRepo,
  "checkout-dirty": () => seedCheckoutRepo(),
  "checkout-index": () => seedCheckoutRepo("checkout-index"),
  "checkout-conflict": () => seedCheckoutRepo("checkout-conflict"),
  "unstage-initial": seedUnstageInitialRepo,
  worktree: seedWorktreeRepo,
  "stage-conflict-paths": seedStageConflictPathsRepo,
  "submodule-diff": seedSubmoduleDiffRepo,
  "submodule-foreign-path": seedSubmoduleForeignPathRepo,
  "submodule-health": seedSubmoduleHealthRepo,
  "gone-upstream": seedGoneUpstreamRepo,
  "pull-deleted": seedPullDeletedRepo,
  "pull-noop": seedPullNoopRepo,
  "pull-disjoint": () => seedPullNoopRepo("pull-disjoint"),
  tags: seedTagsRepo,
  unity: seedUnityRepo,
  operation: seedOperationRepo,
  conflict: seedConflictRepo,
  merge: seedMergeRepo,
  rebase: seedRebaseRepo,
  cherry: seedCherryRepo,
  revert: seedRevertRepo,
  bisect: seedBisectRepo,
};

const only = process.argv.slice(2).filter((arg) => !arg.startsWith("-"));
if (only.length > 0) {
  mkdirSync(repos, { recursive: true });
  if (!existsSync(join(lab, "gitconfig"))) writeFileSync(join(lab, "gitconfig"), "");
  for (const name of only) {
    const seed = restore[name];
    if (!seed) throw new Error(`unknown lab repo ${name}`);
    console.log("restoring", name);
    seed();
  }
  console.log("lab repositories restored");
  process.exit(0);
}

console.log("seeding", lab);
wipe(lab);
mkdirSync(repos, { recursive: true });
mkdirSync(shelf, { recursive: true });
writeFileSync(join(lab, "gitconfig"), "");

initBare(remote);
initBare(remotePush);
initBare(subBare);
initBare(healthBare);
const subSrc = join(repos, "sub-src");
initWork(subSrc);
commit(subSrc, "lib.txt", "library\n", "lib");
git(subSrc, ["remote", "add", "origin", fileUrl(subBare)]);
git(subSrc, ["push", "-u", "origin", "HEAD:main"]);
const subSha = git(subSrc, ["rev-parse", "HEAD"]).stdout.trim();
commit(subSrc, "lib.txt", "library update\n", "lib update");
git(subSrc, ["push", "origin", "HEAD:main"]);
seedSubmoduleDiffRepo();
seedSubmoduleForeignPathRepo();
seedSubmoduleHealthRepo();

const writer = join(repos, "writer");
initWork(writer);
commit(writer, "README.md", "base\n", "base");
git(writer, ["remote", "add", "origin", fileUrl(remote)]);
git(writer, ["push", "-u", "origin", "HEAD:main"]);

const behind = join(repos, "behind");
cloneFrom(fileUrl(remote), behind);

commit(writer, "README.md", "base\nremote moved\n", "remote moved");
git(writer, ["push", "origin", "HEAD:main"]);
git(behind, ["fetch", "origin"]);
// A local branch already holds the fetched tip; the filtered view still fades it.
git(behind, ["branch", "keep", "origin/main"]);

const origin = fileUrl(remote);
const full = join(repos, "full");
const even = join(repos, "even");
const ahead = join(repos, "ahead");
cloneFrom(origin, full);
cloneFrom(origin, even);
cloneFrom(origin, ahead);
// Same folder name as `full` under another parent, for the duplicate tab-label scene.
cloneFrom(origin, join(repos, "copies", "full"));
git(ahead, ["remote", "add", "mirror", origin]);
git(ahead, ["fetch", "mirror"]);
seedBisectRepo();
seedUnityRepo();

for (let i = 1; i <= FILL; i++) {
  commit(ahead, `local/ahead-${pad(i)}.txt`, `ahead ${i}\n`, `local ahead ${i}`);
}
dirtyBatch(ahead, "pending", FILL);

commit(full, "src/keep.ts", "export const keep = true;\n", "keep a source file");
git(full, ["checkout", "-b", "graph"]);
commit(full, "docs/graph.md", "graph branch\n", "record the graph branch");
// Long enough to be clipped by the compact ref chip anywhere the pane does not wrap.
git(full, ["branch", "release/2026.11.24-candidate"]);
git(full, ["checkout", "main"]);
git(full, ["merge", "--no-ff", "graph", "-m", "merge graph"]);
git(full, ["tag", "v1.0.0"]);
git(full, ["tag", "v2.0.0"]);
for (let i = 3; i <= FILL; i++) {
  git(full, ["tag", `v${i}.0.0`]);
}
git(full, ["branch", "old", "HEAD~1"]);
const mergeBase = git(full, ["rev-parse", "HEAD"]).stdout.trim();
for (let i = 1; i <= FILL; i++) {
  git(full, ["branch", `merged/${pad(i)}`, mergeBase]);
}
git(full, ["checkout", "-b", "topic"]);
commit(full, "docs/topic.md", "topic notes\n", "topic notes");
for (let i = 1; i <= FILL; i++) {
  commit(full, `topic/file-${pad(i)}.txt`, `topic file ${i}\n`, `topic file ${i}`);
}
git(full, ["checkout", "main"]);
for (let i = 1; i <= FILL; i++) {
  git(full, ["checkout", "-b", `feat/${pad(i)}`]);
  commit(full, `feat/${pad(i)}.txt`, `feature ${i}\n`, `feature ${i}`);
  git(full, ["checkout", "main"]);
}
git(full, ["-c", "protocol.file.allow=always", "submodule", "add", fileUrl(subBare), "vendor"]);
git(full, ["commit", "-m", "add vendor"]);
const moduleLines = ["[submodule \"vendor\"]", "\tpath = vendor", `\turl = ${fileUrl(subBare)}`];
for (let i = 1; i <= FILL; i++) {
  const name = `libs/lib-${pad(i)}`;
  moduleLines.push(`[submodule "${name}"]`, `\tpath = ${name}`, `\turl = ${fileUrl(subBare)}`);
  git(full, ["update-index", "--add", "--cacheinfo", `160000,${subSha},${name}`]);
}
write(full, ".gitmodules", moduleLines.join("\n") + "\n");
git(full, ["add", ".gitmodules"]);
git(full, ["commit", "-m", "add lab submodules"]);
write(full, ".mailmap", "AweGit Lab <lab@example.com> <lab@example.com>\n");
write(full, ".gitignore", "secret.tmp\nignored/\n*.tmp\n");
git(full, ["add", ".mailmap", ".gitignore"]);
git(full, ["commit", "-m", "mailmap and ignore"]);
write(full, "secret.tmp", "secret\n");
for (let i = 1; i <= FILL; i++) {
  write(full, `ignored/file-${pad(i)}.tmp`, `ignored ${i}\n`);
}
const lfs = spawnSync("git", ["lfs", "version"], { encoding: "utf8" });
if (lfs.status === 0) {
  git(full, ["lfs", "install", "--local"]);
  write(full, ".gitattributes", "*.bin filter=lfs diff=lfs merge=lfs -text\n");
  write(full, "assets/model.bin", "lab-model\n");
  for (let i = 1; i <= FILL; i++) {
    write(full, `assets/model-${pad(i)}.bin`, `lab-model ${i}\n`);
  }
  git(full, ["add", ".gitattributes", "assets"]);
  git(full, ["commit", "-m", "track models with lfs"]);
}
git(full, ["worktree", "add", "-b", "extra", join(repos, "full-wt")]);
for (let i = 1; i <= FILL - 1; i++) {
  git(full, ["worktree", "add", "-b", `wt/${pad(i)}`, join(repos, `full-wt-${pad(i)}`)]);
}
for (let i = 1; i <= FILL; i++) {
  write(full, `wip/stash-${pad(i)}.txt`, `stash wip ${i}\n`);
  git(full, ["stash", "push", "-u", "-m", i === 1 ? "WIP before the palette" : `lab stash ${i}`]);
}
for (let i = 1; i <= FILL; i++) {
  git(full, ["remote", "add", `mirror-${pad(i)}`, fileUrl(remote)], { allowFail: true });
}
for (let i = 1; i <= FILL; i++) {
  commit(full, "README.md", lines(i + 2, "readme"), `readme history ${i}`);
}
for (let i = 1; i <= FILL; i++) {
  write(full, `catalog/item-${pad(i)}.txt`, `catalog ${i}\n`);
}
git(full, ["add", "--", "catalog"]);
git(full, ["commit", "-m", "catalog pack"]);
write(full, "README.md", lines(80, "working readme"));
commit(full, "assets/Config/Generated.asset", bigAsset("base"), "large asset");
write(full, "assets/Config/Generated.asset", bigAsset("working"));
commit(full, "art/mark.png", pngA, "add the mark");
write(full, "art/mark.png", pngB);
commit(full, "data/sheet.xlsx", xlsx("sheet-a"), "add the sheet");
write(full, "data/sheet.xlsx", xlsx("sheet-b"));
commit(full, "data/large.pak", largePak("a"), "add the large pak");
write(full, "data/large.pak", largePak("b"));
commit(full, "src/old-name.txt", "name me\n", "name a file");
git(full, ["mv", "src/old-name.txt", "src/renamed.txt"]);
const longBase = lines(40, "line");
const longNext = lines(220, "line lab");
commit(full, "notes/long.txt", longBase, "long note");
write(full, "notes/long.txt", longNext);
const wrapBase = Array.from({ length: 90 }, (_, index) => `const wrap${index + 1} = "${"base text ".repeat(18)}";`);
commit(full, "notes/wrap.txt", wrapBase.join("\n") + "\n", "wrap note");
const wrapNext = [...wrapBase];
wrapNext[8] = `const wrap9 = "${"changed first hunk ".repeat(10)}";`;
wrapNext[70] = `const wrap71 = "${"changed second hunk ".repeat(10)}";`;
write(full, "notes/wrap.txt", wrapNext.join("\n") + "\n");
write(full, "src/staged.ts", "export const staged = true;\n");
git(full, ["add", "--", "src/staged.ts"]);
for (let i = 1; i <= FILL; i++) {
  write(full, `staged/file-${pad(i)}.ts`, `export const staged${i} = true;\n`);
  git(full, ["add", "--", `staged/file-${pad(i)}.ts`]);
}
for (let i = 1; i <= FILL; i++) {
  write(full, `pending/file-${pad(i)}.txt`, `pending ${i}\n`);
}
write(full, "scratch.txt", "untracked\n");

for (let i = 1; i <= FILL; i++) {
  commit(even, `even/file-${pad(i)}.txt`, `even ${i}\n`, `even ${i}`);
}

for (let i = 1; i <= FILL; i++) {
  commit(writer, `remote/behind-${pad(i)}.txt`, `behind ${i}\n`, `remote behind ${i}`);
}
git(writer, ["push", "origin", "HEAD:main"]);
git(behind, ["fetch", "origin"]);

const pushSrc = join(repos, "push-src");
initWork(pushSrc);
commit(pushSrc, "README.md", "push base\n", "push base");
git(pushSrc, ["remote", "add", "origin", fileUrl(remotePush)]);
git(pushSrc, ["push", "-u", "origin", "HEAD:main"]);
const pushbox = join(repos, "pushbox");
cloneFrom(fileUrl(remotePush), pushbox);
write(pushbox, "note.txt", "lab push note\n");

seedConflictRepo();
seedMergeRepo();
seedRebaseRepo();
seedOperationRepo();
seedUnstageInitialRepo();
seedWorktreeRepo();
seedStageConflictPathsRepo();
seedPullDeletedRepo();
seedPullNoopRepo();
seedPullNoopRepo("pull-disjoint");
seedCherryRepo();
seedRevertRepo();
seedTagsRepo();
seedHistoryImagesRepo();
seedStageManyRepo();
seedStageManyRepo("stage-dense", true);
seedGoneUpstreamRepo();

const detached = join(repos, "detached");
initWork(detached);
for (let i = 1; i <= FILL; i++) {
  commit(detached, "README.md", lines(i, "detached"), `detached ${i}`);
}
git(detached, ["checkout", "--detach", "HEAD"]);
write(detached, "README.md", lines(FILL + 2, "detached edited"));
dirtyBatch(detached, "pending", FILL);

const unborn = join(repos, "unborn");
initWork(unborn);
write(unborn, "unborn.txt", "no commit yet\n");
for (let i = 1; i <= FILL; i++) {
  write(unborn, `pending/unborn-${pad(i)}.txt`, `unborn ${i}\n`);
}

for (let i = 1; i <= FILL; i++) {
  const path = join(shelf, `r${pad(i)}`);
  initWork(path);
  commit(path, "README.md", `shelf ${i}\n`, `shelf ${i}`);
}

for (const name of ["checkout-dirty", "checkout-index", "checkout-conflict"]) seedCheckoutRepo(name);

const recent = readdirSync(shelf)
  .filter((name) => name.startsWith("r"))
  .sort()
  .map((name) => join(shelf, name));
writeFileSync(join(lab, "recent.txt"), recent.join("\n") + "\n");

console.log("lab repositories ready");
