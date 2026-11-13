// Start the lab command server and the Vite page. Callers own the child processes.

import { spawn } from "node:child_process";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const root = resolve(fileURLToPath(new URL(".", import.meta.url)), "..");

export function labEnv(extra = {}) {
  const env = { ...process.env, ...extra };
  for (const key of Object.keys(env)) {
    const name = key.toLowerCase();
    if (name === "git_dir" || name === "git_work_tree") delete env[key];
  }
  const lab = join(root, ".lab");
  env.AWEGIT_LAB = lab;
  env.AWEGIT_SETTINGS = join(lab, "settings.json");
  env.AWEGIT_SECRET_DIR = join(lab, "secrets");
  env.GIT_AUTHOR_NAME = "AweGit Lab";
  env.GIT_AUTHOR_EMAIL = "lab@example.com";
  env.GIT_COMMITTER_NAME = "AweGit Lab";
  env.GIT_COMMITTER_EMAIL = "lab@example.com";
  return env;
}

export function startLab() {
  return spawn(process.execPath, ["scripts/with-rust-path.mjs", "cargo", "run", "-q", "-p", "awegit", "--bin", "awegit-lab"], {
    cwd: root,
    env: labEnv(),
    stdio: "inherit",
  });
}

export function startVite(port, { reuse = false } = {}) {
  const args = reuse ? ["scripts/dev-server.mjs"] : ["node_modules/vite/bin/vite.js", "dev"];
  return spawn(process.execPath, args, {
    cwd: root,
    env: labEnv({ AWEGIT_VITE_PORT: String(port) }),
    stdio: "inherit",
  });
}

export function stop(child) {
  if (!child?.pid) return;
  if (process.platform === "win32") {
    spawn("taskkill", ["/pid", String(child.pid), "/t", "/f"], { stdio: "ignore" });
    return;
  }
  child.kill("SIGTERM");
}

export function run(args, env = process.env) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, args, { cwd: root, env, stdio: "inherit" });
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`${args.join(" ")} exited ${code}`));
    });
  });
}

export async function waitFor(url, seconds) {
  const deadline = Date.now() + seconds * 1000;
  let last = "not ready";
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
      last = `${response.status}`;
    } catch (error) {
      last = error instanceof Error ? error.message : String(error);
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`timed out waiting for ${url}: ${last}`);
}
