// Run only ignored performance benchmarks and preserve their machine-readable results.
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, delimiter, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const manifest = args.find((arg) => arg.startsWith("--manifest="))?.slice(11);
const release = args.includes("--release");
const systemGit = args.includes("--system-git");
const samples = args.find((arg) => arg.startsWith("--samples="))?.slice(10);
if (samples && (!/^\d+$/.test(samples) || Number(samples) < 1 || Number(samples) > 1000)) throw new Error("samples must be between 1 and 1000");
if (args.some((arg) => !["--release", "--writes", "--system-git", "--seed-only"].includes(arg) && !arg.startsWith("--manifest=") && !arg.startsWith("--samples="))) throw new Error("unknown benchmark option");
const env = { ...process.env };
const bundle = join(root, "src-tauri/resources/git/cmd");
if (!systemGit && existsSync(join(bundle, "git.exe"))) {
  const pathKey = Object.keys(env).find((key) => key.toLowerCase() === "path") ?? "PATH";
  env[pathKey] = bundle + delimiter + env[pathKey];
}
if (manifest) env.AWEGIT_PERF_MANIFEST = resolve(manifest);
for (const key of Object.keys(env)) if (["git_dir", "git_work_tree", "git_index_file"].includes(key.toLowerCase())) delete env[key];
if (args.includes("--writes")) env.AWEGIT_PERF_WRITES = "1";
if (samples) env.AWEGIT_PERF_SAMPLES = samples;
const results = [];
const filter = args.includes("--seed-only") ? "flows::tests::seeded_performance_timings" : "flows::tests::";
const command = ["scripts/with-rust-path.mjs", "cargo", "test", "-p", "awegit", "--lib", "--offline", ...(release ? ["--release"] : []), filter, "--", "--ignored", "--nocapture", "--test-threads=1"];
const child = spawn(process.execPath, command, { cwd: root, env, stdio: ["ignore", "pipe", "inherit"] });
let pending = "";
child.stdout.on("data", (chunk) => {
  pending += chunk.toString();
  const lines = pending.split(/\r?\n/);
  pending = lines.pop();
  for (const line of lines) {
    const at = line.indexOf("PERF ");
    if (at >= 0) results.push(JSON.parse(line.slice(at + 5)));
    console.log(line);
  }
});
const code = await new Promise((done, reject) => { child.on("error", reject); child.on("close", done); });
const outputDir = join(root, ".lab/performance");
mkdirSync(outputDir, { recursive: true });
const text = (cmd, args) => spawnSync(cmd, args, { cwd: root, env, encoding: "utf8", windowsHide: true }).stdout?.trim();
const output = join(outputDir, `results-${release ? "release" : "debug"}-${Date.now()}.json`);
writeFileSync(output, JSON.stringify({ at: new Date().toISOString(), revision: text("git", ["rev-parse", "HEAD"]), dirty: !!text("git", ["status", "--porcelain"]), build: release ? "release" : "debug", git: text("git", ["--version"]), platform: process.platform, arch: process.arch, manifest, exitCode: code, results }, null, 2) + "\n");
console.log(output);
process.exitCode = code ?? 1;
