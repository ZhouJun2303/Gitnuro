// Reproducible, disposable performance data. Every run gets a fresh .lab directory.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const options = new Map(process.argv.slice(2).filter((arg) => arg !== "--performance").map((arg) => {
  const match = /^--(tracked|untracked|history|conflicts)=(\d+(?:,\d+)*)$/.exec(arg);
  if (!match) throw new Error(`unknown performance option: ${arg}`);
  return [match[1], match[2]];
}));
const count = (name, fallback) => {
  const value = Number(options.get(name) ?? fallback);
  if (!Number.isSafeInteger(value) || value < 0 || value > 1_000_000) throw new Error(`invalid ${name}`);
  return value;
};
const tracked = Math.max(3, count("tracked", 1000));
const untracked = count("untracked", 1000);
const histories = count("history", 1000);
const conflictCounts = (options.get("conflicts") ?? "10,100,1000").split(",").map(Number);
if (conflictCounts.some((value) => value < 1 || value > 10000)) throw new Error("conflicts must be between 1 and 10000");
const parent = join(root, ".lab", "performance");
mkdirSync(parent, { recursive: true });
const run = mkdtempSync(join(parent, "run-"));
const bundle = join(root, "src-tauri", "resources", "git", "cmd", "git.exe");
const program = existsSync(bundle) ? bundle : "git";
const globalConfig = join(run, "gitconfig");
writeFileSync(globalConfig, "");
const env = { ...process.env, GIT_CONFIG_GLOBAL: globalConfig, GIT_CONFIG_NOSYSTEM: "1" };
for (const key of Object.keys(env)) if (["git_dir", "git_work_tree", "git_index_file"].includes(key.toLowerCase())) delete env[key];
function git(repo, args, input, codes = [0]) {
  const output = spawnSync(program, ["-c", "user.name=AweGit Performance", "-c", "user.email=perf@example.invalid", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=.git/no-hooks", ...args], {
    cwd: repo, env, input, encoding: "utf8", windowsHide: true, maxBuffer: 16 * 1024 * 1024,
  });
  if (output.error || !codes.includes(output.status)) throw new Error(`git ${args[0]}: ${output.error ?? output.stderr}`);
  return output.stdout.trim();
}
function write(repo, name, body) {
  const path = join(repo, name);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
}
function init(name) {
  const repo = join(run, name);
  mkdirSync(repo);
  git(repo, ["init", "--quiet", "--template=", "-b", "main"]);
  for (const [key, value] of [["user.name", "AweGit Performance"], ["user.email", "perf@example.invalid"], ["core.autocrlf", "false"], ["core.hooksPath", ".git/no-hooks"], ["gc.auto", "0"]]) git(repo, ["config", key, value]);
  return repo;
}
const workspace = init("workspace");
for (let index = 0; index < tracked; index++) write(workspace, `tracked/${Math.floor(index / 500)}/file-${index}.txt`, "base\n");
write(workspace, ".gitignore", "ignored/\n");
git(workspace, ["add", "."]);
git(workspace, ["commit", "--quiet", "-m", "base"]);
const base = git(workspace, ["rev-parse", "HEAD"]);
write(workspace, "incoming.txt", "incoming\n");
git(workspace, ["add", "."]);
git(workspace, ["commit", "--quiet", "-m", "incoming"]);
const incoming = git(workspace, ["rev-parse", "HEAD"]);
const remote = join(run, "remote.git");
git(run, ["clone", "--quiet", "--bare", workspace, remote]);
git(workspace, ["reset", "--hard", base]);
git(workspace, ["remote", "add", "origin", remote]);
git(workspace, ["fetch", "--quiet", "origin"]);
git(workspace, ["branch", "--set-upstream-to=origin/main", "main"]);
const conflicts = [];
for (const size of conflictCounts) {
  const repo = init(`conflicts-${size}`);
  for (let index = 0; index < size; index++) write(repo, `file-${index}.txt`, "base\n");
  git(repo, ["add", "."]); git(repo, ["commit", "--quiet", "-m", "base"]);
  git(repo, ["checkout", "--quiet", "-b", "incoming"]);
  for (let index = 0; index < size; index++) write(repo, `file-${index}.txt`, "incoming\n");
  git(repo, ["commit", "--quiet", "-am", "incoming"]);
  git(repo, ["checkout", "--quiet", "main"]);
  for (let index = 0; index < size; index++) write(repo, `file-${index}.txt`, "local\n");
  git(repo, ["commit", "--quiet", "-am", "local"]);
  git(repo, ["merge", "--no-edit", "incoming"], undefined, [1]);
  conflicts.push({ count: size, repo });
}
const blobs = init("blobs");
write(blobs, "note", "base\n");
git(blobs, ["add", "."]); git(blobs, ["commit", "--quiet", "-m", "base"]);
const blobCases = [];
for (const [name, size, nul] of [["text", 7_999_999, -1], ["prefix-nul", 8193, 8191], ["after-prefix-nul", 8193, 8192], ["over-limit", 8_000_001, -1]]) {
  const bytes = Buffer.alloc(size, 120);
  if (nul >= 0) bytes[nul] = 0;
  write(blobs, name, bytes);
  const oid = git(blobs, ["hash-object", "-w", name]);
  git(blobs, ["update-index", "--index-info"], [1, 2, 3].map((stage) => `100644 ${oid} ${stage}\t${name}\n`).join(""));
  write(blobs, name, "resolved text\n");
  blobCases.push({ name, size, nul });
}
const historyDir = join(workspace, ".git", "awegit-flow");
mkdirSync(historyDir, { recursive: true });
const startedAt = Date.now();
for (let index = 0; index < histories; index++) {
  const id = `${startedAt + index}-seed-1`;
  const summary = { id, repo: workspace, kind: "pull", state: "completed", sequence: 1, startedAt: startedAt + index, endedAt: startedAt + index + 1, activeStep: "finished", canAutoClose: true, branch: "main", before: null, after: null, progress: null, pendingStash: "", recordingError: "", previous: "" };
  writeFileSync(join(historyDir, `${id}.json`), JSON.stringify(summary));
  writeFileSync(join(historyDir, `${id}.jsonl`), JSON.stringify({ summary, node: null }) + "\n");
}
const manifest = { run, workspace, remote, base, incoming, tracked, untracked, histories, conflicts, blobs, blobCases, git: program, gitVersion: git(run, ["--version"]), createdAt: startedAt };
writeFileSync(join(run, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
console.log(join(run, "manifest.json"));
