// Builds the ignored .lab git repositories the browser lab opens.
// Uses only local git config (-c and the repo's own config). Never touches the global gitconfig
// and never writes inside the AweGit repository except under .lab/.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL(".", import.meta.url)), "..");
const lab = join(root, ".lab");
const repos = join(lab, "repos");
const shelf = join(repos, "shelf");
const remote = join(lab, "remote.git");
const remotePush = join(lab, "remote-push.git");
const subBare = join(lab, "sub.git");
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
const xlsx = (mark) => Buffer.concat([Buffer.from("PK\u0003\u0004"), Buffer.from(mark), Buffer.from([0])]);

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
  for (let i = 0; i < FILL; i++) {
    const ext = textExts[i % textExts.length];
    commit(path, `conflict/file-${pad(i + 1)}.${ext}`, `base ${ext} ${i + 1}\n`, `base conflict ${i + 1}`);
  }
  git(path, ["checkout", "-b", "topic"]);
  write(path, "src/both-sides.ts", longTopic);
  git(path, ["add", "--", "src/both-sides.ts"]);
  git(path, ["commit", "-m", "topic text", "--", "src/both-sides.ts"]);
  commit(path, "src/many-hunks.ts", manyHunkText("topic"), "topic many hunks");
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

const restore = {
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
const subSrc = join(repos, "sub-src");
initWork(subSrc);
commit(subSrc, "lib.txt", "library\n", "lib");
git(subSrc, ["remote", "add", "origin", fileUrl(subBare)]);
git(subSrc, ["push", "-u", "origin", "HEAD:main"]);
const subSha = git(subSrc, ["rev-parse", "HEAD"]).stdout.trim();

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

const origin = fileUrl(remote);
const full = join(repos, "full");
const even = join(repos, "even");
const ahead = join(repos, "ahead");
cloneFrom(origin, full);
cloneFrom(origin, even);
cloneFrom(origin, ahead);
seedBisectRepo();

for (let i = 1; i <= FILL; i++) {
  commit(ahead, `local/ahead-${pad(i)}.txt`, `ahead ${i}\n`, `local ahead ${i}`);
}
dirtyBatch(ahead, "pending", FILL);

commit(full, "src/keep.ts", "export const keep = true;\n", "keep a source file");
git(full, ["checkout", "-b", "graph"]);
commit(full, "docs/graph.md", "graph branch\n", "record the graph branch");
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
commit(full, "art/mark.png", pngA, "add the mark");
write(full, "art/mark.png", pngB);
commit(full, "data/sheet.xlsx", xlsx("sheet-a"), "add the sheet");
write(full, "data/sheet.xlsx", xlsx("sheet-b"));
commit(full, "src/old-name.txt", "name me\n", "name a file");
git(full, ["mv", "src/old-name.txt", "src/renamed.txt"]);
const longBase = lines(40, "line");
const longNext = lines(220, "line lab");
commit(full, "notes/long.txt", longBase, "long note");
write(full, "notes/long.txt", longNext);
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
seedCherryRepo();
seedRevertRepo();

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

const recent = readdirSync(shelf)
  .filter((name) => name.startsWith("r"))
  .sort()
  .map((name) => join(shelf, name));
writeFileSync(join(lab, "recent.txt"), recent.join("\n") + "\n");

console.log("lab repositories ready");
