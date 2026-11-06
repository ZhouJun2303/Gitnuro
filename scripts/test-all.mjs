// Run both Rust test suites with one thread. pnpm test calls this.
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runner = resolve(root, "scripts", "with-rust-path.mjs");

const suites = [
  ["cargo", "test", "-p", "awegit-git", "--", "--test-threads=1"],
  ["cargo", "test", "-p", "awegit", "--lib", "--", "--test-threads=1"],
];

for (const args of suites) {
  const code = await new Promise((done) => {
    const child = spawn(process.execPath, [runner, ...args], { cwd: root, stdio: "inherit" });
    child.on("exit", (status) => done(status ?? 1));
    child.on("error", () => done(1));
  });
  if (code !== 0) process.exit(code);
}
