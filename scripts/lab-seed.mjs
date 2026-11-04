// Builds the ignored .lab git repositories the browser lab opens.
// Uses only local git config (-c and the repo's own config). Never touches the global gitconfig
// and never writes inside the AweGit repository except under .lab/.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL(".", import.meta.url)), "..");
const lab = join(root, ".lab");
const repos = join(lab, "repos");
const remote = join(lab, "remote.git");
const remotePush = join(lab, "remote-push.git");
const subBare = join(lab, "sub.git");
const editor = join(root, "scripts", "lab-break-editor.mjs");

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

console.log("seeding", lab);
wipe(lab);
mkdirSync(repos, { recursive: true });
writeFileSync(join(lab, "gitconfig"), "");

initBare(remote);
initBare(remotePush);
initBare(subBare);
const subSrc = join(repos, "sub-src");
initWork(subSrc);
commit(subSrc, "lib.txt", "library\n", "lib");
git(subSrc, ["remote", "add", "origin", fileUrl(subBare)]);
git(subSrc, ["push", "-u", "origin", "HEAD:main"]);

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
const bisect = join(repos, "bisect");
cloneFrom(origin, full);
cloneFrom(origin, even);
cloneFrom(origin, ahead);
cloneFrom(origin, bisect);

commit(ahead, "local.txt", "only on this machine\n", "local only");

commit(bisect, "steps.txt", "one\n", "bisect one");
commit(bisect, "steps.txt", "two\n", "bisect two");
commit(bisect, "steps.txt", "three\n", "bisect three");
git(bisect, ["bisect", "start", "HEAD", "HEAD~2"]);

commit(full, "src/keep.ts", "export const keep = true;\n", "keep a source file");
git(full, ["checkout", "-b", "graph"]);
commit(full, "docs/graph.md", "graph branch\n", "record the graph branch");
git(full, ["checkout", "main"]);
git(full, ["merge", "--no-ff", "graph", "-m", "merge graph"]);
git(full, ["tag", "v1.0.0"]);
git(full, ["tag", "v2.0.0"]);
git(full, ["branch", "old", "HEAD~1"]);
git(full, ["checkout", "-b", "topic"]);
commit(full, "docs/topic.md", "topic notes\n", "topic notes");
git(full, ["checkout", "main"]);
git(full, ["-c", "protocol.file.allow=always", "submodule", "add", fileUrl(subBare), "vendor"]);
git(full, ["commit", "-m", "add vendor"]);
write(full, ".mailmap", "AweGit Lab <lab@example.com> <lab@example.com>\n");
write(full, ".gitignore", "secret.tmp\n");
git(full, ["add", ".mailmap", ".gitignore"]);
git(full, ["commit", "-m", "mailmap and ignore"]);
write(full, "secret.tmp", "secret\n");
const lfs = spawnSync("git", ["lfs", "version"], { encoding: "utf8" });
if (lfs.status === 0) {
  git(full, ["lfs", "install", "--local"]);
  write(full, ".gitattributes", "*.bin filter=lfs diff=lfs merge=lfs -text\n");
  write(full, "assets/model.bin", "lab-model\n");
  git(full, ["add", ".gitattributes", "assets/model.bin"]);
  git(full, ["commit", "-m", "track a model with lfs"]);
}
git(full, ["worktree", "add", "-b", "extra", join(repos, "full-wt")]);
write(full, "wip.txt", "wip\n");
git(full, ["stash", "push", "-u", "-m", "WIP before the palette"]);
commit(full, "README.md", "base\nremote moved\nhello from main\n", "readme for the diff");
write(full, "README.md", "base\nremote moved\nhello from the working tree\n");
commit(full, "art/mark.png", pngA, "add the mark");
write(full, "art/mark.png", pngB);
commit(full, "data/sheet.xlsx", xlsx("sheet-a"), "add the sheet");
write(full, "data/sheet.xlsx", xlsx("sheet-b"));
commit(full, "src/old-name.txt", "name me\n", "name a file");
git(full, ["mv", "src/old-name.txt", "src/renamed.txt"]);
const longBase = Array.from({ length: 40 }, (_, index) => `line ${index + 1}`).join("\n") + "\n";
const longNext = Array.from({ length: 220 }, (_, index) => `line ${index + 1} lab`).join("\n") + "\n";
commit(full, "notes/long.txt", longBase, "long note");
write(full, "notes/long.txt", longNext);
write(full, "src/staged.ts", "export const staged = true;\n");
git(full, ["add", "--", "src/staged.ts"]);
write(full, "scratch.txt", "untracked\n");

const pushSrc = join(repos, "push-src");
initWork(pushSrc);
commit(pushSrc, "README.md", "push base\n", "push base");
git(pushSrc, ["remote", "add", "origin", fileUrl(remotePush)]);
git(pushSrc, ["push", "-u", "origin", "HEAD:main"]);
const pushbox = join(repos, "pushbox");
cloneFrom(fileUrl(remotePush), pushbox);
write(pushbox, "note.txt", "lab push note\n");

function conflictRepo(path, pause) {
  initWork(path);
  commit(path, "src/both-sides.ts", "export const side = \"base\";\n", "base text");
  commit(path, "docs/removed-there.md", "keep\n", "base doc");
  commit(path, "art/removed-here.png", pngA, "base image");
  commit(path, "data/sheet.xlsx", xlsx("base"), "base sheet");
  git(path, ["checkout", "-b", "topic"]);
  commit(path, "src/both-sides.ts", "export const side = \"topic\";\n", "topic text");
  git(path, ["rm", "docs/removed-there.md"]);
  git(path, ["commit", "-m", "topic removes the doc"]);
  commit(path, "src/both-added.ts", "export const added = \"topic\";\n", "topic adds a file");
  commit(path, "art/removed-here.png", pngB, "topic image");
  commit(path, "data/sheet.xlsx", xlsx("topic"), "topic sheet");
  git(path, ["checkout", "main"]);
  commit(path, "src/both-sides.ts", "export const side = \"main\";\n", "main text");
  commit(path, "docs/removed-there.md", "keep\nmain edit\n", "main edits the doc");
  commit(path, "src/both-added.ts", "export const added = \"main\";\n", "main adds a file");
  commit(path, "art/removed-here.png", pngC, "main image");
  commit(path, "data/sheet.xlsx", xlsx("main"), "main sheet");
  const merged = git(path, ["merge", "topic"], { allowFail: true });
  if (merged.status === 0) throw new Error(`${path} merge did not conflict`);
  if (!has(path, join(".git", "MERGE_HEAD"))) throw new Error(`${path} has no MERGE_HEAD`);
  if (pause) pauseIndex(path, "MERGE_HEAD");
}

conflictRepo(join(repos, "conflict"), null);
conflictRepo(join(repos, "merge"), pauseIndex);

const rebase = join(repos, "rebase");
initWork(rebase);
commit(rebase, "README.md", "rebase base\n", "rebase base");
git(rebase, ["checkout", "-b", "side"]);
commit(rebase, "side.txt", "side\n", "side work");
git(rebase, ["checkout", "main"]);
commit(rebase, "main.txt", "main\n", "main work");
git(rebase, ["checkout", "side"]);
const editorCmd = `node "${editor.replaceAll("\\", "/")}"`;
const paused = git(rebase, ["rebase", "-i", "main"], {
  allowFail: true,
  extraEnv: { GIT_SEQUENCE_EDITOR: editorCmd },
});
if (!has(rebase, join(".git", "rebase-merge")) && !has(rebase, join(".git", "rebase-apply"))) {
  throw new Error(`rebase did not pause\n${paused.stdout ?? ""}\n${paused.stderr ?? ""}`);
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
    return;
  }
  commit(path, "a.txt", "one\n", "first");
  commit(path, "a.txt", "two\n", "second");
  commit(path, "a.txt", "three\n", "third");
  const result = git(path, ["revert", "--no-edit", "HEAD~1"], { allowFail: true });
  if (result.status === 0) throw new Error(`${path} revert did not conflict`);
  pauseIndex(path, "REVERT_HEAD");
}

diverge(join(repos, "cherry"), "cherry");
diverge(join(repos, "revert"), "revert");

const detached = join(repos, "detached");
initWork(detached);
commit(detached, "README.md", "detached base\n", "detached base");
commit(detached, "README.md", "detached next\n", "detached next");
git(detached, ["checkout", "--detach", "HEAD"]);
write(detached, "README.md", "detached next\nedited while detached\n");

const unborn = join(repos, "unborn");
initWork(unborn);
write(unborn, "unborn.txt", "no commit yet\n");

console.log("lab repositories ready");
